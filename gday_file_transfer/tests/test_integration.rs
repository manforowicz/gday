#![forbid(unsafe_code)]
#![warn(clippy::all)]
use gday_file_transfer::{
    get_file_metas, read_from_async, receive_files, send_files, write_to_async, FileMetaLocal,
    FileOfferMsg, FileResponseMsg,
};
use std::fs::{self, create_dir_all};
use std::io::Write;
use std::{fs::File, path::PathBuf};

/// Returns a temporary directory
/// with the following contents:
///
/// - file1
/// - file2.txt
/// - dir/file1
/// - dir/file2.txt
/// - dir/subdir1/file1
/// - dir/subdir1/file2.txt
/// - dir/subdir2/file1
/// - dir/subdir2/file2.txt
fn make_test_dir() -> tempfile::TempDir {
    let temp_dir = tempfile::tempdir().unwrap();
    let dir_path = temp_dir.path();

    create_dir_all(dir_path.join("dir/subdir1")).unwrap();
    create_dir_all(dir_path.join("dir/subdir2")).unwrap();

    let mut f = File::create_new(dir_path.join("file1")).unwrap();
    write!(f, "This is file1").unwrap();

    let mut f = File::create_new(dir_path.join("file2.txt")).unwrap();
    write!(f, "This is file2.txt").unwrap();

    let mut f = File::create_new(dir_path.join("dir/file1")).unwrap();
    write!(f, "This is dir/file1").unwrap();

    let mut f = File::create_new(dir_path.join("dir/file2.txt")).unwrap();
    write!(f, "This is dir/file2.txt").unwrap();

    let mut f = File::create_new(dir_path.join("dir/subdir1/file1")).unwrap();
    write!(f, "This is dir/subdir1/file1").unwrap();

    let mut f = File::create_new(dir_path.join("dir/subdir1/file2.txt")).unwrap();
    write!(f, "This is dir/subdir1/file2.txt").unwrap();

    let mut f = File::create_new(dir_path.join("dir/subdir2/file1")).unwrap();
    write!(f, "This is dir/subdir2/file1").unwrap();

    let mut f = File::create_new(dir_path.join("dir/subdir2/file2.txt")).unwrap();
    write!(f, "This is dir/subdir2/file2.txt").unwrap();

    temp_dir
}

/// Confirm that [`get_file_metas()`] returns errors
/// when it should.
#[tokio::test]
async fn test_file_metas_errors() {
    let test_dir = make_test_dir();
    let dir_path = test_dir.path();

    // trying to get metadata about file that doesn't exist
    assert!(matches!(
        get_file_metas(&[dir_path.join("dir/non-existent.txt")]),
        Err(gday_file_transfer::Error::IO(..))
    ));

    // both paths end in the same name.
    // this would cause confusion with FileMetaLocal.short_path
    assert!(matches!(
        get_file_metas(&[dir_path.join("file1"), dir_path.join("dir/file1")]),
        Err(gday_file_transfer::Error::PathsHaveSameName(..))
    ));

    // one path is prefix of another. that's an error!
    assert!(matches!(
        get_file_metas(&[dir_path.to_path_buf(), dir_path.join("dir")]),
        Err(gday_file_transfer::Error::PathIsPrefix(..))
    ));

    // one path is prefix of another. that's an error!
    assert!(matches!(
        get_file_metas(&[dir_path.join("dir"), dir_path.to_path_buf()]),
        Err(gday_file_transfer::Error::PathIsPrefix(..))
    ));
}

