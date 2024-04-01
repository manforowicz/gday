# `gday_encryption`
![Crates.io Version](https://img.shields.io/crates/v/gday_encryption) ![docs.rs](https://img.shields.io/docsrs/gday_encryption)

Want to send files easily, securely, and directly, without a relay or port forwarding?
Then go to the [gday page](/gday/README.md).

WARNING! This library has not been officially audited for security.

This library provides a ChaCha20Poly1305-encrypted wrapper around any IO stream.

This library is used by [`gday`](/gday/) when transferring files.
TLS wasn't used because there aren't any Rust TLS libraries with good peer-to-peer support.

