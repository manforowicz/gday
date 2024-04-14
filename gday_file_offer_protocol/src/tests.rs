#![cfg(test)]

use std::path::PathBuf;

use crate::{FileMeta, FileOfferMsg, FileResponseMsg};

/// Test serializing and deserializing messages.
#[test]
fn sending_messages() {
    let mut bytes = std::collections::VecDeque::new();

    for msg in get_offer_msg_examples() {
        crate::to_writer(msg, &mut bytes).unwrap();
    }

    for msg in get_offer_msg_examples() {
        let deserialized_msg: FileOfferMsg = crate::from_reader(&mut bytes).unwrap();
        assert_eq!(msg, deserialized_msg);
    }

    for msg in get_response_msg_examples() {
        crate::to_writer(msg, &mut bytes).unwrap();
    }

    for msg in get_response_msg_examples() {
        let deserialized_msg: FileResponseMsg = crate::from_reader(&mut bytes).unwrap();
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
            accepted: vec![None, Some(0), Some(100)],
        },
        FileResponseMsg {
            accepted: vec![None, None, None],
        },
    ]
}
