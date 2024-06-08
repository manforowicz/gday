# `gday_encryption`
[![Crates.io Version](https://img.shields.io/crates/v/gday_encryption)](https://crates.io/crates/gday_encryption)
[![docs.rs](https://img.shields.io/docsrs/gday_encryption)](https://docs.rs/gday_encryption/)

This library provides a simple encrypted wrapper around an IO stream.
Uses a streaming [chacha20poly1305](https://docs.rs/chacha20poly1305/latest/chacha20poly1305/) cipher.

This library is used by [gday_file_transfer](https://crates.io/crates/gday_file_transfer),
which is used by [gday](https://crates.io/crates/gday).

In general, I recommend using the well-established
[rustls](https://docs.rs/rustls/latest/rustls) for encryption.

[gday_file_transfer](https://crates.io/crates/gday_file_transfer) chose this library
because [rustls](https://docs.rs/rustls/latest/rustls) didn't support
peer-to-peer connections with a shared key.
