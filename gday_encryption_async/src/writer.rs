use crate::helper_buf::HelperBuf;
use chacha20poly1305::aead::generic_array::typenum::Unsigned;
use chacha20poly1305::aead::stream::EncryptorBE32;
use chacha20poly1305::aead::AeadCore;
use chacha20poly1305::aead::Buffer;
use chacha20poly1305::ChaCha20Poly1305;
use pin_project_lite::pin_project;
use std::{
    io::ErrorKind,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tokio::io::AsyncWrite;

pin_project! {
    /// Encrypted writer
    pub struct WriteHalf<T: AsyncWrite> {
        #[pin]
        inner: T,
        encryptor: EncryptorBE32<ChaCha20Poly1305>,
        data: HelperBuf,
        is_flushing: bool,
    }
}

impl<T: AsyncWrite> WriteHalf<T> {
    pub fn new(inner: T, key: &[u8; 32], nonce: &[u8; 7]) -> Self {
        let encryptor = EncryptorBE32::new(key.into(), nonce.into());
        Self {
            inner,
            encryptor,
            data: HelperBuf::with_capacity(2_usize.pow(16) + 2),
            is_flushing: true,
        }
    }

    fn flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), std::io::Error>> {
        let mut me = self.project();

        // no need to flush if there's no data.
        if me.data.len() == 2 {
            *me.is_flushing = false;
            return Poll::Ready(Ok(()));
        }

        // if not flushing, begin flushing
        if !*me.is_flushing {
            // encrypt in place
            let mut msg = me.data.split_off_aead_buf(2);

            me.encryptor
                .encrypt_next_in_place(&[], &mut msg)
                .map_err(|_| std::io::Error::new(ErrorKind::InvalidData, "Encryption error"))?;

            let len = u16::try_from(msg.len()).unwrap().to_be_bytes();

            // write length to header
            me.data[0..2].copy_from_slice(&len);

            *me.is_flushing = true;
        }

        // write until empty or `Poll::Pending`
        while !me.data.is_empty() {
            let bytes_written = ready!(me.inner.as_mut().poll_write(cx, me.data))?;
            me.data.consume(bytes_written);
        }

        *me.is_flushing = false;

        // make space for new header
        me.data.extend_from_slice(&[0, 0]).unwrap();
        Poll::Ready(Ok(()))
    }
}

impl<T: AsyncWrite> AsyncWrite for WriteHalf<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        if self.is_flushing {
            ready!(self.as_mut().flush(cx))?;
        }

        let me = self.as_mut().project();

        let ciphertext_overhead = <ChaCha20Poly1305 as AeadCore>::TagSize::to_usize();

        let bytes_taken = std::cmp::min(
            buf.len(),
            me.data.spare_capacity().len() - ciphertext_overhead,
        );
        me.data.extend_from_slice(&buf[..bytes_taken]).unwrap();

        if me.data.spare_capacity().len() == ciphertext_overhead {
            let _ = self.as_mut().flush(cx)?;
        }
        Poll::Ready(Ok(bytes_taken))
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        ready!(self.as_mut().flush(cx))?;
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
