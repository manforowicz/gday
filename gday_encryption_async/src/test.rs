use tokio::io::{AsyncReadExt, AsyncWriteExt};

// todo: test helper buf



#[tokio::test]
async fn test_all() {
    let nonce = [5; 7];
    let key = [5; 32];
    let (read_stream, write_stream) = tokio::io::duplex(400);
    let mut buf = [0u8; 3];
    let mut writer = crate::WriteHalf::new(write_stream, &key, &nonce);
    let mut reader = crate::ReadHalf::new(read_stream, &key, &nonce);

    let test_data = [
        b"abc", b"def", b"ghi", b"jkl", b"mno", b"prs", b"tuw", b"yzz",
    ];

    for msg in test_data {
        writer.write_all(msg).await.unwrap();
        writer.flush().await.unwrap();
        reader.read_exact(&mut buf).await.unwrap();
        assert_eq!(buf, msg[..]);
    }
}