//! This protocol lets two users exchange their public and (optionally) private socket addresses via a server.
//! 
//! On it's own, this crate doesn't do anything other than define a shared protocol.
//! In most cases, you should use one of the following crates:
//! 
//! - **gday**: A command line tool for sending files to peers.
//! - **gday_hole_punch**: A library for establishing a peer-to-peer TCP connection.
//! - **gday_server**: A server binary that facilitates this protocol.
//!
//! # Process
//!
//! Using this protocol goes something like this:
//!
//! 1. Peer A connects to a server via the internet
//!     and requests a new room with `room_code` using [`ClientMsg::CreateRoom`].
//!
//! 2. The server replies to peer A with [`ServerMsg::RoomCreated`] or [`ServerMsg::ErrorRoomTaken`]
//!     depending on if this `room_code` is in use.
//!
//! 3. Peer A externally tells peer B their `room_code` (by phone call, text message, carrier pigeon, etc.).
//!
//! 4. Both peers send this `room_code` and optionally their local/private socket addresses to the server
//!     via [`ClientMsg::SendAddr`] messages. The server determines their public addresses from the internet connections.
//!     The server replies with [`ServerMsg::ReceivedAddr`] after each of these messages.
//!
//! 5. Both peers send [`ClientMsg::DoneSending`] once they are ready to receive the contact info of each other.
//!
//! 6. The server immediately replies to [`ClientMsg::DoneSending`]
//!     with [`ServerMsg::ClientContact`] which contains the [`FullContact`] of this peer.
//!
//! 7. Once both peers are ready, the server sends (on the same stream where [`ClientMsg::DoneSending`] came from)
//!     each peer a [`ServerMsg::PeerContact`] which contains the [`FullContact`] of the other peer.
//!
//! 8. On their own, the peers use this info to connect directly to each other by using
//!     [hole punching](https://en.wikipedia.org/wiki/Hole_punching_(networking)).
//!     gday_hole_punch is a library that provides tools for hole punching.
//!
#![forbid(unsafe_code)]
#![warn(clippy::all)]

mod tests;

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    fmt::Display,
    io::{Read, Write},
    net::{SocketAddr, SocketAddrV4, SocketAddrV6},
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// The port that contact exchange servers
/// using unencrypted TCP should listen on
pub const DEFAULT_TCP_PORT: u16 = 2310;

/// The port that contact exchange servers
/// using encrypted TLS should listen on
pub const DEFAULT_TLS_PORT: u16 = 2311;

/// A message from client to server.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy)]
#[non_exhaustive]
pub enum ClientMsg {
    /// Request the server to create a new room.
    /// Server responds with [`ServerMsg::RoomCreated`] on success.
    CreateRoom { room_code: u64 },

    /// Tells the server to record the public socket address of this connection.
    /// Optionally sends one of their private/local socket addresses too.
    /// Server responds with [`ServerMsg::ReceivedAddr`] on success.
    SendAddr {
        /// The room this client is in.
        room_code: u64,
        /// Whether this is the client that created this room,
        /// or the other client.
        is_creator: bool,
        /// Optionally the client's private/local socket. If not sent,
        /// the server will only know the public address deduced from
        /// the internet connection.
        private_addr: Option<SocketAddr>,
    },

    /// Tells the server that the client has finished
    /// sending any addresses they want to share.
    /// The server immediately responds with [`ServerMsg::ClientContact`] which
    /// contains this client's contact info.
    ///
    /// After the other peer sends `DoneSending` as well, the server sends
    /// [`ServerMsg::PeerContact`] which contains the peer's contact info.
    DoneSending { room_code: u64, is_creator: bool },
}

/// A message from server to client.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy)]
#[non_exhaustive]
pub enum ServerMsg {
    /// Responds to a [`ClientMsg::CreateRoom`] request.
    /// Indicates that a room with the given ID has been successfully created.
    /// The room will expire/close in a few minutes.
    RoomCreated,

    /// Responds to a [`ClientMsg::SendAddr`] to indicate it was successfully recorded.
    ReceivedAddr,

    /// Immediately responds to a [`ClientMsg::DoneSending`].
    /// Contains the client's contact info.
    ClientContact(FullContact),

    /// After both clients in a room have sent [`ClientMsg::DoneSending`],
    /// the server replies with this message.
    /// Contains the other peer's contact info.
    PeerContact(FullContact),

    /// Responds to a [`ClientMsg::CreateRoom`] if the given
    /// room_id is currently taken.
    ErrorRoomTaken,

    /// If only one client sends [`ClientMsg::DoneSending`] before the room
    /// times out, the server replies with this message instead of
    /// [`ServerMsg::PeerContact`]
    ErrorPeerTimedOut,

    /// The server responds with this if the `room_id` of a [`ClientMsg`]
    /// doesn't exist, either because this room timed out, or never existed.
    ErrorNoSuchRoomID,

    /// Rejects a request if an IP address made too many requests.
    /// The server then closes the connection.
    ErrorTooManyRequests,

    /// The server responds with this if it receives a [`ClientMsg`]
    /// with any sort of improper syntax. The server then closes the connection.
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
            Self::ReceivedAddr => write!(f, "Server received your address."),
            Self::ClientContact(c) => write!(f, "The server says your contact is {c}."),
            Self::PeerContact(c) => write!(f, "The server says your peer's contact is {c}."),
            Self::ErrorRoomTaken => write!(
                f,
                "Can't create room with this id, because it was already created."
            ),
            Self::ErrorPeerTimedOut => write!(
                f,
                "Timed out while waiting for peer to finish sending their address."
            ),
            Self::ErrorNoSuchRoomID => write!(
                f,
                "Can't join room with this id, because it hasn't been created."
            ),
            Self::ErrorTooManyRequests => write!(f, "Too many requests from this IP address."),
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

/// The public and private/local endpoints of an client.
/// 
/// `public` is different from `private` when the entity is behind
/// [NAT (network address translation)](https://en.wikipedia.org/wiki/Network_address_translation).
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy, Default)]
pub struct FullContact {
    /// The peer's private contact in it's local network.
    /// The server knows this from [`ClientMsg::SendAddr::private_addr`].
    pub private: Contact,
    /// The entity's public contact visible to the public internet.
    /// The server determines this by checking where
    /// a [`ClientMsg::SendAddr`] message came from.
    pub public: Contact,
}

impl std::fmt::Display for FullContact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Private: ({})", self.private)?;
        write!(f, "Public:  ({})", self.public)?;
        Ok(())
    }
}

/// Write `msg` to `writer` using [`serde_json`].
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

/// Read `msg` from `reader` using [`serde_json`].
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

/// Asynchronously write `msg` to `writer` using [`serde_json`].
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

/// Asynchronously read `msg` from `reader` using [`serde_json`].
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

    /// A message's length header limits it to 2^32 bytes.
    #[error("Can't send message longer than 2^32 bytes: {0}")]
    MsgTooLong(#[from] std::num::TryFromIntError),
}
