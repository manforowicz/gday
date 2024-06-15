#![forbid(unsafe_code)]
#![warn(clippy::all)]
use gday_encryption::EncryptedStream;
use rand::RngCore;
use std::{
    collections::VecDeque,
    io::{Read, Write},
    net::Ipv6Addr,
    net::SocketAddr,
};

/// Try to spot edge-cases that occur when sending
/// several large messages.
#[test]
fn test_large_messages() {
    let mut bytes = vec![0_u8; 1_000_000];
    rand::thread_rng().fill_bytes(&mut bytes);

    test_transfers(bytes, 200_000);
}

/// Transfer `bytes` over [`EncryptedStream`],
/// flushing every `chunk_size` bytes.
fn test_transfers(bytes: Vec<u8>, chunk_size: usize) {
    // A random encryption key
    let shared_key: [u8; 32] = rand::random();

    // The loopback address that peer_a will connect to.
    let pipe_addr = SocketAddr::from((Ipv6Addr::LOCALHOST, 2000));

    // Listens on the loopback address
    let listener = std::net::TcpListener::bind(pipe_addr).unwrap();

    // A thread that will send data to the loopback address
    let bytes_clone = bytes.clone();
    std::thread::spawn(move || {
        let mut peer_a = std::net::TcpStream::connect(pipe_addr).unwrap();

        let mut stream_a = EncryptedStream::encrypt_connection(&mut peer_a, &shared_key).unwrap();

        for chunk in bytes_clone.chunks(chunk_size) {
            stream_a.write_all(chunk).unwrap();
            stream_a.flush().unwrap();
        }
    });

    // Stream that will receive the test data sent to the loopback address.
    let mut peer_b = listener.accept().unwrap().0;
    let mut stream_b = EncryptedStream::encrypt_connection(&mut peer_b, &shared_key).unwrap();

    // Receive and verify the encrypted test data.
    for chunk in bytes.chunks(chunk_size) {
        let mut received = vec![0; chunk.len()];
        stream_b.read_exact(&mut received).unwrap();
        assert_eq!(*chunk, received);
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
