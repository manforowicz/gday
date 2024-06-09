#![forbid(unsafe_code)]
#![warn(clippy::all)]
use std::{
    collections::VecDeque,
    io::{Read, Write},
};

use gday_encryption::EncryptedStream;

use rand::{rngs::StdRng, RngCore, SeedableRng};

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
    // generate pseudorandom data from a seed
    let mut rng = StdRng::seed_from_u64(5);

    // set up a pipe
    let mut nonce: [u8; 7] = [0; 7];
    let mut key: [u8; 32] = [0; 32];
    rng.fill_bytes(&mut nonce);
    rng.fill_bytes(&mut key);
    let mut pipe = VecDeque::new();
    let mut stream = EncryptedStream::new(&mut pipe, &key, &nonce);

    for &msg in TEST_DATA {
        // write the message
        stream.write_all(msg).unwrap();
        stream.flush().unwrap();

        // receive the message
        let mut buf = vec![0; msg.len()];
        stream.read_exact(&mut buf).unwrap();

        // verify the message is correct
        assert_eq!(buf, msg);
    }
}

/// Try to spot edge-cases that occur when sending
/// several large messages.
#[test]
fn test_large_messages() {
    // generate pseudorandom data from a seed
    let mut rng = StdRng::seed_from_u64(0);

    // set up a pipe
    let mut nonce: [u8; 7] = [0; 7];
    let mut key: [u8; 32] = [0; 32];
    rng.fill_bytes(&mut nonce);
    rng.fill_bytes(&mut key);
    let mut pipe = VecDeque::new();
    let mut stream = EncryptedStream::new(&mut pipe, &key, &nonce);

    let mut msg = vec![123; 200_000];

    for _ in 0..5 {
        // prepare a pseudorandom message to write
        rng.fill_bytes(&mut msg);

        // write the message
        stream.write_all(&msg).unwrap();
        stream.flush().unwrap();

        // receive the message
        let mut received = vec![0; msg.len()];
        stream.read_exact(&mut received).unwrap();

        // verify the message is correct
        assert_eq!(msg, received);
    }
}

/// Confirm there are no infinite loops on EOF
#[test]
fn test_unexpected_eof() {
    let nonce: [u8; 7] = [42; 7];
    let key: [u8; 32] = [123; 32];
    let mut pipe = VecDeque::new();
    let mut writer = EncryptedStream::new(&mut pipe, &key, &nonce);

    let msg = b"fjsdka;8u39fsdkaf";

    writer.write_all(msg).unwrap();
    writer.flush().unwrap();
    pipe.pop_back().unwrap();
    let mut reader = EncryptedStream::new(&mut pipe, &key, &nonce);
    let mut buf = vec![0; msg.len()];
    let result = reader.read_exact(&mut buf);
    assert!(result.is_err());
}
