- Have the hole puncher time out and return a helpful error after a given amount of time.
- Have the hole puncher prefer local sockets over public sockets.

- Give the client and server the option to use plain TCP instead of TLS.
This might be difficult because various inner functions require a get address function.
Maybe I can create a trait that allows for such a function call, and implement this trait
for both raw TCP and TLS? That sounds overly complicated, but maybe it's the only option?
Or potentially just pass an address parameter everywhere??

- Can the hole puncher even be used standalone from contact exchange? Yeah, I suppose
if someone is trying to reconnect or something.

- Make peer authentication not "block" hole punching.

- Potentially keep connection to server open during hole punching?


- Improve error message everywhere in general. Have helpful tips to the user.
- Add checks in the file transfer to avoid TOCTOU bugs.
- Change the progress bar to use indicatif's built-in wrap write.

# Hole punching idea

Ok, here's my genius new idea:

the `get_contact(..., time_limit)` function will try and:

- If a local <-> local connection is authenticated, return early.
- Otherwise, keep trying until the `time_limit` is reached.
- If success, return that TCP connection and corresponding secret key.
- Otherwise, return a struct that gives detailed information about the attempts.

## The error struct

This struct will have a field for each attempted connection (v4 private and public, v6 private and public),
as well as a field for the v4 listener and the v6 listener.

Each of these will store an enum of:
- Didn't try connecting from here because no v4 or v6 was used.
- IO Error
- Established TCP connection, but didn't receive any messages on it.
- Established TCP connection, but received invalid messages.
- Established TCP connection, but the peer's shared secret was incorrect.