# `gday_encryption`
[![Crates.io Version](https://img.shields.io/crates/v/gday_encryption)](https://crates.io/crates/gday_encryption)
[![docs.rs](https://img.shields.io/docsrs/gday_encryption)](https://docs.rs/gday_encryption/)

See the [documentation](https://docs.rs/gday_encryption/).

This library provides a simple encrypted wrapper around an IO stream.
Uses a streaming [chacha20poly1305](https://docs.rs/chacha20poly1305/latest/chacha20poly1305/) cipher.

## Related
- [gday](https://crates.io/crates/gday_server) - Command line tool for sending files.
- [gday_server](https://crates.io/crates/gday_server) - Server that lets two peers share their socket addresses.
- [gday_hole_punch](https://docs.rs/gday_hole_punch/) - Library for establishing peer-to-peer TCP connection.
- [gday_file_transfer](https://docs.rs/gday_file_transfer/) - Library for transferring files over a connection.
- [gday_encryption](https://docs.rs/gday_encryption/) - Library for encrypting an IO stream.
- [gday_contact_exchange_protocol](https://docs.rs/gday_contact_exchange_protocol/) - Library with protocol for two peers to share their socket addresses via a server.
