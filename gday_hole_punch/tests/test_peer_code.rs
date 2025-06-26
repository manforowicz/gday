use gday_hole_punch::{Error, PeerCode};
use std::str::FromStr;

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

#[test]
fn test_random() {
    let peer_code = PeerCode::random(5, 6);

    assert_eq!(peer_code.server_id, 5);
    assert_eq!(peer_code.room_code.len(), 6);
    assert_eq!(peer_code.shared_secret.len(), 6);

    let str = String::try_from(&peer_code).unwrap();
    let received = PeerCode::from_str(&str).unwrap();
    assert_eq!(peer_code, received);
}