/// Confirm that [`get_file_metas()`] returns
/// the correct [`FileMetaLocal`] for the whole directory.
#[tokio::test]
async fn test_get_file_metas_1() {
    let test_dir = make_test_dir();
    let dir_path = test_dir.path();
    let dir_name = PathBuf::from(dir_path.file_name().unwrap());
    let mut result = gday_file_transfer::get_file_metas(&[dir_path.to_path_buf()]).unwrap();

    let mut expected = [
        FileMetaLocal {
            short_path: dir_name.join("file1"),
            local_path: dir_path.join("file1"),
            len: dir_path.join("file1").metadata().unwrap().len(),
        },
        FileMetaLocal {
            short_path: dir_name.join("file2.txt"),
            local_path: dir_path.join("file2.txt"),
            len: dir_path.join("file2.txt").metadata().unwrap().len(),
        },
        FileMetaLocal {
            short_path: dir_name.join("dir/file1"),
            local_path: dir_path.join("dir/file1"),
            len: dir_path.join("dir/file1").metadata().unwrap().len(),
        },
        FileMetaLocal {
            short_path: dir_name.join("dir/file2.txt"),
            local_path: dir_path.join("dir/file2.txt"),
            len: dir_path.join("dir/file2.txt").metadata().unwrap().len(),
        },
        FileMetaLocal {
            short_path: dir_name.join("dir/subdir1/file1"),
            local_path: dir_path.join("dir/subdir1/file1"),
            len: dir_path.join("dir/subdir1/file1").metadata().unwrap().len(),
        },
        FileMetaLocal {
            short_path: dir_name.join("dir/subdir1/file2.txt"),
            local_path: dir_path.join("dir/subdir1/file2.txt"),
            len: dir_path
                .join("dir/subdir1/file2.txt")
                .metadata()
                .unwrap()
                .len(),
        },
        FileMetaLocal {
            short_path: dir_name.join("dir/subdir2/file1"),
            local_path: dir_path.join("dir/subdir2/file1"),
            len: dir_path.join("dir/subdir2/file1").metadata().unwrap().len(),
        },
        FileMetaLocal {
            short_path: dir_name.join("dir/subdir2/file2.txt"),
            local_path: dir_path.join("dir/subdir2/file2.txt"),
            len: dir_path
                .join("dir/subdir2/file2.txt")
                .metadata()
                .unwrap()
                .len(),
        },
    ];

    result.sort_unstable();
    expected.sort_unstable();

    assert_eq!(result, expected);
}

/// Confirm that [`get_file_metas()`] returns
/// the correct [`FileMetaLocal`] for multiple files and directories.
#[tokio::test]
async fn test_get_file_metas_2() {
    let test_dir = make_test_dir();
    let dir_path = test_dir.path();

    let mut result = gday_file_transfer::get_file_metas(&[
        dir_path.join("dir/subdir1/"),
        dir_path.join("dir/subdir2/file1"),
        dir_path.join("dir/subdir2/file2.txt"),
    ])
    .unwrap();

    let mut expected = [
        FileMetaLocal {
            short_path: PathBuf::from("subdir1/file1"),
            local_path: dir_path.join("dir/subdir1/file1"),
            len: dir_path.join("dir/subdir1/file1").metadata().unwrap().len(),
        },
        FileMetaLocal {
            short_path: PathBuf::from("subdir1/file2.txt"),
            local_path: dir_path.join("dir/subdir1/file2.txt"),
            len: dir_path
                .join("dir/subdir1/file2.txt")
                .metadata()
                .unwrap()
                .len(),
        },
        FileMetaLocal {
            short_path: PathBuf::from("file1"),
            local_path: dir_path.join("dir/subdir2/file1"),
            len: dir_path.join("dir/subdir2/file1").metadata().unwrap().len(),
        },
        FileMetaLocal {
            short_path: PathBuf::from("file2.txt"),
            local_path: dir_path.join("dir/subdir2/file2.txt"),
            len: dir_path
                .join("dir/subdir2/file2.txt")
                .metadata()
                .unwrap()
                .len(),
        },
    ];

    result.sort_unstable();
    expected.sort_unstable();

    assert_eq!(result, expected);
}

