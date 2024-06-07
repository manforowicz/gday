//! Note: this crate is still in early-development, so expect breaking changes.
//!
//! Runs a server for the [`gday_contact_exchange_protocol`].
//! Lets two users exchange their public and (optionally) private socket addresses.
#![forbid(unsafe_code)]
#![warn(clippy::all)]

mod connection_handler;
mod state;
mod tests;

use clap::Parser;
use connection_handler::handle_connection;
use gday_contact_exchange_protocol::{DEFAULT_TCP_PORT, DEFAULT_TLS_PORT};
use log::{debug, error, info, warn};
use socket2::{SockRef, TcpKeepalive};
use state::State;
use std::{
    fmt::Display,
    io::BufReader,
    path::{Path, PathBuf},
    process::exit,
    sync::Arc,
    time::Duration,
};
use tokio::net::{TcpListener, ToSocketAddrs};
use tokio_rustls::{
    rustls::{self, pki_types::CertificateDer},
    TlsAcceptor,
};

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// PEM file of private TLS server key
    #[arg(short, long, required_unless_present("unencrypted"))]
    key: Option<PathBuf>,

    /// PEM file of signed TLS server certificate
    #[arg(short, long, required_unless_present("unencrypted"))]
    certificate: Option<PathBuf>,

    /// Use unencrypted TCP instead of TLS
    #[arg(short, long, conflicts_with_all(["key", "certificate"]))]
    unencrypted: bool,

    /// Custom socket address on which to listen.
    /// [default: `[::]:2311` for TLS, `[::]:2310` when --unencrypted]
    #[arg(short, long)]
    address: Option<String>,

    /// Number of seconds before a new room is deleted
    #[arg(short, long, default_value = "600")]
    timeout: u64,

    /// Max number of requests an IP address can
    /// send in a minute before they're rejected
    #[arg(short, long, default_value = "60")]
    request_limit: u32,

    /// Log verbosity. (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    verbosity: log::LevelFilter,
}

#[tokio::main]
async fn main() {
    // read command line arguments
    let args = Args::parse();

    // set the log level according to the command line argument
    env_logger::builder().filter_level(args.verbosity).init();

    let addr = if let Some(addr) = args.address {
        addr
    } else if args.unencrypted {
        format!("[::]:{DEFAULT_TCP_PORT}")
    } else {
        format!("[::]:{DEFAULT_TLS_PORT}")
    };

    // get tcp listener
    let tcp_listener = get_tcp_listener(&addr).await;

    // get the TLS acceptor if applicable
    let tls_acceptor = if let (Some(k), Some(c)) = (args.key, args.certificate) {
        Some(get_tls_acceptor(&k, &c))
    } else {
        None
    };

    // create the shared global state object
    let state = State::new(
        args.request_limit,
        std::time::Duration::from_secs(args.timeout),
    );

    // get local addr to print
    let local_addr = tcp_listener.local_addr().unwrap_or_else(|err| {
        error!("Couldn't determine local address: {err}");
        exit(1)
    });

    info!("Listening on '{local_addr}'.",);
    info!("Is encrypted?: {}", tls_acceptor.is_some());
    info!("Server started.");

    loop {
        // try to accept another connection
        let (stream, addr) = match tcp_listener.accept().await {
            Ok(ok) => ok,
            Err(err) => {
                warn!("Error accepting incoming TCP connection: {err}");
                continue;
            }
        };
        debug!("Accepted incoming TCP connection from {addr}");

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
///
/// Exits with an error message if couldn't bind to `addr`.
async fn get_tcp_listener(addr: impl ToSocketAddrs + Display) -> TcpListener {
    // binds to the socket address
    let listener = TcpListener::bind(&addr).await.unwrap_or_else(|err| {
        error!("Can't listen on '{addr}': {err}");
        exit(1)
    });

    // sets the keepalive to 10 minutes
    let tcp_keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(600))
        .with_interval(Duration::from_secs(10));
    let socket = SockRef::from(&listener);
    socket
        .set_tcp_keepalive(&tcp_keepalive)
        .unwrap_or_else(|err| error!("Couldn't set TCP keepalive: {err}"));

    listener
}

/// Takes paths to a PEM-encoded private key and signed certificate.
/// Returns a [`TlsAcceptor`]
///
/// Exits with an error message if the paths didn't lead to
/// valid files, or there was an error creating the tls config
fn get_tls_acceptor(key_path: &Path, cert_path: &Path) -> TlsAcceptor {
    // try reading the key file
    let mut key = BufReader::new(std::fs::File::open(key_path).unwrap_or_else(|err| {
        error!("Couldn't open key file '{key_path:?}': {err}");
        exit(1)
    }));

    let key = rustls_pemfile::private_key(&mut key)
        .unwrap_or_else(|err| {
            error!("Couldn't parse key file '{key_path:?}': {err}");
            exit(1)
        })
        .unwrap_or_else(|| {
            error!("No private keys found in file '{key_path:?}'");
            exit(1)
        });

    // try reading the certificate file
    let mut cert = BufReader::new(std::fs::File::open(cert_path).unwrap_or_else(|err| {
        error!("Couldn't open certificate file '{cert_path:?}': {err}");
        exit(1)
    }));

    let cert: Result<Vec<CertificateDer<'static>>, _> = rustls_pemfile::certs(&mut cert).collect();
    let cert = cert.unwrap_or_else(|err| {
        error!("Couldn't parse certificate file '{cert_path:?}': {err}");
        exit(1)
    });

    // try creating tls config
    let tls_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert, key)
        .unwrap_or_else(|err| {
            error!("Couldn't configure TLS: {err}");
            exit(1)
        });

    // create a tls acceptor
    tokio_rustls::TlsAcceptor::from(Arc::new(tls_config))
}
