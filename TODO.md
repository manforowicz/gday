# To-Do's
Quick notes on items to consider implementing.
Not all of them are desirable or necessary.

- Update documentation after large async refactoring.

- Add a GUI.

- Confirm gday server works properly on both ipv4 and ipv6. Maybe add a test.

- Confirm that TLS close is now properly sent, and no errors are logged.


## Abandoned ideas

- Maybe add some versioning to the protocols?
  Not needed, since the protocols are so simple, and won't change.

- Make peer authentication not "block" hole punching.
  "Blocking" might be an issue when the peer is receiving other
  incoming connections. But this probably won't happen unless
  the peer's device is acting as some sort of server.

- Allow sending a simple text string instead of only files.
  Though, I don't think this is a common use case.

- Let the client select a source port, to utilize port forwarding.
  However, turns out port forwarding works for inbound connections,
  and not outbound ones, so this wouldn't help.

- Restructure the hole puncher to force keeping connection to server open
  during hole-punching. That might please some NATs that lose state when TCP connection is closed.
  This is not really necessary, since I can just add a comment
  telling any library users to not drop ServerConnection when calling connect to peer.

- Make file transfer response msg list file names, instead of going by index?
  I don't really see the advantage to doing this.

- Support a shared secret longer than u64 in peer code? But then again,
  users can just send their own struct in this case.