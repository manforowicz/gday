#![forbid(unsafe_code)]
#![warn(clippy::all)]

use gday_contact_exchange_protocol::{read_from, write_to, ClientMsg, Contact, ServerMsg};

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
    let server_ipv4 = *server_addrs.iter().find(|a| a.is_ipv4()).unwrap();
    let server_ipv6 = *server_addrs.iter().find(|a| a.is_ipv6()).unwrap();

    tokio::task::spawn_blocking(move || {
        let local_contact_1 = Contact {
            v4: Some("1.8.3.1:2304".parse().unwrap()),
            v6: Some("[ab:41::b:43]:92".parse().unwrap()),
        };

        let local_contact_2 = Contact {
            v4: Some("3.1.4.1:7853".parse().unwrap()),
            v6: Some("[ab:41:ac::b:1]:5052".parse().unwrap()),
        };

        // connect to the server
        let mut stream_v4 = std::net::TcpStream::connect(server_ipv4).unwrap();
        let mut stream_v6 = std::net::TcpStream::connect(server_ipv6).unwrap();

        // successfully create a room
        write_to(
            ClientMsg::CreateRoom {
                room_code: [123; 32],
            },
            &mut stream_v4,
        )
        .unwrap();
        let response: ServerMsg = read_from(&mut stream_v4).unwrap();
        assert_eq!(response, ServerMsg::RoomCreated);

        // room taken
        write_to(
            ClientMsg::CreateRoom {
                room_code: [123; 32],
            },
            &mut stream_v4,
        )
        .unwrap();
        let response: ServerMsg = read_from(&mut stream_v4).unwrap();
        assert_eq!(response, ServerMsg::ErrorRoomTaken);

        // room doesn't exist
        write_to(
            ClientMsg::RecordPublicAddr {
                room_code: [234; 32],
                is_creator: true,
            },
            &mut stream_v6,
        )
        .unwrap();
        let response: ServerMsg = read_from(&mut stream_v6).unwrap();
        assert_eq!(response, ServerMsg::ErrorNoSuchRoomCode);

        // record public address
        write_to(
            ClientMsg::RecordPublicAddr {
                room_code: [123; 32],
                is_creator: true,
            },
            &mut stream_v4,
        )
        .unwrap();
        let response: ServerMsg = read_from(&mut stream_v4).unwrap();
        assert_eq!(response, ServerMsg::ReceivedAddr);

        // record public address
        write_to(
            ClientMsg::RecordPublicAddr {
                room_code: [123; 32],
                is_creator: false,
            },
            &mut stream_v6,
        )
        .unwrap();
        let response: ServerMsg = read_from(&mut stream_v6).unwrap();
        assert_eq!(response, ServerMsg::ReceivedAddr);

        // set creator to done
        write_to(
            ClientMsg::ReadyToShare {
                local_contact: local_contact_1,
                room_code: [123; 32],
                is_creator: true,
            },
            &mut stream_v4,
        )
        .unwrap();
        let response: ServerMsg = read_from(&mut stream_v4).unwrap();
        let ServerMsg::ClientContact(client_contact) = response else {
            panic!("Server replied with {response:?} instead of ClientContact");
        };
        assert_eq!(client_contact.local, local_contact_1);

        // can't update client once it is done
        write_to(
            ClientMsg::RecordPublicAddr {
                room_code: [123; 32],
                is_creator: true,
            },
            &mut stream_v6,
        )
        .unwrap();
        let response: ServerMsg = read_from(&mut stream_v6).unwrap();
        assert_eq!(response, ServerMsg::ErrorUnexpectedMsg);

        // successfully create an unrelated room
        write_to(
            ClientMsg::CreateRoom {
                room_code: [234; 32],
            },
            &mut stream_v6,
        )
        .unwrap();
        let response: ServerMsg = read_from(&mut stream_v6).unwrap();
        assert_eq!(response, ServerMsg::RoomCreated);

        // set joiner to done
        write_to(
            ClientMsg::ReadyToShare {
                local_contact: local_contact_2,
                room_code: [123; 32],
                is_creator: false,
            },
            &mut stream_v6,
        )
        .unwrap();
        let response: ServerMsg = read_from(&mut stream_v6).unwrap();
        let ServerMsg::ClientContact(client_contact) = response else {
            panic!("Server replied with {response:?} instead of ClientContact");
        };
        assert_eq!(client_contact.local, local_contact_2);

        // ensure peer contact 1 properly exchanged
        let response: ServerMsg = read_from(&mut stream_v4).unwrap();
        let ServerMsg::PeerContact(peer_contact) = response else {
            panic!("Server replied with {response:?} instead of PeerContact");
        };
        assert_eq!(peer_contact.local, local_contact_2);

        // ensure peer contact 2 properly exchanged
        let response: ServerMsg = read_from(&mut stream_v6).unwrap();
        let ServerMsg::PeerContact(peer_contact) = response else {
            panic!("Server replied with {response:?} instead of PeerContact");
        };
        assert_eq!(peer_contact.local, local_contact_1);

        // ensure the room was closed, and can be reopened
        write_to(
            ClientMsg::CreateRoom {
                room_code: [123; 32],
            },
            &mut stream_v4,
        )
        .unwrap();
        let response: ServerMsg = read_from(&mut stream_v4).unwrap();
        assert_eq!(response, ServerMsg::RoomCreated);
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn test_request_limit() {
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
    let server_ipv4 = *server_addrs.iter().find(|a| a.is_ipv4()).unwrap();
    let server_ipv6 = *server_addrs.iter().find(|a| a.is_ipv6()).unwrap();

    tokio::task::spawn_blocking(move || {
        // connect to the server
        let mut stream_v4 = std::net::TcpStream::connect(server_ipv4).unwrap();
        let mut stream_v6 = std::net::TcpStream::connect(server_ipv6).unwrap();

        for room_code in 1..=10 {
            // successfully create a room
            write_to(
                ClientMsg::CreateRoom {
                    room_code: [room_code; 32],
                },
                &mut stream_v4,
            )
            .unwrap();
            let response: ServerMsg = read_from(&mut stream_v4).unwrap();
            assert_eq!(response, ServerMsg::RoomCreated);
        }

        // request limit hit
        write_to(
            ClientMsg::CreateRoom {
                room_code: [11; 32],
            },
            &mut stream_v4,
        )
        .unwrap();
        let response: ServerMsg = read_from(&mut stream_v4).unwrap();
        assert_eq!(response, ServerMsg::ErrorTooManyRequests);

        // ensure the server closed the connection
        let result = write_to(
            ClientMsg::CreateRoom {
                room_code: [100; 32],
            },
            &mut stream_v4,
        );
        assert!(matches!(
            result,
            Err(gday_contact_exchange_protocol::Error::IO(_))
        ));

        // ensure other connections are unaffected
        write_to(
            ClientMsg::CreateRoom {
                room_code: [200; 32],
            },
            &mut stream_v6,
        )
        .unwrap();
        let response: ServerMsg = read_from(&mut stream_v6).unwrap();
        assert_eq!(response, ServerMsg::RoomCreated);
    })
    .await
    .unwrap();
}
