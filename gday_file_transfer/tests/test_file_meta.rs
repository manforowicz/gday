use gday_file_transfer::FileMeta;
use std::io::Write;
use std::{fs::File, path::PathBuf};

/// Tests methods of [`FileMeta`] with a non-empty directory.
#[tokio::test]
async fn test_file_meta_1() {
    // create test directory
    let temp_dir = tempfile::tempdir().unwrap();
    let dir_path = temp_dir.path();
    std::fs::create_dir_all(dir_path.join("fol der")).unwrap();

    let mut f = File::create_new(dir_path.join("fol der/file.tar.gz")).unwrap();
    write!(f, "---").unwrap();

    let mut f = File::create_new(dir_path.join("fol der/file (1).tar.gz")).unwrap();
    write!(f, "---").unwrap();

    let mut f = File::create_new(dir_path.join("fol der/file.tar.gz.part5")).unwrap();
    write!(f, "--").unwrap();

    let file_meta = FileMeta {
        short_path: PathBuf::from("fol der/file.tar.gz"),
        len: 5,
    };

    // save path is the save directory joined with the short path
    let save_path = file_meta.get_save_path(dir_path);
    assert_eq!(save_path, dir_path.join("fol der/file.tar.gz"));

    // unoccupied path should increment the appended number by one
    let save_path = file_meta.get_unoccupied_save_path(dir_path).await.unwrap();
    assert_eq!(save_path, dir_path.join("fol der/file (2).tar.gz"));

    // last occupied path
    let save_path = file_meta
        .get_last_occupied_save_path(dir_path)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(save_path, dir_path.join("fol der/file (1).tar.gz"));

    // the file exists, but has the wrong size
    let already_exists = file_meta.already_exists(dir_path).await.unwrap();
    assert!(!already_exists);

    // the path should be suffixed with "part" and the length of the file
    let save_path = file_meta.get_partial_download_path(dir_path).unwrap();
    assert_eq!(save_path, dir_path.join("fol der/file.tar.gz.part5"));

    // a partial download does exist
    let partial_exists = file_meta.partial_download_exists(dir_path).await.unwrap();
    assert_eq!(partial_exists, Some(2));
}

/// Tests methods of [`FileMeta`] with a non-empty directory.
#[tokio::test]
async fn test_file_meta_2() {
    // create test directory
    let temp_dir = tempfile::tempdir().unwrap();
    let dir_path = temp_dir.path();
    std::fs::create_dir_all(dir_path.join("fol der")).unwrap();

    let mut f = File::create_new(dir_path.join("fol der/file.tar.gz")).unwrap();
    write!(f, "---").unwrap();

    let mut f = File::create_new(dir_path.join("fol der/file (1).tar.gz")).unwrap();
    write!(f, "-----").unwrap();

    let mut f = File::create_new(dir_path.join("fol der/file.tar.gz.part7")).unwrap();
    write!(f, "--").unwrap();

    let file_meta = FileMeta {
        short_path: PathBuf::from("fol der/file.tar.gz"),
        len: 5,
    };

    // save path is the save directory joined with the short path
    let save_path = file_meta.get_save_path(dir_path);
    assert_eq!(save_path, dir_path.join("fol der/file.tar.gz"));

    // unoccupied path should increment the appended number by one
    let save_path = file_meta.get_unoccupied_save_path(dir_path).await.unwrap();
    assert_eq!(save_path, dir_path.join("fol der/file (2).tar.gz"));

    // last occupied path
    let save_path = file_meta
        .get_last_occupied_save_path(dir_path)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(save_path, dir_path.join("fol der/file (1).tar.gz"));

    // the file exists with the right size
    let already_exists = file_meta.already_exists(dir_path).await.unwrap();
    assert!(already_exists);

    // the path should be suffixed with "part" and the length of the file
    let save_path = file_meta.get_partial_download_path(dir_path).unwrap();
    assert_eq!(save_path, dir_path.join("fol der/file.tar.gz.part5"));

    // the partial download file has the wrong size suffix
    let partial_exists = file_meta.partial_download_exists(dir_path).await.unwrap();
    assert_eq!(partial_exists, None);
}

/// Tests methods of [`FileMeta`] with an empty directory.
#[tokio::test]
async fn test_file_meta_empty() {
    // create test directory that is empty
    let temp_dir = tempfile::tempdir().unwrap();
    let dir_path = temp_dir.path();

    let file_meta = FileMeta {
        short_path: PathBuf::from("fol der/file.tar.gz"),
        len: 5,
    };

    // save path is the save directory joined with the short path
    let save_path = file_meta.get_save_path(dir_path);
    assert_eq!(save_path, dir_path.join("fol der/file.tar.gz"));

    // unoccupied path should increment the appended number by one
    let save_path = file_meta.get_unoccupied_save_path(dir_path).await.unwrap();
    assert_eq!(save_path, dir_path.join("fol der/file.tar.gz"));

    // last occupied path
    let save_path = file_meta
        .get_last_occupied_save_path(dir_path)
        .await
        .unwrap();
    assert!(save_path.is_none());

    // the file doesn't exist yet
    let already_exists = file_meta.already_exists(dir_path).await.unwrap();
    assert!(!already_exists);

    // the path should be suffixed with "part" and the length of the file
    let save_path = file_meta.get_partial_download_path(dir_path).unwrap();
    assert_eq!(save_path, dir_path.join("fol der/file.tar.gz.part5"));

    // a partial download does not exist
    let partial_exists = file_meta.partial_download_exists(dir_path).await.unwrap();
    assert!(partial_exists.is_none());
}
