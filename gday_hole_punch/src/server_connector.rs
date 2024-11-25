//! Functions for connecting to a Gday server.
use crate::Error;
use gday_contact_exchange_protocol::Contact;
use log::{debug, warn};
use rand::seq::SliceRandom;
use socket2::SockRef;
use std::fmt::Debug;
use std::io::ErrorKind;
use std::net::SocketAddr::{V4, V6};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::net::{TcpStream, ToSocketAddrs};

pub use gday_contact_exchange_protocol::DEFAULT_PORT;

/// List of default public Gday servers.
///
/// Having many server options helps make Gday decentralized!
/// - Submit an issue on Gday's GitHub if you'd like to add your own!
/// - All of these serve encrypted TLS over [`DEFAULT_PORT`].
pub const DEFAULT_SERVERS: &[ServerInfo] = &[ServerInfo {
    domain_name: "gday.manforowicz.com",
    id: 1,
    prefer: true,
}];

/// Information about a single public Gday server
/// that serves over TLS on [`DEFAULT_PORT`]
///
/// See [`DEFAULT_SERVERS`] for a list
/// of [`ServerInfo`]
#[derive(Debug, Clone)]
pub struct ServerInfo {
    /// The DNS name of the server.
    pub domain_name: &'static str,
    /// The unique ID of the server.
    ///
    /// Used in [`crate::PeerCode`] when telling
    /// the other peer which server to connect to.
    /// Should NOT be zero, since peers can use that value to represent
    /// a custom server.
    pub id: u64,
    /// Only servers with `prefer` are considered when choosing a random
    /// server to connect to.
    ///
    /// However, all servers are considered when connecting to an `id`
    /// given by a peer.
    ///
    /// Very new servers shouldn't be preferred, to ensure compatibility with
    /// peers that don't yet know about them.
    pub prefer: bool,
}

/// A TCP or TLS stream to a server.
#[pin_project::pin_project(project = EnumProj)]
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ServerStream {
    TCP(#[pin] tokio::net::TcpStream),
    TLS(#[pin] tokio_rustls::client::TlsStream<tokio::net::TcpStream>),
}

impl ServerStream {
    /// Returns the local socket address of this stream.
    pub fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        match self {
            Self::TCP(tcp) => tcp.local_addr(),
            Self::TLS(tls) => tls.get_ref().0.local_addr(),
        }
    }

    /// Enables SO_REUSEADDR and SO_REUSEPORT (if applicable)
    /// so that this socket can be reused for
    /// hole punching.
    fn enable_reuse(&self) {
        let tcp_stream = match self {
            Self::TCP(tcp) => tcp,
            Self::TLS(tls) => tls.get_ref().0,
        };

        let sock = SockRef::from(tcp_stream);
        let _ = sock.set_reuse_address(true);

        // socket2 only supports this method on these systems
        #[cfg(all(unix, not(any(target_os = "solaris", target_os = "illumos"))))]
        let _ = sock.set_reuse_port(true);
    }
}

impl tokio::io::AsyncRead for ServerStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.project() {
            EnumProj::TCP(tcp) => tcp.poll_read(cx, buf),
            EnumProj::TLS(tls) => tls.poll_read(cx, buf),
        }
    }
}

impl tokio::io::AsyncWrite for ServerStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        match self.project() {
            EnumProj::TCP(tcp) => tcp.poll_write(cx, buf),
            EnumProj::TLS(tls) => tls.poll_write(cx, buf),
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.project() {
            EnumProj::TCP(tcp) => tcp.poll_flush(cx),
            EnumProj::TLS(tls) => tls.poll_flush(cx),
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.project() {
            EnumProj::TCP(tcp) => tcp.poll_shutdown(cx),
            EnumProj::TLS(tls) => tls.poll_shutdown(cx),
        }
    }
}

/// Connection to a Gday server.
///
/// Can hold an IPv4 and/or IPv6 [`ServerStream`] to a Gday server.
#[derive(Debug)]
pub struct ServerConnection {
    pub v4: Option<ServerStream>,
    pub v6: Option<ServerStream>,
}

