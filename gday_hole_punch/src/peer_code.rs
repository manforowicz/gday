use crate::Error;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::str::FromStr;

/// Info that 2 peers must share before they can exchange contacts.
///
/// Use [`PeerCode::fmt()`] and [`PeerCode::try_from()`]
/// to convert to and from a short human-readable code.
#[derive(PartialEq, Eq, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PeerCode {
    /// The ID of the gday contact exchange server
    /// that the peers will connect to.
    ///
    /// Use `0` to indicate a custom server.
    /// Pass to [`crate::server_connector::connect_to_server_id()`]
    /// to connect to the server.
    pub server_id: u64,

    /// The room code within the server.
    ///
    /// Pass to [`crate::ContactSharer`] to specify
    /// which room to exchange contacts in.
    pub room_code: u64,

    /// The shared secret that the peers will use to confirm
    /// each other's identity.
    ///
    /// Pass to [`crate::try_connect_to_peer()`] to authenticate
    /// the other peer when hole-punching.
    pub shared_secret: u64,
}

impl PeerCode {
    /// Calculates a simple hash of the three fields, mod 17
    /// `(self.server_id * 3 + self.room_code * 2 + self.shared_secret) % 17`
    fn get_checksum(&self) -> u64 {
        ((self.server_id % 17) * 3 + (self.room_code % 17) * 2 + (self.shared_secret % 17)) % 17
    }
}

impl Display for PeerCode {
    /// Display as `"server_id.room_code.shared_secret.checksum"`
    /// where each field is in hexadecimal form.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:X}.{:X}.{:X}.{:X}",
            self.server_id,
            self.room_code,
            self.shared_secret,
            self.get_checksum()
        )
    }
}

impl std::str::FromStr for PeerCode {
    type Err = Error;

    /// Converts `str` of hexadecimal form:
    /// `"server_id.room_code.shared_secret.checksum"` into a [`PeerCode`].
    ///
    /// The checksum is optional.
    fn from_str(str: &str) -> Result<Self, Error> {
        // split `str` into period-separated substrings
        let mut substrings = str.trim().split('.');

        // decode each segment independently
        let mut segments = [0, 0, 0];
        for segment in &mut segments {
            let Some(substring) = substrings.next() else {
                // return error if less than 4 substrings
                return Err(Error::WrongNumberOfSegmentsPeerCode);
            };
            *segment = u64::from_str_radix(substring, 16)?;
        }

        // set fields to segments
        let peer_code = PeerCode {
            server_id: segments[0],
            room_code: segments[1],
            shared_secret: segments[2],
        };

        // check checksum
        if let Some(substring) = substrings.next() {
            let checksum = u64::from_str_radix(substring, 16)?;
            // verify checksum
            if checksum != peer_code.get_checksum() {
                return Err(Error::IncorrectChecksumPeerCode);
            }
        }

        // return error if there are too many substrings
        if substrings.next().is_some() {
            return Err(Error::WrongNumberOfSegmentsPeerCode);
        }

        Ok(peer_code)
    }
}

impl TryFrom<&str> for PeerCode {
    type Error = Error;
    /// Converts `str` of hexadecimal form:
    /// `"server_id.room_code.shared_secret.checksum"` into a [`PeerCode`].
    ///
    /// The checksum is optional.
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
            room_code: 314,
            shared_secret: 15,
        };

        let message = peer_code.to_string();
        assert_eq!(message, "1B.13A.F.A");
    }

    #[test]
    fn test_decode() {
        // some uppercase, some lowercase, and spacing
        let message = " 1b.13A.f.a  ";
        let received1 = PeerCode::from_str(message).unwrap();
        let received2: PeerCode = message.parse().unwrap();
        let received3 = PeerCode::try_from(message).unwrap();

        let expected = PeerCode {
            server_id: 27,
            room_code: 314,
            shared_secret: 15,
        };

        assert_eq!(received1, expected);
        assert_eq!(received2, expected);
        assert_eq!(received3, expected);
    }

    #[test]
    fn checksum_test() {
        // checksum omitted
        let received = PeerCode::from_str(" 1b.13A.f  ").unwrap();
        let expected = PeerCode {
            server_id: 27,
            room_code: 314,
            shared_secret: 15,
        };
        assert_eq!(received, expected);

        // checksum incorrect
        let received = PeerCode::from_str(" 1c.13A.f.3  ");
        assert!(matches!(received, Err(Error::IncorrectChecksumPeerCode)));
    }

    #[test]
    fn invalid_encodings() {
        // invalid character q
        let received = PeerCode::from_str(" 21.q.3  ");
        assert!(matches!(received, Err(Error::CouldntParsePeerCode(..))));

        // too many segments
        let received = PeerCode::from_str(" 1b.13A.f.a.4 ");
        assert!(matches!(
            received,
            Err(Error::WrongNumberOfSegmentsPeerCode)
        ));

        // too little segments
        let received = PeerCode::from_str(" 1b.13A ");
        assert!(matches!(
            received,
            Err(Error::WrongNumberOfSegmentsPeerCode)
        ));
    }

    #[test]
    fn test_zeros() {
        let peer_code = PeerCode {
            room_code: 0,
            server_id: 0,
            shared_secret: 0,
        };

        let str: String = peer_code.to_string();
        let received = PeerCode::from_str(&str).unwrap();
        assert_eq!(peer_code, received);
    }

    #[test]
    fn test_large() {
        let peer_code = PeerCode {
            room_code: u64::MAX,
            server_id: u64::MAX,
            shared_secret: u64::MAX,
        };

        let str: String = peer_code.to_string();
        let received = PeerCode::from_str(&str).unwrap();
        assert_eq!(peer_code, received);
    }
}
