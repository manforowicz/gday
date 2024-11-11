//! Lets 2 peers, possibly behind [NAT (network address translation)](https://en.wikipedia.org/wiki/Network_address_translation),
//! try to establish a direct authenticated TCP connection.
//! Uses [TCP hole punching](https://en.wikipedia.org/wiki/TCP_hole_punching)
//! and a helper [gday_server](https://crates.io/crates/gday_server) to do this.
//! This library is used by [gday](https://crates.io/crates/gday), a command line tool for sending files.
//!
//! # Example
//! ```no_run
//! # use gday_hole_punch::server_connector;
//! # use gday_hole_punch::try_connect_to_peer;
//! # use gday_hole_punch::PeerCode;
//! # use gday_hole_punch::share_contacts;
//! # use std::str::FromStr;
//! #
//! # let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
//! # rt.block_on( async {
//! let timeout = std::time::Duration::from_secs(5);
//!
//! //////// Peer 1 ////////
//!
//! // Connect to a random server in the default server list
//! let (mut server_connection, server_id) = server_connector::connect_to_random_server(
//!     server_connector::DEFAULT_SERVERS,
//!     timeout,
//! ).await?;
//!
//! // PeerCode useful for giving rendezvous info to peer,
//! // over an existing channel like email.
//! let peer_code = PeerCode {
//!     server_id,
//!     room_code: "roomcode".to_string(),
//!     shared_secret: "shared_secret".to_string()
//! };
//! let code_to_share = String::try_from(&peer_code)?;
//!
//! // Create a room in the server, and get my contact from it
//! let (my_contact, peer_contact_future) = share_contacts(
//!     &mut server_connection,
//!     peer_code.room_code.as_bytes(),
//!     true,
//! ).await?;
//!
//! // Wait for the server to send the peer's contact
//! let peer_contact = peer_contact_future.await?;
//!
//! // Use TCP hole-punching to connect to the peer,
//! // verify their identity with the shared_secret,
//! // and get a cryptographically-secure shared key
//! let (tcp_stream, strong_key) = try_connect_to_peer(
//!     my_contact.local,
//!     peer_contact,
//!     peer_code.shared_secret.as_bytes(),
//! ).await?;
//!
//! //////// Peer 2 (on a different computer) ////////
//!
//! // Get the peer_code that Peer 1 sent, for example
//! // over email.
//! let peer_code = PeerCode::from_str(&code_to_share)?;
//!
//! // Connect to the same server as Peer 1
//! let mut server_connection = server_connector::connect_to_server_id(
//!     server_connector::DEFAULT_SERVERS,
//!     peer_code.server_id,
//!     timeout,
//! ).await?;
//!
//! // Join the same room in the server, and get my local contact
//! let (my_contact, peer_contact_future) = share_contacts(
//!     &mut server_connection,
//!     peer_code.room_code.as_bytes(),
//!     false,
//! ).await?;
//!
//! let peer_contact = peer_contact_future.await?;
//!
//! let (tcp_stream, strong_key) = try_connect_to_peer(
//!     my_contact.local,
//!     peer_contact,
//!     peer_code.shared_secret.as_bytes(),
//! ).await?;
//!
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! # }).unwrap();
//! ```
//!
#![forbid(unsafe_code)]
#![warn(clippy::all)]

mod contact_sharer;
mod hole_puncher;
mod peer_code;
pub mod server_connector;

pub use contact_sharer::share_contacts;
use gday_contact_exchange_protocol::ServerMsg;
pub use hole_puncher::try_connect_to_peer;
pub use peer_code::PeerCode;

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

    /// Both `v4` and `v6` fields of the given local Contact were None.
    #[error("Both `v4` and `v6` fields of the given local Contact were None.")]
    LocalContactEmpty,

    /// Both `v4` and `v6` fields of the given ServerConnection were None.
    #[error("Both `v4` and `v6` fields of a ServerConnection were None.")]
    ServerConnectionEmpty,

    /// ServerConnection has mismatched streams. Either v4 had an IPv6 stream, or vice-versa.
    #[error(
        "ServerConnection has mismatched streams. Either v4 had an IPv6 stream, or vice-versa."
    )]
    ServerConnectionMismatch,

    /// Connected to peer, but key exchange failed
    #[error(
        "Connected to peer, but key exchange failed: {0}. \
        Check for typos in your peer code and try again."
    )]
    SpakeFailed(#[from] spake2::Error),

    /// Connected to peer, but they had a different shared secret.
    #[error(
        "Connected to peer, but they had a different shared secret. \
        Check for typos in your peer code and try again."
    )]
    PeerAuthenticationFailed,

    /// No contact exchange server with this ID found in server list
    #[error("No contact exchange server with ID '{0}' exists in server list.")]
    ServerIDNotFound(u64),

    /// Couldn't connect to any of the contact exchange servers listed
    #[error("Couldn't connect to any of the contact exchange servers listed.")]
    CouldntConnectToServers,

    /// Invalid server DNS name for TLS
    #[error("Invalid server DNS name for TLS: {0}")]
    InvalidDNSName(#[from] tokio_rustls::rustls::pki_types::InvalidDnsNameError),

    /// Timed out while trying to connect to peer, likely due to an uncooperative
    /// NAT (network address translator).
    #[error(
        "Timed out while trying to connect to peer, likely due to an uncooperative \
    NAT (network address translator). \
    Try from a different network, enable IPv6, or switch to a tool that transfers \
    files over a relay to evade NATs, such as magic-wormhole."
    )]
    HolePunchTimeout,

    /// Couldn't parse server ID of [`PeerCode`]
    #[error("Couldn't parse server ID in your code: {0}. Check it for typos!")]
    CouldntParseServerID(#[from] std::num::ParseIntError),

    /// The room_code or shared_secret of the peer code contained a period.
    #[error(
        "The room_code or shared_secret of the peer code contained a period. \
    Period aren't allowed because they're used as code delimeters."
    )]
    PeerCodeContainedPeriod,

    /// Couldn't parse [`PeerCode`]
    #[error("Wrong number of segments in your code. Check it for typos!")]
    WrongNumberOfSegmentsPeerCode,
}
