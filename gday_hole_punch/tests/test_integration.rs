#![forbid(unsafe_code)]
#![warn(clippy::all)]

use gday_hole_punch::{server_connector, share_contacts, try_connect_to_peer, PeerCode};
use std::str::FromStr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn test_integration() {
    // start the server in the background
    let args = gday_server::Args {
        key: None,
        certificate: None,
        unencrypted: true,
        addresses: vec!["0.0.0.0:0".parse().unwrap(), "[::]:0".parse().unwrap()],
        timeout: 3600,
        request_limit: 10,
        verbosity: log::LevelFilter::Off,
    };
    let (server_addrs, _joinset) = gday_server::start_server(args).unwrap();

    let server_addr_1 = server_addrs[0];

    let timeout = std::time::Duration::from_secs(5);

    // Channel for Peer 1 to send the PeerCode to Peer 2
    let (code_tx, code_rx) = tokio::sync::oneshot::channel();

    let handle = tokio::spawn(async move {
        //////// Peer 1 ////////
        // Rendezvous settings
        let peer_code = PeerCode {
            server_id: 0,
            room_code: "123".to_string(),
            shared_secret: "456".to_string(),
        };

        // Connect to the server
        let mut server_connection = server_connector::connect_tcp(server_addr_1, timeout)
            .await
            .unwrap();

        // Create a room in the server, and get my contact from it
        let (my_contact, peer_contact_fut) =
            share_contacts(&mut server_connection, peer_code.room_code.as_bytes(), true)
                .await
                .unwrap();

        // Send PeerCode to peer
        let code_to_share = String::try_from(&peer_code).unwrap();
        code_tx.send(code_to_share).unwrap();

        // Wait for the server to send the peer's contact
        let peer_contact = peer_contact_fut.await.unwrap();

        // Use TCP hole-punching to connect to the peer,
        // verify their identity with the shared_secret,
        // and get a cryptographically-secure shared key
        let (mut tcp_stream, strong_key) = try_connect_to_peer(
            my_contact.local,
            peer_contact,
            peer_code.shared_secret.as_bytes(),
        )
        .await
        .unwrap();

        tcp_stream.write_all(b"Hello peer!").await.unwrap();

        // never send a secret outside of tests
        tcp_stream.write_all(&strong_key).await.unwrap();
        tcp_stream.flush().await.unwrap();
    });

    //////// Peer 2 (on a different computer) ////////

    let received_code = code_rx.await.unwrap();

    let peer_code = PeerCode::from_str(&received_code).unwrap();

    // Connect to the same server as Peer 1
    let mut server_connection = server_connector::connect_tcp(server_addr_1, timeout)
        .await
        .unwrap();

    // Join the same room in the server, and get my local contact
    let (my_contact, peer_contact_fut) = share_contacts(
        &mut server_connection,
        peer_code.room_code.as_bytes(),
        false,
    )
    .await
    .unwrap();

    // Get peer's contact
    let peer_contact = peer_contact_fut.await.unwrap();

    // Use hole-punching to connect to peer.
    let (mut tcp_stream, strong_key) = try_connect_to_peer(
        my_contact.local,
        peer_contact,
        peer_code.shared_secret.as_bytes(),
    )
    .await
    .unwrap();

    // Ensure the direct connection works
    let mut received = [0_u8; 11];
    tcp_stream.read_exact(&mut received).await.unwrap();
    assert_eq!(&received, b"Hello peer!");

    // Ensure the peer has the same strong key
    let mut received = [0_u8; 32];
    tcp_stream.read_exact(&mut received).await.unwrap();
    assert_eq!(received, strong_key);

    handle.await.unwrap();
}
