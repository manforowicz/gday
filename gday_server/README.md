Note: this crate is still in early-development, so expect breaking changes.

# `gday_server`
[![Crates.io Version](https://img.shields.io/crates/v/gday_server)](https://crates.io/crates/gday_server)

A server that runs the [gday_contact_exchange_protocol](/gday_contact_exchange_protocol/).

Submit a GitHub issue if you would like your gday_server to be added to the
default server list in [gday_hole_punch](https://docs.rs/gday_hole_punch/).

## Installation

### Executable

1. Go to [releases](https://github.com/manforowicz/gday/releases)
and download the correct file for your platform.
2. Extract the executable 
(on Linux, try: `tar xf <file>`).
3. Run the executable: `./<path to executable>/gday_server`

### Cargo

`cargo install gday_server`.

### Brew

`brew install manforowicz/tap/gday_server`.


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