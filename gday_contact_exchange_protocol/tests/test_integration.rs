#![forbid(unsafe_code)]
#![warn(clippy::all)]
use gday_contact_exchange_protocol::{
    read_from, read_from_async, write_to, write_to_async, ClientMsg, Contact, Error, FullContact,
    ServerMsg,
};
use std::io::Write;
use tokio::io::AsyncWriteExt;

/// Test serializing and deserializing messages.
#[test]
fn sending_messages() {
    let mut pipe = std::collections::VecDeque::new();

    for msg in get_client_msg_examples() {
        write_to(msg, &mut pipe).unwrap();
    }

    for msg in get_client_msg_examples() {
        let deserialized_msg: ClientMsg = read_from(&mut pipe).unwrap();
        assert_eq!(msg, deserialized_msg);
    }

    for msg in get_server_msg_examples() {
        write_to(msg, &mut pipe).unwrap();
    }

    for msg in get_server_msg_examples() {
        let deserialized_msg: ServerMsg = read_from(&mut pipe).unwrap();
        assert_eq!(msg, deserialized_msg);
    }
}

#[test]
fn error_on_invalid_json() {
    let mut pipe = std::collections::VecDeque::new();

    // gibberish json
    pipe.write_all(&[0, 1, 0, 5, 52, 45, 77, 123, 12]).unwrap();
    let result: Result<ServerMsg, Error> = read_from(&mut pipe);
    assert!(matches!(result, Err(Error::JSON(_))));
}

#[test]
fn error_on_incompatible_version() {
    let mut pipe = std::collections::VecDeque::new();

    // invalid version
    pipe.write_all(&[0, 2, 0, 5, 52, 45, 77, 123, 12]).unwrap();
    let result: Result<ServerMsg, Error> = read_from(&mut pipe);
    assert!(matches!(result, Err(Error::IncompatibleProtocol)));
}

/// Test serializing and deserializing messages asynchronously.
#[tokio::test]
async fn sending_messages_async() {
    let (mut writer, mut reader) = tokio::io::duplex(1000);

    for msg in get_client_msg_examples() {
        write_to_async(msg, &mut writer).await.unwrap();
    }

    for msg in get_client_msg_examples() {
        let deserialized_msg: ClientMsg = read_from_async(&mut reader).await.unwrap();
        assert_eq!(msg, deserialized_msg);
    }

    for msg in get_server_msg_examples() {
        write_to_async(msg, &mut writer).await.unwrap();
    }

    for msg in get_server_msg_examples() {
        let deserialized_msg: ServerMsg = read_from_async(&mut reader).await.unwrap();
        assert_eq!(msg, deserialized_msg);
    }
}

#[tokio::test]
async fn error_on_invalid_json_async() {
    let (mut writer, mut reader) = tokio::io::duplex(1000);
    // gibberish json
    writer
        .write_all(&[0, 1, 0, 5, 52, 45, 77, 123, 12])
        .await
        .unwrap();
    let result: Result<ServerMsg, Error> = read_from_async(&mut reader).await;
    assert!(matches!(result, Err(Error::JSON(_))));
}

#[tokio::test]
async fn error_on_incompatible_version_async() {
    let (mut writer, mut reader) = tokio::io::duplex(1000);
    // gibberish json
    writer
        .write_all(&[0, 2, 0, 5, 52, 45, 77, 123, 12])
        .await
        .unwrap();
    let result: Result<ServerMsg, Error> = read_from_async(&mut reader).await;
    assert!(matches!(result, Err(Error::IncompatibleProtocol)));
}

/// Get a [`Vec`] of example [`ClientMsg`]s.
fn get_client_msg_examples() -> Vec<ClientMsg> {
    vec![
        ClientMsg::CreateRoom {
            room_code: *b"fjdsafdssds89fph9ewafhusdp9afhas",
        },
        ClientMsg::RecordPublicAddr {
            room_code: *b"fdsjafp89rejfnsdi;ofnsdo;jfsadif",
            is_creator: true,
        },
        ClientMsg::ReadyToShare {
            room_code: *b"jfdsi9uapfj89erpajf98sdpfajisdaf",
            is_creator: false,
            local_contact: Contact {
                v4: Some("31.31.65.31:324".parse().unwrap()),
                v6: Some("[2001:db8::1]:8080".parse().unwrap()),
            },
        },
    ]
}

/// Get a [`Vec`] of example [`ServerMsg`]s.
fn get_server_msg_examples() -> Vec<ServerMsg> {
    vec![
        ServerMsg::RoomCreated,
        ServerMsg::ReceivedAddr,
        ServerMsg::ClientContact(FullContact {
            local: Contact {
                v4: Some("31.31.65.31:324".parse().unwrap()),
                v6: Some("[2001:db8::1]:8080".parse().unwrap()),
            },
            public: Contact {
                v4: Some("31.31.65.31:324".parse().unwrap()),
                v6: Some("[2001:db8::1]:8080".parse().unwrap()),
            },
        }),
        ServerMsg::PeerContact(FullContact {
            local: Contact {
                v4: Some("31.31.65.31:324".parse().unwrap()),
                v6: Some("[2001:db8::1]:8080".parse().unwrap()),
            },
            public: Contact {
                v4: Some("31.31.65.31:324".parse().unwrap()),
                v6: Some("[2001:db8::1]:8080".parse().unwrap()),
            },
        }),
        ServerMsg::ErrorRoomTaken,
        ServerMsg::ErrorPeerTimedOut,
        ServerMsg::ErrorNoSuchRoomCode,
        ServerMsg::ErrorTooManyRequests,
        ServerMsg::ErrorSyntax,
        ServerMsg::ErrorConnection,
    ]
}
