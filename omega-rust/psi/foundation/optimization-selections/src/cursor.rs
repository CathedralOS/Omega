//! Shared bounded-cursor error vocabulary for the workspace's wire decoders:
//! the two unit error shapes every crate-local cursor returns. Each decoder
//! maps these onto its own domain error through `From`.

/// The cursor exhausted before the requested read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Truncated;

/// The read failed either by exhaustion or by an oversized length; each
/// decoder maps it onto its own enum's Truncated/LengthOverflow variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TakeError {
    Truncated,
    LengthOverflow,
}

/// The shared bounded byte cursor for wire decoders: a slice view plus a
/// position. Truncation is the only failure the cursor itself owns; each
/// decoder maps it into its own error enum through `From`.
pub struct Cursor<'a> {
    encoded: &'a [u8],
    pub offset: usize,
}

impl<'a> Cursor<'a> {
    pub const fn new(encoded: &'a [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    pub fn remaining(&self) -> usize {
        self.encoded.len().saturating_sub(self.offset)
    }

    pub fn take(&mut self, count: usize) -> Result<&'a [u8], Truncated> {
        let end = self.offset.checked_add(count).ok_or(Truncated)?;
        let value = self.encoded.get(self.offset..end).ok_or(Truncated)?;
        self.offset = end;
        Ok(value)
    }

    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], Truncated> {
        self.take(N)?.try_into().map_err(|_| Truncated)
    }

    pub fn byte(&mut self) -> Result<u8, Truncated> {
        Ok(self.array::<1>()?[0])
    }

    pub fn u16(&mut self) -> Result<u16, Truncated> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub fn u64(&mut self) -> Result<u64, Truncated> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}
