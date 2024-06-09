Note: this crate is still in early-development, so expect breaking changes.

# `gday_file_transfer`
[![Crates.io Version](https://img.shields.io/crates/v/gday_file_transfer)](https://crates.io/crates/gday_file_transfer)
[![docs.rs](https://img.shields.io/docsrs/gday_file_transfer)](https://docs.rs/gday_file_transfer/)

This library lets you offer and transfer files to another peer,
assuming you already have a TCP connection established.

This library is used by [gday](https://crates.io/crates/gday), a command line
tool for sending files.

# Example steps

1. The peers encrypt their connection,
using a crate such as [gday_encryption](https://docs.rs/gday_encryption/).

2. Peer A calls `get_file_metas()` to get a `Vec` of `FileMetaLocal`
containing metadata about the files they'd like to send.

3. Peer A calls `FileOfferMsg::from()` on the `Vec<FileMetaLocal>`, to get
a serializable `FileOfferMsg`.

4. Peer A sends `FileOfferMsg` to Peer B using `write_to()`.

5. Peer B sends `FileResponseMsg` to Peer A, containing a corresponding
`Vec` of `Option<u64>` indicating how much of each offered file to send.
Each `None` rejects the offered file at the corresponding index.
Each `Some(0)` accepts the entire file at the corresponding index.
Each `Some(k)` requests only the part of the file starting at the `k`th byte
to be sent.

6. Peer A calls `send_files()`.

7. Peer B calls `receive_files()`.
