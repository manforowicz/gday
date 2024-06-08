Note: this project is still in early-development, so expect breaking changes.

# gday

A command line tool for sending files.

<pre>
<b style="color:lime;">peer_1:</b> gday send msg.txt image.jpg
<i>&lt;Asks for confirmation&gt;</i>
Tell your mate to run "gday receive <b>1.188T.W3H.E</b>".
<b>Transfer complete.</b>
</pre>

<pre>
<b style="color:lime;">peer_2:</b> gday receive <b>1.188T.W3H.E</b>
<i>&lt;Asks for confirmation&gt;</i>
<b>Transfer complete.</b>
</pre>

[![asciicast](https://asciinema.org/a/662397.svg)](https://asciinema.org/a/662397)

## Installation

### Executable

1. Go to [releases](https://github.com/manforowicz/gday/releases)
and download the correct file for your platform.
2. Extract the executable 
(on Linux, try: `tar xf <file>`).
3. Run the executable: `./<path to executable>/gday`

### Cargo

If you have `cargo`, run `cargo install gday`.

### Brew

If you have `brew`, run `brew install manforowicz/tap/gday`.

## Features
- File transfer is always direct, without relays.
A server is only used to help the devices find each other.
- Doesn't require port forwarding.
- Files encrypted with streaming
[ChaCha20Poly1305](https://en.wikipedia.org/wiki/ChaCha20-Poly1305).
- Automatically tries both IPv4 and IPv6.
- Immune to malicious servers trying to impersonate your peer.
Uses password authenticated key exchange
([SPAKE2](https://datatracker.ietf.org/doc/rfc9382/))
to derive a strong encryption key from a weak shared password.

## How it works
Uses [TCP Hole Punching](https://bford.info/pub/net/p2pnat/)
with the help of a server
to establish a direct peer-to-peer connection,
even between different private networks.
Note: This may not work on networks with very restrictive
[NATs](https://en.wikipedia.org/wiki/Network_address_translation).

## Usage
```
Usage: gday [OPTIONS] <COMMAND>

Commands:
  send     Send files
  receive  Receive files. Input the code your peer told you
  help     Print this message or the help of the given subcommand(s)

Options:
  -s, --server <SERVER>        Use a custom gday server with this domain name
  -p, --port <PORT>            Which server port to connect to
  -u, --unencrypted            Use unencrypted TCP instead of TLS to the custom server
  -v, --verbosity <VERBOSITY>  Verbosity. (trace, debug, info, warn, error) [default: warn]
  -h, --help                   Print help
  -V, --version                Print version
```

## Similar Projects

I took inspiration from these amazing projects!

<table>
    <tr>
        <th></th>
        <th>Always direct (no relay)</th>
        <th>Can work beyond LAN, through most <a href="https://en.wikipedia.org/wiki/Network_address_translation">NATs</a></th>
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

Open an [issue](https://github.com/manforowicz/gday/issues) on GitHub to add more projects.

## In this repository

- [gday](/gday/) - Command line tool for sending files.
- [gday_server](/gday_server/) - Server that lets two peers share their socket addresses.
- [gday_hole_punch](/gday_hole_punch/) - Library for establishing peer-to-peer TCP connection.
- [gday_file_transfer](/gday_file_offer_protocol/) - Library for transferring files over a direct connection.
- [gday_contact_exchange_protocol](/gday_contact_exchange_protocol/) - Library that specifies a protocol for two peers to share their socket
addresses via a server.
- [gday_encryption](/gday_encryption/) - Library for encrypting an IO stream.

![gday crate dependency graph](/images/dependency_graph.svg)