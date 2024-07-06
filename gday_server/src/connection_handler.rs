use crate::state::{self, State};
use gday_contact_exchange_protocol::{read_from_async, write_to_async, ClientMsg, ServerMsg};
use log::{info, warn};
use std::net::SocketAddr;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio_rustls::TlsAcceptor;

/// Handle this incoming `tcp_stream`.
/// Establishes a TLS connection if `tls_acceptor.is_some()`
/// Handles all incoming requests.
/// Logs information and errors with [`log`].
pub async fn handle_connection(
    mut tcp_stream: TcpStream,
    tls_acceptor: Option<TlsAcceptor>,
    state: State,
) {
    // try establishing a TLS connectio
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
                warn!("Error establishing TLS connection with '{origin}': {err}");
                return;
            }
        };
        handle_requests(&mut tls_stream, state, origin)
            .await
            .unwrap_or_else(|err| {
                info!("Dropping connection with '{origin}' because: {err}");
            });
    } else {
        handle_requests(&mut tcp_stream, state, origin)
            .await
            .unwrap_or_else(|err| {
                info!("Dropping connection with '{origin}' because: {err}");
            });
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
                write_to_async(ServerMsg::ErrorNoSuchRoomCode, stream).await?;
            }
            Err(HandleMessageError::Receiver(_)) => {
                write_to_async(ServerMsg::ErrorPeerTimedOut, stream).await?;
            }
            Err(HandleMessageError::State(state::Error::RoomCodeTaken)) => {
                write_to_async(ServerMsg::ErrorRoomTaken, stream).await?;
            }
            Err(HandleMessageError::State(state::Error::TooManyRequests)) => {
                write_to_async(ServerMsg::ErrorTooManyRequests, stream).await?;
                return result;
            }
            Err(HandleMessageError::Protocol(_)) => {
                write_to_async(ServerMsg::ErrorSyntax, stream).await?;
                return result;
            }
            Err(HandleMessageError::UnknownMessage(_)) => {
                write_to_async(ServerMsg::ErrorSyntax, stream).await?;
            }
            Err(HandleMessageError::IO(_)) => {
                write_to_async(ServerMsg::ErrorConnection, stream).await?;
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
    // try to deserialize the message
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

        ClientMsg::ShareContact {
            room_code,
            is_creator,
            local_contact,
        } => {
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

            // wait for the peer to be done sending as well
            let peer_contact = rx.await?;

            // send the peer's contact info to this client
            write_to_async(ServerMsg::PeerContact(peer_contact), stream).await?;
        }
        unknown_msg => return Err(HandleMessageError::UnknownMessage(unknown_msg)),
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

    #[error("Unknown message from client: {0:?}")]
    UnknownMessage(gday_contact_exchange_protocol::ClientMsg),
}
