use std::{
    collections::VecDeque,
    io::{Read, Write},
};

const TEST_DATA: &[&[u8]] = &[
    b"abc5423gsgdds43",
    b"def432gfd2354",
    b"ggdsgdst43646543hi",
    b"g",
    b"mgresgdfgno",
    b"463prs",
    b"tufdxb5436w",
    b"y4325tzz",
    b"132ddsagasfa",
    b"vds dagdsfa",
    b"ete243yfdga",
    b"dbasbalp35",
    b";kbfagp98845",
    b"bjkdal;f023590qjva",
    b"balkdlsaj353osdfa.b",
    b"bfaa;489ajdfakl;db",
];

/// Test sending and receiving many small messages.
#[test]
fn test_small_messages() {
    let nonce: [u8; 7] = [42; 7];
    let key: [u8; 32] = [123; 32];
    let mut pipe = VecDeque::new();
    let mut stream = crate::EncryptedStream::new(&mut pipe, &key, &nonce);

    for &msg in TEST_DATA {
        stream.write_all(msg).unwrap();
        stream.flush().unwrap();
        let mut buf = vec![0; msg.len()];
        stream.read_exact(&mut buf).unwrap();
        assert_eq!(buf, msg);
    }
}

/// Try to spot edge-cases that occur when sending
/// several large messages.
#[test]
fn test_large_messages() {
    let nonce: [u8; 7] = [75; 7];
    let key: [u8; 32] = [22; 32];
    let pipe = VecDeque::new();
    let mut stream = crate::EncryptedStream::new(pipe, &key, &nonce);

    let msg = vec![123; 200_000];

    for _ in 0..5 {
        stream.write_all(&msg).unwrap();
        stream.flush().unwrap();
        let mut received = vec![0; msg.len()];
        stream.read_exact(&mut received).unwrap();
        assert_eq!(msg, received);
    }
}

/// Confirm there are no infinite loops on EOF
#[test]
fn test_unexpected_eof() {
    let nonce: [u8; 7] = [42; 7];
    let key: [u8; 32] = [123; 32];
    let mut pipe = VecDeque::new();
    let mut writer = crate::EncryptedStream::new(&mut pipe, &key, &nonce);

    let msg = TEST_DATA[0];

    writer.write_all(msg).unwrap();
    writer.flush().unwrap();
    pipe.pop_back().unwrap();
    let mut reader = crate::EncryptedStream::new(&mut pipe, &key, &nonce);
    let mut buf = vec![0; msg.len()];
    let result = reader.read_exact(&mut buf);
    assert!(result.is_err());
}
