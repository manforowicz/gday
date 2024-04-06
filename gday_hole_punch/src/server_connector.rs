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

/// The default list of public Gday servers.
pub const DEFAULT_SERVERS: &[ServerInfo] = &[ServerInfo {
    domain_name: "gday.manforowicz.com",
    id: 1,
    prefer: true,
}];

/// The port that public Gday servers listen on.
pub const DEFAULT_PORT: u16 = 234;

/// Information about a single Gday server.
pub struct ServerInfo {
    /// The domain name of the server.
    pub domain_name: &'static str,
    /// The ID of the server. Helpful when telling the other peer which
    /// server to connect to.
    /// Should NOT be zero, since peers can use that value to represent
    /// a custom server.
    pub id: u64,
    /// Only servers with `prefer` are considered when connecting to a
    /// random server. All servers are available when connecting to a
    /// specific server.
    ///
    /// New servers shouldn't be preferred, to ensure compatibility with
    /// peers that don't know about them.
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

/// Sequentially try connecting to the given servers, returning the first successful connection.
///
/// Returns (
/// - The [`ServerConnection`] to the server.
/// - The `id` of the server that [`ServerConnection`] connected to.
/// )
///
/// Returns an error if connecting fails.
pub fn connect_to_random_server(servers: &[ServerInfo]) -> Result<(ServerConnection, u64), Error> {
    let preferred: Vec<&ServerInfo> = servers.iter().filter(|s| s.prefer).collect();
    let preferred_names: Vec<&str> = preferred.iter().map(|s| s.domain_name).collect();
    let (conn, i) = connect_to_random_address(&preferred_names)?;
    Ok((conn, preferred[i].id))
}

/// Try connecting to the server with this `server_id` and returning a [`ServerConnection`].
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
    connect_to_domain_name(server.domain_name, true)
}

/// Sequentially try connecting to the given addresses, returning the first successful connection.
///
/// Returns (
/// - The [`ServerConnection`] to the server.
/// - The index of the address in `addresses` that the [`ServerConnection`] connected to.
/// )
///
/// Returns an error if connecting fails.
pub fn connect_to_random_address(
    domain_names: &[&str],
) -> Result<(ServerConnection, usize), Error> {
    let mut indices: Vec<usize> = (0..domain_names.len()).collect();
    indices.shuffle(&mut rand::thread_rng());

    for i in indices {
        let server = domain_names[i];
        let streams = match connect_to_domain_name(server, true) {
            Ok(streams) => streams,
            Err(err) => {
                error!("Couldn't connect to \"{}\": {}", server, err);
                continue;
            }
        };
        return Ok((streams, i));
    }
    Err(Error::CouldntConnectToServers)
}

/// Try connecting to this `domain_name` and returning a [`ServerConnection`]
/// TODO: Add info about `encrypt`
pub fn connect_to_domain_name(domain_name: &str, encrypt: bool) -> Result<ServerConnection, Error> {
    let address = format!("{domain_name}:{DEFAULT_PORT}");
    debug!("Connecting to '{address}`");
    let addrs: Vec<SocketAddr> = address.to_socket_addrs()?.collect();

    // try establishing a TCP connection on IPv4
    let tcp_v4 = addrs.iter().find_map(|addr| {
        if let SocketAddr::V4(_) = addr {
            Some(TcpStream::connect_timeout(addr, Duration::from_secs(2)))
        } else {
            None
        }
    });

    // try establishing a TCP connection on IPv6
    let tcp_v6 = addrs.iter().find_map(|addr| {
        if let SocketAddr::V6(_) = addr {
            Some(TcpStream::connect_timeout(addr, Duration::from_secs(2)))
        } else {
            None
        }
    });

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
            let conn_v4 = rustls::ClientConnection::new(tls_config.clone(), name.clone())?;
            let tls_stream = rustls::StreamOwned::new(conn_v4, tcp_v4);
            server_connection.v4 = Some(ServerStream::TLS(tls_stream));
        }

        if let Some(Ok(tcp_v6)) = tcp_v6 {
            let conn_v6 = rustls::ClientConnection::new(tls_config.clone(), name.clone())?;
            let tls_stream = rustls::StreamOwned::new(conn_v6, tcp_v6);
            server_connection.v6 = Some(ServerStream::TLS(tls_stream));
        }
    } else {
        if let Some(Ok(tcp_v4)) = tcp_v4 {
            server_connection.v4 = Some(ServerStream::TCP(tcp_v4));
        }

        if let Some(Ok(tcp_v6)) = tcp_v6 {
            server_connection.v4 = Some(ServerStream::TCP(tcp_v6));
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
