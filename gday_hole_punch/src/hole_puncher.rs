use crate::Error;
use gday_contact_exchange_protocol::{Contact, FullContact};
use log::{debug, trace};
use socket2::{SockRef, TcpKeepalive};
use spake2::{Ed25519Group, Identity, Password, Spake2};
use std::{net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpSocket,
};

type PeerConnection = (std::net::TcpStream, [u8; 32]);

const RETRY_INTERVAL: Duration = Duration::from_millis(200);

// TODO: Update all comments here!

// TODO: ADD BETTER ERROR REPORTING.
// add a timeout.
// if fails, specify if it failed on connecting to peer, or verifying peer.

/// Tries to establish a TCP connection with the other peer by using
/// [TCP hole punching](https://en.wikipedia.org/wiki/TCP_hole_punching).
///
/// - `local_contact` should be the `private` field of your [`FullContact`]
/// that the [`crate::ContactSharer`] returned when you created or joined a room.
/// - `peer_contact` should be the [`FullContact`] returned by [`crate::ContactSharer::get_peer_contact()`].
/// - `shared_secret` should be a secret that both peers know. It will be used to verify
/// the peer's identity, and derive a stronger shared key using [SPAKE2](https://docs.rs/spake2/latest/spake2/).
///
/// Returns:
/// - A [`std::net::TcpStream`] to the other peer.
/// - A `[u8; 32]` shared key that was derived using
///     [SPAKE2](https://docs.rs/spake2/latest/spake2/) and the weaker `shared_secret`.
pub fn try_connect_to_peer(
    local_contact: Contact,
    peer_contact: FullContact,
    shared_secret: &[u8],
    timeout: std::time::Duration,
) -> Result<PeerConnection, Error> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .expect("Tokio async runtime error.");

    // hole punch asynchronously
    match runtime.block_on(async {
        tokio::time::timeout(
            timeout,
            hole_punch(local_contact, peer_contact, shared_secret),
        )
        .await
    }) {
        Ok(result) => result,
        Err(..) => Err(Error::HolePunchTimeout),
    }
}

/// TODO: Comment
async fn hole_punch(
    local_contact: Contact,
    peer_contact: FullContact,
    shared_secret: &[u8],
) -> Result<PeerConnection, Error> {
    // shorten the variable name for conciseness
    let p = shared_secret;

    let mut futs = tokio::task::JoinSet::new();
    if let Some(local) = local_contact.v4 {
        futs.spawn(try_accept(local, p.to_vec()));

        if let Some(peer) = peer_contact.private.v4 {
            futs.spawn(try_connect(local, peer, p.to_vec()));
        }

        if let Some(peer) = peer_contact.public.v4 {
            futs.spawn(try_connect(local, peer, p.to_vec()));
        }
    }

    if let Some(local) = local_contact.v6 {
        futs.spawn(try_accept(local, p.to_vec()));

        if let Some(peer) = peer_contact.private.v6 {
            futs.spawn(try_connect(local, peer, p.to_vec()));
        }
        if let Some(peer) = peer_contact.public.v6 {
            futs.spawn(try_connect(local, peer, p.to_vec()));
        }
    }
    match futs.join_next().await {
        Some(Ok(result)) => result,
        Some(Err(..)) => panic!("Tokio join error."),
        None => Err(Error::ContactEmpty),
    }
}

/// Tries to TCP connect to `peer` from `local`.
/// Returns the most recent error if not successful by `end_time`.
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
        interval.tick().await;
    };

    debug!("Connected to {peer} from {local}. Will try to authenticate.");
    verify_peer(&shared_secret, stream).await
}

/// Tries to accept a peer TCP connection on `local`.
/// Returns the most recent error if not successful by `end_time`.
async fn try_accept(
    local: impl Into<SocketAddr>,
    shared_secret: Vec<u8>,
) -> Result<PeerConnection, Error> {
    let local = local.into();
    trace!("Waiting to accept connections on {local}.");
    let local_socket = get_local_socket(local)?;
    let listener = local_socket.listen(1024)?;
    let mut interval = tokio::time::interval(RETRY_INTERVAL);

    let (stream, addr) = loop {
        if let Ok(ok) = listener.accept().await {
            break ok;
        }
        // wait some time to avoid flooding the network
        interval.tick().await;
    };

    debug!(
        "Connected from {} to {}. Will try to authenticate.",
        addr,
        stream.local_addr()?
    );

    verify_peer(&shared_secret, stream).await
}

/// Uses [SPAKE 2](https://docs.rs/spake2/latest/spake2/)
/// to derive a cryptographically secure secret from
/// a potentially weak `shared_secret`.
/// Verifies that the other peer derived the same secret.
/// If successful, returns a [`PeerConnection`].
async fn verify_peer(
    shared_secret: &[u8],
    mut stream: tokio::net::TcpStream,
) -> Result<PeerConnection, Error> {
    //// Password authenticated key exchange ////
    let (spake, outbound_msg) = Spake2::<Ed25519Group>::start_symmetric(
        &Password::new(shared_secret),
        &Identity::new(b"gday mates"),
    );

    stream.write_all(&outbound_msg).await?;
    stream.flush().await?;

    let mut inbound_msg = [0; 33];
    stream.read_exact(&mut inbound_msg).await?;

    let shared_key: [u8; 32] = spake
        .finish(&inbound_msg)?
        .try_into()
        .expect("Unreachable: Key is always 32 bytes long.");

    debug!("Derived a strong key with the peer. Will now verify we both have the same key.");

    //// Mutually verify that we have the same `shared_key` ////

    // send a random challenge to the peer
    let my_challenge: [u8; 32] = rand::random();
    stream.write_all(&my_challenge).await?;
    stream.flush().await?;

    // receive the peer's random challenge
    let mut peer_challenge = [0; 32];
    stream.read_exact(&mut peer_challenge).await?;

    // reply with the solution hash to the peer's challenge
    let mut hasher = blake3::Hasher::new();
    hasher.update(&shared_key);
    hasher.update(&peer_challenge);
    let my_hash = hasher.finalize();
    stream.write_all(my_hash.as_bytes()).await?;
    stream.flush().await?;

    // receive peer's hash to my challenge
    let mut peer_hash = [0; 32];
    stream.read_exact(&mut peer_hash).await?;

    // confirm peer's hash to my challenge
    let mut hasher = blake3::Hasher::new();
    hasher.update(&shared_key);
    hasher.update(&my_challenge);
    let expected = hasher.finalize();

    if expected == peer_hash {
        let stream = stream.into_std()?;
        stream.set_nonblocking(false)?;
        Ok((stream, shared_key))
    } else {
        Err(Error::PeerAuthenticationFailed)
    }
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

    let _ = sock.set_reuse_address(true);
    let _ = sock.set_reuse_port(true);

    let keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(5))
        .with_interval(Duration::from_secs(2))
        .with_retries(5);
    let _ = sock.set_tcp_keepalive(&keepalive);

    socket.bind(local_addr)?;
    Ok(socket)
}
