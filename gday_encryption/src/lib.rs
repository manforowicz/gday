//! A simple encrypted wrapper around an IO stream.
//!
//! Uses a streaming [chacha20poly1305](https://docs.rs/chacha20poly1305/latest/chacha20poly1305/) cipher.
//!
//! This library is used by [gday_file_transfer](https://crates.io/crates/gday_file_transfer),
//! which is used by [gday](https://crates.io/crates/gday).
//!
//! In general, I recommend using the well-established
//! [rustls](https://docs.rs/rustls/latest/rustls) for encryption.
//! [gday_file_transfer](https://crates.io/crates/gday_file_transfer) chose this library
//! because [rustls](https://docs.rs/rustls/latest/rustls) didn't support
//! peer-to-peer connections with a shared key.
//!
//! # Example
//! ```no_run
//! # use gday_encryption::EncryptedStream;
//! # use std::io::{Read, Write};
//! #
//! let shared_key: [u8; 32] = [42; 32];
//!
//! //////// Peer A ////////
//! # let mut tcp_stream = std::collections::VecDeque::new();
//! let mut encrypted_stream = EncryptedStream::encrypt_connection(&mut tcp_stream, &shared_key)?;
//! encrypted_stream.write_all(b"Hello!")?;
//! encrypted_stream.flush()?;
//!
//! //////// Peer B (on a different computer) ////////
//! # let mut tcp_stream = std::collections::VecDeque::new();
//! let mut encrypted_stream = EncryptedStream::encrypt_connection(&mut tcp_stream, &shared_key)?;
//!
//! let mut received = [0u8; 6];
//! encrypted_stream.read_exact(&mut received)?;
//! # Ok::<(), std::io::Error>(())
//! ```
//!
#![forbid(unsafe_code)]
#![warn(clippy::all)]

mod helper_buf;

use chacha20poly1305::aead::stream::{DecryptorBE32, EncryptorBE32};
use chacha20poly1305::aead::Buffer;
use chacha20poly1305::ChaCha20Poly1305;
use helper_buf::HelperBuf;
use std::io::{BufRead, ErrorKind, Read, Write};

/// How many bytes larger an encrypted chunk is
/// from an unencrypted chunk.
const TAG_SIZE: usize = 16;

/// A simple encrypted wrapper around an IO stream.
/// Uses [`chacha20poly1305`] with the [`chacha20poly1305::aead::stream`].
pub struct EncryptedStream<T> {
    /// The IO stream to be wrapped in encryption
    inner: T,

    /// Stream decryptor
    decryptor: DecryptorBE32<ChaCha20Poly1305>,

    /// Stream encryptor
    encryptor: EncryptorBE32<ChaCha20Poly1305>,

    /// Encrypted data received from the inner IO stream.
    /// - Invariant: Never stores a complete chunk(s).
    /// As soon as full chunks are read, moves and decrypts them
    /// into `decrypted`.
    received: HelperBuf,

    /// Data that has been decrypted from `received`.
    /// - Invariant: This must be empty when calling
    /// [`Self::inner_read()`]
    decrypted: HelperBuf,

    /// Data to be sent. Encrypted only when flushing.
    /// - Invariant: the first 2 bytes are always
    /// reserved for the length header
    to_send: HelperBuf,
}

impl<T> EncryptedStream<T> {
    /// Wraps `io_stream` in an [`EncryptedStream`].
    ///
    /// - Both peers must have the same `key` and `nonce`.
    /// - The `key` must be a cryptographically random secret.
    /// - The `nonce` shouldn't be reused, but doesn't need to be secret.
    ///
    /// - See [`Self::encrypt_connection()`] if you'd like an auto-generated nonce.
    pub fn new(io_stream: T, key: &[u8; 32], nonce: &[u8; 7]) -> Self {
        let mut to_send = HelperBuf::with_capacity(u16::MAX as usize + 2);
        // add 2 bytes for length header to uphold invariant
        to_send.extend_from_slice(&[0, 0]).expect("unreachable");

        Self {
            inner: io_stream,
            decryptor: DecryptorBE32::new(key.into(), nonce.into()),
            encryptor: EncryptorBE32::new(key.into(), nonce.into()),
            received: HelperBuf::with_capacity(u16::MAX as usize + 2),
            decrypted: HelperBuf::with_capacity(u16::MAX as usize + 2),
            to_send,
        }
    }
}

impl<T: Read + Write> EncryptedStream<T> {
    /// Establish an [`EncryptedStream`] between two peers with an auto-generated nonce.
    ///
    /// - Both peers must have the same `key`.
    /// - The `key` must be a cryptographically random secret.
    ///
    /// This function sends random bytes to the peer, and receives some from the peer.
    /// The nonce is set to the XOR of the two byte strings.
    /// Both peers must call this function for this to work.
    ///
    /// - See [`Self::new()`] if you'd like to provide your own nonce.
    pub fn encrypt_connection(mut io_stream: T, shared_key: &[u8; 32]) -> std::io::Result<Self> {
        // Exchange random seeds with peer.
        let my_seed: [u8; 7] = rand::random();
        io_stream.write_all(&my_seed)?;
        io_stream.flush()?;
        let mut peer_seed = [0; 7];
        io_stream.read_exact(&mut peer_seed)?;

        // The nonce is the XOR of the random seeds.
        peer_seed
            .iter_mut()
            .zip(my_seed.iter())
            .for_each(|(x1, x2)| *x1 ^= *x2);

        Ok(Self::new(io_stream, shared_key, &peer_seed))
    }
}

