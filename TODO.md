# To-Do's
Quick notes on items to consider implementing.
Not all of them are desirable or necessary.

- Deduplicate errors in the error vec the hole puncher can return.
  This might give a more helpful error message to the user.

- Make peer authentication not "block" hole punching.
  "Blocking" might be an issue when the peer is receiving other
  incoming connections. But this probably won't happen unless
  the peer's device is acting as some sort of server.

- Improve error message everywhere in general. Have helpful tips to the user.

- Add checks in the file transfer to avoid TOCTOU bugs.

- Add some sort of end-to-end integration tests.

- Maybe add some versioning to the protocols?

- Add error handling for if the peer's file accept response is the wrong length.

- Make sure reader returns an EOF error if interrupted?

- Have hole punch error say what connection path each error occured from.

- Think: What other functionality can I pull out into gday_file_offer_protocol.

## Low-priority ideas

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