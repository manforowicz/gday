//! Runs a server for the `gday_contact_exchange_protocol`.
//! Lets two users exchange their public and (optionally) private socket addresses.
#![forbid(unsafe_code)]
#![warn(clippy::all)]

mod connection_handler;
mod state;
mod tests;

use clap::Parser;
use connection_handler::handle_connection;
use log::{error, info};
use socket2::{SockRef, TcpKeepalive};
use state::State;
use std::{
    io::BufReader,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::net::{TcpListener, ToSocketAddrs};
use tokio_rustls::{
    rustls::{self},
    TlsAcceptor,
};

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// PEM-encoded private TLS server key
    #[arg(short, long)]
    key: PathBuf,

    /// PEM-encoded signed TLS server certificate
    #[arg(short, long)]
    certificate: PathBuf,

    /// The socket address from which to listen
    #[arg(short, long, default_value = "::8080")]
    address: String,

    /// Number of seconds before a new room is deleted.
    #[arg(short, long, default_value = "300")]
    timeout: u64,

    /// Max number of requests an IP address can
    /// send in a minute before they're rejected.
    #[arg(short, long, default_value = "300")]
    request_limit: u32,

    /// Log verbosity. (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "warn")]
    verbosity: log::LevelFilter,
}

#[tokio::main]
async fn main() {
    // read command line arguments
    let args = Args::parse();

    // set the log level according to the command line argument
    env_logger::builder().filter_level(args.verbosity).init();

    if let Err(err) = run(args).await {
        error!("Exiting because: {err}")
    }
}

async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    // get tcp listener and acceptor
    let tcp_listener = get_tcp_listener(args.address).await?;
    let tls_acceptor = get_tls_acceptor(&args.key, &args.certificate)?;

    // create the shared global state object
    let state = State::new(args.request_limit, args.timeout);

    loop {
        // try to accept another connection
        let (stream, _addr) = match tcp_listener.accept().await {
            Ok(ok) => ok,
            Err(err) => {
                info!("Error accepting connection: {err}");
                continue;
            }
        };

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
async fn get_tcp_listener(
    addr: impl ToSocketAddrs,
) -> Result<TcpListener, Box<dyn std::error::Error>> {
    // binds to the socket address
    let listener = TcpListener::bind(addr).await?;

    // sets the keepalive to 10 minutes
    let socket = SockRef::from(&listener);
    let tcp_keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(540))
        .with_interval(Duration::from_secs(6))
        .with_retries(10);
    let _ = socket.set_tcp_keepalive(&tcp_keepalive);

    Ok(listener)
}

/// Takes paths to a PEM-encoded private key and signed certificate.
/// Returns a [`TlsAcceptor`]
///
/// Exits with an error message if the paths didn't lead to
/// valid files, or there was an error creating the tls config
fn get_tls_acceptor(key: &Path, cert: &Path) -> Result<TlsAcceptor, Box<dyn std::error::Error>> {
    // try reading the key file
    let mut key = BufReader::new(std::fs::File::open(key)?);
    let key = rustls_pemfile::private_key(&mut key).unwrap().unwrap();

    // try reading the certificate file
    let mut cert = BufReader::new(std::fs::File::open(cert)?);
    let cert = rustls_pemfile::certs(&mut cert).next().unwrap()?;

    // try creating tls config
    let tls_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)?;

    // create a tls acceptor
    Ok(tokio_rustls::TlsAcceptor::from(Arc::new(tls_config)))
}
