Note: this crate is still in early-development, so expect breaking changes.

# `gday_hole_punch`
[![Crates.io Version](https://img.shields.io/crates/v/gday_hole_punch)](https://crates.io/crates/gday_hole_punch)
[![docs.rs](https://img.shields.io/docsrs/gday_hole_punch)](https://docs.rs/gday_hole_punch/)

See the [documentation](https://docs.rs/gday_hole_punch/).

Lets peers behind [NAT (network address translation)](https://en.wikipedia.org/wiki/Network_address_translation)
try to establish a direct authenticated TCP connection.
Uses [TCP hole punching](https://en.wikipedia.org/wiki/TCP_hole_punching)
and a helper [gday_server](https://crates.io/crates/gday_server) to do this.

## Related
- [gday](https://crates.io/crates/gday_server) - Command line tool for sending files.
- [gday_server](https://crates.io/crates/gday_server) - Server that lets two peers share their socket addresses.
- [gday_hole_punch](https://docs.rs/gday_hole_punch/) - Library for establishing peer-to-peer TCP connection.
- [gday_file_transfer](https://docs.rs/gday_file_transfer/) - Library for transferring files over a connection.
- [gday_encryption](https://docs.rs/gday_encryption/) - Library for encrypting an IO stream.
- [gday_contact_exchange_protocol](https://docs.rs/gday_contact_exchange_protocol/) - Library with protocol for two peers to share their socket addresses via a server.
