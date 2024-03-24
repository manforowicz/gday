//! TODO: ADD DOC
use chacha20poly1305::aead;
use std::ops::{Deref, DerefMut};

/// Buffer for storing bytes.
pub struct HelperBuf {
    buf: Box<[u8]>,
    l_cursor: usize,
    r_cursor: usize,
}

impl HelperBuf {
    /// Creates a new [`HelperBuf`] with `capacity`.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: vec![0; capacity].into_boxed_slice(),
            l_cursor: 0,
            r_cursor: 0,
        }
    }

    /// Removes the first `num_bytes` bytes.
    /// Panics if `num_bytes` > `self.len()`
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

    /// Use after putting data to `spare_capacity()`.
    pub fn increase_len(&mut self, size: usize) {
        self.r_cursor += size;
        assert!(self.r_cursor <= self.buf.len());
    }

    /// Returns the internal spare capacity after the stored data.
    pub fn spare_capacity(&mut self) -> &mut [u8] {
        &mut self.buf[self.r_cursor..]
    }

    /// Moves the stored data to the beginning of the internal buffer.
    /// Maximizes `spare_capacity_len()` without changing anything else.
    pub fn left_align(&mut self) {
        self.buf.copy_within(self.l_cursor..self.r_cursor, 0);
        self.r_cursor -= self.l_cursor;
        self.l_cursor = 0;
    }

    /// Returns a mutable view into the part of this
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
    fn extend_from_slice(&mut self, other: &[u8]) -> aead::Result<()> {
        let new_r_cursor = self.r_cursor + other.len();
        if new_r_cursor > self.buf.len() {
            return Err(aead::Error);
        }
        self.buf[self.r_cursor..new_r_cursor].copy_from_slice(other);
        self.r_cursor = new_r_cursor;
        Ok(())
    }

    fn truncate(&mut self, len: usize) {
        let new_r_cursor = self.l_cursor + len;
        assert!(new_r_cursor <= self.r_cursor);
        self.r_cursor = new_r_cursor;
    }
}

// The 4 following impls let the user treat this
// struct as a slice with the data-containing portion
impl Deref for HelperBuf {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.buf[self.l_cursor..self.r_cursor]
    }
}

impl DerefMut for HelperBuf {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buf[self.l_cursor..self.r_cursor]
    }
}

impl AsRef<[u8]> for HelperBuf {
    fn as_ref(&self) -> &[u8] {
        self
    }
}

impl AsMut<[u8]> for HelperBuf {
    fn as_mut(&mut self) -> &mut [u8] {
        self
    }
}

/// A mutable view into the back part of a `HelperBuf`.
pub struct HelperBufPart<'a> {
    /// The `HelperBuf` this struct references.
    parent: &'a mut HelperBuf,
    /// The index in `parent` where this view begins.
    start_i: usize,
}

impl<'a> aead::Buffer for HelperBufPart<'a> {
    fn extend_from_slice(&mut self, other: &[u8]) -> aead::Result<()> {
        let new_r_cursor = self.parent.r_cursor + other.len();
        if new_r_cursor > self.parent.buf.len() {
            return Err(aead::Error);
        }
        self.parent.buf[self.parent.r_cursor..new_r_cursor].copy_from_slice(other);
        self.parent.r_cursor = new_r_cursor;
        Ok(())
    }

    fn truncate(&mut self, len: usize) {
        let new_r_cursor = self.start_i + len;
        assert!(new_r_cursor <= self.parent.r_cursor);
        self.parent.r_cursor = new_r_cursor;
    }
}

// The 4 following impls let the user treat this
// struct as a slice with the data-containing portion
impl<'a> Deref for HelperBufPart<'a> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.parent.buf[self.start_i..self.parent.r_cursor]
    }
}

impl<'a> DerefMut for HelperBufPart<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.parent.buf[self.start_i..self.parent.r_cursor]
    }
}

impl<'a> AsRef<[u8]> for HelperBufPart<'a> {
    fn as_ref(&self) -> &[u8] {
        self
    }
}

impl<'a> AsMut<[u8]> for HelperBufPart<'a> {
    fn as_mut(&mut self) -> &mut [u8] {
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::helper_buf::HelperBuf;
    use chacha20poly1305::aead::Buffer;

    #[test]
    fn test_helper_buf() {
        let mut buf = HelperBuf::with_capacity(4);
        assert_eq!(buf.buf.len(), 4);
        assert!(buf.is_empty());
        assert_eq!(buf.spare_capacity().len(), 4);

        buf.extend_from_slice(&[1, 2, 3]).unwrap();
        assert_eq!(buf[..], [1, 2, 3][..]);
        assert_eq!(*buf.buf, [1, 2, 3, 0]);
        assert_eq!(buf.spare_capacity().len(), 1);

        buf.consume(1);
        assert_eq!(buf[..], [2, 3][..]);
        assert_eq!(*buf.buf, [1, 2, 3, 0]);
        assert_eq!(buf.spare_capacity().len(), 1);

        buf.left_align();
        assert_eq!(buf[..], [2, 3][..]);
        assert_eq!(*buf.buf, [2, 3, 3, 0]);
        assert_eq!(buf.spare_capacity().len(), 2);

        buf.consume(1);
        assert_eq!(buf[..], [3][..]);
        assert_eq!(*buf.buf, [2, 3, 3, 0]);
        assert_eq!(buf.spare_capacity().len(), 2);
    }
}
