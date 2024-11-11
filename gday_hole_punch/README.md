# gday_hole_punch
[![Crates.io Version](https://img.shields.io/crates/v/gday_hole_punch)](https://crates.io/crates/gday_hole_punch)
[![docs.rs](https://img.shields.io/docsrs/gday_hole_punch)](https://docs.rs/gday_hole_punch/)

Lets peers behind [NAT (network address translation)](https://en.wikipedia.org/wiki/Network_address_translation)
try to establish a direct authenticated TCP connection.
Uses [TCP hole punching](https://en.wikipedia.org/wiki/TCP_hole_punching)
and a helper [gday_server](https://crates.io/crates/gday_server) to do this.

See the [documentation](https://docs.rs/gday_hole_punch/).

## Used by
- [gday](https://crates.io/crates/gday) - Command line tool for sending files.

## Depends on
- [gday_contact_exchange_protocol](https://docs.rs/gday_contact_exchange_protocol/) - Library with protocol for two peers to share their socket addresses via a server.
