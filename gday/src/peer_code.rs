use thiserror::Error;

/// Represents the code that one peer must give another
/// to start establishing contact.
///
/// Can be converted to and from [`String`] in hexadecimal of form:
/// `"server_id.room_code.shared_secret.checksum"`.
#[derive(PartialEq, Debug)]
pub struct PeerCode {
    /// The id of the gday server the peers will connect to
    pub server_id: u64,
    /// The room code that the peers will use
    pub room_code: u64,
    /// A shared secret the peers will use to authenticate each other
    pub shared_secret: u64,
}

impl PeerCode {
    /// Converts `str` of hexadecimal form:
    /// `"server_id.room_code.shared_secret.checksum"` into a [`PeerCode`].
    ///
    /// Checksum is not required if `require_checksum` is false.
    pub fn parse(str: &str, require_checksum: bool) -> Result<Self, Error> {
        // split `str` into period-separated substrings
        let mut substrings = str.trim().split('.');

        // decode each segment independently
        let mut segments = [0, 0, 0];
        for segment in &mut segments {
            let Some(substring) = substrings.next() else {
                // return error if less than 4 substrings
                return Err(Error::WrongNumberOfSegments);
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
                return Err(Error::IncorrectChecksum);
            }
        } else if require_checksum {
            return Err(Error::MissingChecksum);
        }

        // return error if there are too many substrings
        if substrings.next().is_some() {
            return Err(Error::WrongNumberOfSegments);
        }

        Ok(peer_code)
    }

    /// Converts [`PeerCode`] into [`String`] in hexadecimal string of form:
    /// `"server_id.room_code.shared_secret.checksum"`.
    pub fn to_str(&self) -> String {
        let mut s = format!(
            "{:X}.{:X}.{:X}.",
            self.server_id, self.room_code, self.shared_secret
        );

        // append the checksum as the 4-th segment
        s.push_str(&format!("{:X}", self.get_checksum()));

        s
    }

    /// Calculates a simple hash of the fields, mod 17
    fn get_checksum(&self) -> u64 {
        ((self.server_id % 17) + (self.room_code % 17) * 2 + (self.shared_secret % 17) * 3) % 17
    }
}

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("Couldn't decode your code: {0}. Check it for typos!")]
    CouldntParse(#[from] std::num::ParseIntError),

    #[error("Your code's checksum (last digit) is incorrect. Check it for typos!")]
    IncorrectChecksum,

    #[error("Wrong number of segments in your code. Check it for typos!")]
    WrongNumberOfSegments,

    #[error("Your code is missing the required checksum digit. Check it for typos!")]
    MissingChecksum,
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Test encoding a message.
    #[test]
    fn test_encode() {
        let peer_code = PeerCode {
            server_id: 27,
            room_code: 314,
            shared_secret: 15,
        };

        let message = peer_code.to_str();
        assert_eq!(message, "1B.13A.F.3");
    }

    #[test]
    fn test_decode() {
        // some uppercase, some lowercase, and spacing
        let message = " 1b.13A.f.3  ";
        let received = PeerCode::parse(message, true).unwrap();

        let expected = PeerCode {
            server_id: 27,
            room_code: 314,
            shared_secret: 15,
        };

        assert_eq!(received, expected);
    }

    #[test]
    fn checksum_test() {
        let message = " 1b.13A.f  ";

        let received = PeerCode::parse(message, true);
        assert!(matches!(received, Err(Error::MissingChecksum)));

        let received = PeerCode::parse(message, false).unwrap();
        let expected = PeerCode {
            server_id: 27,
            room_code: 314,
            shared_secret: 15,
        };
        assert_eq!(received, expected);

        let message = " 1c.13A.f.3  ";
        let received = PeerCode::parse(message, true);
        assert!(matches!(received, Err(Error::IncorrectChecksum)));
    }

    #[test]
    fn invalid_encodings() {
        let message = " 21.q.3  ";

        let received = PeerCode::parse(message, false);
        assert!(matches!(received, Err(Error::CouldntParse(..))));

        let message = " 1b.13A.f.3.4 ";

        let received = PeerCode::parse(message, false);
        assert!(matches!(received, Err(Error::WrongNumberOfSegments)));
    }

    #[test]
    fn test_zeros() {
        let peer_code = PeerCode {
            room_code: 0,
            server_id: 0,
            shared_secret: 0,
        };

        let str: String = peer_code.to_str();
        let received = PeerCode::parse(&str, true).unwrap();
        assert_eq!(peer_code, received);
    }

    #[test]
    fn test_large() {
        let peer_code = PeerCode {
            room_code: u64::MAX,
            server_id: u64::MAX,
            shared_secret: u64::MAX,
        };

        let str: String = peer_code.to_str();
        println!("{str}");
        let received = PeerCode::parse(&str, true).unwrap();
        assert_eq!(peer_code, received);
    }
}