impl<T: Read> Read for EncryptedStream<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // if we're out of decrypted data, read more
        if self.decrypted.is_empty() {
            self.inner_read()?;
        }

        let num_bytes = std::cmp::min(self.decrypted.len(), buf.len());
        buf[0..num_bytes].copy_from_slice(&self.decrypted[0..num_bytes]);
        self.decrypted.consume(num_bytes);
        Ok(num_bytes)
    }
}

impl<T: Read> BufRead for EncryptedStream<T> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        // if we're out of plaintext, read more
        if self.decrypted.is_empty() {
            self.inner_read()?;
        }

        Ok(&self.decrypted)
    }

    fn consume(&mut self, amt: usize) {
        self.decrypted.consume(amt);
    }
}

impl<T: Write> Write for EncryptedStream<T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let bytes_taken = std::cmp::min(buf.len(), self.to_send.spare_capacity().len() - TAG_SIZE);
        self.to_send
            .extend_from_slice(&buf[0..bytes_taken])
            .expect("unreachable");

        // if `to_send` is full, flush it
        if self.to_send.spare_capacity().len() - TAG_SIZE == 0 {
            self.flush_write_buf()?;
        }
        Ok(bytes_taken)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.flush_write_buf()?;
        self.inner.flush()
    }
}

impl<T: Read> EncryptedStream<T> {
    /// Reads and decrypts at least 1 new chunk into `self.decrypted`,
    /// unless reached EOF.
    /// - Invariant: must only be called when `self.decrypted` is empty,
    ///     so that it has space to decrypt into.
    fn inner_read(&mut self) -> std::io::Result<()> {
        // ensure we have the full buffer to decrypt into
        assert!(self.decrypted.is_empty());

        // maximize room to receive more data
        self.received.left_align();

        // read at least the first 2-byte header
        while self.received.len() < 2 {
            let read_buf = self.received.spare_capacity();
            let bytes_read = self.inner.read(read_buf)?;
            if bytes_read == 0 && self.received.is_empty() {
                // EOF at chunk boundary
                return Ok(());
            } else if bytes_read == 0 && !self.received.is_empty() {
                // Unexpected EOF within chunk
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "Unexpected EOF within encrypted chunk.",
                ));
            }
            self.received.increase_len(bytes_read);
        }

        // determine the length of the first chunk
        let chunk_len: [u8; 2] = self.received[0..2].try_into().expect("unreachable");
        let chunk_len = u16::from_be_bytes(chunk_len) as usize + 2;

        // read at least one full chunk
        while self.received.len() < chunk_len {
            let read_buf = self.received.spare_capacity();
            let bytes_read = self.inner.read(read_buf)?;
            if bytes_read == 0 {
                // Unexpected EOF within chunk
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "Unexpected EOF within encrypted chunk.",
                ));
            }
            self.received.increase_len(bytes_read);
        }

        /// If there is a full chunk at the beginning of `data`,
        /// returns it.
        fn peek_cipher_chunk(data: &[u8]) -> Option<&[u8]> {
            let len: [u8; 2] = data.get(0..2)?.try_into().expect("unreachable");
            let len = u16::from_be_bytes(len) as usize;
            data.get(2..2 + len)
        }

        // decrypt all chunks in `self.received`
        while let Some(cipher_chunk) = peek_cipher_chunk(&self.received) {
            // decrypt in `self.decrypted`
            let mut decryption_space = self.decrypted.split_off_aead_buf(self.decrypted.len());

            decryption_space
                .extend_from_slice(cipher_chunk)
                .expect("Unreachable");

            self.received.consume(cipher_chunk.len() + 2);

            self.decryptor
                .decrypt_next_in_place(&[], &mut decryption_space)
                .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "Decryption error"))?;
        }

        Ok(())
    }
}

impl<T: Write> EncryptedStream<T> {
    /// Encrypts and fully flushes [`Self::to_send`].
    fn flush_write_buf(&mut self) -> std::io::Result<()> {
        // encrypt in place
        let mut msg = self.to_send.split_off_aead_buf(2);
        self.encryptor
            .encrypt_next_in_place(&[], &mut msg)
            .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "Encryption error"))?;

        let len = u16::try_from(msg.len())
            .expect("unreachable: Length of message buffer should always fit in u16")
            .to_be_bytes();

        // write length to header
        self.to_send[0..2].copy_from_slice(&len);

        // write until empty
        while !self.to_send.is_empty() {
            let bytes_written = self.inner.write(&self.to_send)?;
            self.to_send.consume(bytes_written);
        }

        // make space for new header
        self.to_send
            .extend_from_slice(&[0, 0])
            .expect("unreachable: to_send must have space for the header.");
        Ok(())
    }
}
