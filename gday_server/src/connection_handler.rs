use crate::state::{self, State};
use gday_contact_exchange_protocol::{read_from_async, write_to_async, ClientMsg, ServerMsg};
use log::{error, info, warn};
use std::net::SocketAddr;
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
};
use tokio_rustls::TlsAcceptor;

/// Handle this incoming `tcp_stream`.
/// Establishes a TLS connection if `tls_acceptor.is_some()`
/// Handles all incoming requests.
/// Logs information and errors with [`log`].
pub async fn handle_connection(
    mut tcp_stream: TcpStream,
    origin: SocketAddr,
    tls_acceptor: Option<TlsAcceptor>,
    state: State,
) {
    if let Some(tls_acceptor) = tls_acceptor {
        let mut tls_stream = match tls_acceptor.accept(tcp_stream).await {
            Ok(tls_stream) => tls_stream,
            Err(err) => {
                warn!("Error establishing TLS connection with '{origin}': {err}");
                return;
            }
        };
        let _ = handle_requests(&mut tls_stream, state, origin).await;
        // Graceful TLS termination
        let _ = tls_stream.shutdown().await;
    } else {
        let _ = handle_requests(&mut tcp_stream, state, origin).await;
    }
}

/// Handles requests from this connection.
/// Returns an error if any problem is encountered.
async fn handle_requests(
    stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
    mut state: State,
    origin: SocketAddr,
) -> Result<(), HandleMessageError> {
    loop {
        let result = handle_message(stream, &mut state, origin).await;
        match result {
            Ok(()) => (),
            Err(HandleMessageError::State(state::Error::NoSuchRoomCode)) => {
                warn!("Replying with ServerMsg::ErrorNoSuchRoomCode.");
                write_to_async(ServerMsg::ErrorNoSuchRoomCode, stream).await?;
            }
            Err(HandleMessageError::Receiver(_)) => {
                warn!("Replying with ServerMsg::ErrorPeerTimedOut.");
                write_to_async(ServerMsg::ErrorPeerTimedOut, stream).await?;
            }
            Err(HandleMessageError::State(state::Error::RoomCodeTaken)) => {
                warn!("Replying with ServerMsg::ErrorRoomTaken.");
                write_to_async(ServerMsg::ErrorRoomTaken, stream).await?;
            }
            Err(HandleMessageError::State(state::Error::TooManyRequests)) => {
                warn!("Replying with ServerMsg::ErrorTooManyRequests and disconnecting.");
                write_to_async(ServerMsg::ErrorTooManyRequests, stream).await?;
                return result;
            }
            Err(HandleMessageError::State(state::Error::CantUpdateDoneClient)) => {
                warn!("Replying with ServerMsg::ErrorUnexpectedMsg.");
                write_to_async(ServerMsg::ErrorUnexpectedMsg, stream).await?;
            }
            Err(HandleMessageError::Protocol(ref err)) => {
                warn!("Replying with ServerMsg::ErrorSyntax and disconnecting, because: {err}");
                write_to_async(ServerMsg::ErrorSyntax, stream).await?;
                return result;
            }
            Err(HandleMessageError::UnknownMessage(msg)) => {
                warn!("Replying with ServerMsg::ErrorSyntax because received unknown message: {msg:?}");
                write_to_async(ServerMsg::ErrorSyntax, stream).await?;
                return result;
            }
            Err(HandleMessageError::IO(_)) => {
                info!("'{origin}' disconnected.");
                return result;
            }
        }
    }
}

/// Read and handle a single message
async fn handle_message(
    stream: &mut (impl AsyncRead + AsyncWrite + Unpin),
    state: &mut State,
    origin: SocketAddr,
) -> Result<(), HandleMessageError> {
    // read the next message from the client
    let msg: ClientMsg = read_from_async(stream).await?;

    match msg {
        ClientMsg::CreateRoom { room_code } => {
            // try to create a room
            state.create_room(room_code, origin.ip())?;

            // acknowledge that a room was created
            write_to_async(ServerMsg::RoomCreated, stream).await?;
        }

        ClientMsg::RecordPublicAddr {
            room_code,
            is_creator,
        } => {
            // record their public socket address from the connection
            state.update_client(room_code, is_creator, origin, true, origin.ip())?;

            // acknowledge the receipt
            write_to_async(ServerMsg::ReceivedAddr, stream).await?;
        }

        ClientMsg::ReadyToShare {
            room_code,
            is_creator,
            local_contact,
        } => {
            // record the given private socket addresses
            if let Some(sockaddr_v4) = local_contact.v4 {
                state.update_client(
                    room_code,
                    is_creator,
                    sockaddr_v4.into(),
                    false,
                    origin.ip(),
                )?;
            }
            if let Some(sockaddr_v6) = local_contact.v6 {
                state.update_client(
                    room_code,
                    is_creator,
                    sockaddr_v6.into(),
                    false,
                    origin.ip(),
                )?;
            }

            let (client_contact, rx) = state.set_client_done(room_code, is_creator, origin.ip())?;

            // responds to the client with their own contact info
            write_to_async(ServerMsg::ClientContact(client_contact), stream).await?;

            info!("Sent client '{origin}' their contact of '{client_contact}'.");

            // wait for the peer to be done sending as well
            let peer_contact = rx.await?;

            // send the peer's contact info to this client
            write_to_async(ServerMsg::PeerContact(peer_contact), stream).await?;

            info!("Sent client '{origin}' their peer's contact of '{client_contact}'.");
        }
        unknown_msg => return Err(HandleMessageError::UnknownMessage(unknown_msg)),
    }
    Ok(())
}

#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
enum HandleMessageError {
    /// Serialization/deserialization error
    #[error("Serialization/deserialization error: {0}")]
    Protocol(#[from] gday_contact_exchange_protocol::Error),

    /// Error updating server state
    #[error("Error updating server state: {0}")]
    State(#[from] state::Error),

    /// Timed out while waiting for other peer to share contact
    #[error("Timed out while waiting for other peer to share contact: {0}")]
    Receiver(#[from] tokio::sync::oneshot::error::RecvError),

    /// IO Error
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),

    /// Received unknown message from client
    #[error("Received unknown message from client:\n{0:?}")]
    UnknownMessage(gday_contact_exchange_protocol::ClientMsg),
}
