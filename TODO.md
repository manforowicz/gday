# To-Do's
Quick notes on items to consider implementing.
Not all of them are desirable or necessary.

- Deduplicate errors in the error vec the hole puncher can return.
This might give a more helpful error message to the user.

- Give the client the option to select a server to use.

- Restructure the hole puncher to force keeping connection to server open
during hole-punching. That might please some NATs that lose state when TCP connection is closed.

- Make peer authentication not "block" hole punching.
"Blocking" might be an issue when the peer is receiving other
incoming connections. But this probably won't happen unless
the peer's device is acting as some sort of server.

- Improve error message everywhere in general. Have helpful tips to the user.

- Add checks in the file transfer to avoid TOCTOU bugs.

- Add some sort of end-to-end integration tests.

- Maybe add some versioning to the protocols?

## Low-priority ideas

- Allow sending a simple text string instead of only files.
    Though, I don't think this is a common use case, so will only
    add if I get requests.