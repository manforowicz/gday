//! Note: this crate is still in early-development, so expect breaking changes.
//!
//! Lets 2 peers behind [NAT (network address translation)](https://en.wikipedia.org/wiki/Network_address_translation)
//! try to establish a direct authenticated TCP connection.
//! Uses [TCP hole punching](https://en.wikipedia.org/wiki/TCP_hole_punching)
//! and a helper [gday_server](https://crates.io/crates/gday_server) to do this.
//! This library is used by [gday](https://crates.io/crates/gday), a command line tool for sending files.
//!
//! # Example
//! ```no_run
//! # use gday_hole_punch::server_connector;
//! # use gday_hole_punch::ContactSharer;
//! # use gday_hole_punch::try_connect_to_peer;
//! # use gday_hole_punch::PeerCode;
//! # use std::str::FromStr;
//! #
//! let servers = server_connector::DEFAULT_SERVERS;
//! let timeout = std::time::Duration::from_secs(5);
//! let room_code = 123;
//! let shared_secret = 456;
//!
//! //////// Peer 1 ////////
//!
//! // Connect to a random server in the default server list
//! let (mut server_connection, server_id) = server_connector::connect_to_random_server(
//!     servers,
//!     timeout
//! )?;
//!
//! // PeerCode useful for giving rendezvous info to peer
//! let peer_code = PeerCode { server_id, room_code, shared_secret };
//! let code_to_share = peer_code.to_string();
//!
//! // Create a room in the server, and get my contact from it
//! let (contact_sharer, my_contact) = ContactSharer::create_room(
//!     &mut server_connection,
//!     room_code
//! )?;
//!
//! // Wait for the server to send the peer's contact
//! let peer_contact = contact_sharer.get_peer_contact()?;
//!
//! // Use TCP hole-punching to connect to the peer,
//! // verify their identity with the shared_secret,
//! // and get a cryptographically-secure shared key
//! let (tcp_stream, strong_key) = try_connect_to_peer(
//!     my_contact.local,
//!     peer_contact,
//!     &shared_secret.to_be_bytes(),
//!     timeout
//! )?;
//!
//! //////// Peer 2 (on a different computer) ////////
//!
//! let peer_code = PeerCode::from_str(&code_to_share)?;
//!
//! // Connect to the same server as Peer 1
//! let mut server_connection = server_connector::connect_to_server_id(
//!     servers,
//!     peer_code.server_id,
//!     timeout
//! )?;
//!
//! // Join the same room in the server, and get my local contact
//! let (contact_sharer, my_contact) = ContactSharer::join_room(
//!     &mut server_connection,
//!     peer_code.room_code
//! )?;
//!
//! let peer_contact = contact_sharer.get_peer_contact()?;
//!
//! let (tcp_stream, strong_key) = try_connect_to_peer(
//!     my_contact.local,
//!     peer_contact,
//!     &peer_code.shared_secret.to_be_bytes(),
//!     timeout
//! )?;
//!
//! # Ok::<(), gday_hole_punch::Error>(())
//! ```
//!
#![forbid(unsafe_code)]
#![warn(clippy::all)]

mod contact_sharer;
mod hole_puncher;
mod peer_code;
pub mod server_connector;

pub use contact_sharer::ContactSharer;
pub use hole_puncher::try_connect_to_peer;
pub use peer_code::PeerCode;

use gday_contact_exchange_protocol::ServerMsg;

/// `gday_hole_punch` error
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// IO Error
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),

    /// Error talking with contact exchange server
    #[error("Error talking with contact exchange server: {0}")]
    ServerProtocolError(#[from] gday_contact_exchange_protocol::Error),

    /// Unexpected reply from server
    #[error("Unexpected reply from server: {0}")]
    UnexpectedServerReply(ServerMsg),

    /// Connected to peer, but key exchange failed
    #[error(
        "Connected to peer, but key exchange failed: {0}. \
        Ensure your peer has the same shared secret."
    )]
    SpakeFailed(#[from] spake2::Error),

    /// Connected to peer, but couldn't verify their shared secret.
    #[error(
        "Connected to peer, but couldn't verify their shared secret. \
        Ensure your peer has the same shared secret."
    )]
    PeerAuthenticationFailed,

    /// Couldn't resolve contact exchange server domain name
    #[error("Couldn't resolve contact exchange server domain name '{0}'")]
    CouldntResolveServer(String),

    /// TLS error with contact exchange server
    #[error("TLS error with contact exchange server: {0}")]
    Rustls(#[from] rustls::Error),

    /// No contact exchange server with this ID found in the given list
    #[error("No contact exchange server with ID '{0}' exists in this server list.")]
    ServerIDNotFound(u64),

    /// Couldn't connect to any of the contact exchange servers listed
    #[error("Couldn't connect to any of the contact exchange servers listed.")]
    CouldntConnectToServers,

    /// Invalid server DNS name for TLS
    #[error("Invalid server DNS name for TLS: {0}")]
    InvalidDNSName(#[from] rustls::pki_types::InvalidDnsNameError),

    /// Timed out while trying to connect to peer, likely due to an uncooperative
    /// NAT (network address translator).
    #[error(
        "Timed out while trying to connect to peer, likely due to an uncooperative \
    NAT (network address translator). \
    Try from a different network, enable IPv6, or switch to a tool that transfers \
    files over a relay to circumvent NATs, such as magic-wormhole."
    )]
    HolePunchTimeout,

    /// Couldn't parse [`PeerCode`]
    #[error("Couldn't parse your code: {0}. Check it for typos!")]
    CouldntParsePeerCode(#[from] std::num::ParseIntError),

    /// Incorrect checksum when parsing [`PeerCode`]
    #[error("Your code's checksum (last digit) is incorrect. Check it for typos!")]
    IncorrectChecksumPeerCode,

    /// Couldn't parse [`PeerCode`]
    #[error("Wrong number of segments in your code. Check it for typos!")]
    WrongNumberOfSegmentsPeerCode,
}
