//! TODO: ADD DOC
#![forbid(unsafe_code)]
#![warn(clippy::all)]

mod helper_buf;
#[cfg(test)]
mod test;

use chacha20poly1305::aead::stream::{DecryptorBE32, EncryptorBE32};
use chacha20poly1305::aead::Buffer;
use chacha20poly1305::ChaCha20Poly1305;
use helper_buf::HelperBuf;
use std::io::{BufRead, ErrorKind, Read, Write};

const TAG_SIZE: usize = 16;

/// An encrypted wrapper around an IO stream.
/// Uses a ChaCha20Poly12305 BE32 stream.
pub struct EncryptedStream<T> {
    /// The IO stream to be wrapped in encryption
    inner: T,

    /// Stream decryptor
    decryptor: DecryptorBE32<ChaCha20Poly1305>,

    /// Encrypted data received from the inner IO stream.
    /// Invariant: Always stores only an incomplete chunk.
    /// As soon as the full chunk arrives, moves and decrypts it
    /// into `decrypted`.
    received: HelperBuf,

    /// Data that has been decrypted from `received`
    decrypted: HelperBuf,

    /// Stream encryptor
    encryptor: EncryptorBE32<ChaCha20Poly1305>,

    /// Data to be sent. `is_flushing` indicates it's encrypted
    to_send: HelperBuf,

    /// Indicates whether `to_send` has been encrypted
    is_flushing: bool,
}

impl<T> EncryptedStream<T> {
    /// Wraps `inner` in an `EncryptedStream`.
    /// Both sides must have the same `key` and `nonce`.
    /// The `key` must be a secure random secret.
    /// The `nonce` should be random, but doesn't need to be secret.
    pub fn new(inner: T, key: &[u8; 32], nonce: &[u8; 7]) -> Self {
        Self {
            inner,
            decryptor: DecryptorBE32::new(key.into(), nonce.into()),
            encryptor: EncryptorBE32::new(key.into(), nonce.into()),
            to_send: HelperBuf::with_capacity(u16::MAX as usize + 2),
            received: HelperBuf::with_capacity(u16::MAX as usize + 2),
            decrypted: HelperBuf::with_capacity(u16::MAX as usize + 2),
            is_flushing: true,
        }
    }
}

impl<T: Read> EncryptedStream<T> {
    /// Reads at least 1 new chunk into `self.plaintext`.
    /// Otherwise returns `Poll::pending`
    fn inner_read(&mut self) -> std::io::Result<()> {
        // ensure at least a 2-byte header will fit in
        // the spare `received` capacity
        if self.received.len() + self.received.spare_capacity().len() < 2 {
            self.received.left_align();
        }

        // read at least the first 2-byte header
        while self.received.len() < 2 {
            let read_buf = self.received.spare_capacity();
            let bytes_read = self.inner.read(read_buf)?;
            self.received.increase_len(bytes_read);
        }

        // determine the length of the first chunk
        let chunk_len: [u8; 2] = self.received[0..2].try_into().expect("unreachable");
        let chunk_len = u16::from_be_bytes(chunk_len) as usize + 2;

        // left-align if `chunk_len` won't fit
        if self.received.len() + self.received.spare_capacity().len() < chunk_len {
            self.received.left_align();
        }

        // read at least one full chunk
        while self.received.len() < chunk_len {
            let read_buf = self.received.spare_capacity();
            let bytes_read = self.inner.read(read_buf)?;
            self.received.increase_len(bytes_read);
        }

        self.decrypt_all_available()?;
        Ok(())
    }

    /// Decrypts all the full chunks in `self.ciphertext`, and
    /// moves them into `self.plaintext`
    fn decrypt_all_available(&mut self) -> std::io::Result<()> {
        // while there's another full encrypted chunk:
        while let Some(cipher_chunk) = peek_cipher_chunk(&self.received) {
            // exit if there isn't enough room to put the
            // decrypted plaintext
            if self.decrypted.spare_capacity().len() < cipher_chunk.len() {
                return Ok(());
            }

            // decrypt in `self.plaintext`
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
    fn flush_write_buf(&mut self) -> std::io::Result<()> {
        // no need to flush if there's no data.
        if self.to_send.len() == 2 {
            self.is_flushing = false;
            return Ok(());
        }

        // if not flushing, begin flushing
        if !self.is_flushing {
            // encrypt in place
            let mut msg = self.to_send.split_off_aead_buf(2);

            self.encryptor
                .encrypt_next_in_place(&[], &mut msg)
                .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "Encryption error"))?;

            let len = u16::try_from(msg.len()).unwrap().to_be_bytes();

            // write length to header
            self.to_send[0..2].copy_from_slice(&len);

            self.is_flushing = true;
        }

        // write until empty or `Poll::Pending`
        while !self.to_send.is_empty() {
            let bytes_written = self.inner.write(&self.to_send)?;
            self.to_send.consume(bytes_written);
        }

        self.is_flushing = false;

        // make space for new header
        self.to_send.extend_from_slice(&[0, 0]).unwrap();
        Ok(())
    }
}

/// If there is a full chunk at the beginning of `data`,
/// returns it.
fn peek_cipher_chunk(data: &[u8]) -> Option<&[u8]> {
    let len: [u8; 2] = data.get(0..2)?.try_into().expect("unreachable");
    let len = u16::from_be_bytes(len) as usize;
    data.get(2..2 + len)
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
        if self.is_flushing {
            self.flush_write_buf()?;
        }

        let bytes_taken = std::cmp::min(buf.len(), self.to_send.spare_capacity().len() - TAG_SIZE);
        self.to_send.extend_from_slice(&buf[..bytes_taken]).unwrap();

        if self.to_send.spare_capacity().len() == TAG_SIZE {
            self.flush_write_buf()?;
        }
        Ok(bytes_taken)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.flush_write_buf()?;
        self.inner.flush()
    }
}
