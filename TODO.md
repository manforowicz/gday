# To-Do's
Quick notes on items to consider implementing.
Not all of them are desirable or necessary.

- Improve error message everywhere in general. Have helpful tips to the user.

- Add some sort of end-to-end integration tests.

- Add error handling for if the peer's file accept response is the wrong length.

- Make sure reader returns an EOF error if interrupted?

- Think: What other functionality can I pull out into gday_file_offer_protocol.

- Add explanation in readme, of how this differs from magic wormhole.

## Abandoned ideas

- Maybe add some versioning to the protocols?
  Not needed, since the protocols are so simple, and won't change.

- Make peer authentication not "block" hole punching.
  "Blocking" might be an issue when the peer is receiving other
  incoming connections. But this probably won't happen unless
  the peer's device is acting as some sort of server.

- Allow sending a simple text string instead of only files.
  Though, I don't think this is a common use case, so will only
  add if I get requests.

- Let the client select a source port, to utilize port forwarding.
  However, turns out port forwarding works for inbound connections,
  and not outbound ones, so this wouldn't help.

- Restructure the hole puncher to force keeping connection to server open
  during hole-punching. That might please some NATs that lose state when TCP connection is closed.
  This is not really necessary, since I can just add a comment
  telling any library users to not drop ServerConnection when calling connect to peer.