use crate::Error;
use gday_contact_exchange_protocol::{Contact, FullContact};
use socket2::{SockRef, TcpKeepalive};
use spake2::{Ed25519Group, Identity, Password, Spake2};
use std::{future::Future, net::SocketAddr, pin::Pin, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpSocket, TcpStream},
};

type PeerConnection = (TcpStream, [u8; 32]);

/// Tries to establish a TCP connection with the other peer by using
/// [TCP hole punching](https://en.wikipedia.org/wiki/TCP_hole_punching).

pub fn try_connect_to_peer(
    local_contact: Contact,
    peer_contact: FullContact,
    shared_secret: &[u8],
) -> std::io::Result<PeerConnection> {
    let p = shared_secret;
    let mut futs: Vec<Pin<Box<dyn Future<Output = std::io::Result<PeerConnection>>>>> =
        Vec::with_capacity(6);

    if let Some(local) = local_contact.v6 {
        futs.push(Box::pin(try_accept(local, p)));

        if let Some(peer) = peer_contact.private.v6 {
            futs.push(Box::pin(try_connect(local, peer, p)));
        }
        if let Some(peer) = peer_contact.public.v6 {
            futs.push(Box::pin(try_connect(local, peer, p)));
        }
    }

    if let Some(local) = local_contact.v4 {
        futs.push(Box::pin(try_accept(local, p)));

        if let Some(peer) = peer_contact.private.v4 {
            futs.push(Box::pin(try_connect(local, peer, p)));
        }

        if let Some(peer) = peer_contact.public.v4 {
            futs.push(Box::pin(try_connect(local, peer, p)));
        }
    }

    Ok(futures::executor::block_on(futures::future::select_ok(futs))?.0)
}

/// Tries to connect to a socket address.
async fn try_connect<T: Into<SocketAddr>>(
    local: T,
    peer: T,
    shared_secret: &[u8],
) -> std::io::Result<PeerConnection> {
    let local = local.into();
    let peer = peer.into();
    loop {
        let local_socket = get_local_socket(local)?;
        let stream = local_socket.connect(peer).await?;
        if let Ok(connection) = verify_peer(shared_secret, stream).await {
            return Ok(connection);
        }
    }
}

/// Tries to accept a connection from a socket address.
async fn try_accept(
    local: impl Into<SocketAddr>,
    shared_secret: &[u8],
) -> std::io::Result<PeerConnection> {
    let local = local.into();
    let local_socket = get_local_socket(local)?;
    let listener = local_socket.listen(1024)?;
    loop {
        let (stream, _addr) = listener.accept().await?;
        if let Ok(connection) = verify_peer(shared_secret, stream).await {
            return Ok(connection);
        }
    }
}

/// Uses SPAKE 2 to generate a cryptographically stronger secret using `shared_secret`.
/// Confirms that the other peer has the same strong shared secret.
/// If successfull, returns a [`PeerConnection`].
async fn verify_peer(shared_secret: &[u8], mut stream: TcpStream) -> Result<PeerConnection, Error> {
    // Password authenticated key exchange
    let (spake, outbound_msg) = Spake2::<Ed25519Group>::start_symmetric(
        &Password::new(shared_secret),
        &Identity::new(b"gday peers"),
    );

    stream.write_all(&outbound_msg).await?;
    stream.flush().await?;

    let mut inbound_msg = [0; 33];
    stream.read_exact(&mut inbound_msg).await?;

    let shared_key: [u8; 32] = spake.finish(&inbound_msg)?.try_into().expect("unreachable");

    // mutually verify that we have the same `shared_key`

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
        .with_time(Duration::from_secs(10))
        .with_interval(Duration::from_secs(1))
        .with_retries(10);
    let _ = sock.set_tcp_keepalive(&keepalive);

    socket.bind(local_addr)?;
    Ok(socket)
}
