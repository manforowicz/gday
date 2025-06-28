use crate::Error;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Info that 2 peers must share before they can exchange contacts.
///
/// Use [`String::try_from()`] and [`PeerCode::from_str()`]
/// to convert to and from a short human-readable code.
#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct PeerCode {
    /// The ID of the gday contact exchange server
    /// that the peers will connect to.
    /// Use `0` to indicate a custom server.
    ///
    /// Usually the first peer will get this value from
    /// [`crate::server_connector::connect_to_random_server()`]
    /// and the other peer will pass this value to
    /// [`crate::server_connector::connect_to_server_id()`]
    pub server_id: u64,

    /// The room code within the server.
    ///
    /// Usually the first peer will randomize this value.
    ///
    /// Both peers pass this value to [`crate::share_contacts()`]
    /// to specify which room to exchange contacts in.
    pub room_code: String,

    /// The shared secret that the peers will use to confirm
    /// each other's identity, and derive a stronger key from.
    ///
    /// Usually the first peer will randomize this value.
    ///
    /// Both peers pass this value to [`crate::try_connect_to_peer()`]
    /// to authenticate the other peer when hole-punching.
    pub shared_secret: String,
}

impl PeerCode {
    /// Returns a [`PeerCode`] with this `server_id`
    /// and a random `room_code` and `shared_secret`,
    /// both of length `len` characters,
    /// built from the alphabet `2345689abcdefghjkmnpqrstvwxyz`.
    pub fn random(server_id: u64, len: usize) -> Self {
        const ALPHABET: &[u8] = b"2345689abcdefghjkmnpqrstvwxyz";

        let mut rng = rand::rng();
        let range = rand::distr::Uniform::new(0, ALPHABET.len()).unwrap();

        let room_code: String = (0..len)
            .map(|_| ALPHABET[rng.sample(range)] as char)
            .collect();

        let shared_secret: String = (0..len)
            .map(|_| ALPHABET[rng.sample(range)] as char)
            .collect();

        Self {
            server_id,
            room_code,
            shared_secret,
        }
    }
}

impl TryFrom<&PeerCode> for String {
    type Error = Error;

    fn try_from(value: &PeerCode) -> Result<Self, Self::Error> {
        if value.room_code.contains('.') || value.shared_secret.contains('.') {
            Err(Error::PeerCodeContainedPeriod)
        } else {
            Ok(format!(
                "{}.{}.{}",
                value.server_id, value.room_code, value.shared_secret,
            ))
        }
    }
}

impl std::str::FromStr for PeerCode {
    type Err = Error;

    /// Converts `str` of hexadecimal form:
    /// `"server_id.room_code.shared_secret"` into a [`PeerCode`].
    fn from_str(str: &str) -> Result<Self, Error> {
        // split `str` into period-separated substrings
        let substrings: Vec<&str> = str.split('.').collect();

        if substrings.len() != 3 {
            return Err(Error::WrongNumberOfSegmentsPeerCode);
        }

        // set fields to segments
        Ok(PeerCode {
            server_id: substrings[0].parse()?,
            room_code: substrings[1].to_owned(),
            shared_secret: substrings[2].to_owned(),
        })
    }
}

impl TryFrom<&str> for PeerCode {
    type Error = Error;
    /// Converts `str` of hexadecimal form:
    /// `"server_id.room_code.shared_secret"` into a [`PeerCode`].
    fn try_from(str: &str) -> Result<Self, Error> {
        Self::from_str(str)
    }
}
