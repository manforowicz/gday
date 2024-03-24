use thiserror::Error;

/// The Crockford base-32 alphabet
const ALPHABET: [u8; 32] = *b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Represents the code that one peer must give another
/// to start establishing contact.
///
///
/// Can be converted to and from [`String`] in Crockford base-32 of form:
/// `"server_id.room_code.shared_secret.checksum"` in
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
    /// Converts `str` in Crockford base-32 of form:
    /// `"server_id.room_code.shared_secret.checksum"` into [`PeerCode`]
    pub fn from_str(str: &str) -> Result<Self, Error> {
        // split `str` into period-separated substrings
        let mut substrings = str.trim().split('.');

        // decode each segment independently
        let mut segments = [0, 0, 0, 0];
        for segment in &mut segments {
            let Some(substring) = substrings.next() else {
                // return error if less than 4 substrings
                return Err(Error::WrongNumberOfSegments);
            };
            *segment = decode(substring)?;
        }

        // return error if more than 4 substrings
        if substrings.next().is_some() {
            return Err(Error::WrongNumberOfSegments);
        }

        // set fields to segments
        let peer_code = PeerCode {
            server_id: segments[0],
            room_code: segments[1],
            shared_secret: segments[2],
        };

        // verify checksum
        if peer_code.get_checksum() != segments[3] {
            return Err(Error::IncorrectChecksum);
        }

        Ok(peer_code)
    }

    /// Converts [`PeerCode`] into [`String`] in Crockford base-32 of form:
    /// `"server_id.room_code.shared_secret.checksum"`.
    pub fn to_str(&self) -> String {
        let mut s = String::new();

        let fields = [self.server_id, self.room_code, self.shared_secret];

        for field in fields {
            s.push_str(&encode(field));
            s.push('.');
        }

        // append the checksum as the 4-th segment
        s.push_str(&encode(self.get_checksum()));

        s
    }

    /// Calculates a simple hash of the fields, mod 31
    fn get_checksum(&self) -> u64 {
        ((self.server_id % 31) + (self.room_code % 31) * 2 + (self.shared_secret % 31) * 3) % 31
    }
}

/// Convert a crockford base32 string to a `usize`
fn decode(s: &str) -> Result<u64, Error> {
    let mut s = s.trim().to_uppercase().into_bytes();

    // replace similar characters
    for char in &mut s {
        *char = match char {
            b'I' | b'L' => b'1',
            b'O' => b'0',
            _ => *char,
        };
    }

    let mut value: u64 = 0;
    for (place, char) in s.iter().rev().enumerate() {
        // Ignore dashes
        if *char == b'-' {
            continue;
        }

        // Convert character to numerical "digit"
        let Some(digit) = ALPHABET.iter().position(|c| c == char) else {
            return Err(Error::InvalidCharacter);
        };

        // Get the place value of this "digit"
        let Some(place_value) = 32_u64.checked_pow(place as u32) else {
            return Err(Error::ValueTooGreat);
        };

        // Multiply the "digit" by its place value
        let Some(addend) = (digit as u64).checked_mul(place_value) else {
            return Err(Error::ValueTooGreat);
        };

        // Add the result to the total value
        let Some(new_value) = value.checked_add(addend) else {
            return Err(Error::ValueTooGreat);
        };
        value = new_value;
    }
    Ok(value)
}

/// Convert a `usize` to a crockford base32 string
fn encode(mut num: u64) -> String {
    let mut s = Vec::<u8>::new();

    while num != 0 {
        s.push(ALPHABET[(num % 32) as usize]);
        num /= 32;
    }

    s.reverse();

    if s.is_empty() {
        s.push(b'0');
    }

    String::from_utf8(s).unwrap()
}

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    #[error("Invalid character in this base 32 code.")]
    InvalidCharacter,

    #[error("Base 32 value is too large to fit in 64-bit integer")]
    ValueTooGreat,

    #[error("Incorrect checksum. Double check your code!")]
    IncorrectChecksum,

    #[error("Wrong number of segments in this code.")]
    WrongNumberOfSegments,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_general() {
        let peer_code = PeerCode {
            room_code: 17535328141421925132,
            server_id: 4358432574238545432,
            shared_secret: 9175435743820743890,
        };

        let str: String = peer_code.to_str();
        let received = PeerCode::from_str(&str).unwrap();
        assert_eq!(peer_code, received);
    }

    #[test]
    fn test_zeros() {
        let peer_code = PeerCode {
            room_code: 0,
            server_id: 0,
            shared_secret: 0,
        };

        let str: String = peer_code.to_str();
        println!("{str}");
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

        let str: String = peer_code.to_str();
        let received = PeerCode::from_str(&str).unwrap();
        assert_eq!(peer_code, received);
    }
}
