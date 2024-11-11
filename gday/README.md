# gday
[![Crates.io Version](https://img.shields.io/crates/v/gday)](https://crates.io/crates/gday)

Command line tool to securely send files (without a relay or port forwarding).

<pre>
<b style="color:lime;">peer_1:</b> gday send image.jpg folder
<i>&lt;Asks for confirmation&gt;</i>
Tell your mate to run "gday get <b>1.1C30.C71E.A</b>".
<b>Transfer complete.</b>
</pre>

<pre>
<b style="color:lime;">peer_2:</b> gday get <b>1.1C30.C71E.A</b>
<i>&lt;Asks for confirmation&gt;</i>
<b>Transfer complete.</b>
</pre>

[![asciicast](https://asciinema.org/a/1jjPVyccHweqgwA5V3un4tCnU.svg)](https://asciinema.org/a/1jjPVyccHweqgwA5V3un4tCnU)

## Installation

To run the executable directly:

1. Go to [releases](https://github.com/manforowicz/gday/releases)
and download the correct file for your platform.
2. Extract it (on Linux: `tar xf <file>`).
3. Run it: `./gday`

To install with **cargo**:
```
$ cargo install gday
```

To install with **brew**:
```
$ brew install manforowicz/tap/gday
```

## Features
- File transfer is always direct, without relay servers.
A server is only used to exchange socket addresses at the beginning.
- No limit on the size of files and folders sent.
- Doesn't require port forwarding.
Instead, uses [TCP Hole Punching](https://bford.info/pub/net/p2pnat/) to traverse
[NATs](https://en.wikipedia.org/wiki/Network_address_translation).
Note: this may not work on very restrictive NATs.
- Server connection encrypted with [TLS](https://docs.rs/rustls/)
and file transfer encrypted with [ChaCha20Poly1305](https://docs.rs/chacha20poly1305/).
- Automatically tries both IPv4 and IPv6.
- Immune to malicious servers impersonating your peer.
Uses [SPAKE2](https://docs.rs/spake2/) password authenticated key exchange
to derive an encryption key from a shared secret.
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
  -u, --unencrypted            Use raw TCP without TLS
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

## Related
- [gday](https://crates.io/crates/gday) - Command line tool for sending files.
- [gday_server](https://crates.io/crates/gday_server) - Server that lets two peers share their socket addresses.
- [gday_hole_punch](https://docs.rs/gday_hole_punch/) - Library for establishing peer-to-peer TCP connection.
- [gday_file_transfer](https://docs.rs/gday_file_transfer/) - Library for transferring files over a connection.
- [gday_encryption](https://docs.rs/gday_encryption/) - Library for encrypting an IO stream.
- [gday_contact_exchange_protocol](https://docs.rs/gday_contact_exchange_protocol/) - Library with protocol for two peers to share their socket addresses via a server.
