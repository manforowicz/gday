use crate::state::{self, State};
use gday_contact_exchange_protocol::{
    deserialize_from_async, serialize_into_async, ClientMsg, ServerMsg,
};
use log::{info, warn};
use tokio::net::TcpStream;
use tokio_rustls::{server::TlsStream, TlsAcceptor};

/// Establishes a tls connection with the `tls_acceptor` on this `tcp_stream`.
/// Handles all incoming requests.
/// Exits with an error message if an issue is encountered.
pub async fn handle_connection(tcp_stream: TcpStream, tls_acceptor: TlsAcceptor, state: State) {
    // try establishing a TLS connection
    let mut tls_stream = match tls_acceptor.accept(tcp_stream).await {
        Ok(tls_stream) => tls_stream,
        Err(err) => {
            warn!("Error establishing TLS connection: {err}");
            return;
        }
    };

    // try handling the requests
    if let Err(err) = handle_requests(&mut tls_stream, state).await {
        info!("Dropping connection because: {err}");
    }
}

/// Handles requests from this connection.
/// Returns an error if any problem is encountered.
async fn handle_requests(
    tls: &mut TlsStream<TcpStream>,
    mut state: State,
) -> Result<(), HandleMessageError> {
    loop {
        let result = handle_message(tls, &mut state).await;
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
                serialize_into_async(ServerMsg::SyntaxError, tls).await?;
                return result;
            }
            Err(HandleMessageError::IO(_)) => {
                serialize_into_async(ServerMsg::ConnectionError, tls).await?;
                return result;
            }
        }
    }
}

async fn handle_message(
    tls: &mut TlsStream<TcpStream>,
    state: &mut State,
) -> Result<(), HandleMessageError> {
    // get this connection's ip address
    let origin = tls.get_ref().0.peer_addr()?;

    // make a buffer to deserialize into
    let buf = &mut [0; gday_contact_exchange_protocol::MAX_MSG_SIZE];

    // try to deserialize the message
    let msg = deserialize_from_async(tls, buf).await?;

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
    #[error("Protocol error")]
    Protocol(#[from] gday_contact_exchange_protocol::Error),

    #[error("Server state error")]
    State(#[from] state::Error),

    #[error("Peer contact receiver error.")]
    Receiver(#[from] tokio::sync::oneshot::error::RecvError),

    #[error("IO Error")]
    IO(#[from] std::io::Error),
}
