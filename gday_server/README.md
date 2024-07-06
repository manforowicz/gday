Note: this crate is still in early-development, so expect breaking changes.

# gday_server
[![Crates.io Version](https://img.shields.io/crates/v/gday_server)](https://crates.io/crates/gday_server)

A server that runs the [gday_contact_exchange_protocol](https://docs.rs/gday_contact_exchange_protocol/).

## Installation

### Executable

1. Go to [releases](https://github.com/manforowicz/gday/releases)
and download the correct file for your platform.
2. Extract it 
(on Linux, try: `tar xf <file>`).
3. Run it: `./<path to executable>/gday_server`

### Cargo

`cargo install gday_server`

### Brew

`brew install manforowicz/tap/gday_server`

## Usage
```
Usage: gday_server [OPTIONS]

Options:
  -k, --key <KEY>                      PEM file of private TLS server key
  -c, --certificate <CERTIFICATE>      PEM file of signed TLS server certificate
  -u, --unencrypted                    Use unencrypted TCP instead of TLS
  -a, --address <ADDRESS>              Custom socket address on which to listen. [default: `[::]:443` for TLS, `[::]:80` when --unencrypted]
  -t, --timeout <TIMEOUT>              Number of seconds before a new room is deleted [default: 600]
  -r, --request-limit <REQUEST_LIMIT>  Max number of requests an IP address can send in a minute before they're rejected [default: 60]
  -v, --verbosity <VERBOSITY>          Log verbosity. (trace, debug, info, warn, error) [default: info]
  -h, --help                           Print help
  -V, --version                        Print version
```

## Deployment

One of the strengths of gday is its decentralized nature.
Want to add your own server to the list of
[default servers](https://docs.rs/gday_hole_punch/latest/gday_hole_punch/server_connector/constant.DEFAULT_SERVERS.html)?
Here's how:

1. Get a [virtual private server](https://en.wikipedia.org/wiki/Virtual_private_server) (VPS) from a hosting service. It must have public IPv4 and IPv6 addresses and not be behind [NAT](https://en.wikipedia.org/wiki/Network_address_translation).
2. Buy/configure a domain name to point at your VPS.
3. On the VPS, get a TLS certificate using [certbot](https://certbot.eff.org/) with your domain name.
4. On the VPS, use a tool such as `wget` to download gday_server from the [releases page](https://github.com/manforowicz/gday/releases).
5. On the VPS, run the `gday_server` with the correct TLS arguments.
6. On a local device, verify you can use `gday` with your server domain name passed as an argument.
7. On the VPS, follow instructions in [gday_server.service](https://github.com/manforowicz/gday/blob/main/other/gday_server.service) to set up a [systemd service](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html).
8. Verify `gday_server` auto-starts in the background, even when you reboot the server.
9. Submit an [issue](https://github.com/manforowicz/gday/issues), asking for your server to be added to the [default server list](https://docs.rs/gday_hole_punch/latest/gday_hole_punch/server_connector/constant.DEFAULT_SERVERS.html).

## Related
- [gday](https://crates.io/crates/gday_server) - Command line tool for sending files.
- [gday_server](https://crates.io/crates/gday_server) - Server that lets two peers share their socket addresses.
- [gday_hole_punch](https://docs.rs/gday_hole_punch/) - Library for establishing peer-to-peer TCP connection.
- [gday_file_transfer](https://docs.rs/gday_file_transfer/) - Library for transferring files over a connection.
- [gday_encryption](https://docs.rs/gday_encryption/) - Library for encrypting an IO stream.
- [gday_contact_exchange_protocol](https://docs.rs/gday_contact_exchange_protocol/) - Library with protocol for two peers to share their socket addresses via a server.
