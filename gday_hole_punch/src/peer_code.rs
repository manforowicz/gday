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
    /// Usually the first peer will get this value from [`crate::server_connector::connect_to_random_server()`]
    /// and the other peer will pass this value to [`crate::server_connector::connect_to_server_id()`]
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

        let mut rng = rand::thread_rng();
        let range = rand::distributions::Uniform::new(0, ALPHABET.len());

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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::{Error, PeerCode};

    /// Test encoding a message.
    #[test]
    fn test_encode() {
        let peer_code = PeerCode {
            server_id: 27,
            room_code: " hel lo123".to_string(),
            shared_secret: "coded ".to_string(),
        };

        let message = String::try_from(&peer_code).unwrap();
        assert_eq!(message, "27. hel lo123.coded ");
    }

    #[test]
    fn test_decode() {
        // some uppercase, some lowercase, and spacing
        let message = "83221.room codefoo.secret123  ";
        let received1 = PeerCode::from_str(message).unwrap();
        let received2: PeerCode = message.parse().unwrap();
        let received3 = PeerCode::try_from(message).unwrap();

        let expected = PeerCode {
            server_id: 83221,
            room_code: "room codefoo".to_string(),
            shared_secret: "secret123  ".to_string(),
        };

        assert_eq!(received1, expected);
        assert_eq!(received2, expected);
        assert_eq!(received3, expected);
    }

    #[test]
    fn invalid_decodes() {
        // invalid character q
        let received = PeerCode::from_str("asd.q.3");
        assert!(matches!(received, Err(Error::CouldntParseServerID(..))));

        // too many segments
        let received = PeerCode::from_str("1.13A.f.a");
        assert!(matches!(
            received,
            Err(Error::WrongNumberOfSegmentsPeerCode)
        ));

        // too little segments
        let received = PeerCode::from_str("1.13A");
        assert!(matches!(
            received,
            Err(Error::WrongNumberOfSegmentsPeerCode)
        ));
    }

    #[test]
    fn invalid_encodes() {
        let peer_code = PeerCode {
            server_id: 0,
            room_code: "hi.there".to_string(),
            shared_secret: "what.".to_string(),
        };

        let result = String::try_from(&peer_code);

        assert!(matches!(result, Err(Error::PeerCodeContainedPeriod)))
    }

    #[test]
    fn test_zeros() {
        let peer_code = PeerCode {
            server_id: 0,
            room_code: "".to_string(),
            shared_secret: "".to_string(),
        };

        let str = String::try_from(&peer_code).unwrap();
        let received = PeerCode::from_str(&str).unwrap();
        assert_eq!(peer_code, received);
    }

    #[test]
    fn test_large() {
        let peer_code = PeerCode {
            server_id: u64::MAX,
            room_code: " j fisd;af  ljks da; ".to_string(),
            shared_secret: "r f98032 fsf 02f a".to_string(),
        };

        let str = String::try_from(&peer_code).unwrap();
        let received = PeerCode::from_str(&str).unwrap();
        assert_eq!(peer_code, received);
    }
}
