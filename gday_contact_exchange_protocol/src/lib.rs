//! This protocol lets two users exchange their public and (optionally) private socket addresses via a server.
//! On it's own, this crate doesn't do anything other than define a shared protocol.
//! This is done with the following process:
//!
//! 1. `peer A` connects to a server via the internet
//!     and requests a new room using [`ClientMsg::CreateRoom`].
//!
//! 2. The server replies to `peer A` with a random unused room code via [`ServerMsg::RoomCreated`].
//!
//! 3. `peer A` externally tells `peer B` this code (by phone call, text message, carrier pigeon, etc.).
//!
//! 4. Both peers send this room code and optionally their local/private socket addresses to the server
//!     via [`ClientMsg::SendAddr`] messages. The server determines their public addresses from the internet connections.
//!     The server replies with [`ServerMsg::ReceivedAddr`] after each of these messages.
//!
//! 5. Both peers send [`ClientMsg::DoneSending`] once they are ready to receive the contact info of each other.
//!
//! 6. The server sends each peer their contact information via [`ServerMsg::ClientContact`]
//!
//! 7. Once both peers are ready, the server sends each peer the public and private socket addresses
//!     of the other peer via [`ServerMsg::PeerContact`].
//!
//! 8. On their own, the peers use this info to connect directly to each other by using
//!     [hole punching](https://en.wikipedia.org/wiki/Hole_punching_(networking)).
#![forbid(unsafe_code)]
#![warn(clippy::all)]

mod tests;

use postcard::{from_bytes, to_slice};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    net::{SocketAddr, SocketAddrV4, SocketAddrV6},
};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// A message from client to server.
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy)]
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
    SyntaxError,

    /// The server responds with this if it has any sort of connection error.
    /// The server then closes the connection.
    ConnectionError,
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

/// The public and private/local endpoints of an client.
/// `public` is different from `private` when the entity is behind
/// [NAT (network address translation)](https://en.wikipedia.org/wiki/Network_address_translation).
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone, Copy, Default)]
pub struct FullContact {
    /// The peer's private contact in it's local network.
    /// The server records this from `private_addr` [`ClientMsg::SendAddr`].
    pub private: Contact,
    /// The entity's public contact visible to the public internet.
    /// The server records this by checking where a [`ClientMsg::SendAddr`] came from.
    pub public: Contact,
}

//////////////////////////////////////////////////////////////////

// Calculations using the [Postcard wire format](https://postcard.jamesmunns.com/wire-format)
// The max size of an  `Option<SocketAddr>` in Postcard is
// 5 (option) + 5 (SocketAddr) + 5 (octet bytes len) + 16 (octet bytes) + 3 (port) = 29 bytes
// The max size of a `Contact` in Postcard is
// 29 + 29 = 58 bytes
// The max size of a `FullContact` is
// 58 + 58 = 116 bytes
// The max size of a `ClientMsg` is
// 5 + 10 + 1 + 29 = 55 bytes
// The max size of a `ServerMsg` is
// 5 + 116 = 121 bytes
pub const MAX_CLIENT_MSG: usize = 55;
pub const MAX_SERVER_MSG: usize = 121;
pub const MAX_MSG_SIZE: usize = 121;

pub fn serialize_into(msg: impl Serialize, writer: &mut impl Write) -> Result<(), Error> {
    let mut buf = [0_u8; MAX_MSG_SIZE];
    let len = to_slice(&msg, &mut buf[1..])?.len();
    let len_byte = u8::try_from(len).expect("Unreachable: Message always shorter than u8::MAX");
    buf[0] = len_byte;
    writer.write_all(&buf[0..len + 1])?;
    Ok(())
}

pub fn deserialize_from<'a, T: Deserialize<'a>>(reader: &mut impl Read, buf: &'a mut [u8]) -> Result<T, Error> {
    let mut len = [0_u8; 1];
    reader.read_exact(&mut len)?;
    let len = len[0] as usize;
    reader.read_exact(&mut buf[0..len])?;
    Ok(from_bytes(&buf[0..len])?)
}


pub async fn serialize_into_async(msg: impl Serialize, writer: &mut (impl AsyncWrite + Unpin)) -> Result<(), Error> {
    let mut buf = [0_u8; MAX_MSG_SIZE];
    let len = to_slice(&msg, &mut buf[1..])?.len();
    let len_byte = u8::try_from(len).expect("Unreachable: Message always shorter than u8::MAX");
    buf[0] = len_byte;
    writer.write_all(&buf[0..len + 1]).await?;
    Ok(())
}

