//! Note: this crate is still in early-development, so expect breaking changes.
//!
//! This protocol lets two users exchange their public and (optionally) private socket addresses via a server.
//!
//! On it's own, this library doesn't do anything other than define a shared protocol.
//! In most cases, you should use one of the following crates:
//!
//! - [**gday**](https://crates.io/crates/gday):
//!     A command line tool for sending files to peers.
//! - [**gday_hole_punch**](https://docs.rs/gday_hole_punch/):
//!     A library for establishing a peer-to-peer TCP connection.
//! - [**gday_server**](https://crates.io/crates/gday_server):
//!     A server binary that facilitates this protocol.
//!
//! # Example
//! First, both peers connect with TLS on both IPv4 and IPv6 (if possible)
//! to a gday server with [`DEFAULT_PORT`].
//! Then they exchange contacts like so:
//! ```no_run
//! # use gday_contact_exchange_protocol::{
//! #    ServerMsg,
//! #    ClientMsg,
//! #    write_to,
//! #    read_from,
//! #    Contact
//! # };
//! # let mut tls_ipv4 = std::collections::VecDeque::new();
//! # let mut tls_ipv6 = std::collections::VecDeque::new();
//! #
//! let room_code = 42;
//!
//! // A client tells the server to create a room.
//! // The server responds with ServerMsg::RoomCreated or
//! // ServerMsg::ErrorRoomTaken.
//! let request = ClientMsg::CreateRoom { room_code };
//! write_to(request, &mut tls_ipv4)?;
//! let response: ServerMsg = read_from(&mut tls_ipv4)?;
//!
//! // Each peer sends ClientMsg::RecordPublicAddr
//! // from all their endpoints.
//! // The server records the client's public addresses from these connections.
//! // The server responds with ServerMsg::ReceivedAddr
//! let request = ClientMsg::RecordPublicAddr { room_code, is_creator: true };
//! write_to(request, &mut tls_ipv4)?;
//! let response: ServerMsg = read_from(&mut tls_ipv4)?;
//! write_to(request, &mut tls_ipv6)?;
//! let response: ServerMsg = read_from(&mut tls_ipv6)?;
//!
//! // Both peers share their local address with the server.
//! // The server immediately responds with ServerMsg::ClientContact,
//! // containing each client's FullContact.
//! let local_contact = Contact {
//!     v4: todo!("local v4 addr"),
//!     v6: todo!("local v6 addr")
//! };
//! let request = ClientMsg::ReadyToShare { local_contact, room_code, is_creator: true };
//! write_to(request, &mut tls_ipv4)?;
//! let response: ServerMsg = read_from(&mut tls_ipv4)?;
//!
//! // Once both clients have sent ClientMsg::ShareContact,
//! // the server sends both clients a ServerMsg::PeerContact
//! // containing the FullContact of the peer.
//! let response: ServerMsg = read_from(&mut tls_ipv4)?;
//!
//! // The server then closes the room, and the peers disconnect.
//!
//! // The peers then connect directly to each other using a library
//! // such as gday_hole_punch.
//! #
//! # Ok::<(), gday_contact_exchange_protocol::Error>(())
//! ```
//!
#![forbid(unsafe_code)]
#![warn(clippy::all)]

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    fmt::Display,
    io::{Read, Write},
    net::{SocketAddrV4, SocketAddrV6},
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// The port that contact exchange servers
/// using encrypted TLS should listen on.
pub const DEFAULT_PORT: u16 = 2311;

/// A message from client to server.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy)]
#[non_exhaustive]
pub enum ClientMsg {
    /// Requests the server to create a new room.
    ///
    /// Server responds with [`ServerMsg::RoomCreated`] on success
    /// and [`ServerMsg::ErrorRoomTaken`] if the room already exists.
    CreateRoom { room_code: u64 },

    /// Tells the server to record the client's public socket address
    /// of the connection on which this message was sent.
    ///
    /// Server responds with [`ServerMsg::ReceivedAddr`] on success.
    RecordPublicAddr {
        /// The room this client is in.
        room_code: u64,
        /// Whether this is the client that created this room,
        /// or the other client.
        is_creator: bool,
    },

