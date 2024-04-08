//! Functions for connecting to a Gday server.
//! TODO: Tidy up this file

use crate::Error;
use log::{debug, error};
use rand::seq::SliceRandom;
use socket2::SockRef;
use std::io::{Read, Write};
use std::net::SocketAddr::{V4, V6};
use std::{
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};

/// The default list of default public Gday servers.
/// - Submit an issue on Gday's GitHub if you'd like to add your own.
/// - All of these only serve encrypted TLS and listen on port 2311.
pub const DEFAULT_SERVERS: &[ServerInfo] = &[ServerInfo {
    domain_name: "gday.manforowicz.com",
    id: 1,
    prefer: true,
}];

/// The port that unencrypted TCP Gday servers listen on.
pub const DEFAULT_TCP_PORT: u16 = 2310;

/// The port that encrypted TLS Gday servers listen on.
pub const DEFAULT_TLS_PORT: u16 = 2311;

/// Information about a single Gday server.
pub struct ServerInfo {
    /// The domain name of the server.
    pub domain_name: &'static str,
    /// The ID of the server. Helpful when telling the other peer which
    /// server to connect to.
    ///
    /// Should NOT be zero, since peers can use that value to represent
    /// a custom server.
    pub id: u64,
    /// Only servers with `prefer` are considered when choosing a random
    /// server to connect to.
    ///
    /// New servers shouldn't be preferred, to ensure compatibility with
    /// peers that don't yet know about them.
    pub prefer: bool,
}

#[allow(clippy::large_enum_variant)]
pub enum ServerStream {
    TCP(TcpStream),
    TLS(rustls::StreamOwned<rustls::ClientConnection, TcpStream>),
}

impl Read for ServerStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::TCP(stream) => stream.read(buf),
            Self::TLS(stream) => stream.read(buf),
        }
    }
}

impl Write for ServerStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::TCP(stream) => stream.write(buf),
            Self::TLS(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::TCP(stream) => stream.flush(),
            Self::TLS(stream) => stream.flush(),
        }
    }
}

impl ServerStream {
    /// Returns the local socket address of this stream
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        match self {
            Self::TCP(stream) => stream.local_addr(),
            Self::TLS(stream) => stream.get_ref().local_addr(),
        }
    }

    /// Enables SO_REUSEADDR and SO_REUSEPORT
    /// So that this socket can be reused for
    /// hole-punching.
    fn enable_reuse(&self) {
        let stream = match self {
            Self::TCP(stream) => stream,
            Self::TLS(stream) => stream.get_ref(),
        };

        let sock = SockRef::from(stream);
        let _ = sock.set_reuse_address(true);
        let _ = sock.set_reuse_port(true);
    }
}

/// Can hold both a IPv4 and IPv6 [`ServerStream`] to a Gday server.
pub struct ServerConnection {
    pub v4: Option<ServerStream>,
    pub v6: Option<ServerStream>,
}

/// Some private helper functions used by [`ContactSharer`]
impl ServerConnection {
    /// Enables `SO_REUSEADDR` and `SO_REUSEPORT` so that the ports of
    /// these streams can be reused for hole punching.
    ///
    /// Returns an error if both streams are `None`.
    /// Returns an error if a `v4` is passed where `v6` should, or vice versa.
    pub(super) fn configure(&self) -> Result<(), Error> {
        if self.v4.is_none() && self.v6.is_none() {
            return Err(Error::NoStreamsProvided);
        }

        if let Some(stream) = &self.v4 {
            let addr = stream.local_addr()?;
            if !matches!(addr, V4(_)) {
                return Err(Error::ExpectedIPv4);
            };
            stream.enable_reuse();
        }

        if let Some(stream) = &self.v6 {
            let addr = stream.local_addr()?;
            if !matches!(addr, V6(_)) {
                return Err(Error::ExpectedIPv6);
            };
            stream.enable_reuse();
        }
        Ok(())
    }

    /// Returns a [`Vec`] of all the [`TLSStream`]s in this connection.
    /// Will return IPV6 followed by IPV4
    pub(super) fn streams(&mut self) -> Vec<&mut ServerStream> {
        let mut streams = Vec::new();

        if let Some(stream) = &mut self.v6 {
            streams.push(stream);
        }

        if let Some(stream) = &mut self.v4 {
            streams.push(stream);
        }

        streams
    }
}

/// In random order, sequentially try connecting to the given `servers`.
/// (connects to port [`DEFAULT_TLS_PORT`] (2311) via TLS)
///
/// Returns
/// - The [`ServerConnection`] of the first successful connection.
/// - The `id` of the server that [`ServerConnection`] connected to.
///
/// Returns an error if all connection attempts failed.
pub fn connect_to_random_server(servers: &[ServerInfo]) -> Result<(ServerConnection, u64), Error> {
    let preferred: Vec<&ServerInfo> = servers.iter().filter(|s| s.prefer).collect();
    let preferred_names: Vec<&str> = preferred.iter().map(|s| s.domain_name).collect();
    let (conn, i) = connect_to_random_domain_name(&preferred_names)?;
    Ok((conn, preferred[i].id))
}

