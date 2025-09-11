use gday_hole_punch::{Error, PeerCode};
use std::str::FromStr;

/// Test encoding a message.
#[test]
fn test_encode() {
    let peer_code = PeerCode::new(27, "hello123".to_string(), "coded".to_string()).unwrap();

    let message = peer_code.to_string();
    assert_eq!(message, "27.hello123.coded");
}

#[test]
fn test_decode() {
    // some uppercase, some lowercase, and spacing
    let message = "  83221.roomcodefoo.secret123   ";
    let received1 = PeerCode::from_str(message).unwrap();
    let received2: PeerCode = message.parse().unwrap();
    let received3 = PeerCode::try_from(message).unwrap();

    let expected =
        PeerCode::new(83221, "roomcodefoo".to_string(), "secret123".to_string()).unwrap();

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
fn invalid_encodes_1() {
    let peer_code = PeerCode::new(0, "hithere".to_string(), "what.".to_string());

    assert!(matches!(
        peer_code,
        Err(Error::PeerCodeContainedInvalidChar)
    ));
}

#[test]
fn invalid_encodes_2() {
    let peer_code = PeerCode::new(0, "hi there".to_string(), "what".to_string());

    assert!(matches!(
        peer_code,
        Err(Error::PeerCodeContainedInvalidChar)
    ));
}

#[test]
fn test_zeros() {
    let peer_code = PeerCode::new(0, "".to_string(), "".to_string()).unwrap();

    let str = peer_code.to_string();
    let received = PeerCode::from_str(&str).unwrap();
    assert_eq!(peer_code, received);
}

#[test]
fn test_large() {
    let peer_code = PeerCode::new(
        u64::MAX,
        "jfisd;afljksda;".to_string(),
        "rf98032fsf02fa".to_string(),
    )
    .unwrap();

    let str = peer_code.to_string();
    let received = PeerCode::from_str(&str).unwrap();
    assert_eq!(peer_code, received);
}

#[test]
fn test_random() {
    let peer_code = PeerCode::random(5, 6);

    assert_eq!(peer_code.server_id(), 5);
    assert_eq!(peer_code.room_code().len(), 6);
    assert_eq!(peer_code.shared_secret().len(), 6);

    let str = peer_code.to_string();
    let received = PeerCode::from_str(&str).unwrap();
    assert_eq!(peer_code, received);
}