    /// Tells the server that this client has finished using [`ClientMsg::RecordPublicAddr`]
    /// to record public addresses.
    /// The server immediately responds with [`ServerMsg::ClientContact`] which
    /// contains this client's contact info.
    ///
    /// The server then waits for the other peer to also send [`ClientMsg::ReadyToShare`]
    /// as well. During this time, no messages should be sent on this
    /// connection.
    ///
    /// Once the other peer also sends [`ClientMsg::ReadyToShare`],
    /// the server responds with [`ServerMsg::PeerContact`]
    /// which contains the peer's contact info.
    /// The room then closes.
    ReadyToShare {
        local_contact: Contact,
        room_code: u64,
        is_creator: bool,
    },
}

/// A message from server to client.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy)]
#[non_exhaustive]
pub enum ServerMsg {
    /// Immediately responds to a [`ClientMsg::CreateRoom`] request.
    /// Indicates that a room with the given ID has been successfully created.
    /// The room will expire/close in a few minutes.
    RoomCreated,

    /// Immediately responds to a [`ClientMsg::RecordPublicAddr`]
    /// to indicate it was successfully recorded.
    ReceivedAddr,

    /// Immediately responds to a [`ClientMsg::ReadyToShare`].
    /// Contains the client's contact info.
    ClientContact(FullContact),

    /// After both clients in a room have sent [`ClientMsg::ReadyToShare`],
    /// the server sends with this message.
    /// Contains the other peer's contact info.
    PeerContact(FullContact),

    /// Responds to a [`ClientMsg::CreateRoom`] if the given
    /// `room_code` is currently taken.
    ErrorRoomTaken,

    /// If only one client sends [`ClientMsg::ReadyToShare`] before the room
    /// times out, the server replies with this message instead of
    /// [`ServerMsg::PeerContact`]
    ErrorPeerTimedOut,

    /// The server responds with this if the `room_code` of a [`ClientMsg`]
    /// doesn't exist, either because this room timed out, or never existed.
    ErrorNoSuchRoomCode,

    /// The server responds with this if a client sends [`ClientMsg::RecordPublicAddr`]
    /// after sending [`ClientMsg::ReadyToShare`] on a different connection.
    ErrorUnexpectedMsg,

    /// Rejects a request if an IP address made too many requests.
    /// The server then closes the connection.
    ErrorTooManyRequests,

    /// The server responds with this if it receives a [`ClientMsg`]
    /// it doesn't understand. The server then closes the connection.
    ErrorSyntax,

    /// The server responds with this if it has any sort of connection error.
    /// The server then closes the connection.
    ErrorConnection,

    /// The server responds with this if it has an internal error.
    /// The server then closes the connection.
    ErrorInternal,
}

impl Display for ServerMsg {
    /// Formats this [`ServerMsg`]. Useful for pretty-printing error messages
    /// to users.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RoomCreated => write!(f, "Room in server created successfully."),
            Self::ReceivedAddr => write!(f, "Server recorded your public address."),
            Self::ClientContact(c) => write!(f, "The server says your contact is {c}."),
            Self::PeerContact(c) => write!(f, "The server says your peer's contact is {c}."),
            Self::ErrorRoomTaken => write!(
                f,
                "Can't create room with this code, because it was already created."
            ),
            Self::ErrorPeerTimedOut => write!(
                f,
                "Timed out while waiting for peer to finish sending their address."
            ),
            Self::ErrorNoSuchRoomCode => write!(f, "No room with this room code has been created."),
            Self::ErrorUnexpectedMsg => write!(
                f,
                "Server received RecordPublicAddr message after a ReadyToShare message. \
                Maybe someone else tried to join this room with your identity?"
            ),
            Self::ErrorTooManyRequests => write!(
                f,
                "Exceeded request limit from this IP address. Try again in a minute."
            ),
            Self::ErrorSyntax => write!(f, "Server couldn't parse message syntax from client."),
            Self::ErrorConnection => write!(f, "Connection error to client."),
            Self::ErrorInternal => write!(f, "Internal server error."),
        }
    }
}

