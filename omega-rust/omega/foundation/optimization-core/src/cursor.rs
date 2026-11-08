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

    /// A counted sequence's pre-flight: read the u32 count, then refuse a count
    /// whose elements cannot all fit the bytes that remain at `width` bytes
    /// apiece. Refusing before any allocation or per-element read keeps a
    /// corrupt count from requesting memory the input does not carry.
    pub fn counted(&mut self, width: usize) -> Result<usize, Truncated> {
        let count = self.u32()? as usize;
        if count > self.remaining() / width {
            return Err(Truncated);
        }
        Ok(count)
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

/// Expands the standard two-branch `From<TakeError>` decode-error adapter for
/// a crate-local error enum whose `Truncated`/`LengthOverflow` variants mirror
/// the shared cursor errors.
#[macro_export]
macro_rules! take_error_from_adapter {
    ($error:ty) => {
        impl From<$crate::cursor::TakeError> for $error {
            fn from(error: $crate::cursor::TakeError) -> Self {
                match error {
                    $crate::cursor::TakeError::Truncated => Self::Truncated,
                    $crate::cursor::TakeError::LengthOverflow => Self::LengthOverflow,
                }
            }
        }
    };
}

/// Expands the standard `From<Truncated>` decode-error adapter for a
/// crate-local error enum. The bare form maps to `Self::Truncated`; the
/// `=>` arm maps onto the caller's own variant expression.
#[macro_export]
macro_rules! truncated_from_adapter {
    ($error:ty) => {
        impl From<$crate::cursor::Truncated> for $error {
            fn from(_: $crate::cursor::Truncated) -> Self {
                Self::Truncated
            }
        }
    };
    ($error:ty => $variant:expr) => {
        impl From<$crate::cursor::Truncated> for $error {
            fn from(_: $crate::cursor::Truncated) -> Self {
                $variant
            }
        }
    };
}
