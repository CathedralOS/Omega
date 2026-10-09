//! Shared bounded byte cursor for the crate's wire decoders: a slice view
//! plus a position. Truncation is the only failure the cursor itself owns;
//! each decoder maps it into its own error enum through `From<Truncated>`.

/// The cursor exhausted before the requested read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Truncated;

pub(crate) struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }

    pub(crate) fn take(&mut self, length: usize) -> Result<&'a [u8], Truncated> {
        let end = self.position.checked_add(length).ok_or(Truncated)?;
        let value = self.bytes.get(self.position..end).ok_or(Truncated)?;
        self.position = end;
        Ok(value)
    }

    pub(crate) fn array<const N: usize>(&mut self) -> Result<[u8; N], Truncated> {
        self.take(N)?.try_into().map_err(|_| Truncated)
    }

    pub(crate) fn byte(&mut self) -> Result<u8, Truncated> {
        Ok(self.array::<1>()?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16, Truncated> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, Truncated> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, Truncated> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}
