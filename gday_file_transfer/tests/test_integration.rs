#![forbid(unsafe_code)]
#![warn(clippy::all)]
use std::collections::HashMap;
use std::fs::{File, create_dir_all};
use std::io::Write;
use std::path::PathBuf;

use gday_file_transfer::*;
use tokio::io::AsyncReadExt;

const TEST_FILENAMES: &[&str] = &[
    "file 1",
    "file 2.txt",
    "dir/subdir 1/file 1",
    "dir/subdir 1/file 2.txt",
    "dir/subdir 2/file 1",
    "dir/subdir 2/file 2.tar.gz",
];

/// Returns a temporary directory
/// with all of [`TEST_FILENAMES`]
/// created in it.
///
/// Each file contains "This is" followed
/// by its filename.
fn make_test_dir() -> tempfile::TempDir {
    let temp_dir = tempfile::tempdir().unwrap();
    let dir_path = temp_dir.path();

    for filename in TEST_FILENAMES {
        create_dir_all(dir_path.join(filename).parent().unwrap()).unwrap();
        let mut f = File::create_new(dir_path.join(filename)).unwrap();
        write!(f, "This is {filename}").unwrap();
    }
    temp_dir
}

/// Confirm that [`create_file_offer()`] returns errors
/// when it should.
#[tokio::test]
async fn test_create_file_offer_errors() {
    let test_dir = make_test_dir();
    let dir_path = test_dir.path().canonicalize().unwrap();

    // trying to get metadata about file that doesn't exist
    assert!(matches!(
        create_file_offer(&[dir_path.join("dir/non-existent.txt")]),
        Err(gday_file_transfer::Error::IO(..))
    ));

    // both paths end in the same name.
    // this would cause confusion with FileMetaLocal.short_path
    assert!(matches!(
        create_file_offer(&[
            dir_path.join("file 1"),
            dir_path.join("dir/subdir 1/file 1")
        ]),
        Err(gday_file_transfer::Error::PathsHaveSameName(..))
    ));

    // one path is prefix of another. that's an error!
    assert!(matches!(
        create_file_offer(&[dir_path.to_path_buf(), dir_path.join("dir")]),
        Err(gday_file_transfer::Error::PathIsPrefix(..))
    ));

    // one path is prefix of another. that's an error!
    assert!(matches!(
        create_file_offer(&[dir_path.join("dir"), dir_path.to_path_buf()]),
        Err(gday_file_transfer::Error::PathIsPrefix(..))
    ));

    // one path is prefix of another. that's an error!
    assert!(matches!(
        create_file_offer(&[
            dir_path.join("dir"),
            dir_path.join("dir/subdir 1/file 2.txt")
        ]),
        Err(gday_file_transfer::Error::PathIsPrefix(..))
    ));

    // one path is prefix of another. that's an error!
    assert!(matches!(
        create_file_offer(&[
            dir_path.join("dir/subdir 1/file 2.txt"),
            dir_path.join("dir")
        ]),
        Err(gday_file_transfer::Error::PathIsPrefix(..))
    ));
}

/// Confirm that [`create_file_offer()`] works.
#[tokio::test]
async fn test_create_file_offer() {
    let test_dir = make_test_dir();
    let dir_path = test_dir.path().canonicalize().unwrap();

    let result = gday_file_transfer::create_file_offer(&[
        dir_path.join("file 1"),
        dir_path.join("dir/subdir 1"),
    ])
    .unwrap();

    let expected_paths = [
        ("file 1", "file 1"),
        ("dir/subdir 1/file 1", "subdir 1/file 1"),
        ("dir/subdir 1/file 2.txt", "subdir 1/file 2.txt"),
    ];

    let mut expected = LocalFileOffer {
        offer: FileOfferMsg {
            offer: HashMap::new(),
        },
        offered_path_to_local: HashMap::new(),
    };

    for (full_path, offered_path) in expected_paths {
        let full_path = dir_path.join(full_path);
        let offered_path = PathBuf::from(offered_path);
        let meta = full_path.metadata().unwrap();

        expected.offer.offer.insert(
            offered_path.clone(),
            FileMetadata {
                size: meta.len(),
                last_modified: meta.modified().unwrap(),
            },
        );

        expected
            .offered_path_to_local
            .insert(offered_path, full_path);
    }

    assert_eq!(result, expected);
}

