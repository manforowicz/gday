use chacha20poly1305::aead;
use std::ops::{Deref, DerefMut};

/// Buffer for storing bytes.
/// - Implemented as a heap-allocated array
///     with a left and right cursor defining
///     the in-use portion.
pub struct HelperBuf {
    inner: Box<[u8]>,
    l_cursor: usize,
    r_cursor: usize,
}

impl HelperBuf {
    /// Creates a new [`HelperBuf`] with `capacity`.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: vec![0; capacity].into_boxed_slice(),
            l_cursor: 0,
            r_cursor: 0,
        }
    }

    /// Increments the left cursor by `num_bytes` bytes.
    ///
    /// - Effectively "removes" the first `num_bytes`.
    /// - Panics if `num_bytes` > `self.len()`.
    pub fn consume(&mut self, num_bytes: usize) {
        self.l_cursor += num_bytes;
        assert!(self.l_cursor <= self.r_cursor);

        // if there is now no data stored,
        // move cursor to beginning
        if self.l_cursor == self.r_cursor {
            self.l_cursor = 0;
            self.r_cursor = 0;
        }
    }

    /// Returns the internal spare capacity after the right cursor.
    /// - Copy data to the spare capacity, then use [`Self::increase_len()`]
    pub fn spare_capacity(&mut self) -> &mut [u8] {
        &mut self.inner[self.r_cursor..]
    }

    /// Increment the right cursor by `num_bytes`.
    /// - Do this after copying data to [`Self::spare_capacity()`].
    pub fn increase_len(&mut self, num_bytes: usize) {
        self.r_cursor += num_bytes;
        debug_assert!(self.r_cursor <= self.inner.len());
    }

    /// Shifts the stored data to the beginning of the internal buffer.
    /// Maximizes `spare_capacity_len()` without changing anything else.
    pub fn left_align(&mut self) {
        self.inner.copy_within(self.l_cursor..self.r_cursor, 0);
        self.r_cursor -= self.l_cursor;
        self.l_cursor = 0;
    }

    /// Returns a mutable [`aead::Buffer`] view into the part of this
    /// buffer starting at index `i`.
    pub fn split_off_aead_buf(&mut self, i: usize) -> HelperBufPart {
        let start_i = self.l_cursor + i;
        HelperBufPart {
            parent: self,
            start_i,
        }
    }
}

impl aead::Buffer for HelperBuf {
    /// Extends the [`HelperBuf`] with `other`.
    /// - Returns an [`aead::Error`] if there's not enough capacity.
    fn extend_from_slice(&mut self, other: &[u8]) -> aead::Result<()> {
        let new_r_cursor = self.r_cursor + other.len();
        if new_r_cursor > self.inner.len() {
            return Err(aead::Error);
        }
        self.inner[self.r_cursor..new_r_cursor].copy_from_slice(other);
        self.r_cursor = new_r_cursor;
        Ok(())
    }

    /// Shortens the length of [`HelperBuf`] to `len`
    /// by cutting off data at the end.
    fn truncate(&mut self, len: usize) {
        let new_r_cursor = self.l_cursor + len;
        debug_assert!(new_r_cursor <= self.r_cursor);
        self.r_cursor = new_r_cursor;
    }
}

// The 4 following impls let the user treat this
// struct as a slice with the data-containing portion
impl Deref for HelperBuf {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.inner[self.l_cursor..self.r_cursor]
    }
}

impl DerefMut for HelperBuf {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner[self.l_cursor..self.r_cursor]
    }
}

impl AsRef<[u8]> for HelperBuf {
    fn as_ref(&self) -> &[u8] {
        &self.inner[self.l_cursor..self.r_cursor]
    }
}

impl AsMut<[u8]> for HelperBuf {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.inner[self.l_cursor..self.r_cursor]
    }
}

/// A mutable view into the back part of a [`HelperBuf`].
pub struct HelperBufPart<'a> {
    /// The [`HelperBuf`] this struct references.
    parent: &'a mut HelperBuf,
    /// The index in [`Self::parent`] where this view begins.
    start_i: usize,
}

