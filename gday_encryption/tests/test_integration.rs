#![forbid(unsafe_code)]
#![warn(clippy::all)]
use gday_encryption::EncryptedStream;
use rand::{RngCore, SeedableRng};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

/// Transfer `bytes` over [`EncryptedStream`],
/// flushing every `chunk_size` bytes.
#[tokio::test]
async fn test_transfers() {
    // A pseudorandom encryption key
    let mut rng = rand::rngs::StdRng::seed_from_u64(5);
    let mut shared_key = [0u8; 32];
    rng.fill_bytes(&mut shared_key);

    // A pseudorandom test vector
    let mut rng = rand::rngs::StdRng::seed_from_u64(10);
    let mut bytes = vec![0_u8; 1_000_000];
    rng.fill_bytes(&mut bytes);

    // How many bytes will be sent at a time
    let chunk_size = 200_000;

    // Listens on the loopback address
    let listener = tokio::net::TcpListener::bind("[::]:0").await.unwrap();
    let pipe_addr = listener.local_addr().unwrap();

    // A thread that will send data to the loopback address
    let bytes_clone = bytes.clone();
    tokio::spawn(async move {
        let mut peer_a = tokio::net::TcpStream::connect(pipe_addr).await.unwrap();

        let mut stream_a = EncryptedStream::encrypt_connection(&mut peer_a, &shared_key)
            .await
            .unwrap();

        for chunk in bytes_clone.chunks(chunk_size) {
            stream_a.write_all(chunk).await.unwrap();
            stream_a.flush().await.unwrap();
        }
        // Ensure calling shutdown multiple times works
        stream_a.shutdown().await.unwrap();
        stream_a.shutdown().await.unwrap();
    });

    // Stream that will receive the test data sent to the loopback address.
    let mut peer_b = listener.accept().await.unwrap().0;
    let mut stream_b = EncryptedStream::encrypt_connection(&mut peer_b, &shared_key)
        .await
        .unwrap();

    // Receive and verify the encrypted test data.
    for chunk in bytes.chunks(chunk_size) {
        let mut received = vec![0; chunk.len()];
        stream_b.read_exact(&mut received).await.unwrap();
        assert_eq!(*chunk, received);
    }

    // EOF should return 0
    assert_eq!(stream_b.read(&mut [0, 0, 0]).await.unwrap(), 0);
    assert_eq!(stream_b.read(&mut [0, 0, 0]).await.unwrap(), 0);
}

/// Test bufread
#[tokio::test]
async fn test_bufread() {
    // A pseudorandom encryption key
    let mut rng = rand::rngs::StdRng::seed_from_u64(20);
    let mut shared_key = [0u8; 32];
    rng.fill_bytes(&mut shared_key);

    // A pseudorandom test vector
    let mut rng = rand::rngs::StdRng::seed_from_u64(25);
    let mut bytes = vec![0_u8; 1_000_000];
    rng.fill_bytes(&mut bytes);
    bytes.push(0);

    // How many bytes will be sent at a time
    let chunk_size = 200_000;

    // Listens on the loopback address
    let listener = tokio::net::TcpListener::bind("[::]:0").await.unwrap();
    let pipe_addr = listener.local_addr().unwrap();

    // A thread that will send data to the loopback address
    let bytes_clone = bytes.clone();
    tokio::spawn(async move {
        let mut peer_a = tokio::net::TcpStream::connect(pipe_addr).await.unwrap();

        let mut stream_a = EncryptedStream::encrypt_connection(&mut peer_a, &shared_key)
            .await
            .unwrap();

        for chunk in bytes_clone.chunks(chunk_size) {
            stream_a.write_all(chunk).await.unwrap();
            stream_a.flush().await.unwrap();
        }

        stream_a.shutdown().await.unwrap();
        stream_a.shutdown().await.unwrap();
    });

    // Stream that will receive the test data sent to the loopback address.
    let mut peer_b = listener.accept().await.unwrap().0;
    let mut stream_b = EncryptedStream::encrypt_connection(&mut peer_b, &shared_key)
        .await
        .unwrap();
    let mut stream_b = tokio::io::BufReader::new(&mut stream_b);

    // Receive and verify the encrypted test data.
    let mut received = Vec::new();
    let zeros = bytes.iter().filter(|num| **num == 0).count();
    for _ in 0..zeros {
        let bytes_read = stream_b.read_until(0, &mut received).await.unwrap();
        assert_ne!(bytes_read, 0);
    }
    assert_eq!(received, bytes);

    // EOF should return 0
    assert_eq!(stream_b.read(&mut [0, 0, 0]).await.unwrap(), 0);
    assert_eq!(stream_b.read(&mut [0, 0, 0]).await.unwrap(), 0);
}

/// Confirm there are no infinite loops on EOF
#[tokio::test]
async fn test_unexpected_eof() {
    let nonce: [u8; 7] = [42; 7];
    let key: [u8; 32] = [123; 32];
    let mut pipe = Vec::new();
    let mut writer = EncryptedStream::new(&mut pipe, &key, &nonce);

    // write the message
    let msg = b"fjsdka;8u39fsdkaf";
    writer.write_all(msg).await.unwrap();
    writer.flush().await.unwrap();

    // remove part of it
    pipe.pop().unwrap();

    // try receiving the broken message
    let mut reader = EncryptedStream::new(&pipe[..], &key, &nonce);
    let mut buf = vec![0; msg.len()];
    let result = reader.read_exact(&mut buf).await;

    // confirm its an error
    assert!(result.is_err());
}
