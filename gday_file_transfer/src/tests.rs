#![cfg(test)]

use std::path::PathBuf;

use crate::{FileMeta, FileOfferMsg, FileResponseMsg};

/// Test serializing and deserializing messages.
#[test]
fn sending_messages() {
    let mut bytes =
        crate::encrypt_connection(std::collections::VecDeque::new(), &[42; 32]).unwrap();

    for msg in get_offer_msg_examples() {
        crate::write_to(msg, &mut bytes).unwrap();
    }

    for msg in get_offer_msg_examples() {
        let deserialized_msg: FileOfferMsg = crate::read_from(&mut bytes).unwrap();
        assert_eq!(msg, deserialized_msg);
    }

    for msg in get_response_msg_examples() {
        crate::write_to(msg, &mut bytes).unwrap();
    }

    for msg in get_response_msg_examples() {
        let deserialized_msg: FileResponseMsg = crate::read_from(&mut bytes).unwrap();
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
