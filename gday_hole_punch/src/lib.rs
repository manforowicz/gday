//! Note: this crate is still in early-development, so expect breaking changes.
//!
//! Lets peers behind [NAT (network address translation)](https://en.wikipedia.org/wiki/Network_address_translation)
//! try to establish a direct authenticated TCP connection.
//!
//! Uses [TCP hole punching](https://en.wikipedia.org/wiki/TCP_hole_punching)
//! and a helper **gday_contact_exchange_server** to do this.
//!
//! This library is used by **gday**, a command line tool for sending files.
//!
//! # Example steps
//!
//! 1. Peer A connects to a **gday_contact_exchange_server** using
//! a function such as [`server_connector::connect_to_random_server()`].
//!
//! 2. Peer A creates a room in the server using [`ContactSharer::create_room()`] with a random room code.
//!
//! 3. Peer A tells Peer B which server and room code to join, possibly by giving them a [`PeerCode`]
//!     (done via phone call, email, etc.).
//!
//! 4. Peer B connects to the same server using [`server_connector::connect_to_server_id()`].
//!
//! 5. Peer B joins the same room using [`ContactSharer::join_room()`].
//!
//! 6. Both peers call [`ContactSharer::get_peer_contact()`] to get their peer's contact.
//!
//! 7. Both peers pass this contact and a shared secret to [`try_connect_to_peer()`],
//!    which returns a TCP stream, and an authenticated cryptographically-secure shared key.
//!
#![forbid(unsafe_code)]
#![warn(clippy::all)]

mod contact_sharer;
mod hole_puncher;
mod peer_code;
pub mod server_connector;

pub use contact_sharer::ContactSharer;
pub use gday_contact_exchange_protocol::DEFAULT_TCP_PORT;
pub use gday_contact_exchange_protocol::DEFAULT_TLS_PORT;
pub use hole_puncher::try_connect_to_peer;
pub use peer_code::PeerCode;

use gday_contact_exchange_protocol::ServerMsg;

/// `gday_hole_punch` error
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// The given ServerConnection contains no streams
    #[error("The given ServerConnection contains no streams.")]
    NoStreamsProvided,

    /// Expected IPv4 address, but received an IPv6 address
    #[error("Expected IPv4 address, but received an IPv6 address.")]
    ExpectedIPv4,

    /// Expected IPv6 address, but received an IPv4 address
    #[error("Expected IPv6 address, but received an IPv4 address.")]
    ExpectedIPv6,

    /// Local contact or peer contact were empty, so couldn't try connecting
    #[error("Local contact or peer contact were empty, so couldn't try connecting.")]
    ContactEmpty,

    /// IO Error
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),

    /// Error talking with contact exchange server
    #[error("Error talking with contact exchange server: {0}")]
    MessengerError(#[from] gday_contact_exchange_protocol::Error),

    /// Unexpected reply from server
    #[error("Server unexpectedly replied: {0}")]
    UnexpectedServerReply(ServerMsg),

    /// Connected to peer, but key exchange failed
    #[error("Connected to peer, but key exchange failed: {0}. Check the peer shared secret.")]
    SpakeFailed(#[from] spake2::Error),

    /// Connected to peer, but couldn't verify their shared secret.
    /// This could be due to a man-in-the-middle attack or a mismatched shared secret.
    #[error(
        "Connected to peer, but couldn't verify their shared secret. \
        This could be due to a man-in-the-middle attack or a mismatched shared secret.
        Re-check your shared secret."
    )]
    PeerAuthenticationFailed,

    /// Couldn't resolve any IP addresses for this contact exchange server
    #[error("Couldn't resolve any IP addresses for contact exchange server '{0}'")]
    CouldntResolveAddress(String),

    /// TLS error with contact exchange server
    #[error("TLS error with contact exchange server: {0}")]
    Rustls(#[from] rustls::Error),

    /// No contact exchange server with this ID found in the given list
    #[error("No contact exchange server with ID '{0}' exists in this server list.")]
    ServerIDNotFound(u64),

    /// Couldn't connect to any of these contact exchange servers
    #[error("Couldn't connect to any of these contact exchange servers.")]
    CouldntConnectToServers,
    /// Invalid server DNS name for TLS

    #[error("Invalid server DNS name for TLS: {0}")]
    InvalidDNSName(#[from] rustls::pki_types::InvalidDnsNameError),

    /// Timed out while trying to connect to peer, likely due to an uncooperative
    /// NAT (network address translator).
    #[error(
        "Timed out while trying to connect to peer, likely due to an uncooperative \
    NAT (network address translator). \
    Try from a different network, enable IPv6, or use a tool that transfers \
    files over a relay to circumvent NATs, such as magic-wormhole."
    )]
    HolePunchTimeout,

    /// Couldn't parse [`PeerCode`]
    #[error("Couldn't parse your code: {0}. Check it for typos!")]
    CouldntParse(#[from] std::num::ParseIntError),

    /// Incorrect checksum when parsing [`PeerCode`]
    #[error("Your code's checksum (last digit) is incorrect. Check it for typos!")]
    IncorrectChecksum,

    /// Couldn't parse [`PeerCode`]
    #[error("Wrong number of segments in your code. Check it for typos!")]
    WrongNumberOfSegments,

    /// Missing required checksum in [`PeerCode`]
    #[error("Your code is missing the required checksum digit. Check it for typos!")]
    MissingChecksum,
}
