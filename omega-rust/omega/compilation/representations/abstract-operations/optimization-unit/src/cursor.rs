//! Bounds-checked byte cursor shared by the crate's record codecs.

/// Read outcome for the shared cursor. Domain codecs map it onto their own
/// sealed error enums at the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TakeError {
    Truncated,
    LengthOverflow,
}

pub(crate) struct Cursor<'encoded> {
    encoded: &'encoded [u8],
    offset: usize,
}

impl<'encoded> Cursor<'encoded> {
    pub(crate) const fn new(encoded: &'encoded [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    pub(crate) fn take(&mut self, length: usize) -> Result<&'encoded [u8], TakeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(TakeError::Truncated)?;
        let value = self
            .encoded
            .get(self.offset..end)
            .ok_or(TakeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    pub(crate) fn array<const N: usize>(&mut self) -> Result<[u8; N], TakeError> {
        self.take(N)?.try_into().map_err(|_| TakeError::Truncated)
    }

    pub(crate) fn byte(&mut self) -> Result<u8, TakeError> {
        Ok(self.array::<1>()?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16, TakeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, TakeError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, TakeError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub(crate) fn length(&mut self) -> Result<usize, TakeError> {
        usize::try_from(self.u64()?).map_err(|_| TakeError::LengthOverflow)
    }

    pub(crate) fn length_prefixed(&mut self) -> Result<&'encoded [u8], TakeError> {
        let length = self.length()?;
        self.take(length)
    }

    pub(crate) fn remaining(&self) -> usize {
        self.encoded.len() - self.offset
    }
}
