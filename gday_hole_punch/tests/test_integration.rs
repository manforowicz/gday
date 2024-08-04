#![forbid(unsafe_code)]
#![warn(clippy::all)]

use gday_hole_punch::{server_connector, try_connect_to_peer, ContactSharer, PeerCode};
use std::{
    io::{Read, Write},
    str::FromStr,
};

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

    tokio::task::spawn_blocking(move || {
        let timeout = std::time::Duration::from_secs(5);

        // Channel for Peer 1 to send the PeerCode to Peer 2
        let (code_tx, code_rx) = std::sync::mpsc::channel();

        //////// Peer 1 ////////
        std::thread::spawn(move || {
            // Rendezvous settings
            let peer_code = PeerCode {
                server_id: 0,
                room_code: 123,
                shared_secret: 456,
            };

            // Connect to the server
            let mut server_connection =
                server_connector::connect_tcp(server_addr_1, timeout).unwrap();

            // Create a room in the server, and get my contact from it
            let (contact_sharer, my_contact) =
                ContactSharer::enter_room(&mut server_connection, peer_code.room_code, true).unwrap();

            // Send PeerCode to peer
            let code_to_share = peer_code.to_string();
            code_tx.send(code_to_share).unwrap();

            // Wait for the server to send the peer's contact
            let peer_contact = contact_sharer.get_peer_contact().unwrap();

            // Use TCP hole-punching to connect to the peer,
            // verify their identity with the shared_secret,
            // and get a cryptographically-secure shared key
            let (mut tcp_stream, strong_key) = try_connect_to_peer(
                my_contact.local,
                peer_contact,
                &peer_code.shared_secret.to_be_bytes(),
                timeout,
            )
            .unwrap();

            tcp_stream.write_all(b"Hello peer!").unwrap();

            // never send a secret outside of tests
            tcp_stream.write_all(&strong_key).unwrap();
            tcp_stream.flush().unwrap();
        });

        //////// Peer 2 (on a different computer) ////////

        let received_code = code_rx.recv().unwrap();

        let peer_code = PeerCode::from_str(&received_code).unwrap();

        // Connect to the same server as Peer 1
        let mut server_connection = server_connector::connect_tcp(server_addr_1, timeout).unwrap();

        // Join the same room in the server, and get my local contact
        let (contact_sharer, my_contact) =
            ContactSharer::enter_room(&mut server_connection, peer_code.room_code, false).unwrap();

        // Get peer's contact
        let peer_contact = contact_sharer.get_peer_contact().unwrap();

        // Use hole-punching to connect to peer.
        let (mut tcp_stream, strong_key) = try_connect_to_peer(
            my_contact.local,
            peer_contact,
            &peer_code.shared_secret.to_be_bytes(),
            timeout,
        )
        .unwrap();

        // Ensure the direct connection works
        let mut received = [0_u8; 11];
        tcp_stream.read_exact(&mut received).unwrap();
        assert_eq!(&received, b"Hello peer!");

        // Ensure the peer has the same strong key
        let mut received = [0_u8; 32];
        tcp_stream.read_exact(&mut received).unwrap();
        assert_eq!(received, strong_key);
    })
    .await
    .unwrap();
}