impl<'a> aead::Buffer for HelperBufPart<'a> {
    /// Extends the [`HelperBufPart`] with `other`.
    /// - Returns an [`aead::Error`] if there's not enough capacity.
    fn extend_from_slice(&mut self, other: &[u8]) -> aead::Result<()> {
        let new_r_cursor = self.parent.r_cursor + other.len();
        if new_r_cursor > self.parent.inner.len() {
            return Err(aead::Error);
        }
        self.parent.inner[self.parent.r_cursor..new_r_cursor].copy_from_slice(other);
        self.parent.r_cursor = new_r_cursor;
        Ok(())
    }

    /// Shortens the length of this [`HelperBufPart`] to `len`
    /// by cutting off data at the end.
    fn truncate(&mut self, len: usize) {
        let new_r_cursor = self.start_i + len;
        debug_assert!(new_r_cursor <= self.parent.r_cursor);
        self.parent.r_cursor = new_r_cursor;
    }
}

// The 4 following impls let the user treat this
// struct as a slice with the data-containing portion
impl<'a> Deref for HelperBufPart<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.parent.inner[self.start_i..self.parent.r_cursor]
    }
}

impl<'a> DerefMut for HelperBufPart<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.parent.inner[self.start_i..self.parent.r_cursor]
    }
}

impl<'a> AsRef<[u8]> for HelperBufPart<'a> {
    fn as_ref(&self) -> &[u8] {
        &self.parent.inner[self.start_i..self.parent.r_cursor]
    }
}

impl<'a> AsMut<[u8]> for HelperBufPart<'a> {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.parent.inner[self.start_i..self.parent.r_cursor]
    }
}

#[cfg(test)]
mod tests {
    use crate::helper_buf::HelperBuf;
    use chacha20poly1305::aead::{self, Buffer};

    #[test]
    fn test_helper_buf() {
        let mut buf = HelperBuf::with_capacity(4);
        assert!(buf.is_empty());
        assert!(buf[..].is_empty());
        assert_eq!(buf.spare_capacity(), [0, 0, 0, 0]);
        assert_eq!(*buf.inner, [0, 0, 0, 0]);

        buf.extend_from_slice(&[1, 2, 3]).unwrap();
        assert_eq!(*buf, [1, 2, 3]);
        assert_eq!(buf.spare_capacity(), [0]);
        assert_eq!(*buf.inner, [1, 2, 3, 0]);

        buf.consume(1);
        assert_eq!(*buf, [2, 3]);
        assert_eq!(buf.spare_capacity(), [0]);
        assert_eq!(*buf.inner, [1, 2, 3, 0]);

        buf.as_mut()[0] = 7;
        assert_eq!(*buf, [7, 3]);
        assert_eq!(buf.spare_capacity(), [0]);
        assert_eq!(*buf.inner, [1, 7, 3, 0]);

        buf.left_align();
        assert_eq!(*buf, [7, 3]);
        assert_eq!(buf.spare_capacity(), [3, 0]);
        assert_eq!(*buf.inner, [7, 3, 3, 0]);

        buf.spare_capacity()[0] = 5;
        buf.increase_len(1);
        assert_eq!(*buf, [7, 3, 5]);
        assert_eq!(buf.spare_capacity(), [0]);
        assert_eq!(*buf.inner, [7, 3, 5, 0]);

        // Trying to extend by slice longer than spare capacity
        // results in an error
        assert_eq!(buf.extend_from_slice(&[2, 2, 2, 2]), Err(aead::Error));

        buf.truncate(1);
        assert_eq!(*buf, [7]);
        assert_eq!(buf.spare_capacity(), [3, 5, 0]);
        assert_eq!(*buf.inner, [7, 3, 5, 0]);
    }

    #[test]
    fn test_helper_buf_part() {
        let mut buf = HelperBuf::with_capacity(4);

        buf.extend_from_slice(&[1, 2, 3]).unwrap();
        assert_eq!(*buf, [1, 2, 3]);
        let mut part = buf.split_off_aead_buf(1);
        assert_eq!(*part, [2, 3]);

        part[0] = 5;
        assert_eq!(*part, [5, 3]);

        part.extend_from_slice(&[6]).unwrap();
        assert_eq!(*part, [5, 3, 6]);

        assert_eq!(part.extend_from_slice(&[0]), Err(aead::Error));

        part.truncate(1);
        assert_eq!(*part, [5]);

        assert_eq!(*buf, [1, 5]);
    }
}
