//! Protocol for peers to exchange their socket addresses via a server.
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
//! to a gday server on [`DEFAULT_PORT`].
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
//! let room_code = *b"32-bytes. May be a password hash";
//!
//! // One client tells the server to create a room.
//! // The server responds with ServerMsg::RoomCreated or
//! // an error message.
//! let request = ClientMsg::CreateRoom { room_code };
//! write_to(request, &mut tls_ipv4)?;
//! let ServerMsg::RoomCreated = read_from(&mut tls_ipv4)? else { panic!() };
//!
//! // Both peers sends ClientMsg::RecordPublicAddr
//! // from their IPv4 and/or IPv6 endpoints.
//! // The server records the client's public addresses from these connections.
//! // The server responds with ServerMsg::ReceivedAddr or an error message.
//! let request = ClientMsg::RecordPublicAddr { room_code, is_creator: true };
//! write_to(request, &mut tls_ipv4)?;
//! let ServerMsg::ReceivedAddr = read_from(&mut tls_ipv4)? else { panic!() };
//! write_to(request, &mut tls_ipv6)?;
//! let ServerMsg::ReceivedAddr = read_from(&mut tls_ipv6)? else { panic!() };
//!
//! // Both peers share their local address with the server.
//! // The server immediately responds with ServerMsg::ClientContact,
//! // containing the client's FullContact.
//! let local_contact = Contact {
//!     v4: Some("1.8.3.1:2304".parse()?),
//!     v6: Some("[ab:41::b:43]:92".parse()?),
//! };
//! let request = ClientMsg::ReadyToShare { local_contact, room_code, is_creator: true };
//! write_to(request, &mut tls_ipv4)?;
//! let ServerMsg::ClientContact(my_contact) = read_from(&mut tls_ipv4)? else { panic!() };
//!
//! // Once both clients have sent ClientMsg::ReadyToShare,
//! // the server sends both clients a ServerMsg::PeerContact
//! // containing the FullContact of the peer.
//! let ServerMsg::PeerContact(peer_contact) = read_from(&mut tls_ipv4)? else { panic!() };
//!
//! // The server then closes the room, and the peers disconnect.
//!
//! // The peers then connect directly to each other using a library
//! // such as gday_hole_punch.
//! #
//! # Ok::<(), Box<dyn std::error::Error>>(())
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

/// Version of the protocol.
/// Different numbers wound indicate
/// incompatible protocol breaking changes.
pub const PROTOCOL_VERSION: u8 = 1;

/// A message from client to server.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy)]
#[non_exhaustive]
pub enum ClientMsg {
    /// Requests the server to create a new room.
    ///
    /// The server should automatically delete new rooms after roughly 10 minutes.
    ///
    /// More than one room can be created per connection.
    ///
    /// Server responds with [`ServerMsg::RoomCreated`] on success
    /// or [`ServerMsg::ErrorRoomTaken`] in the unlikely case that this room is taken.
    CreateRoom { room_code: [u8; 32] },

    /// Tells the server to record this client's public socket address
    /// from the connection on which this message was sent.
    ///
    /// Server responds with [`ServerMsg::ReceivedAddr`] on success
    /// or an error [`ServerMsg`] on failure.
    RecordPublicAddr {
        /// The room this client is in.
        room_code: [u8; 32],
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
    /// the server sends both peers a [`ServerMsg::PeerContact`]
    /// which contains the other peer's contact info.
    /// The room then closes, but the server doesn't disconnect.
    ReadyToShare {
        /// The local contact to share.
        local_contact: Contact,
        /// The room this client is in.
        room_code: [u8; 32],
        /// Whether this is the client that created this room,
        /// or the other client.
        is_creator: bool,
    },
}

/// A message from server to client.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy)]
#[non_exhaustive]
pub enum ServerMsg {
    /// Immediately responds to a [`ClientMsg::CreateRoom`] request.
    /// Indicates that a room with the given ID has been successfully created.
    /// The room will automatically close in roughly 10 minutes.
    RoomCreated,

    /// Immediately responds to a [`ClientMsg::RecordPublicAddr`]
    /// to indicate a client's public address was successfully recorded.
    ReceivedAddr,

    /// Immediately responds to a [`ClientMsg::ReadyToShare`].
    /// Contains the client's contact info.
    ClientContact(FullContact),

    /// After both clients in a room have sent [`ClientMsg::ReadyToShare`],
    /// the server sends this message.
    /// Contains the other peer's contact info.
    PeerContact(FullContact),

    /// Responds to a [`ClientMsg::CreateRoom`] if the given
    /// `room_code` is currently taken.
    ErrorRoomTaken,