// some private helper functions used by contact_sharer
impl ServerConnection {
    /// Enables `SO_REUSEADDR` and `SO_REUSEPORT` so that the ports of
    /// these sockets can be reused for hole punching.
    ///
    /// Returns an error if both streams are `None`.
    /// Returns an error if a `v4` is passed where `v6` should, or vice versa.
    pub(super) fn enable_reuse(&self) -> Result<(), Error> {
        if self.v4.is_none() && self.v6.is_none() {
            return Err(Error::ServerConnectionEmpty);
        }

        if let Some(stream) = &self.v4 {
            let addr = stream.local_addr()?;
            if !matches!(addr, V4(_)) {
                return Err(Error::ServerConnectionMismatch);
            };
            stream.enable_reuse();
        }

        if let Some(stream) = &self.v6 {
            let addr = stream.local_addr()?;
            if !matches!(addr, V6(_)) {
                return Err(Error::ServerConnectionMismatch);
            };
            stream.enable_reuse();
        }
        Ok(())
    }

    /// Returns a [`Vec`] of all the [`ServerStream`]s in this connection.
    /// Will return `v6` followed by `v4`
    pub(super) fn streams(&mut self) -> Vec<&mut ServerStream> {
        let mut streams = Vec::with_capacity(2);

        if let Some(stream) = &mut self.v6 {
            streams.push(stream);
        }

        if let Some(stream) = &mut self.v4 {
            streams.push(stream);
        }

        streams
    }

    /// Returns the local [`Contact`] of this server stream.
    pub fn local_contact(&self) -> Result<Contact, Error> {
        let mut contact = Contact { v4: None, v6: None };

        if let Some(stream) = &self.v4 {
            if let SocketAddr::V4(addr_v4) = stream.local_addr()? {
                contact.v4 = Some(addr_v4);
            } else {
                return Err(Error::ServerConnectionMismatch);
            }
        }

        if let Some(stream) = &self.v6 {
            if let SocketAddr::V6(addr_v6) = stream.local_addr()? {
                contact.v6 = Some(addr_v6);
            } else {
                return Err(Error::ServerConnectionMismatch);
            }
        }

        Ok(contact)
    }
}

/// In random order, sequentially try connecting to `servers`.
///
/// You may pass [`DEFAULT_SERVERS`] as `servers`.
///
/// Ignores servers that don't have `prefer == true`.
/// Connects to port [`DEFAULT_PORT`] via TLS.
/// Tries the next server after `timeout` time.
///
/// Returns
/// - The [`ServerConnection`] of the first successful connection.
/// - The `id` of the server that [`ServerConnection`] connected to.
///
/// Returns an error if all connection attempts failed.
pub async fn connect_to_random_server(
    servers: &[ServerInfo],
    timeout: Duration,
) -> Result<(ServerConnection, u64), Error> {
    // Filter out non-preferred servers
    let preferred: Vec<&ServerInfo> = servers.iter().filter(|s| s.prefer).collect();

    // Get the domain names of the preferred servers
    let preferred_names: Vec<&str> = preferred.iter().map(|s| s.domain_name).collect();

    // Try connecting to the them in a random order
    let (conn, i) = connect_to_random_domain_name(&preferred_names, timeout).await?;
    Ok((conn, preferred[i].id))
}

/// Tries connecting to the server with this `server_id`
///
/// You may pass [`DEFAULT_SERVERS`] as `servers`.
///
/// Connects to port [`DEFAULT_PORT`] via TLS.
/// Gives up after `timeout` time.
///
/// Returns an error if `servers` contains no server with id `server_id` or connecting
/// to the server fails.
pub async fn connect_to_server_id(
    servers: &[ServerInfo],
    server_id: u64,
    timeout: Duration,
) -> Result<ServerConnection, Error> {
    let Some(server) = servers.iter().find(|server| server.id == server_id) else {
        return Err(Error::ServerIDNotFound(server_id));
    };
    connect_tls(server.domain_name.to_string(), DEFAULT_PORT, timeout).await
}

/// In random order, sequentially tries connecting to the given `domain_names`.
///
/// Connects to port [`DEFAULT_PORT`] via TLS.
/// Tries the next connection after `timeout` time.
///
/// Returns
/// - The [`ServerConnection`] of the first successful connection.
/// - The index of the address in `addresses` that the [`ServerConnection`] connected to.
///
/// Returns an error only if all connection attempts failed.
pub async fn connect_to_random_domain_name(
    domain_names: &[&str],
    timeout: Duration,
) -> Result<(ServerConnection, usize), Error> {
    let mut indices: Vec<usize> = (0..domain_names.len()).collect();
    indices.shuffle(&mut rand::thread_rng());

    let mut recent_error = Error::CouldntConnectToServers;

    for i in indices {
        let server = domain_names[i];
        match connect_tls(server.to_string(), DEFAULT_PORT, timeout).await {
            Ok(streams) => return Ok((streams, i)),
            Err(err) => {
                recent_error = err;
                warn!("Couldn't connect to \"{server}:{DEFAULT_PORT}\": {recent_error}");
                continue;
            }
        };
    }
    Err(recent_error)
}