/// Test the file transfer.
#[tokio::test]
async fn test_file_transfer() {
    // Listens on the loopback address
    let listener = tokio::net::TcpListener::bind("[::1]:0").await.unwrap();
    let pipe_addr = listener.local_addr().unwrap();

    // dir_a contains test files, some of which
    // will be sent
    let dir_a = make_test_dir();
    let dir_a_path = dir_a.path().canonicalize().unwrap();

    // fille offer
    let offered_paths = [dir_a_path.join("file 1"), dir_a_path.join("dir")];
    let offer = create_file_offer(&offered_paths).unwrap();
    let offered_size = offer.offer.get_total_offered_size();

    // A thread that will send data to the loopback address
    tokio::spawn(async move {
        // There will be an interruption after each byte sent
        for _ in 0..offered_size - 1 {
            let mut stream_a = tokio::net::TcpStream::connect(pipe_addr).await.unwrap();
            // send offer, and read response
            write_to_async(&offer.offer, &mut stream_a).await.unwrap();
            let response: FileRequestsMsg = read_from_async(&mut stream_a).await.unwrap();

            // send the files
            let _ = send_files(&offer, &response, &mut stream_a, |_| {}).await;
        }

        // Send the final byte!
        let mut stream_a = tokio::net::TcpStream::connect(pipe_addr).await.unwrap();

        // send offer, and read response
        write_to_async(&offer.offer, &mut stream_a).await.unwrap();
        let response: FileRequestsMsg = read_from_async(&mut stream_a).await.unwrap();

        // send the files
        send_files(&offer, &response, &mut stream_a, |_| {})
            .await
            .unwrap();
    });

    // dir_b will receive the files in
    let dir_b = tempfile::tempdir().unwrap();
    let dir_b_path = dir_b.path().canonicalize().unwrap();
    let mut f = File::create_new(dir_b_path.join("unrelated")).unwrap();
    write!(f, "unrelated").unwrap();

    for _ in 0..offered_size - 1 {
        let mut stream_b = listener.accept().await.unwrap().0;
        let received_offer: FileOfferMsg = read_from_async(&mut stream_b).await.unwrap();
        let response_msg =
            FileRequestsMsg::accept_only_new_and_interrupted(&received_offer, &dir_b_path).unwrap();
        write_to_async(&response_msg, &mut stream_b).await.unwrap();

        let res = receive_files(
            &received_offer,
            &response_msg,
            &dir_b_path,
            tokio::io::BufReader::new(stream_b.take(1)),
            |_| {},
        )
        .await;

        assert!(matches!(res, Err(Error::IO(_))));
    }

    let mut stream_b = listener.accept().await.unwrap().0;
    let received_offer: FileOfferMsg = read_from_async(&mut stream_b).await.unwrap();
    let response_msg =
        FileRequestsMsg::accept_only_new_and_interrupted(&received_offer, &dir_b_path).unwrap();
    write_to_async(&response_msg, &mut stream_b).await.unwrap();

    receive_files(
        &received_offer,
        &response_msg,
        &dir_b_path,
        tokio::io::BufReader::new(stream_b.take(1)),
        |_| {},
    )
    .await
    .unwrap();

    // Ensure sender's directory is unchanged from original
    let dir_original = make_test_dir();
    assert!(!dir_diff::is_different(dir_original.path(), dir_a.path()).unwrap());

    // Ensure receiver's directory is as expected
    let dir_expected = make_test_dir();
    let mut f = File::create_new(dir_expected.path().join("unrelated")).unwrap();
    write!(f, "unrelated").unwrap();
    std::fs::remove_file(dir_expected.path().join("file 2.txt")).unwrap();

    assert!(!dir_diff::is_different(dir_expected.path(), dir_b.path()).unwrap());
}