pub async fn deserialize_from_async<'a, T: Deserialize<'a>>(reader: &mut (impl AsyncRead + Unpin), buf: &'a mut [u8]) -> Result<T, Error> {
    let mut len = [0_u8; 1];
    reader.read_exact(&mut len).await?;
    let len = len[0] as usize;
    reader.read_exact(&mut buf[0..len]).await?;
    Ok(from_bytes(&buf[0..len])?)
}



/*

/// A wrapper around a generic IO stream.
/// Allows sending and receiving [`ServerMsg`] and [`ClientMsg`]
/// using a standard format:
/// [`postcard`] serialized messages, prefixed with their length
/// as a big-endian `u16`.
#[derive(Debug)]
pub struct Messenger<T: Read + Write> {
    pub stream: T,
    buf: Vec<u8>,
}

impl<T: Read + Write> Messenger<T> {
    /// Create a [`Messenger`] that wraps `stream`.
    pub fn new(stream: T) -> Self {
        Self {
            stream,
            buf: Vec::new(),
        }
    }

    /// Read the next message from the inner IO stream.
    /// Each message must be prefixed by 2 bytes (big-endian)
    /// specifying the length of the following content.
    ///
    /// # Errors
    /// Returns an error if the message couldn't be received or deserialized.
    pub fn receive<'b, U: Deserialize<'b>>(&'b mut self) -> Result<U, Error> {
        let mut len = [0; 2];
        self.stream.read_exact(&mut len)?;
        let len = u16::from_be_bytes(len) as usize;
        self.buf.resize(len, 0);

        self.stream.read_exact(&mut self.buf)?;
        Ok(from_bytes(&self.buf)?)
    }

    /// Write `msg` to the inner IO stream.
    /// Prefixes it by 2 bytes (big-endian) representing the following message's length.
    /// # Errors
    /// Returns an error if the message couldn't be serialized or sent.
    pub fn send(&mut self, msg: impl Serialize) -> Result<(), Error> {
        self.buf.clear();
        to_io(&msg, &mut self.buf)?;
        let len =
            u16::try_from(self.buf.len()).expect("Unreachable: message can't be longer than u16.");
        let len = len.to_be_bytes();
        self.stream.write_all(&len)?;
        self.stream.write_all(&self.buf)?;
        self.stream.flush()?;
        Ok(())
    }
}

/// A wrapper around a generic async IO stream.
/// Allows sending and receiving [`ServerMsg`] and [`ClientMsg`]
/// using a standard format:
/// [`postcard`] serialized messages, prefixed with their length
/// as a big-endian `u16`.
#[derive(Debug)]
pub struct AsyncMessenger<T: AsyncRead + AsyncWrite + Unpin> {
    /// A stream from which to read and write messages.
    pub stream: T,
    /// Buffer to store read messages, and messages to write.
    buf: Vec<u8>,
}

impl<T: AsyncRead + AsyncWrite + Unpin> AsyncMessenger<T> {
    /// Create a [`Messenger`] that wraps `stream`.
    pub fn new(stream: T) -> Self {
        Self {
            stream,
            buf: Vec::new(),
        }
    }

    /// Read the next message from the inner IO stream.
    /// Each message must be prefixed by 2 bytes (big-endian)
    /// specifying the length of the following content.
    ///
    /// # Errors
    /// Returns an error if the message couldn't be received or deserialized.
    pub async fn receive<'b, U: Deserialize<'b>>(&'b mut self) -> Result<U, Error> {
        let mut len = [0; 2];
        self.stream.read_exact(&mut len).await?;
        let len = u16::from_be_bytes(len) as usize;
        self.buf.resize(len, 0);

        self.stream.read_exact(&mut self.buf).await?;
        Ok(from_bytes(&self.buf)?)
    }

    /// Write `msg` to the inner IO stream.
    /// Prefixes it by 2 bytes (big-endian) representing the following message's length.
    /// # Errors
    /// Returns an error if the message couldn't be serialized or sent.
    pub async fn send(&mut self, msg: impl Serialize) -> Result<(), Error> {
        self.buf.clear();
        to_io(&msg, &mut self.buf)?;
        let len =
            u16::try_from(self.buf.len()).expect("Unreachable: message can't be longer than u16.");
        let len = len.to_be_bytes();
        self.stream.write_all(&len).await?;
        self.stream.write_all(&self.buf).await?;
        self.stream.flush().await?;
        Ok(())
    }
}

*/



/// Error from [`Messenger`].
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// [`postcard`] error encoding or decoding a message.
    #[error("Postcard error encoding/decoding message: {0}")]
    Postcard(#[from] postcard::Error),

    /// IO Error sending or receiving a message
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),
}