/// The addresses of a single network endpoint.
/// May have IPv6, IPv4, none, or both.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy, Default)]
pub struct Contact {
    /// Endpiont's IPv4 socket address if known.
    pub v4: Option<SocketAddrV4>,
    /// Endpoint's IPv6 socket address if known.
    pub v6: Option<SocketAddrV6>,
}

impl std::fmt::Display for Contact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IPv4: ")?;
        if let Some(v4) = self.v4 {
            write!(f, "{}", v4)?;
        } else {
            write!(f, "None")?;
        }

        write!(f, ", IPv6: ")?;
        if let Some(v6) = self.v6 {
            write!(f, "{}", v6)?;
        } else {
            write!(f, "None")?;
        }

        Ok(())
    }
}

/// The public and local endpoints of an client.
///
/// [`FullContact::public`] is different from [`FullContact::local`] when the entity is behind
/// [NAT (network address translation)](https://en.wikipedia.org/wiki/Network_address_translation).
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy, Default)]
pub struct FullContact {
    /// The peer's private contact in it's local network.
    /// The server knows this from [`ClientMsg::ReadyToShare::local_contact`].
    pub local: Contact,
    /// The entity's public contact visible to the internet.
    /// The server determines this by checking where
    /// [`ClientMsg::RecordPublicAddr`] messages came from.
    pub public: Contact,
}

impl std::fmt::Display for FullContact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Private: ({})", self.local)?;
        write!(f, "Public:  ({})", self.public)?;
        Ok(())
    }
}

/// Writes `msg` to `writer` using [`serde_json`], and flushes.
///
/// Prefixes the message with 4 big-endian bytes that hold its length.
pub fn write_to(msg: impl Serialize, writer: &mut impl Write) -> Result<(), Error> {
    let vec = serde_json::to_vec(&msg)?;
    let len_byte = u32::try_from(vec.len())?;
    writer.write_all(&len_byte.to_be_bytes())?;
    writer.write_all(&vec)?;
    writer.flush()?;
    Ok(())
}

/// Asynchronously writes `msg` to `writer` using [`serde_json`], and flushes.
///
/// Prefixes the message with a 4 big-endian bytes that hold its length.
pub async fn write_to_async(
    msg: impl Serialize,
    writer: &mut (impl AsyncWrite + Unpin),
) -> Result<(), Error> {
    let vec = serde_json::to_vec(&msg)?;
    let len_byte = u32::try_from(vec.len())?;
    writer.write_all(&len_byte.to_be_bytes()).await?;
    writer.write_all(&vec).await?;
    writer.flush().await?;
    Ok(())
}

/// Reads a message from `reader` using [`serde_json`].
///
/// Assumes the message is prefixed with 4 big-endian bytes that holds its length.
pub fn read_from<T: DeserializeOwned>(reader: &mut impl Read) -> Result<T, Error> {
    let mut len = [0_u8; 4];
    reader.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len) as usize;

    let mut buf = vec![0; len];
    reader.read_exact(&mut buf)?;
    Ok(serde_json::from_reader(&buf[..])?)
}

/// Asynchronously reads a message from `reader` using [`serde_json`].
///
/// Assumes the message is prefixed with 4 big-endian bytes that hold its length.
pub async fn read_from_async<T: DeserializeOwned>(
    reader: &mut (impl AsyncRead + Unpin),
) -> Result<T, Error> {
    let mut len = [0_u8; 4];
    reader.read_exact(&mut len).await?;
    let len = u32::from_be_bytes(len) as usize;

    let mut buf = vec![0; len];
    reader.read_exact(&mut buf).await?;
    Ok(serde_json::from_reader(&buf[..])?)
}

/// Message serialization/deserialization error.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// JSON error serializing or deserializing message.
    #[error("JSON error: {0}")]
    JSON(#[from] serde_json::Error),

    /// IO Error.
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),

    /// Can't send message longer than 2^32 bytes.
    #[error("Can't send message longer than 2^32 bytes: {0}")]
    MsgTooLong(#[from] std::num::TryFromIntError),
}
