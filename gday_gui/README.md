# gday_gui
[![Crates.io Version](https://img.shields.io/crates/v/gday_gui)](https://crates.io/crates/gday_gui)

GUI for trying to send files directly between computers, without a relay.
Works through most [NATs](https://en.wikipedia.org/wiki/Network_address_translation), but not all.

For a higher success rate, consider a tool that uses a relay server, such as [magic-wormhole](https://github.com/magic-wormhole/magic-wormhole).

See the [gday](https://crates.io/crates/gday) command line tool for more features and information.

## Installation

1. Download an executable from [releases](https://github.com/manforowicz/gday/releases).
2. Extract it (on Linux: `tar xf <file>`).
3. Run it: `./gday_gui`

Alternatively:
```
cargo install gday_gui
```
or
```
brew install manforowicz/tap/gday_gui
```

## Related

- [gday](https://crates.io/crates/gday) - Command line tool for sending files.
- [gday_server](https://crates.io/crates/gday_server) - Server that lets two peers share their socket addresses.
- [gday_hole_punch](https://crates.io/crates/gday_hole_punch) - Library for establishing peer-to-peer TCP connection.
- [gday_file_transfer](https://crates.io/crates/gday_file_transfer) - Library for transferring files over a connection.
- [gday_encryption](https://crates.io/crates/gday_encryption) - Library for encrypting an IO stream.
- [gday_contact_exchange_protocol](https://crates.io/crates/gday_contact_exchange_protocol) - Library with protocol for two peers to share their socket
addresses via a server.

![gday dependency graph](https://github.com/manforowicz/gday/blob/main/other/dependency_graph.png?raw=true)