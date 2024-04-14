//! Lets peers behind [NAT (network address translation)](https://en.wikipedia.org/wiki/Network_address_translation)
//! try to establish a direct authenticated TCP connection.
//!
//! Uses [TCP hole punching](https://en.wikipedia.org/wiki/TCP_hole_punching) and a helper server to do this.
//!
//! Steps:
//!
//! 1. Both peers connect to an external Gday server using the functions in [`server_connector`].
//! 2. One peer creates a room in the server using [`ContactSharer::create_room()`] with a random room code.
//! 3. The other peers joins this room using [`ContactSharer::join_room()`] with the same room code.
//! 4. Both peers call [`ContactSharer::get_peer_contact()`] to get their peer's contact.
//! 5. Both peers pass this contact and a shared secret to [`try_connect_to_peer()`],
//!    which returns a TCP stream, and an authenticated shared key.
#![forbid(unsafe_code)]
#![warn(clippy::all)]

mod contact_sharer;
mod hole_puncher;
pub mod server_connector;

pub use contact_sharer::ContactSharer;
pub use gday_contact_exchange_protocol::DEFAULT_TCP_PORT;
pub use gday_contact_exchange_protocol::DEFAULT_TLS_PORT;
pub use hole_puncher::try_connect_to_peer;

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

    /// TODO: Improve this error message
    #[error("Unexpected reply from server: {0:?}")]
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
}
