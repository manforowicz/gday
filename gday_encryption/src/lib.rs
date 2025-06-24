#![forbid(unsafe_code)]
#![warn(clippy::all)]
//! Simple encrypted ChaCha20Poly1305 wrapper around an async IO stream.
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
//! ```rust
//! # use gday_encryption::EncryptedStream;
//! # use tokio::io::{AsyncReadExt, AsyncWriteExt};
//! #
//! # let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
//! # rt.block_on( async {
//! // Example pipe (like a TCP connection).
//! let (mut sender, mut receiver) = tokio::io::duplex(64);
//!
//! // Both peers must have the same key
//! let key: [u8; 32] = [123; 32];
//!
//! let handle = tokio::spawn(async move {
//!     // Peer 1 sends "Hello!"
//!     let mut stream = EncryptedStream::encrypt_connection(&mut sender, &key).await?;
//!     stream.write_all(b"Hello!").await?;
//!     stream.flush().await?;
//!
//!     Ok::<(), std::io::Error>(())
//! });
//!
//! // Peer 2 receives the "Hello!".
//! let mut stream = EncryptedStream::encrypt_connection(&mut receiver, &key).await?;
//! let mut received = [0u8; 6];
//! stream.read_exact(&mut received).await?;
//!
//! assert_eq!(b"Hello!", &received);
//!
//! handle.await??;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! # }).unwrap();
//! ```

mod helper_buf;

use chacha20poly1305::ChaCha20Poly1305;
use chacha20poly1305::aead::Buffer;
use chacha20poly1305::aead::stream::{DecryptorBE32, EncryptorBE32};
use helper_buf::HelperBuf;

use pin_project::pin_project;
use std::io::ErrorKind;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use tokio::io::{AsyncBufRead, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};

/// How many bytes larger an encrypted chunk is
/// from an unencrypted chunk.
const TAG_SIZE: usize = 16;

/// A simple encrypted wrapper around an IO stream.
/// Uses [`chacha20poly1305`] with the [`chacha20poly1305::aead::stream`].
#[pin_project]
pub struct EncryptedStream<T> {
    /// The IO stream to be wrapped in encryption
    #[pin]
    inner: T,

    /// Stream decryptor
    decryptor: DecryptorBE32<ChaCha20Poly1305>,

    /// Stream encryptor
    encryptor: EncryptorBE32<ChaCha20Poly1305>,

    /// Encrypted data received from the inner IO stream.
    /// - Invariant: Never stores a complete chunk(s).
    ///
    /// As soon as full chunk(s) are read, moves and decrypts them
    /// into `decrypted`.
    received: HelperBuf,

    /// Data that has been decrypted from `received`.
    /// - Invariant: This must be empty when calling [`Self::inner_read()`]
    decrypted: HelperBuf,

    /// Data to be sent. Encrypted only when [`Self::flushing`].
    /// - Invariant: the first 2 bytes are always reserved for the length
    /// - Invariant: Data can only be appended when `flushing` is false.
    to_send: HelperBuf,

    /// Is the content of `to_send` encrypted and ready to write?
    flushing: bool,
}

impl<T> EncryptedStream<T> {
    /// Wraps `io_stream` in an [`EncryptedStream`].
    ///
    /// - Both peers must have the same `key` and `nonce`.
    /// - The `key` must be a cryptographically random secret.
    /// - The `nonce` shouldn't be reused, but doesn't need to be secret.
    ///
    /// - See [`Self::encrypt_connection()`] if you'd like an auto-generatcan't
    ///   createed nonce.
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
            flushing: false,
        }
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> EncryptedStream<T> {
    /// Establish an [`EncryptedStream`] between two peers with an
    /// auto-generated nonce.
    ///
    /// - Both peers must have the same `key`.
    /// - The `key` must be a cryptographically random secret.
    ///
    /// This function sends random bytes to the peer, and receives some from the
    /// peer. The nonce is set to the XOR of the two byte strings.
    /// Both peers must call this function for this to work.
    ///
    /// - See [`Self::new()`] if you'd like to provide your own nonce.
    pub async fn encrypt_connection(
        mut io_stream: T,
        shared_key: &[u8; 32],
    ) -> std::io::Result<Self> {
        // Exchange random seeds with peer.
        let my_seed: [u8; 7] = rand::random();
        io_stream.write_all(&my_seed).await?;
        io_stream.flush().await?;
        let mut peer_seed = [0; 7];
        io_stream.read_exact(&mut peer_seed).await?;

        // The nonce is the XOR of the random seeds.
        peer_seed
            .iter_mut()
            .zip(my_seed.iter())
            .for_each(|(x1, x2)| *x1 ^= *x2);

        Ok(Self::new(io_stream, shared_key, &peer_seed))
    }
}

