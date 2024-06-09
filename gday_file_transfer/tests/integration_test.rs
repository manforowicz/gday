#![forbid(unsafe_code)]
#![warn(clippy::all)]
use std::fs::create_dir_all;
use std::io::Write;
use std::{fs::File, path::PathBuf};

use gday_file_transfer::{
    read_from, write_to, FileMeta, FileMetaLocal, FileOfferMsg, FileResponseMsg,
};

/// Test [`FileMeta`] and [`FileMetaLocal`]
#[test]
fn test_file_metas_errors() {
    let test_dir = set_up_test_dir();
    let dir_path = test_dir.path();

    // both paths end in the same name. that's an error!
    assert!(matches!(
        gday_file_transfer::get_file_metas(&[dir_path.join("file1"), dir_path.join("dir/file1")]),
        Err(gday_file_transfer::Error::PathsHaveSameName(..))
    ));

    // one path is prefix of another. that's an error!
    assert!(matches!(
        gday_file_transfer::get_file_metas(&[dir_path.to_path_buf(), dir_path.join("dir")]),
        Err(gday_file_transfer::Error::PathIsPrefix(..))
    ));

    // one path is prefix of another. that's an error!
    assert!(matches!(
        gday_file_transfer::get_file_metas(&[dir_path.join("dir"), dir_path.to_path_buf()]),
        Err(gday_file_transfer::Error::PathIsPrefix(..))
    ));
}

#[test]
fn test_get_file_metas_1() {
    let test_dir = set_up_test_dir();
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
            short_path: dir_name.join("file2"),
            local_path: dir_path.join("file2"),
            len: dir_path.join("file2").metadata().unwrap().len(),
        },
        FileMetaLocal {
            short_path: dir_name.join("dir/file1"),
            local_path: dir_path.join("dir/file1"),
            len: dir_path.join("dir/file1").metadata().unwrap().len(),
        },
        FileMetaLocal {
            short_path: dir_name.join("dir/file2"),
            local_path: dir_path.join("dir/file2"),
            len: dir_path.join("dir/file2").metadata().unwrap().len(),
        },
        FileMetaLocal {
            short_path: dir_name.join("dir/subdir1/file1"),
            local_path: dir_path.join("dir/subdir1/file1"),
            len: dir_path.join("dir/subdir1/file1").metadata().unwrap().len(),
        },
        FileMetaLocal {
            short_path: dir_name.join("dir/subdir1/file2"),
            local_path: dir_path.join("dir/subdir1/file2"),
            len: dir_path.join("dir/subdir1/file2").metadata().unwrap().len(),
        },
        FileMetaLocal {
            short_path: dir_name.join("dir/subdir2/file1"),
            local_path: dir_path.join("dir/subdir2/file1"),
            len: dir_path.join("dir/subdir2/file1").metadata().unwrap().len(),
        },
        FileMetaLocal {
            short_path: dir_name.join("dir/subdir2/file2"),
            local_path: dir_path.join("dir/subdir2/file2"),
            len: dir_path.join("dir/subdir2/file2").metadata().unwrap().len(),
        },
    ];

    assert_eq!(files.len(), expected.len());
    for e in expected {
        assert!(files.contains(&e));
    }
}

#[test]
fn test_get_file_metas_2() {
    let test_dir = set_up_test_dir();
    let dir_path = test_dir.path();

    let files = gday_file_transfer::get_file_metas(&[
        dir_path.join("dir/subdir1/"),
        dir_path.join("dir/subdir2/file1"),
        dir_path.join("dir/subdir2/file2"),
    ])
    .unwrap();

    let expected = [
        FileMetaLocal {
            short_path: PathBuf::from("subdir1/file1"),
            local_path: dir_path.join("dir/subdir1/file1"),
            len: dir_path.join("dir/subdir1/file1").metadata().unwrap().len(),
        },
        FileMetaLocal {
            short_path: PathBuf::from("subdir1/file2"),
            local_path: dir_path.join("dir/subdir1/file2"),
            len: dir_path.join("dir/subdir1/file2").metadata().unwrap().len(),
        },
        FileMetaLocal {
            short_path: PathBuf::from("file1"),
            local_path: dir_path.join("dir/subdir2/file1"),
            len: dir_path.join("dir/subdir2/file1").metadata().unwrap().len(),
        },
        FileMetaLocal {
            short_path: PathBuf::from("file2"),
            local_path: dir_path.join("dir/subdir2/file2"),
            len: dir_path.join("dir/subdir2/file2").metadata().unwrap().len(),
        },
    ];

    assert_eq!(files.len(), expected.len());
    for e in expected {
        assert!(files.contains(&e));
    }
}

/// Sets up a test directory
fn set_up_test_dir() -> tempfile::TempDir {
    let temp_dir = tempfile::tempdir().unwrap();
    let dir_path = temp_dir.path();

    create_dir_all(dir_path.join("dir/subdir1")).unwrap();
    create_dir_all(dir_path.join("dir/subdir2")).unwrap();

    let mut f = File::create_new(dir_path.join("file1")).unwrap();
    writeln!(f, "This is file1.").unwrap();

    let mut f = File::create_new(dir_path.join("file2")).unwrap();
    writeln!(f, "This is file2.").unwrap();

    let mut f = File::create_new(dir_path.join("dir/file1")).unwrap();
    writeln!(f, "This is dir/file1.").unwrap();

    let mut f = File::create_new(dir_path.join("dir/file2")).unwrap();
    writeln!(f, "This is dir/file2.").unwrap();

    let mut f = File::create_new(dir_path.join("dir/subdir1/file1")).unwrap();
    writeln!(f, "This is dir/subdir1/file1.").unwrap();

    let mut f = File::create_new(dir_path.join("dir/subdir1/file2")).unwrap();
    writeln!(f, "This is dir/subdir1/file2.").unwrap();

    let mut f = File::create_new(dir_path.join("dir/subdir2/file1")).unwrap();
    writeln!(f, "This is dir/subdir2/file1.").unwrap();

    let mut f = File::create_new(dir_path.join("dir/subdir2/file2")).unwrap();
    writeln!(f, "This is dir/subdir2/file2.").unwrap();

    temp_dir
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
