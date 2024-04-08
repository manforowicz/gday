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
pub use hole_puncher::try_connect_to_peer;

use gday_contact_exchange_protocol::ServerMsg;

#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("The given ServerConnection contains no streams.")]
    NoStreamsProvided,

    #[error("Expected IPv4 address, but received an IPv6 address.")]
    ExpectedIPv4,

    #[error("Expected IPv6 address, but received an IPv4 address.")]
    ExpectedIPv6,

    #[error("IO Error: {0}.")]
    IO(#[from] std::io::Error),

    #[error("Error talking with server: {0}")]
    MessengerError(#[from] gday_contact_exchange_protocol::Error),

    #[error("Unexpected reply from server: {0:?}")]
    UnexpectedServerReply(ServerMsg),

    #[error("Connected to peer, but key exchange failed: {0}. Check the peer shared secret.")]
    SpakeFailed(#[from] spake2::Error),

    #[error("Connected, but couldn't authenticate peer. Check the peer shared secret.")]
    PeerAuthenticationFailed,

    #[error("Couldn't resolve any IP addresses for server '{0}'")]
    CouldntResolveAddress(String),

    #[error("TLS error: {0}")]
    Rustls(#[from] rustls::Error),

    #[error("No server with ID '{0}' exists.")]
    ServerIDNotFound(u64),

    #[error("Couldn't connect to any of these servers.")]
    CouldntConnectToServers,

    #[error("Invalid server DNS name: {0}")]
    InvalidDNSName(#[from] rustls::pki_types::InvalidDnsNameError),

    #[error(
        "Timed out while trying to connect to peer, likely due to an uncooperative \
    NAT (network address translator). \
    Try from a different network or use a tool that transfers \
    files over a relay to circumvent NATs, such as magic-wormhole."
    )]
    HolePunchTimeout,
}
