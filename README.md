# Gday Project

**WORK IN PROGRESS - THIS PROJECT IS NOT READY YET**

Want to send files easily, securely, and directly, without a relay or port forwarding?
Then go to the [gday page](gday/README.md).

## Similar Projects

I heavily took inspiration from these projects.
I'm very grateful to their creators and contributors.
We stand on the shoulders of giants.

<table>
    <tr>
        <th></th>
        <th>Always direct (no relays)</th>
        <th>Can work beyond LAN, through most <a href="https://en.wikipedia.org/wiki/Network_address_translation">NATs</a></th>
        <th>Works through restrictive <a href="https://en.wikipedia.org/wiki/Network_address_translation">NATs</a></th>
        <th>Works without port forwarding or opening</th>
        <th>Encrypted</th>
        <th>Can resume interrupted transfers</th>
        <th>Free & open source</th>
    </tr>
    <tr>
        <td><strong><a href="https://github.com/manforowicz/gday">gday</a></strong></td>
        <td>✅</td>
        <td>✅</td>
        <td>❌</td>
        <td>✅</td>
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
        <td>✅</td>
    </tr>
    <tr>
        <td><a href="https://github.com/nirvik/iWant">iwant</a></td>
        <td>✅</td>
        <td>❌</td>
        <td>❌</td>
        <td>✅</td>
        <td>❌</td>
        <td>❌</td>
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
        <td>✅</td>
    </tr>
    <tr>
        <td><a href="https://github.com/nils-werner/zget">zget</a></td>
        <td>✅</td>
        <td>❌</td>
        <td>❌</td>
        <td>✅</td>
        <td>❌</td>
        <td>❌</td>
        <td>✅</td>
    </tr>
    <tr>
        <td><a href="https://github.com/cowbell/sharedrop">sharedrop</a></td>
        <td>❌</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>❌</td>
        <td>✅</td>
    </tr>
    <tr>
        <td><a href="https://github.com/kern/filepizza">filepizza</a></td>
        <td>❌</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>❌</td>
        <td>✅</td>
    </tr>
    <tr>
        <td><a href="https://github.com/zerotier/toss">toss</a></td>
        <td>✅</td>
        <td>❌</td>
        <td>❌</td>
        <td>✅</td>
        <td>❌</td>
        <td>❌</td>
        <td>✅</td>
    </tr>
    <tr>
        <td>Personal <a href="https://en.wikipedia.org/wiki/Secure_Shell">SSH</a> or <a href="https://en.wikipedia.org/wiki/HTTPS">HTTPS</a> server</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>❌</td>
        <td>✅</td>
        <td>❌</td>
        <td>✅</td>
    </tr>
    <tr>
        <td>Personal <a href="https://en.wikipedia.org/wiki/File_Transfer_Protocol">FTP</a> server</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>❌</td>
        <td>❌</td>
        <td>❌</td>
        <td>✅</td>
    </tr>
    <tr>
        <td>Dropbox, Google Drive, etc.</td>
        <td>❌</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>✅</td>
        <td>❌</td>
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
        <td>❓</td>
    </tr>
</table>

Open an issue on GitHub to add more projects.

## In this repository

- [gday](/gday/) - Command line tool for sending files.
- [gday_server](/gday_server/) - Server that lets two peers share their socket addresses.
- [gday_hole_punch](/gday_hole_punch/) - Tries to establish a peer-to-peer TCP connection.
- [gday_encryption](/gday_encryption/) - Encrypts an IO stream.
- [gday_contact_exchange_protocol](/gday_contact_exchange_protocol/) - Protocol for two peers to share their socket
addresses via a server.
- [gday_file_offer_protocol](/gday_file_offer_protocol/) - Protocol for peers to offer to send each other files.

![gday crate dependency graph](/images/dependency_graph.svg)

## Motivation
![xkcd about sending files](/images/file_transfer.png)