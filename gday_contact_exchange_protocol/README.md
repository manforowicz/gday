# gday_contact_exchange_protocol

This protocol lets two users exchange their public and (optionally) private socket addresses via a server.
On it's own, this crate doesn't do anything other than define a shared protocol, and functions to
send and receive messages of this protocol.

## Process

Using this protocol goes something like this:

1. Peer A connects to a server via the internet
    and requests a new room with `room_code` using [`ClientMsg::CreateRoom`].

2. The server replies to peer A with [`ServerMsg::RoomCreated`] or [`ServerMsg::ErrorRoomTaken`]
    depending on if this `room_code` is in use.

3. Peer A externally tells peer B their `room_code` (by phone call, text message, carrier pigeon, etc.).

4. Both peers send this `room_code` and optionally their local/private socket addresses to the server
    via [`ClientMsg::SendAddr`] messages. The server determines their public addresses from the internet connections.
    The server replies with [`ServerMsg::ReceivedAddr`] after each of these messages.

5. Both peers send [`ClientMsg::DoneSending`] once they are ready to receive the contact info of each other.

6. The server immediately replies to [`ClientMsg::DoneSending`]
    with [`ServerMsg::ClientContact`] which contains the [`FullContact`] of this peer.

7. Once both peers are ready, the server sends (on the same stream where [`ClientMsg::DoneSending`] came from)
    each peer [`ServerMsg::PeerContact`] which contains the [`FullContact`] of the other peer..

8. On their own, the peers use this info to connect directly to each other by using
    [hole punching](https://en.wikipedia.org/wiki/Hole_punching_(networking)).
