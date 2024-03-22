use crate::helper_buf::HelperBuf;
use chacha20poly1305::aead::stream::DecryptorBE32;
use chacha20poly1305::aead::Buffer;
use chacha20poly1305::ChaCha20Poly1305;
use pin_project_lite::pin_project;
use std::io::ErrorKind;
use std::pin::Pin;
use std::task::{ready, Context, Poll};
use tokio::io::{AsyncBufRead, AsyncRead, ReadBuf};

pin_project! {
    /// Encrypted reader
    pub struct ReadHalf<T>
{
        #[pin]
        inner: T,
        decryptor: DecryptorBE32<ChaCha20Poly1305>,
        plaintext: HelperBuf,
        ciphertext: HelperBuf,
    }
}

impl<T: AsyncRead> ReadHalf<T> {
    pub fn new(inner: T, key: &[u8; 32], nonce: &[u8; 7]) -> Self {
        let decryptor = DecryptorBE32::new(key.into(), nonce.into());
        Self {
            inner,
            decryptor,
            plaintext: HelperBuf::with_capacity(u16::MAX as usize + 2),
            ciphertext: HelperBuf::with_capacity(u16::MAX as usize + 2),
        }
    }

    /// Reads at least 1 new chunk into `self.plaintext`.
    /// Otherwise returns `Poll::pending`
    fn read(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let mut me = self.as_mut().project();

        // ensure at least a 2-byte header will fit in
        // the spare `ciphertext` capacity
        if me.ciphertext.spare_capacity().len() <= 2 {
            me.ciphertext.left_align();
        }

        // read at least the first 2-byte header
        while me.ciphertext.len() < 2 {
            let mut read_buf = ReadBuf::new(me.ciphertext.spare_capacity());
            ready!(me.inner.as_mut().poll_read(cx, &mut read_buf))?;
            let bytes_read = read_buf.filled().len();
            me.ciphertext.increase_len(bytes_read);
        }

        // determine the length of the first chunk
        let chunk_len: [u8; 2] = me.ciphertext[0..2].try_into().expect("unreachable");
        let chunk_len = u16::from_be_bytes(chunk_len) as usize + 2;

        // left-align if `chunk_len` won't fit
        if me.ciphertext.len() + me.ciphertext.spare_capacity().len() < chunk_len {
            me.ciphertext.left_align();
        }

        // read at least one full chunk
        while me.ciphertext.len() < chunk_len {
            let mut read_buf = ReadBuf::new(me.ciphertext.spare_capacity());
            ready!(me.inner.as_mut().poll_read(cx, &mut read_buf))?;
            let bytes_read = read_buf.filled().len();
            me.ciphertext.increase_len(bytes_read);
        }

        self.as_mut().decrypt_all_available()?;
        Poll::Ready(Ok(()))
    }

    /// Decrypts all the full chunks in `self.ciphertext`, and
    /// moves them into `self.plaintext`
    fn decrypt_all_available(self: Pin<&mut Self>) -> std::io::Result<()> {
        let this = self.project();
        // while there's another full encrypted chunk:
        while let Some(cipher_chunk) = peek_cipher_chunk(this.ciphertext) {
            // exit if there isn't enough room to put the
            // decrypted plaintext
            if this.plaintext.spare_capacity().len() < cipher_chunk.len() {
                return Ok(());
            }

            // decrypt in `self.plaintext`
            let mut decryption_space = this.plaintext.split_off_aead_buf(this.plaintext.len());

            decryption_space
                .extend_from_slice(cipher_chunk)
                .expect("Unreachable");

            this.ciphertext.consume(cipher_chunk.len() + 2);

            this.decryptor
                .decrypt_next_in_place(&[], &mut decryption_space)
                .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "Decryption error"))?;
        }

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

impl<T: AsyncRead> AsyncRead for ReadHalf<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        // if we're out of plaintext, read more
        if self.plaintext.is_empty() {
            ready!(self.as_mut().read(cx))?;
        }

        let num_bytes = std::cmp::min(self.plaintext.len(), buf.remaining());
        buf.put_slice(&self.plaintext[0..num_bytes]);
        self.project().plaintext.consume(num_bytes);
        Poll::Ready(Ok(()))
    }
}

impl<T: AsyncRead> AsyncBufRead for ReadHalf<T> {
    fn poll_fill_buf(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<&[u8]>> {
        // if we're out of plaintext, read more
        if self.plaintext.is_empty() {
            ready!(self.as_mut().read(cx))?;
        }

        Poll::Ready(Ok(self.project().plaintext))
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        self.project().plaintext.consume(amt);
    }
}
