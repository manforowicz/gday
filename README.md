# gday
[![Crates.io Version](https://img.shields.io/crates/v/gday)](https://crates.io/crates/gday)

Command line tool to securely send files (without a relay or port forwarding).

<pre>
<b style="color:lime;">peer_1:</b> gday send file.mp4 folder
Tell your mate to run "gday get <b>1.n5xn8.wvqsf</b>".
</pre>

<pre>
<b style="color:lime;">peer_2:</b> gday get <b>1.n5xn8.wvqsf</b>
<b>Transfer complete.</b>
</pre>

[![asciicast](https://asciinema.org/a/Z8OJJr8xHRAJh6fuqocNcm9Zu.svg)](https://asciinema.org/a/Z8OJJr8xHRAJh6fuqocNcm9Zu)

## Installation

To run the executable directly:

1. Download an executable from [releases](https://github.com/manforowicz/gday/releases).
2. Extract it (on Linux: `tar xf <file>`).
3. Run it: `./gday`

To install with **cargo**:
```
cargo install gday
```

To install with **brew**:
```
brew install manforowicz/tap/gday
```

## Features

- No limit on the size of files and folders sent.

- Files are sent directly, without relay servers.
A server is only used to exchange socket addresses at the beginning.

- Automatically resumes interrupted transfers. Just `gday send` the same files, and partial downloads will be detected and resumed.

- Doesn't require port forwarding.
Instead, uses [TCP Hole Punching](https://bford.info/pub/net/p2pnat/) to traverse
[NATs](https://en.wikipedia.org/wiki/Network_address_translation).
This may not work on very restrictive NATs. If that happens, enable IPv6 or move to a different network.

- If a contact exchange server is down, just uses a different one from the default list. Or specify your own with `--server`.

- Server connection encrypted with
[TLS](https://en.wikipedia.org/wiki/Transport_Layer_Security)
and file transfer end-to-end encrypted with
[ChaCha20Poly1305](https://en.wikipedia.org/wiki/ChaCha20-Poly1305).

- Automatically tries both IPv4 and IPv6.

- Resistant to malicious servers impersonating your peer.
Uses [SPAKE2](https://datatracker.ietf.org/doc/rfc9382/) to derive an
encryption key from a shared secret.

- No `unsafe` Rust in this repository.


## Usage
```
Usage: gday [OPTIONS] <COMMAND>

Commands:
  send  Send files and/or directories
  get   Receive files
  help  Print this message or the help of the given subcommand(s)

Options:
  -s, --server <SERVER>        Use a custom gday server with this domain name
  -p, --port <PORT>            Connect to a custom server port
  -u, --unencrypted            Use TCP without TLS
  -v, --verbosity <VERBOSITY>  Verbosity. (trace, debug, info, warn, error) [default: warn]
  -h, --help                   Print help
  -V, --version                Print version
```

## Similar Projects

<table>
    <tr>
        <th></th>
        <th>No relays</th>
        <th>Works beyond LAN</th>
        <th>Works through very strict <a href="https://en.wikipedia.org/wiki/Network_address_translation">NATs</a></th>
        <th>No port forwarding</th>
        <th>Encrypted</th>
        <th>Can resume interrupted transfers</th>
    </tr>
    <tr>
        <td><strong><a href="https://github.com/manforowicz/gday">gday</a></strong></td>
        <td>✅</td>
        <td>✅</td>
        <td>❌</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
    </tr>
    <tr>
        <td><a href="https://github.com/magic-wormhole/magic-wormhole">magic-wormhole</a></td>
        <td>❌</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
    </tr>
    <tr>
        <td><a href="https://github.com/schollz/croc">croc</a></td>
        <td>❌</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
    </tr>
        <tr>
        <td><a href="https://github.com/psantosl/p2pcopy">p2pcopy</a></td>
        <td>✅</td>
        <td>✅</td>
        <td>❌</td>
        <td>✅</td>
        <td>❌</td>
        <td>❌</td>
    </tr>
    <tr>
        <td><a href="https://github.com/nirvik/iWant">iwant</a></td>
        <td>✅</td>
        <td>❌</td>
        <td>❌</td>
        <td>✅</td>
        <td>❌</td>
        <td>❌</td>
    </tr>
    <tr>
        <td><a href="https://github.com/nils-werner/zget">zget</a></td>
        <td>✅</td>
        <td>❌</td>
        <td>❌</td>
        <td>✅</td>
        <td>❌</td>
        <td>❌</td>
    </tr>
    <tr>
        <td><a href="https://github.com/cowbell/sharedrop">sharedrop</a></td>
        <td>❌</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>❌</td>
    </tr>
    <tr>
        <td><a href="https://github.com/kern/filepizza">filepizza</a></td>
        <td>❌</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>❌</td>
    </tr>
    <tr>
        <td>Personal <a href="https://en.wikipedia.org/wiki/Secure_Shell">SSH</a> or <a href="https://en.wikipedia.org/wiki/HTTPS">HTTPS</a></td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>❌</td>
        <td>✅</td>
        <td>❌</td>
    </tr>
    <tr>
        <td>Personal <a href="https://en.wikipedia.org/wiki/File_Transfer_Protocol">FTP</a></td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>❌</td>
        <td>❌</td>
        <td>❌</td>
    </tr>
    <tr>
        <td>Dropbox, Google Drive, etc.</td>
        <td>❌</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>❌</td>
    </tr>
    <tr>
        <td>Delivering a USB drive</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>❌</td>
    </tr>
</table>

Open an [issue](https://github.com/manforowicz/gday/issues) to add more projects to this list.

## In this repository

- [gday](/gday/) - Command line tool for sending files.
- [gday_server](/gday_server/) - Server that lets two peers share their socket addresses.
- [gday_hole_punch](/gday_hole_punch/) - Library for establishing peer-to-peer TCP connection.
- [gday_file_transfer](/gday_file_transfer/) - Library for transferring files over a connection.
- [gday_encryption](/gday_encryption/) - Library for encrypting an IO stream.
- [gday_contact_exchange_protocol](/gday_contact_exchange_protocol/) - Library with protocol for two peers to share their socket
addresses via a server.

![gday dependency graph](/other/dependency_graph.png)

See [CONTRIBUTING.md](/CONTRIBUTING.md) if you'd like to contribute to this project.
