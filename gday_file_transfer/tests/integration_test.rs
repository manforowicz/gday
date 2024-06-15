#![forbid(unsafe_code)]
#![warn(clippy::all)]
use gday_file_transfer::{
    get_file_metas, read_from, receive_files, send_files, write_to, FileMeta, FileMetaLocal,
    FileOfferMsg, FileResponseMsg,
};
use std::fs::{self, create_dir_all};
use std::io::Write;
use std::net::{Ipv6Addr, SocketAddr};
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
#[test]
fn test_file_metas_errors() {
    let test_dir = make_test_dir();
    let dir_path = test_dir.path();

    // trying to get metadata about file that doesn't exist
    assert!(matches!(
        get_file_metas(&[dir_path.join("dir/non-existent.txt")]),
        Err(gday_file_transfer::Error::IO(..))
    ));

    // both paths end in the same name. that's an error!
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
#[test]
fn test_get_file_metas_1() {
    let test_dir = make_test_dir();
    let dir_path = test_dir.path();
    let dir_name = PathBuf::from(dir_path.file_name().unwrap());
    let files = gday_file_transfer::get_file_metas(&[dir_path.to_path_buf()]).unwrap();

    let expected = [
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

    assert_eq!(files.len(), expected.len());
    for e in expected {
        assert!(files.contains(&e));
    }
}

/// Confirm that [`get_file_metas()`] returns
/// the correct [`FileMetaLocal`] for multiple files and directories.
#[test]
fn test_get_file_metas_2() {
    let test_dir = make_test_dir();
    let dir_path = test_dir.path();

    let files = gday_file_transfer::get_file_metas(&[
        dir_path.join("dir/subdir1/"),
        dir_path.join("dir/subdir2/file1"),
        dir_path.join("dir/subdir2/file2.txt"),
    ])
    .unwrap();

    let expected = [
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

    assert_eq!(files.len(), expected.len());
    for e in expected {
        assert!(files.contains(&e));
    }
}

/// Test the file transfer.
#[test]
fn file_transfer() {
    // The loopback address that peer_a will connect to.
    let pipe_addr = SocketAddr::from((Ipv6Addr::LOCALHOST, 2000));

    // Listens on the loopback address
    let listener = std::net::TcpListener::bind(pipe_addr).unwrap();

    // dir_a contains test files, some of which
    // will be sent
    let dir_a = make_test_dir();
    let dir_a_path = dir_a.path().to_path_buf();

    // A thread that will send data to the loopback address
    std::thread::spawn(move || {
        let mut stream_a = std::net::TcpStream::connect(pipe_addr).unwrap();

        // offer to send file1 and dir
        let paths = [dir_a_path.join("file1"), dir_a_path.join("dir")];
        let file_metas = get_file_metas(&paths).unwrap();
        let file_offer = FileOfferMsg::from(file_metas.clone());
        write_to(file_offer, &mut stream_a).unwrap();

        // read the response from the peer
        let response: FileResponseMsg = read_from(&mut stream_a).unwrap();

        // send the files
        send_files(file_metas, response, &mut stream_a, |_| {}).unwrap();
    });

    // directory to receive the files in
    let dir_b = tempfile::tempdir().unwrap();

    // create pre-existing files, that shouldn't be affected
    let mut f = File::create_new(dir_b.path().join("file1")).unwrap();
    write!(f, "This is a pre-existing file1").unwrap();
    let mut f = File::create_new(dir_b.path().join("file1 (1)")).unwrap();
    write!(f, "This is a pre-existing file1 (1)").unwrap();

    create_dir_all(dir_b.path().join("dir/")).unwrap();
    let mut f = File::create_new(dir_b.path().join("dir/file2.txt")).unwrap();
    write!(f, "This is a pre-existing dir/file2.txt").unwrap();

    // create an unrelated file, that shouldn't be effected
    let mut f = File::create_new(dir_b.path().join("unrelated")).unwrap();
    write!(f, "This file is unrelated").unwrap();

    // create a partially downloaded file, whose transfer
    // should be resumed
    let mut f = File::create_new(dir_b.path().join("dir/file1.part")).unwrap();
    write!(f, "This is d").unwrap();

    // Stream that will receive the files to the loopback address.
    let mut stream_b = listener.accept().unwrap().0;

    // read the file offer message
    let file_offer: FileOfferMsg = read_from(&mut stream_b).unwrap();

    // get indices of two files to reject
    let reject1 = file_offer
        .files
        .iter()
        .position(|p| p.short_path == PathBuf::from("dir/subdir1/file1"))
        .unwrap();
    let reject2 = file_offer
        .files
        .iter()
        .position(|p| p.short_path == PathBuf::from("dir/subdir1/file2.txt"))
        .unwrap();

    // get index of file to resume
    let resume = file_offer
        .files
        .iter()
        .position(|p| p.short_path == PathBuf::from("dir/file1"))
        .unwrap();

    // respond to the peer's file offer
    let mut response = vec![Some(0); file_offer.files.len()];
    response[reject1] = None;
    response[reject2] = None;
    response[resume] = Some(9);
    let response_msg = FileResponseMsg { response };

    assert_eq!(response_msg.get_total_num_accepted(), 5);
    assert_eq!(response_msg.get_num_partially_accepted(), 1);

    write_to(&response_msg, &mut stream_b).unwrap();

    receive_files(
        file_offer,
        response_msg,
        dir_b.path(),
        &mut stream_b,
        |_| {},
    )
    .unwrap();

    // confirm that the offered and accepted
    // files were downloaded
    assert_eq!(
        fs::read(dir_a.path().join("file1")).unwrap(),
        fs::read(dir_b.path().join("file1 (2)")).unwrap()
    );
    assert_eq!(
        fs::read(dir_a.path().join("dir/file1")).unwrap(),
        fs::read(dir_b.path().join("dir/file1")).unwrap()
    );
    assert_eq!(
        fs::read(dir_a.path().join("dir/file2.txt")).unwrap(),
        fs::read(dir_b.path().join("dir/file2 (1).txt")).unwrap()
    );
    assert_eq!(
        fs::read(dir_a.path().join("dir/subdir2/file1")).unwrap(),
        fs::read(dir_b.path().join("dir/subdir2/file1")).unwrap()
    );
    assert_eq!(
        fs::read(dir_a.path().join("dir/subdir2/file2.txt")).unwrap(),
        fs::read(dir_b.path().join("dir/subdir2/file2.txt")).unwrap()
    );

    // assert that existing files weren't modified
    assert_eq!(
        fs::read(dir_b.path().join("file1")).unwrap(),
        b"This is a pre-existing file1"
    );
    assert_eq!(
        fs::read(dir_b.path().join("file1 (1)")).unwrap(),
        b"This is a pre-existing file1 (1)"
    );
    assert_eq!(
        fs::read(dir_b.path().join("dir/file2.txt")).unwrap(),
        b"This is a pre-existing dir/file2.txt"
    );
    assert_eq!(
        fs::read(dir_b.path().join("unrelated")).unwrap(),
        b"This file is unrelated"
    );

    // confirm that files rejected or not offered
    // weren't downloaded
    assert!(fs::read(dir_b.path().join("file2")).is_err());
    assert!(fs::read(dir_b.path().join("dir/subdir1/file1")).is_err());
    assert!(fs::read(dir_b.path().join("dir/subdir1/file2")).is_err());
}

/// Test serializing and deserializing [`FileOfferMsg`] and [`FileResponseMsg`].
#[test]
fn sending_messages() {
    let mut pipe = std::collections::VecDeque::new();

    for msg in get_offer_msg_examples() {
        write_to(msg, &mut pipe).unwrap();
    }

    for msg in get_offer_msg_examples() {
        let deserialized_msg: FileOfferMsg = read_from(&mut pipe).unwrap();
        assert_eq!(msg, deserialized_msg);
    }

    for msg in get_response_msg_examples() {
        write_to(msg, &mut pipe).unwrap();
    }

    for msg in get_response_msg_examples() {
        let deserialized_msg: FileResponseMsg = read_from(&mut pipe).unwrap();
        assert_eq!(msg, deserialized_msg);
    }
}

fn get_offer_msg_examples() -> Vec<FileOfferMsg> {
    vec![
        FileOfferMsg {
            files: vec![
                FileMeta {
                    short_path: PathBuf::from("example/path"),
                    len: 43,
                },
                FileMeta {
                    short_path: PathBuf::from("/foo/hello"),
                    len: 50,
                },
            ],
        },
        FileOfferMsg { files: Vec::new() },
    ]
}

fn get_response_msg_examples() -> Vec<FileResponseMsg> {
    vec![
        FileResponseMsg {
            response: vec![None, Some(0), Some(100)],
        },
        FileResponseMsg {
            response: vec![None, None, None],
        },
    ]
}
