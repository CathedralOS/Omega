//! Shared bounded byte cursor for the crate's wire decoders: a slice view
//! plus a position. Truncation is the only failure the cursor itself owns;
//! each decoder maps it into its own error enum through `From<Truncated>`.

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

    pub fn u32(&mut self) -> Result<u32, Truncated> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub fn u16(&mut self) -> Result<u16, Truncated> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub fn u64(&mut self) -> Result<u64, Truncated> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub fn u128(&mut self) -> Result<u128, Truncated> {
        Ok(u128::from_le_bytes(self.array()?))
    }

    pub fn i32(&mut self) -> Result<i32, Truncated> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    pub fn i64(&mut self) -> Result<i64, Truncated> {
        Ok(i64::from_le_bytes(self.array()?))
    }

    pub fn i128(&mut self) -> Result<i128, Truncated> {
        Ok(i128::from_le_bytes(self.array()?))
    }

    /// u64-framed length with an overflow-deciding second failure mode.
    pub fn length(&mut self) -> Result<usize, TakeError> {
        let value = self.u64().map_err(|_| TakeError::Truncated)?;
        usize::try_from(value).map_err(|_| TakeError::LengthOverflow)
    }

    /// A length-framed payload: the u64 length then that many bytes.
    pub fn length_prefixed(&mut self) -> Result<&'a [u8], TakeError> {
        let length = self.length()?;
        self.take(length).map_err(|_| TakeError::Truncated)
    }
}
