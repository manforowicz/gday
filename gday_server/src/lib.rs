//! Note: this crate is still in early-development, so expect breaking changes.
//!
//! Runs a server for the [`gday_contact_exchange_protocol`].
//! Lets two users exchange their public and (optionally) private socket addresses.
#![forbid(unsafe_code)]
#![warn(clippy::all)]

mod connection_handler;
mod state;

use clap::Parser;
use connection_handler::handle_connection;
use gday_contact_exchange_protocol::{DEFAULT_TCP_PORT, DEFAULT_TLS_PORT};
use log::{debug, error, info, warn};
use socket2::{SockRef, TcpKeepalive};
use state::State;
use std::net::{SocketAddr, ToSocketAddrs};
use std::{
    fmt::Display,
    io::{BufReader, ErrorKind},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio_rustls::{
    rustls::{self, pki_types::CertificateDer},
    TlsAcceptor,
};

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Args {
    /// PEM file of private TLS server key
    #[arg(short, long, required_unless_present("unencrypted"))]
    pub key: Option<PathBuf>,

    /// PEM file of signed TLS server certificate
    #[arg(short, long, required_unless_present("unencrypted"))]
    pub certificate: Option<PathBuf>,

    /// Use unencrypted TCP instead of TLS
    #[arg(short, long, conflicts_with_all(["key", "certificate"]))]
    pub unencrypted: bool,

    /// Custom socket address on which to listen.
    /// [default: `[::]:2311` for TLS, `[::]:2310` when --unencrypted]
    #[arg(short, long)]
    pub address: Option<String>,

    /// Number of seconds before a new room is deleted
    #[arg(short, long, default_value = "3600")]
    pub timeout: u64,

    /// Max number of requests an IP address can
    /// send in a minute before they're rejected
    #[arg(short, long, default_value = "60")]
    pub request_limit: u32,

    /// Log verbosity. (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    pub verbosity: log::LevelFilter,
}

/// Run a gday server.
///
/// `server_started` will send as soon as the server is ready to accept
/// requests.
pub fn start_server(
    args: Args,
) -> Result<(impl std::future::Future<Output = ()>, SocketAddr), Error> {
    // set the log level according to the command line argument
    if let Err(err) = env_logger::builder()
        .filter_level(args.verbosity)
        .try_init()
    {
        error!("Non-fatal error. Couldn't initialize logger: {err}")
    }

    let addr = if let Some(addr) = args.address {
        addr
    } else if args.unencrypted {
        format!("[::]:{DEFAULT_TCP_PORT}")
    } else {
        format!("[::]:{DEFAULT_TLS_PORT}")
    };

    // get tcp listener
    let tcp_listener = get_tcp_listener(addr)?;

    // get the TLS acceptor if applicable
    let tls_acceptor = if let (Some(k), Some(c)) = (args.key, args.certificate) {
        Some(get_tls_acceptor(&k, &c)?)
    } else {
        None
    };

    // create the shared global state object
    let state = State::new(
        args.request_limit,
        std::time::Duration::from_secs(args.timeout),
    );

    // log starting information
    let local_addr = tcp_listener.local_addr().map_err(|source| Error {
        msg: "Couldn't determine local address".to_string(),
        source,
    })?;
    info!("Listening on {local_addr}.",);
    info!("Is encrypted?: {}", tls_acceptor.is_some());
    info!(
        "Requests per minute per IP address limit: {}",
        args.request_limit
    );
    info!(
        "Number of seconds before a new room is deleted: {}",
        args.timeout
    );
    info!("Server started.");

    let server = run_server(state, tcp_listener, tls_acceptor);

    Ok((server, local_addr))
}

async fn run_server(
    state: State,
    tcp_listener: tokio::net::TcpListener,
    tls_acceptor: Option<TlsAcceptor>,
) {
    loop {
        // try to accept another connection
        let (stream, addr) = match tcp_listener.accept().await {
            Ok(ok) => ok,
            Err(err) => {
                warn!("Error accepting incoming TCP connection: {err}.");
                continue;
            }
        };
        debug!("Accepted incoming TCP connection from {addr}.");

        // spawn a thread to handle the connection
        tokio::spawn(handle_connection(
            stream,
            tls_acceptor.clone(),
            state.clone(),
        ));
    }
}

/// Returns a [`TcpListener`] with the provided address.
///
/// Sets the socket's TCP keepalive so that unresponsive
/// connections close after 10 minutes to save resources.
fn get_tcp_listener(addr: impl ToSocketAddrs + Display) -> Result<tokio::net::TcpListener, Error> {
    // binds to the socket address
    let listener = std::net::TcpListener::bind(&addr).map_err(|source| Error {
        msg: format!("Can't listen on '{addr}'"),
        source,
    })?;

    // make the listener non-blocking
    listener.set_nonblocking(true).map_err(|source| Error {
        msg: "Couldn't set TCP listener to non-blocking".to_string(),
        source,
    })?;

    // sets the keepalive to 10 minutes
    let tcp_keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(600))
        .with_interval(Duration::from_secs(10));
    let socket = SockRef::from(&listener);
    socket
        .set_tcp_keepalive(&tcp_keepalive)
        .map_err(|source| Error {
            msg: "Couldn't set TCP keepalive".to_string(),
            source,
        })?;

    // convert to a tokio listener
    let listener = tokio::net::TcpListener::from_std(listener).map_err(|source| Error {
        msg: "Couldn't create async TCP listener".to_string(),
        source,
    })?;

    Ok(listener)
}

/// Takes paths to a PEM-encoded private key and signed certificate.
/// Returns a [`TlsAcceptor`]
fn get_tls_acceptor(key_path: &Path, cert_path: &Path) -> Result<TlsAcceptor, Error> {
    // try reading the key file
    let mut key = BufReader::new(std::fs::File::open(key_path).map_err(|source| Error {
        msg: format!("Couldn't open key file {key_path:?}."),
        source,
    })?);

    let key = rustls_pemfile::private_key(&mut key)
        .map_err(|source| Error {
            msg: format!("Couldn't parse key file {key_path:?}."),
            source,
        })?
        .ok_or(Error {
            msg: format!("No private keys found in file {key_path:?}."),
            source: ErrorKind::NotFound.into(),
        })?;

    // try reading the certificate file
    let mut cert = BufReader::new(std::fs::File::open(cert_path).map_err(|source| Error {
        msg: format!("Couldn't open certificate file {cert_path:?}."),
        source,
    })?);

    let cert: Result<Vec<CertificateDer<'static>>, _> = rustls_pemfile::certs(&mut cert).collect();
    let cert = cert.map_err(|source| Error {
        msg: format!("Couldn't parse certificate file {cert_path:?}."),
        source,
    })?;

    // try creating tls config
    let tls_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert, key)
        .map_err(|source| Error {
            msg: "Couldn't configure TLS".to_string(),
            source: std::io::Error::new(ErrorKind::InvalidInput, source),
        })?;

    // create a tls acceptor
    Ok(tokio_rustls::TlsAcceptor::from(Arc::new(tls_config)))
}

#[derive(thiserror::Error, Debug)]
#[error("{msg}\n{source}")]
pub struct Error {
    pub msg: String,
    pub source: std::io::Error,
}
