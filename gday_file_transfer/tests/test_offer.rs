use gday_file_transfer::{FileMetaLocal, FileOfferMsg, FileResponseMsg};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

#[test]
fn test_file_offer() {
    // create test directory
    let temp_dir = tempfile::tempdir().unwrap();
    let dir_path = temp_dir.path();

    let mut f = File::create_new(dir_path.join("completely_exists.tar.gz")).unwrap();
    write!(f, "--").unwrap();

    let mut f = File::create_new(dir_path.join("completely_exists (1).tar.gz")).unwrap();
    write!(f, "---").unwrap();

    let mut f = File::create_new(dir_path.join("wrong_size_exists.tar.gz")).unwrap();
    write!(f, "--").unwrap();

    let mut f = File::create_new(dir_path.join("wrong_size_exists (1).tar.gz")).unwrap();
    write!(f, "---").unwrap();

    let mut f = File::create_new(dir_path.join("just_partial.tar.gz.part9")).unwrap();
    write!(f, "----").unwrap();

    let mut f = File::create_new(dir_path.join("partial_wrong_size.tar.gz.part6")).unwrap();
    write!(f, "----").unwrap();

    let mut f = File::create_new(dir_path.join("exists_and_has_partial.tar.gz")).unwrap();
    write!(f, "----").unwrap();

    let mut f = File::create_new(dir_path.join("exists_and_has_partial.tar.gz.part4")).unwrap();
    write!(f, "-").unwrap();

    let sender_path = PathBuf::from("/random/example/");

    let offer_files = vec![
        FileMetaLocal {
            short_path: PathBuf::from("completely_exists.tar.gz"),
            local_path: sender_path.join("completely_exists.tar.gz"),
            len: 3,
        },
        FileMetaLocal {
            short_path: PathBuf::from("wrong_size_exists.tar.gz"),
            local_path: sender_path.join("wrong_size_exists.tar.gz"),
            len: 2,
        },
        FileMetaLocal {
            short_path: PathBuf::from("just_partial.tar.gz"),
            local_path: sender_path.join("just_partial.tar.gz"),
            len: 9,
        },
        FileMetaLocal {
            short_path: PathBuf::from("partial_wrong_size.tar.gz"),
            local_path: sender_path.join("partial_wrong_size.tar.gz"),
            len: 10,
        },
        FileMetaLocal {
            short_path: PathBuf::from("exists_and_has_partial.tar.gz"),
            local_path: sender_path.join("exists_and_has_partial.tar.gz"),
            len: 4,
        },
        FileMetaLocal {
            short_path: PathBuf::from("completely_unseen_file.tar.gz"),
            local_path: sender_path.join("completely_unseen_file.tar.gz"),
            len: 2,
        },
    ];

    let offer = FileOfferMsg::from(offer_files);

    let offered_size = offer.get_total_offered_size();
    assert_eq!(offered_size, 30);

    let accept_all = FileResponseMsg::accept_all_files(&offer);
    assert_eq!(
        accept_all.response,
        vec![Some(0), Some(0), Some(0), Some(0), Some(0), Some(0)]
    );
    assert_eq!(accept_all.get_num_fully_accepted(), 6);
    assert_eq!(accept_all.get_num_partially_accepted(), 0);
    assert_eq!(accept_all.get_num_not_rejected(), 6);
    assert_eq!(offer.get_transfer_size(&accept_all).unwrap(), 30);

    let reject_all = FileResponseMsg::reject_all_files(&offer);
    assert_eq!(
        reject_all.response,
        vec![None, None, None, None, None, None,]
    );
    assert_eq!(reject_all.get_num_fully_accepted(), 0);
    assert_eq!(reject_all.get_num_partially_accepted(), 0);
    assert_eq!(reject_all.get_num_not_rejected(), 0);
    assert_eq!(offer.get_transfer_size(&reject_all).unwrap(), 0);

    let only_new = FileResponseMsg::accept_only_full_new_files(&offer, dir_path).unwrap();
    assert_eq!(
        only_new.response,
        vec![None, Some(0), Some(0), Some(0), None, Some(0)]
    );
    assert_eq!(only_new.get_num_fully_accepted(), 4);
    assert_eq!(only_new.get_num_partially_accepted(), 0);
    assert_eq!(only_new.get_num_not_rejected(), 4);
    assert_eq!(offer.get_transfer_size(&only_new).unwrap(), 23);

    let only_remaining = FileResponseMsg::accept_only_remaining_portions(&offer, dir_path).unwrap();
    assert_eq!(
        only_remaining.response,
        vec![None, None, Some(4), None, Some(1), None]
    );
    assert_eq!(only_remaining.get_num_fully_accepted(), 0);
    assert_eq!(only_remaining.get_num_partially_accepted(), 2);
    assert_eq!(only_remaining.get_num_not_rejected(), 2);
    assert_eq!(offer.get_transfer_size(&only_remaining).unwrap(), 8);

    let only_new_and_interrupted =
        FileResponseMsg::accept_only_new_and_interrupted(&offer, dir_path).unwrap();
    assert_eq!(
        only_new_and_interrupted.response,
        vec![None, Some(0), Some(4), Some(0), Some(1), Some(0)]
    );
    assert_eq!(only_new_and_interrupted.get_num_fully_accepted(), 3);
    assert_eq!(only_new_and_interrupted.get_num_partially_accepted(), 2);
    assert_eq!(only_new_and_interrupted.get_num_not_rejected(), 5);
    assert_eq!(
        offer.get_transfer_size(&only_new_and_interrupted).unwrap(),
        22
    );
}
