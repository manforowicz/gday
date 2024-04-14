#![cfg(test)]
use crate::{ClientMsg, Contact, FullContact, ServerMsg};

/// Test serializing and deserializing messages.
#[test]
fn sending_messages() {
    let mut bytes = std::collections::VecDeque::new();

    for msg in get_client_msg_examples() {
        crate::to_writer(msg, &mut bytes).unwrap();
    }

    for msg in get_client_msg_examples() {
        let deserialized_msg: ClientMsg = crate::from_reader(&mut bytes).unwrap();
        assert_eq!(msg, deserialized_msg);
    }

    for msg in get_server_msg_examples() {
        crate::to_writer(msg, &mut bytes).unwrap();
    }

    for msg in get_server_msg_examples() {
        let deserialized_msg: ServerMsg = crate::from_reader(&mut bytes).unwrap();
        assert_eq!(msg, deserialized_msg);
    }
}

/// Test serializing and deserializing messages asynchronously.
#[tokio::test]
async fn sending_messages_async() {
    let (mut writer, mut reader) = tokio::io::duplex(1000);

    for msg in get_client_msg_examples() {
        crate::serialize_into_async(msg, &mut writer).await.unwrap();
    }

    for msg in get_client_msg_examples() {
        let deserialized_msg: ClientMsg = crate::deserialize_from_async(&mut reader).await.unwrap();
        assert_eq!(msg, deserialized_msg);
    }

    for msg in get_server_msg_examples() {
        crate::serialize_into_async(msg, &mut writer).await.unwrap();
    }

    for msg in get_server_msg_examples() {
        let deserialized_msg: ServerMsg = crate::deserialize_from_async(&mut reader).await.unwrap();
        assert_eq!(msg, deserialized_msg);
    }
}

/// Get a [`Vec`] of example [`ClientMsg`]s.
fn get_client_msg_examples() -> Vec<ClientMsg> {
    vec![
        ClientMsg::CreateRoom { room_code: 452932 },
        ClientMsg::SendAddr {
            room_code: 2345,
            is_creator: true,
            private_addr: Some("31.31.65.31:324".parse().unwrap()),
        },
        ClientMsg::DoneSending {
            room_code: 24325423,
            is_creator: false,
        },
    ]
}

/// Get a [`Vec`] of example [`ServerMsg`]s.
fn get_server_msg_examples() -> Vec<ServerMsg> {
    vec![
        ServerMsg::RoomCreated,
        ServerMsg::ReceivedAddr,
        ServerMsg::ClientContact(FullContact {
            private: Contact {
                v4: Some("31.31.65.31:324".parse().unwrap()),
                v6: Some("[2001:db8::1]:8080".parse().unwrap()),
            },
            public: Contact {
                v4: Some("31.31.65.31:324".parse().unwrap()),
                v6: Some("[2001:db8::1]:8080".parse().unwrap()),
            },
        }),
        ServerMsg::PeerContact(FullContact {
            private: Contact {
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
        ServerMsg::ErrorNoSuchRoomID,
        ServerMsg::ErrorTooManyRequests,
        ServerMsg::ErrorSyntax,
        ServerMsg::ErrorConnection,
    ]
}
