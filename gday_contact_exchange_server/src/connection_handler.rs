use crate::state::{self, State};
use gday_contact_exchange_protocol::{
    deserialize_from_async, serialize_into_async, ClientMsg, ServerMsg,
};
use log::{debug, warn};
use std::net::SocketAddr;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio_rustls::TlsAcceptor;

/// Establishes a tls connection with the `tls_acceptor` on this `tcp_stream`.
/// Handles all incoming requests.
/// Exits with an error message if an issue is encountered.
pub async fn handle_connection(
    mut tcp_stream: TcpStream,
    tls_acceptor: Option<TlsAcceptor>,
    state: State,
) {
    // try establishing a TLS connection

    let origin = match tcp_stream.peer_addr() {
        Ok(origin) => origin,
        Err(err) => {
            warn!("Couldn't get client's IP address: {err}");
            return;
        }
    };

    if let Some(tls_acceptor) = tls_acceptor {
        let mut tls_stream = match tls_acceptor.accept(tcp_stream).await {
            Ok(tls_stream) => tls_stream,
            Err(err) => {
                warn!("Error establishing TLS connection: {err}");
                return;
            }
        };
        handle_requests(&mut tls_stream, state, origin)
            .await
            .unwrap_or_else(|err| {
                debug!("Dropping connection because: {err}");
            });
    } else {
        handle_requests(&mut tcp_stream, state, origin)
            .await
            .unwrap_or_else(|err| {
                debug!("Dropping connection because: {err}");
            });
    }
}

/// Handles requests from this connection.
/// Returns an error if any problem is encountered.
async fn handle_requests(
    tls: &mut (impl AsyncRead + AsyncWrite + Unpin),
    mut state: State,
    origin: SocketAddr,
) -> Result<(), HandleMessageError> {
    loop {
        let result = handle_message(tls, &mut state, origin).await;
        match result {
            Ok(()) => (),
            Err(HandleMessageError::State(state::Error::NoSuchRoomCode)) => {
                serialize_into_async(ServerMsg::ErrorNoSuchRoomID, tls).await?;
            }
            Err(HandleMessageError::Receiver(_)) => {
                serialize_into_async(ServerMsg::ErrorPeerTimedOut, tls).await?;
            }
            Err(HandleMessageError::State(state::Error::RoomCodeTaken)) => {
                serialize_into_async(ServerMsg::ErrorRoomTaken, tls).await?;
            }
            Err(HandleMessageError::State(state::Error::TooManyRequests)) => {
                serialize_into_async(ServerMsg::ErrorTooManyRequests, tls).await?;
                return result;
            }
            Err(HandleMessageError::Protocol(_)) => {
                serialize_into_async(ServerMsg::ErrorSyntax, tls).await?;
                return result;
            }
            Err(HandleMessageError::IO(_)) => {
                serialize_into_async(ServerMsg::ErrorConnection, tls).await?;
                return result;
            }
        }
    }
}

async fn handle_message(
    tls: &mut (impl AsyncRead + AsyncWrite + Unpin),
    state: &mut State,
    origin: SocketAddr,
) -> Result<(), HandleMessageError> {
    // try to deserialize the message
    let msg: ClientMsg = deserialize_from_async(tls).await?;

    // handle the message
    match msg {
        ClientMsg::CreateRoom { room_code } => {
            // try to create a room
            state.create_room(room_code, origin.ip())?;

            // acknowledge that a room was created
            serialize_into_async(ServerMsg::RoomCreated, tls).await?;
        }

        ClientMsg::SendAddr {
            room_code,
            is_creator,
            private_addr,
        } => {
            // record their public socket address from the connection
            state.update_client(room_code, is_creator, origin, true, origin.ip())?;

            // record their private socket address if they provided one
            if let Some(private_addr) = private_addr {
                state.update_client(room_code, is_creator, private_addr, false, origin.ip())?;
            }

            // acknowledge the receipt
            serialize_into_async(ServerMsg::ReceivedAddr, tls).await?;
        }

        ClientMsg::DoneSending {
            room_code,
            is_creator,
        } => {
            let (client_contact, rx) = state.set_client_done(room_code, is_creator, origin.ip())?;

            // responds to the client with their own contact info
            serialize_into_async(ServerMsg::ClientContact(client_contact), tls).await?;

            // wait for the peer to be done sending as well
            let peer_contact = rx.await?;

            // send the peer's contact info to this client
            serialize_into_async(ServerMsg::PeerContact(peer_contact), tls).await?;
        }
    }
    Ok(())
}

#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
enum HandleMessageError {
    #[error("Protocol error: {0}")]
    Protocol(#[from] gday_contact_exchange_protocol::Error),

    #[error("Server state error: {0}")]
    State(#[from] state::Error),

    #[error("Peer timed out waiting for other peer.")]
    Receiver(#[from] tokio::sync::oneshot::error::RecvError),

    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),
}
