//! Shared bounded byte cursor for the crate's wire decoders: a slice view
//! plus a position. Truncation is the only failure the cursor itself owns;
//! each decoder maps it into its own error enum through `From<Truncated>`
//! and the u64-sized length read through [`TakeError`].

/// The cursor exhausted before the requested read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Truncated;

/// The read failed either by exhaustion or by an oversized length; each
/// decoder maps it onto its own enum's Truncated/LengthOverflow variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TakeError {
    Truncated,
    LengthOverflow,
}

pub(crate) struct Cursor<'a> {
    encoded: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) const fn new(encoded: &'a [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.encoded.len().saturating_sub(self.offset)
    }

    pub(crate) fn take(&mut self, count: usize) -> Result<&'a [u8], Truncated> {
        let end = self.offset.checked_add(count).ok_or(Truncated)?;
        let value = self.encoded.get(self.offset..end).ok_or(Truncated)?;
        self.offset = end;
        Ok(value)
    }

    pub(crate) fn array<const N: usize>(&mut self) -> Result<[u8; N], Truncated> {
        self.take(N)?.try_into().map_err(|_| Truncated)
    }

    pub(crate) fn byte(&mut self) -> Result<u8, Truncated> {
        Ok(self.array::<1>()?[0])
    }

    pub(crate) fn length(&mut self) -> Result<usize, TakeError> {
        let value = u64::from_le_bytes(self.array().map_err(|_| TakeError::Truncated)?);
        usize::try_from(value).map_err(|_| TakeError::LengthOverflow)
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

    pub(crate) fn u128(&mut self) -> Result<u128, Truncated> {
        Ok(u128::from_le_bytes(self.array()?))
    }
}
