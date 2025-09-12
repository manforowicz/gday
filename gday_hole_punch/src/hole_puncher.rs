use crate::Error;
use gday_contact_exchange_protocol::{Contact, FullContact};
use log::{debug, info, trace};
use sha2::Digest;
use socket2::{SockRef, TcpKeepalive};
use spake2::{Ed25519Group, Identity, Password, Spake2};
use std::{net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpSocket,
};

/// Alias to the return type of [`try_connect_to_peer()`].
type PeerConnection = (tokio::net::TcpStream, [u8; 32]);

/// How often a connection attempt is made during hole punching.
const RETRY_INTERVAL: Duration = Duration::from_millis(200);

/// Tries to connect to the other peer using
/// [TCP hole punching](https://en.wikipedia.org/wiki/TCP_hole_punching).
///
/// Call this function _after_ you've gotten the peer's contacts with
/// [`crate::share_contacts()`].
///
/// Arguments:
/// - `local_contact` should be the `local` field of your [`FullContact`] that
///   [`crate::share_contacts()`] returned.
/// - `peer_contact` should be the peer's [`FullContact`] returned by the future
///   from [`crate::share_contacts()`].
/// - `shared_secret` should be a secret that both peers know.
///   It will be used to verify the peer's identity, and derive a stronger shared key
///   using [SPAKE2](https://docs.rs/spake2/).
///
/// Returns:
/// - An authenticated [`std::net::TcpStream`] connected to the other peer.
/// - A `[u8; 32]` shared key that was derived using [SPAKE2](https://docs.rs/spake2/)
///   from the weaker `shared_secret`.
pub async fn try_connect_to_peer(
    local_contact: Contact,
    peer_contact: FullContact,
    shared_secret: impl AsRef<[u8]>,
) -> Result<PeerConnection, Error> {
    let p = shared_secret.as_ref();

    // A set of tasks that will run concurrently,
    // trying to establish a connection to the peer.
    let mut tasks = tokio::task::JoinSet::new();

    // If we have an IPv4 socket address
    if let Some(local) = local_contact.v4 {
        // listen to connections from the peer
        tasks.spawn(try_accept(local, p.to_vec()));

        // try connecting to the peer's private socket address
        if let Some(peer) = peer_contact.local.v4 {
            tasks.spawn(try_connect(local, peer, p.to_vec()));
        }

        // try connecting to the peer's public socket address
        if let Some(peer) = peer_contact.public.v4 {
            tasks.spawn(try_connect(local, peer, p.to_vec()));
        }
    }

    // If we have an IPv6 socket address
    if let Some(local) = local_contact.v6 {
        // listen to connections from the peer
        tasks.spawn(try_accept(local, p.to_vec()));

        // try connecting to the peer's private socket address
        if let Some(peer) = peer_contact.local.v6 {
            tasks.spawn(try_connect(local, peer, p.to_vec()));
        }

        // try connecting to the peer's public socket address
        if let Some(peer) = peer_contact.public.v6 {
            tasks.spawn(try_connect(local, peer, p.to_vec()));
        }
    }

    // Wait for the first hole-punch attempt to complete.
    // Return its outcome.
    // Note: the try_connect() and try_accept() functions
    // will only return error, when something critical goes
    // wrong. Otherwise they'll keep trying.
    match tasks.join_next().await {
        // A task finished
        Some(Ok(result)) => result,

        // Couldn't join the task
        Some(Err(..)) => panic!("Tokio join error."),

        // No tasks were spawned
        None => Err(Error::LocalContactEmpty),
    }
}

/// Tries to TCP connect from `local` to `peer`,
/// and authenticate using `shared_secret`.
async fn try_connect<T: Into<SocketAddr>>(
    local: T,
    peer: T,
    shared_secret: Vec<u8>,
) -> Result<PeerConnection, Error> {
    let local = local.into();
    let peer = peer.into();
    let mut interval = tokio::time::interval(RETRY_INTERVAL);
    trace!("Trying to connect from {local} to {peer}.");

    let stream = loop {
        let local_socket = get_local_socket(local)?;
        if let Ok(stream) = local_socket.connect(peer).await {
            break stream;
        }
        // wait some time to avoid flooding the network
        interval.tick().await;
    };

    debug!("Connected from {local} to {peer}. Will try to authenticate.");
    verify_peer(&shared_secret, stream).await
}

