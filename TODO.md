- Switch to a simple self-describing binary protocol such as MessagePack or CBOR.
- Figure out a simple way to use these formats with async.
- Have the hole puncher time out and return a helpful error after a given amount of time.
- Have the hole puncher prefer local sockets over public sockets.

- Allow the server to run without TLS.
- Allow the client to not use TLS.
- Improve error message everywhere in general. Have helpful tips to the user.
- Add checks in the file transfer to avoid TOCTOU bugs.