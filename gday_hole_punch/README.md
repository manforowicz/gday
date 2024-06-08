Note: this crate is still in early-development, so expect breaking changes.

# `gday_hole_punch`
[![Crates.io Version](https://img.shields.io/crates/v/gday_hole_punch)](https://crates.io/crates/gday_hole_punch)
[![docs.rs](https://img.shields.io/docsrs/gday_hole_punch)](https://docs.rs/gday_hole_punch/)

Lets peers behind [NAT (network address translation)](https://en.wikipedia.org/wiki/Network_address_translation)
try to establish a direct authenticated TCP connection.

Uses [TCP hole punching](https://en.wikipedia.org/wiki/TCP_hole_punching)
and a helper [gday_server](https://crates.io/crates/gday_server) to do this.

This library is used by [gday](https://crates.io/crates/gday), a command line tool for sending files.

# Example steps

1. Peer A connects to a [gday_server](https://crates.io/crates/gday_server) using
a function such as [`server_connector::connect_to_random_server()`].

2. Peer A creates a room in the server using [`ContactSharer::create_room()`] with a random room code.

3. Peer A tells Peer B which server and room code to join, possibly by giving them a [`PeerCode`]
    (done via phone call, email, etc.).

4. Peer B connects to the same server using [`server_connector::connect_to_server_id()`].

5. Peer B joins the same room using [`ContactSharer::join_room()`].

6. Both peers call [`ContactSharer::get_peer_contact()`] to get their peer's contact.

7. Both peers pass this contact and a shared secret to [`try_connect_to_peer()`],
   which returns a TCP stream, and an authenticated cryptographically-secure shared key.
