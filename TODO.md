# To-Do's
Items to consider implementing. Not all of them are desirable or necessary.
These are just some quick notes that might not make sense.

- Have the hole puncher actively prefer local sockets over public sockets.
But I don't think this matters much since
most NATs don't support hairpin translation, and if they do, I doubt its much slower than a direct connection.

- Deduplicate errors in the error vec the hole puncher can return.
This might give a more helpful error message to the user.

- Give the client and server the option to use plain TCP instead of TLS.
This might be difficult because various inner functions require a get address function.
Maybe I can create a trait that allows for such a function call, and implement this trait
for both raw TCP and TLS? That sounds overly complicated, but maybe it's the only option?
Or potentially just pass an address parameter everywhere??

- Restructure the hole puncher to force keeping connection to server open
during hole-punching. That might please some NATs that lose state when TCP connection is closed.

- Make peer authentication not "block" hole punching.
"Blocking" might be an issue when the peer is receiving other
incoming connections. But this probably won't happen unless
the peer's device is acting as some sort of server.

- Improve error message everywhere in general. Have helpful tips to the user.

- Add checks in the file transfer to avoid TOCTOU bugs.

- Add some sort of end-to-end integration tests.

- Allow sending a simple text string instead of only files.