/// Try connecting to the server with this `server_id` and returning a [`ServerConnection`].
/// (connects to port [`DEFAULT_TLS_PORT`] (2311) via TLS)
///
/// Returns an error if `servers` contains no server with id `server_id` or connecting
/// to the server fails.
pub fn connect_to_server_id(
    servers: &[ServerInfo],
    server_id: u64,
) -> Result<ServerConnection, Error> {
    let Some(server) = servers.iter().find(|server| server.id == server_id) else {
        return Err(Error::ServerIDNotFound(server_id));
    };
    connect_to_domain_name(server.domain_name, DEFAULT_TLS_PORT, true)
}

/// In random order, sequentially tries connecting to the given `domain_names`.
/// (connects to port [`DEFAULT_TLS_PORT`] (2311) via TLS)
///
/// Returns
/// - The [`ServerConnection`] of the first successful connection.
/// - The index of the address in `addresses` that the [`ServerConnection`] connected to.
///
/// Returns an error if all connection attempts failed.
pub fn connect_to_random_domain_name(
    domain_names: &[&str],
) -> Result<(ServerConnection, usize), Error> {
    let mut indices: Vec<usize> = (0..domain_names.len()).collect();
    indices.shuffle(&mut rand::thread_rng());

    let mut recent_error = Error::CouldntConnectToServers;

    for i in indices {
        let server = domain_names[i];
        let streams = match connect_to_domain_name(server, DEFAULT_TLS_PORT, true) {
            Ok(streams) => streams,
            Err(err) => {
                recent_error = err;
                error!("Couldn't connect to \"{}\": {}", server, recent_error);
                continue;
            }
        };
        return Ok((streams, i));
    }
    Err(recent_error)
}

const SERVER_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

/// Tries connecting to this `domain_name` and `port`, on both IPv4 and IPv6.
/// - Gives up connecting to each TCP address after 2 seconds.
/// - Returns an error if each attempted failed.
/// - Uses TLS if `encrypt` is true, otherwise uses unencrypted TCP.
/// - Returns an error for any issues with TLS.
/// - Returns a [`ServerConnection`] with all the successful streams.
pub fn connect_to_domain_name(
    domain_name: &str,
    port: u16,
    encrypt: bool,
) -> Result<ServerConnection, Error> {
    let address = format!("{domain_name}:{port}");
    debug!("Connecting to server '{address}`");
    let addrs = address.to_socket_addrs()?;

    let addr_v4: Option<SocketAddr> = addrs.clone().find(|a| a.is_ipv4());
    let tcp_v4 = addr_v4.map(|addr| TcpStream::connect_timeout(&addr, SERVER_CONNECT_TIMEOUT));

    let addr_v6: Option<SocketAddr> = addrs.clone().find(|a| a.is_ipv4());
    let tcp_v6 = addr_v6.map(|addr| TcpStream::connect_timeout(&addr, SERVER_CONNECT_TIMEOUT));

    // return an error if couldn't establish any connections
    if !matches!(tcp_v4, Some(Ok(_))) && !matches!(tcp_v6, Some(Ok(_))) {
        if let Some(Err(err_v4)) = tcp_v4 {
            return Err(Error::IO(err_v4));
        } else if let Some(Err(err_v6)) = tcp_v6 {
            return Err(Error::IO(err_v6));
        } else {
            return Err(Error::CouldntResolveAddress(domain_name.to_string()));
        }
    }

    let mut server_connection = ServerConnection { v4: None, v6: None };
    if encrypt {
        // wrap the DNS name of the server
        let name = rustls::pki_types::ServerName::try_from(domain_name.to_string())?;

        // get the TLS config
        let tls_config = get_tls_config();

        // wrap the TCP in TLS streams, throwing an error if anything goes wrong
        if let Some(Ok(tcp_v4)) = tcp_v4 {
            let conn = rustls::ClientConnection::new(tls_config.clone(), name.clone())?;
            let tls_stream = rustls::StreamOwned::new(conn, tcp_v4);
            server_connection.v4 = Some(ServerStream::TLS(tls_stream));
        }

        if let Some(Ok(tcp_v6)) = tcp_v6 {
            let conn = rustls::ClientConnection::new(tls_config.clone(), name.clone())?;
            let tls_stream = rustls::StreamOwned::new(conn, tcp_v6);
            server_connection.v6 = Some(ServerStream::TLS(tls_stream));
        }
    } else {
        if let Some(Ok(tcp_v4)) = tcp_v4 {
            server_connection.v4 = Some(ServerStream::TCP(tcp_v4));
        }

        if let Some(Ok(tcp_v6)) = tcp_v6 {
            server_connection.v6 = Some(ServerStream::TCP(tcp_v6));
        }
    }
    Ok(server_connection)
}

/// Get default TLS config
fn get_tls_config() -> Arc<rustls::ClientConfig> {
    let root_store =
        rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    Arc::new(
        rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth(),
    )
}