/// Test the file transfer.
#[tokio::test]
async fn file_transfer() {
    // Listens on the loopback address
    let listener = tokio::net::TcpListener::bind("[::1]:0").await.unwrap();
    let pipe_addr = listener.local_addr().unwrap();

    // dir_a contains test files, some of which
    // will be sent
    let dir_a = make_test_dir();
    let dir_a_path = dir_a.path().to_path_buf();

    // A thread that will send data to the loopback address
    tokio::spawn(async move {
        let mut stream_a = tokio::net::TcpStream::connect(pipe_addr).await.unwrap();

        // offer to send file1 and dir
        let paths = [
            dir_a_path.join("file1"),
            dir_a_path.join("file2.txt"),
            dir_a_path.join("dir/subdir1"),
        ];
        let file_metas = get_file_metas(&paths).unwrap();
        let file_offer = FileOfferMsg::from(file_metas.clone());

        // send offer, and read response
        write_to_async(file_offer, &mut stream_a).await.unwrap();
        let response: FileResponseMsg = read_from_async(&mut stream_a).await.unwrap();

        // send the files
        send_files(&file_metas, &response, &mut stream_a, |_| {})
            .await
            .unwrap();
    });

    // dir_b will receive the files in
    let dir_b = tempfile::tempdir().unwrap();

    // create pre-existing file1 and file1 (1)
    let mut f = File::create_new(dir_b.path().join("file1")).unwrap();
    write!(f, "This is a pre-existing file1").unwrap();
    let mut f = File::create_new(dir_b.path().join("file1 (1)")).unwrap();
    write!(f, "This is file1").unwrap();

    // create pre-existing file2.txt
    let mut f = File::create_new(dir_b.path().join("file2.txt")).unwrap();
    write!(f, "This is a pre-existing file2.txt").unwrap();

    // create a partially downloaded file, whose transfer
    // should be resumed
    create_dir_all(dir_b.path().join("subdir1")).unwrap();
    let mut f = File::create_new(dir_b.path().join("subdir1/file2.txt.part29")).unwrap();
    write!(f, "This is dir/subdi").unwrap();

    // Stream that will receive the files from the loopback address.
    let mut stream_b = listener.accept().await.unwrap().0;

    // read the file offer message
    let file_offer: FileOfferMsg = read_from_async(&mut stream_b).await.unwrap();

    let response_msg =
        FileResponseMsg::accept_only_new_and_interrupted(&file_offer, dir_b.path()).unwrap();

    assert_eq!(response_msg.get_num_not_rejected(), 3);
    assert_eq!(response_msg.get_num_partially_accepted(), 1);
    assert_eq!(response_msg.get_num_fully_accepted(), 2);

    write_to_async(&response_msg, &mut stream_b).await.unwrap();

    receive_files(
        &file_offer,
        &response_msg,
        dir_b.path(),
        tokio::io::BufReader::new(stream_b),
        |_| {},
    )
    .await
    .unwrap();

    // confirm that the offered and accepted
    // files were downloaded
    assert_eq!(
        fs::read(dir_a.path().join("dir/subdir1/file1")).unwrap(),
        fs::read(dir_b.path().join("subdir1/file1")).unwrap()
    );
    assert_eq!(
        fs::read(dir_a.path().join("dir/subdir1/file2.txt")).unwrap(),
        fs::read(dir_b.path().join("subdir1/file2.txt")).unwrap()
    );

    // assert that existing files weren't modified
    assert_eq!(
        fs::read(dir_b.path().join("file1")).unwrap(),
        b"This is a pre-existing file1"
    );
    assert_eq!(
        fs::read(dir_b.path().join("file1 (1)")).unwrap(),
        b"This is file1"
    );
    assert_eq!(
        fs::read(dir_b.path().join("file2.txt")).unwrap(),
        b"This is a pre-existing file2.txt"
    );

    // confirm that files rejected or not offered
    // weren't downloaded
    assert!(fs::read(dir_b.path().join("dir/file1")).is_err());
    assert!(fs::read(dir_b.path().join("dir/file1 (2)")).is_err());
    assert!(fs::read(dir_b.path().join("dir/file2.txt")).is_err());
    assert!(fs::read(dir_b.path().join("dir/subdir2/file1")).is_err());
    assert!(fs::read(dir_b.path().join("dir/subdir2/file2.txt")).is_err());
}
