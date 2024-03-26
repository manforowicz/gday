- Have the hole puncher time out and return a helpful error after a given amount of time.
- Have the hole puncher prefer local sockets over public sockets.

- Give the client and server the option to use plain TCP instead of TLS.
This might be difficult because various inner functions require a get address function.
Maybe I can create a trait that allows for such a function call, and implement this trait
for both raw TCP and TLS? That sounds overly complicated, but maybe it's the only option?

- Make peer authentication not "block" hole punching.

- Potentially keep connection to server open during hole punching?


- Improve error message everywhere in general. Have helpful tips to the user.
- Add checks in the file transfer to avoid TOCTOU bugs.