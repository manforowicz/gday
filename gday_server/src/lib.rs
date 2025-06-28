#![forbid(unsafe_code)]
#![warn(clippy::all)]
//! Runs a server for the [`gday_contact_exchange_protocol`].
//! Lets two users exchange their public and (optionally) private socket
//! addresses.

mod connection_handler;
mod state;

use anyhow::Context;
use anyhow::anyhow;
use clap::Parser;
use connection_handler::handle_connection;
use log::{debug, error, info, warn};
use socket2::{Domain, Protocol, TcpKeepalive, Type};
use state::State;
use std::future::Future;
use std::net::SocketAddr;
use std::{
    io::BufReader,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::task::JoinSet;
use tokio_rustls::{
    TlsAcceptor,
    rustls::{self, pki_types::CertificateDer},
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

    /// Socket addresses on which to listen.
    #[arg(short, long, default_values = ["0.0.0.0:2311", "[::]:2311"])]
    pub addresses: Vec<SocketAddr>,

    /// Number of seconds before a new room is deleted
    #[arg(short, long, default_value = "600")]
    pub timeout: u64,

    /// Max number of create room requests and
    /// requests with an invalid room code
    /// an IP address can send per minute
    /// before they're rejected.
    #[arg(short, long, default_value = "10")]
    pub request_limit: u32,

    /// Log verbosity. (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "debug")]
    pub verbosity: log::LevelFilter,
}

/// Spawns a tokio server in the background.
///
/// Returns the addresses that the server is listening on and
/// a handle that can be dropped to stop the server.
///
/// Must be called from a tokio async context.
pub fn start_server(args: Args) -> anyhow::Result<(Vec<SocketAddr>, impl Future<Output = ()>)> {
    // set the log level according to the command line argument
    if let Err(err) = env_logger::builder()
        .filter_level(args.verbosity)
        .try_init()
    {
        error!("Non-fatal error. Couldn't initialize logger: {err}")
    }

    // get TCP listeners
    let tcp_listeners: anyhow::Result<Vec<tokio::net::TcpListener>> =
        args.addresses.into_iter().map(get_tcp_listener).collect();
    let tcp_listeners = tcp_listeners?;

    // get the addresses that we've actually bound to
    let addresses: std::io::Result<Vec<SocketAddr>> =
        tcp_listeners.iter().map(|l| l.local_addr()).collect();
    let addresses = addresses.context("Couldn't determine local address")?;

    // get the TLS acceptor if applicable
    let tls_acceptor = if let (Some(key), Some(cert)) = (args.key, args.certificate) {
        Some(get_tls_acceptor(&key, &cert)?)
    } else {
        None
    };

    // create the shared global state object
    let state = State::new(
        args.request_limit,
        std::time::Duration::from_secs(args.timeout),
    );

    let mut joinset = JoinSet::new();

    for tcp_listener in tcp_listeners {
        joinset.spawn(run_single_server(
            state.clone(),
            tcp_listener,
            tls_acceptor.clone(),
        ));
    }

    let handle = async {
        joinset.join_all().await;
    };

    // log the addresses being listened on
    info!("Listening on these addresses: {addresses:?}");
    info!("Is encrypted?: {}", tls_acceptor.is_some());
    info!(
        "Critical requests per minute per IP address limit: {}",
        args.request_limit
    );
    info!(
        "Number of seconds before a new room is deleted: {}",
        args.timeout
    );
    info!("Server is now running.");

    Ok((addresses, handle))
}

async fn run_single_server(
    state: State,
    tcp_listener: tokio::net::TcpListener,
    tls_acceptor: Option<TlsAcceptor>,
) {
    loop {
        // try to accept another connection
        let (stream, origin) = match tcp_listener.accept().await {
            Ok(ok) => ok,
            Err(err) => {
                error!("Error accepting incoming TCP connection: {err}.");
                continue;
            }
        };
        debug!("Accepted incoming TCP connection from {origin}.");

        // spawn a thread to handle the connection
        tokio::spawn(handle_connection(
            stream,
            origin,
            tls_acceptor.clone(),
            state.clone(),
        ));
    }
}

/// Returns a [`tokio::net::TcpListener`] with the provided address.
///
/// Sets the socket's TCP keepalive so that unresponsive
/// connections close after 10 minutes to save resources.
fn get_tcp_listener(addr: SocketAddr) -> anyhow::Result<tokio::net::TcpListener> {
    // create a socket
    let socket = socket2::Socket::new(Domain::for_address(addr), Type::STREAM, Some(Protocol::TCP))
        .context("Couldn't create TCP socket")?;

    // if this is an IPv6 listener, make it not listen
    // to IPv4.
    if addr.is_ipv6() {
        socket
            .set_only_v6(true)
            .with_context(|| format!("Couldn't set IPV6_V6ONLY on {addr}"))?;
    }

    // sets the keepalive to 10 minutes
    let tcp_keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(60))
        .with_interval(Duration::from_secs(10));
    socket
        .set_tcp_keepalive(&tcp_keepalive)
        .context("Couldn't set TCP keepalive")?;

    socket
        .bind(&addr.into())
        .with_context(|| format!("Couldn't bind socket to address {addr}"))?;

    socket
        .listen(128)
        .with_context(|| format!("Couldn't listen on {addr}"))?;

    let listener: std::net::TcpListener = socket.into();

    listener
        .set_nonblocking(true)
        .context("Couldn't set TCP socket to non blocking")?;

    // convert to a tokio listener
    let listener = tokio::net::TcpListener::from_std(listener)
        .context("Couldn't create async TCP listener")?;

    Ok(listener)
}

/// Takes paths to a PEM-encoded private key and signed certificate.
/// Returns a [`TlsAcceptor`].
fn get_tls_acceptor(key_path: &Path, cert_path: &Path) -> anyhow::Result<TlsAcceptor> {
    // try reading the key file
    let key = std::fs::File::open(key_path)
        .with_context(|| format!("Couldn't open key file {key_path:?}."))?;
    let mut key = BufReader::new(key);

    // try parsing the key file
    let key = rustls_pemfile::private_key(&mut key)
        .with_context(|| format!("Couldn't parse key file {key_path:?}."))?
        .ok_or(anyhow!("No private keys found in file {key_path:?}."))?;

    // try reading the certificate file
    let cert = std::fs::File::open(cert_path)
        .with_context(|| format!("Couldn't open certificate file {cert_path:?}."))?;
    let mut cert = BufReader::new(cert);

    // try parsing the certificate file
    let cert: Result<Vec<CertificateDer<'static>>, _> = rustls_pemfile::certs(&mut cert).collect();
    let cert = cert.with_context(|| format!("Couldn't parse certificate file {cert_path:?}."))?;

    // try creating tls config
    let tls_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert, key)
        .context("Couldn't configure TLS")?;

    // create a tls acceptor
    Ok(tokio_rustls::TlsAcceptor::from(Arc::new(tls_config)))
}
