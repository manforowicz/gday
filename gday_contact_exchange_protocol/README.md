Note: this crate is still in early-development, so expect breaking changes.

# `gday_contact_exchange_protocol`
[![Crates.io Version](https://img.shields.io/crates/v/gday_contact_exchange_protocol)](https://crates.io/crates/gday_contact_exchange_protocol)
[![docs.rs](https://img.shields.io/docsrs/gday_contact_exchange_protocol)](https://docs.rs/gday_contact_exchange_protocol/)

This protocol lets two users exchange their public and (optionally) private socket addresses via a server.

On it's own, this library doesn't do anything other than define a shared protocol.
In most cases, you should use one of the following crates:

- [**gday**](https://crates.io/crates/gday):
    A command line tool for sending files to peers.
- [**gday_hole_punch**](https://docs.rs/gday_hole_punch/):
    A library for establishing a peer-to-peer TCP connection.
- [**gday_server**](https://docs.rs/gday_server/):
    A server binary that facilitates this protocol.

# Example steps

1. Peer A connects to a server via TCP (port `DEFAULT_TCP_PORT`) or
    TLS (port `DEFAULT_TLS_PORT`).
    
2. Peer A requests a new room with a random `room_code` using `ClientMsg::CreateRoom`.

3. The server replies to peer A with `ServerMsg::RoomCreated` or `ServerMsg::ErrorRoomTaken`
    depending on if this `room_code` is in use.

4. Peer A externally tells peer B their `room_code` (by phone call, text message, carrier pigeon, etc.).

5. Both peers send this `room_code` and optionally their local/private
    socket addresses to the server via `ClientMsg::SendAddr` messages.
    The server determines their public addresses from their internet connections.
    The server replies with `ServerMsg::ReceivedAddr` after each of these messages.

6. Both peers send `ClientMsg::DoneSending` once they are ready to receive the contact info of each other.

7. The server immediately replies to `ClientMsg::DoneSending`
    with `ServerMsg::ClientContact` which contains the `FullContact` of this peer.

8. Once both peers are ready, the server sends (on the same stream where `ClientMsg::DoneSending` came from)
    each peer a `ServerMsg::PeerContact` which contains the `FullContact` of the other peer.

9. On their own, the peers use this info to connect directly to each other by using
    [hole punching](https://en.wikipedia.org/wiki/Hole_punching_(networking)).
    [gday_hole_punch](https://docs.rs/gday_hole_punch/) is a library that provides tools for hole punching.


This library is used by
[gday_hole_punch](/gday_hole_punch/) and [gday_server](/gday_server/).