/// Tries to TLS connect to `domain_name` over both IPv4 and IPv6.
///
/// - Returns a [`ServerConnection`] with all the successful TLS streams.
/// - Gives up connecting to each TCP address after `timeout` time.
/// - Returns an error if couldn't connect to any of IPv4 and IPv6.
/// - Returns an error for any issues with TLS.
pub async fn connect_tls(
    domain_name: String,
    port: u16,
    timeout: Duration,
) -> Result<ServerConnection, Error> {
    debug!("Connecting to server '{domain_name}:{port}'");

    // Connect to the server over TCP
    let mut connection: ServerConnection =
        connect_tcp((domain_name.as_str(), port), timeout).await?;

    // wrap the DNS name of the server
    let name = tokio_rustls::rustls::pki_types::ServerName::try_from(domain_name)?;

    // get the TLS config
    let tls_config = get_tls_config();

    let connector = tokio_rustls::TlsConnector::from(tls_config);

    if let Some(tcp_v4) = connection.v4 {
        let ServerStream::TCP(tcp_v4) = tcp_v4 else {
            unreachable!()
        };
        connection.v4 = Some(ServerStream::TLS(
            connector.connect(name.clone(), tcp_v4).await?,
        ));
    }

    if let Some(tcp_v6) = connection.v6 {
        let ServerStream::TCP(tcp_v6) = tcp_v6 else {
            unreachable!()
        };
        connection.v6 = Some(ServerStream::TLS(
            connector.connect(name.clone(), tcp_v6).await?,
        ));
    }

    Ok(connection)
}

/// Tries to TCP connect to `addrs` over both IPv4 and IPv6.
///
/// - Returns a [`ServerConnection`] with all the successful TCP streams.
/// - Gives up connecting to each TCP address after `timeout` time.
/// - Returns an error if couldn't connect to any of IPv4 and IPv6.
pub async fn connect_tcp(
    addrs: impl ToSocketAddrs + Debug,
    timeout: Duration,
) -> std::io::Result<ServerConnection> {
    // Try to get an IPv4 and IPv6 socket address.
    let mut addr_v4 = None;
    let mut addr_v6 = None;
    for addr in tokio::net::lookup_host(&addrs).await? {
        if addr.is_ipv6() && addr_v6.is_none() {
            addr_v6 = Some(addr);
        } else if addr.is_ipv4() && addr_v4.is_none() {
            addr_v4 = Some(addr);
        } else if addr_v4.is_some() && addr_v6.is_some() {
            break;
        }
    }

    // try connecting to the first IPv4 address
    let tcp_v4 = if let Some(addr) = addr_v4 {
        if let Ok(result) = tokio::time::timeout(timeout, TcpStream::connect(addr)).await {
            Some(result)
        } else {
            Some(Err(std::io::Error::new(
                ErrorKind::TimedOut,
                format!("Timed out while trying to connect to {addrs:?}."),
            )))
        }
    } else {
        None
    };

    // try connecting to the first IPv6 addresss
    let tcp_v6 = if let Some(addr) = addr_v6 {
        if let Ok(result) = tokio::time::timeout(timeout, TcpStream::connect(addr)).await {
            Some(result)
        } else {
            Some(Err(std::io::Error::new(
                ErrorKind::TimedOut,
                format!("Timed out while trying to connect to {addrs:?}."),
            )))
        }
    } else {
        None
    };

    // return an error if couldn't establish any connections
    if !matches!(tcp_v4, Some(Ok(_))) && !matches!(tcp_v6, Some(Ok(_))) {
        if let Some(Err(err)) = tcp_v4 {
            return Err(err);
        } else if let Some(Err(err)) = tcp_v6 {
            return Err(err);
        } else {
            return Err(std::io::Error::new(
                ErrorKind::NotFound,
                format!("Couldn't resolve address {addrs:?}"),
            ));
        }
    }

    let server_connection = ServerConnection {
        v4: if let Some(Ok(v4)) = tcp_v4 {
            Some(ServerStream::TCP(v4))
        } else {
            None
        },
        v6: if let Some(Ok(v6)) = tcp_v6 {
            Some(ServerStream::TCP(v6))
        } else {
            None
        },
    };

    Ok(server_connection)
}

/// Get default TLS config
fn get_tls_config() -> Arc<tokio_rustls::rustls::ClientConfig> {
    let root_store = tokio_rustls::rustls::RootCertStore::from_iter(
        webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
    );

    Arc::new(
        tokio_rustls::rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth(),
    )
}