impl<T: AsyncRead> AsyncRead for EncryptedStream<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        // if we're out of decrypted data, read more
        if self.decrypted.is_empty() {
            ready!(self.as_mut().inner_read(cx))?;
        }

        let me = self.project();

        let num_bytes = std::cmp::min(me.decrypted.len(), buf.remaining());
        buf.put_slice(&me.decrypted[0..num_bytes]);
        me.decrypted.consume(num_bytes);
        Poll::Ready(Ok(()))
    }
}

impl<T: AsyncRead> AsyncBufRead for EncryptedStream<T> {
    fn consume(self: std::pin::Pin<&mut EncryptedStream<T>>, amt: usize) {
        self.project().decrypted.consume(amt);
    }

    fn poll_fill_buf(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<&[u8]>> {
        // if we're out of plaintext, read more
        if self.decrypted.is_empty() {
            ready!(self.as_mut().inner_read(cx))?;
        }

        Poll::Ready(Ok(self.project().decrypted))
    }
}

impl<T: AsyncWrite> AsyncWrite for EncryptedStream<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        // Finish up any flushes before proceeding.
        if self.flushing {
            ready!(self.as_mut().flush_write_buf(cx))?;
        }

        let me = self.as_mut().project();

        let bytes_taken = std::cmp::min(buf.len(), me.to_send.spare_capacity().len() - TAG_SIZE);
        me.to_send
            .extend_from_slice(&buf[0..bytes_taken])
            .expect("unreachable");

        // if `to_send` is full, start the process
        // of flushing it
        if me.to_send.spare_capacity().len() - TAG_SIZE == 0 {
            let _ = self.flush_write_buf(cx)?;
        }
        Poll::Ready(Ok(bytes_taken))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        ready!(self.as_mut().flush_write_buf(cx))?;
        self.project().inner.poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        ready!(self.as_mut().poll_flush(cx))?;
        self.project().inner.poll_shutdown(cx)
    }
}

impl<T: AsyncRead> EncryptedStream<T> {
    /// Reads and decrypts at least 1 new chunk into [`Self::decrypted`],
    /// unless reached EOF or the inner reader returned [`Poll::Pending`].
    /// - Invariant: must only be called when [`Self::decrypted`] is empty, so
    ///   that it has space to decrypt into.
    fn inner_read(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let mut me = self.project();

        // ensure we have the full buffer to decrypt into
        debug_assert!(me.decrypted.is_empty());

        // maximize room to receive more data
        me.received.left_align();

        /// If there is a full chunk at the beginning of `data`,
        /// returns it.
        fn peek_cipher_chunk(data: &[u8]) -> Option<&[u8]> {
            let len: [u8; 2] = data.get(0..2)?.try_into().expect("unreachable");
            let len = u16::from_be_bytes(len) as usize;
            data.get(2..2 + len)
        }

        // read at least the first 2-byte header
        while peek_cipher_chunk(me.received).is_none() {
            let mut read_buf = ReadBuf::new(me.received.spare_capacity());
            ready!(me.inner.as_mut().poll_read(cx, &mut read_buf))?;
            let bytes_read = read_buf.filled().len();
            if bytes_read == 0 {
                if me.received.is_empty() {
                    // EOF at chunk boundary
                    return Poll::Ready(Ok(()));
                } else {
                    // Unexpected EOF within chunk
                    return Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "Unexpected EOF within encrypted chunk.",
                    )));
                }
            }
            me.received.increase_len(bytes_read);
        }

        // decrypt all chunks in `self.received`
        while let Some(cipher_chunk) = peek_cipher_chunk(me.received) {
            // decrypt in `self.decrypted`
            let mut decryption_space = me.decrypted.split_off_aead_buf(me.decrypted.len());

            decryption_space
                .extend_from_slice(cipher_chunk)
                .expect("Unreachable");

            me.received.consume(cipher_chunk.len() + 2);

            me.decryptor
                .decrypt_next_in_place(&[], &mut decryption_space)
                .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "Decryption error"))?;
        }

        Poll::Ready(Ok(()))
    }
}

impl<T: AsyncWrite> EncryptedStream<T> {
    /// Encrypts and fully flushes [`Self::to_send`].
    fn flush_write_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let mut me = self.project();

        // If we're just starting a flush,
        // encrypt the data.
        if !*me.flushing {
            *me.flushing = true;
            // encrypt in place
            let mut msg = me.to_send.split_off_aead_buf(2);
            me.encryptor
                .encrypt_next_in_place(&[], &mut msg)
                .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "Encryption error"))?;

            let len = u16::try_from(msg.len())
                .expect("unreachable: Length of message buffer should always fit in u16")
                .to_be_bytes();

            // write length to header
            me.to_send[0..2].copy_from_slice(&len);
        }

        // write until empty
        while !me.to_send.is_empty() {
            let bytes_written = ready!(me.inner.as_mut().poll_write(cx, me.to_send))?;
            me.to_send.consume(bytes_written);
        }

        // if we've reached this point, flushing has finished
        *me.flushing = false;

        // make space for new header
        me.to_send
            .extend_from_slice(&[0, 0])
            .expect("unreachable: to_send must have space for the header.");
        Poll::Ready(Ok(()))
    }
}