    /// If only one client sends [`ClientMsg::ReadyToShare`] before the room
    /// times out, the server replies with this message instead of
    /// [`ServerMsg::PeerContact`].
    ErrorPeerTimedOut,

    /// The server responds with this if the `room_code` of a [`ClientMsg`]
    /// doesn't exist, either because this room timed out, or never existed.
    ErrorNoSuchRoomCode,

    /// The server may respond with this if a client sends [`ClientMsg::RecordPublicAddr`]
    /// after already sending [`ClientMsg::ReadyToShare`].
    ErrorUnexpectedMsg,

    /// Rejects a request if an IP address made too many requests.
    /// The server then closes the connection.
    ErrorTooManyRequests,

    /// The server responds with this if it receives a [`ClientMsg`]
    /// it doesn't understand.
    /// The server then closes the connection.
    ErrorSyntax,

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
                "Can't create a room with this room code, because it's already taken."
            ),
            Self::ErrorPeerTimedOut => write!(
                f,
                "Timed out while waiting for peer to finish sending their addresses to the server."
            ),
            Self::ErrorNoSuchRoomCode => {
                write!(f, "No room with this code currently exists on the server.")
            }
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
            Self::ErrorInternal => write!(f, "Server had an internal error."),
        }
    }
}

/// The addresses of a single client.
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

/// The local and public endpoints of an client.
///
/// [`FullContact::local`] is only different from [`FullContact::public`] when the client is behind
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
/// Prefixes the message with 1 byte holding the [`PROTOCOL_VERSION`]
/// and 2 bytes holding the length of the following message (all in big-endian).
pub fn write_to(msg: impl Serialize, writer: &mut impl Write) -> Result<(), Error> {
    let vec = serde_json::to_vec(&msg)?;
    let len = u16::try_from(vec.len())?;

    let mut header = [0; 3];
    header[0] = PROTOCOL_VERSION;
    header[1..3].copy_from_slice(&len.to_be_bytes());

    writer.write_all(&header)?;
    writer.write_all(&vec)?;
    writer.flush()?;
    Ok(())
}

/// Asynchronously writes `msg` to `writer` using [`serde_json`], and flushes.
///
/// Prefixes the message with 1 byte holding the [`PROTOCOL_VERSION`]
/// and 2 bytes holding the length of the following message (all in big-endian).
pub async fn write_to_async(
    msg: impl Serialize,
    writer: &mut (impl AsyncWrite + Unpin),
) -> Result<(), Error> {
    let vec = serde_json::to_vec(&msg)?;
    let len = u16::try_from(vec.len())?;

    let mut header = [0; 3];
    header[0] = PROTOCOL_VERSION;
    header[1..3].copy_from_slice(&len.to_be_bytes());

    writer.write_all(&header).await?;
    writer.write_all(&vec).await?;
    writer.flush().await?;
    Ok(())
}

/// Reads a message from `reader` using [`serde_json`].
///
/// Assumes the message is prefixed with 1 byte holding the [`PROTOCOL_VERSION`]
/// and 2 big-endian bytes holding the length of the following message.
pub fn read_from<T: DeserializeOwned>(reader: &mut impl Read) -> Result<T, Error> {
    let mut header = [0_u8; 3];
    reader.read_exact(&mut header)?;
    if header[0] != PROTOCOL_VERSION {
        return Err(Error::IncompatibleProtocol);
    }
    let len = u16::from_be_bytes(header[1..3].try_into().unwrap()) as usize;

    let mut buf = vec![0; len];
    reader.read_exact(&mut buf)?;
    Ok(serde_json::from_reader(&buf[..])?)
}

/// Asynchronously reads a message from `reader` using [`serde_json`].
///
/// Assumes the message is prefixed with 1 byte holding the [`PROTOCOL_VERSION`]
/// and 2 big-endian bytes holding the length of the following message.
pub async fn read_from_async<T: DeserializeOwned>(
    reader: &mut (impl AsyncRead + Unpin),
) -> Result<T, Error> {
    let mut header = [0_u8; 3];
    reader.read_exact(&mut header).await?;
    if header[0] != PROTOCOL_VERSION {
        return Err(Error::IncompatibleProtocol);
    }
    let len = u16::from_be_bytes(header[1..3].try_into().unwrap()) as usize;

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

    /// Can't send message longer than 2^16 bytes.
    #[error("Can't send message longer than 2^16 bytes: {0}")]
    MsgTooLong(#[from] std::num::TryFromIntError),

    /// Received a message with an incompatible protocol version.
    /// Check if this software is up-to-date.
    #[error(
        "Received a message with an incompatible protocol version. \
        Check if this software is up-to-date."
    )]
    IncompatibleProtocol,
}