/// Tries to accept a peer TCP connection on `local`,
/// and authenticate using `shared_secret`.
async fn try_accept(
    local: impl Into<SocketAddr>,
    shared_secret: Vec<u8>,
) -> Result<PeerConnection, Error> {
    let local = local.into();
    let mut interval = tokio::time::interval(RETRY_INTERVAL);
    trace!("Waiting to accept connections on {local}.");

    let local_socket = get_local_socket(local)?;
    let listener = local_socket.listen(128)?;

    let (stream, addr) = loop {
        if let Ok(ok) = listener.accept().await {
            break ok;
        }
        // wait some time to avoid flooding the network
        interval.tick().await;
    };

    debug!("Received connection on {local} from {addr}. Will try to authenticate.");
    verify_peer(&shared_secret, stream).await
}

/// Uses [SPAKE 2](https://docs.rs/spake2/latest/spake2/)
/// to derive a cryptographically secure secret from
/// a `weak_secret`.
/// Verifies that the other peer derived the same secret.
/// If successful, returns a [`PeerConnection`].
async fn verify_peer(
    weak_secret: &[u8],
    mut stream: tokio::net::TcpStream,
) -> Result<PeerConnection, Error> {
    info!("Connected. Verifying peer's identity.");

    // send greeting to peer
    stream.write_all(b"gday_hole_punch").await?;

    // confirm peer sent greeting
    let mut greeting = [0_u8; 15];
    stream.read_exact(&mut greeting).await?;
    if greeting != *b"gday_hole_punch" {
        return Err(Error::WrongGreeting);
    }

    //// Password authenticated key exchange ////
    let (spake, outbound_msg) = Spake2::<Ed25519Group>::start_symmetric(
        &Password::new(weak_secret),
        &Identity::new(b"gday_hole_punch"),
    );

    stream.write_all(&outbound_msg).await?;
    stream.flush().await?;

    let mut inbound_msg = [0; 33];
    stream.read_exact(&mut inbound_msg).await?;

    let shared_key: [u8; 32] = spake
        .finish(&inbound_msg)?
        .try_into()
        .expect("Unreachable: Key is always 32 bytes long.");

    //// Mutually verify that we have the same `shared_key` ////

    // send a random challenge to the peer
    let my_challenge: [u8; 32] = rand::random();
    stream.write_all(&my_challenge).await?;
    stream.flush().await?;

    // receive the peer's random challenge
    let mut peer_challenge = [0; 32];
    stream.read_exact(&mut peer_challenge).await?;

    // reply with the solution hash to the peer's challenge
    let mut hasher = sha2::Sha256::new();
    hasher.update(shared_key);
    hasher.update(peer_challenge);
    let my_hash = hasher.finalize();
    stream.write_all(&my_hash).await?;
    stream.flush().await?;

    // receive peer's hash to my challenge
    let mut peer_hash = [0; 32];
    stream.read_exact(&mut peer_hash).await?;

    // confirm peer's hash to my challenge
    let mut hasher = sha2::Sha256::new();
    hasher.update(shared_key);
    hasher.update(my_challenge);
    let expected = hasher.finalize();

    // Peer authentication failed
    if expected != peer_hash.into() {
        return Err(Error::PeerAuthenticationFailed);
    }

    info!("Peer authentication suceeded.");

    Ok((stream, shared_key))
}

/// Makes a new socket with this address.
/// Enables `SO_REUSEADDR` and `SO_REUSEPORT` so that the ports of
/// these streams can be reused for hole punching.
/// Enables TCP keepalive to avoid dead connections.
fn get_local_socket(local_addr: SocketAddr) -> std::io::Result<TcpSocket> {
    let socket = match local_addr {
        SocketAddr::V6(_) => TcpSocket::new_v6()?,
        SocketAddr::V4(_) => TcpSocket::new_v4()?,
    };

    let sock = SockRef::from(&socket);

    sock.set_reuse_address(true)?;

    // socket2 only supports this method on these systems
    #[cfg(all(
        unix,
        not(any(target_os = "solaris", target_os = "illumos", target_os = "cygwin"))
    ))]
    sock.set_reuse_port(true)?;

    let keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(60))
        .with_interval(Duration::from_secs(10));
    sock.set_tcp_keepalive(&keepalive)?;

    socket.bind(local_addr)?;
    Ok(socket)
}
