

#![cfg(test)]
use super::*;

#[tokio::test]
async fn messenger_send_1() {
    let (mut stream_a, mut stream_b) = tokio::io::duplex(1000);
    let mut messenger_a = AsyncMessenger::new(&mut stream_a);
    let mut messenger_b = AsyncMessenger::new(&mut stream_b);

    let sent = ServerMsg::ErrorNoSuchRoomID;

    messenger_a.send(&sent).await.unwrap();

    let received: ServerMsg = messenger_b.receive().await.unwrap();

    assert_eq!(sent, received);
}

#[tokio::test]
async fn messenger_send_2() {
    let (mut stream_a, mut stream_b) = tokio::io::duplex(1000);
    let mut messenger_a = AsyncMessenger::new(&mut stream_a);
    let mut messenger_b = AsyncMessenger::new(&mut stream_b);

    let socket = SocketAddr::V6(SocketAddrV6::new(578674694309532.into(), 1456, 0, 0));

    let sent = ClientMsg::SendAddr {
        room_code: 65721,
        is_creator: false,
        private_addr: Some(socket),
    };

    messenger_a.send(&sent).await.unwrap();

    let received: ClientMsg = messenger_b.receive().await.unwrap();

    assert_eq!(sent, received);
}

#[tokio::test]
async fn messenger_invalid_data() {
    let (mut stream_a, mut stream_b) = tokio::io::duplex(1000);

    // gibberish data
    stream_a
        .write_all(&[0, 12, 53, 24, 85, 52, 24, 123, 32, 52, 52, 52, 13, 35])
        .await
        .unwrap();
    let mut messenger_b = AsyncMessenger::new(&mut stream_b);
    let result: Result<ServerMsg, Error> = messenger_b.receive().await;

    assert!(result.is_err());
}
