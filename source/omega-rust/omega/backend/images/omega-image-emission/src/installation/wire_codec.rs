//! Little-endian format-36 byte primitives.
//!
//! All higher-level installation codecs share this bounds-checked cursor and
//! these canonical scalar writers, preserving their existing error order.

use super::InstallationError;

pub(super) fn decode_boolean(value: u8) -> Result<bool, InstallationError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(InstallationError::InvalidBoolean(value)),
    }
}

pub(super) fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub(super) fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub(super) fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub(super) fn push_u128(bytes: &mut Vec<u8>, value: u128) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub(super) struct Reader<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}

impl<'bytes> Reader<'bytes> {
    pub(super) const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(super) fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    pub(super) fn take(&mut self, len: usize) -> Result<&'bytes [u8], InstallationError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(InstallationError::UnexpectedEnd)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(InstallationError::UnexpectedEnd)?;
        self.offset = end;
        Ok(bytes)
    }

    pub(super) fn array<const N: usize>(&mut self) -> Result<[u8; N], InstallationError> {
        self.take(N)?
            .try_into()
            .map_err(|_| InstallationError::UnexpectedEnd)
    }

    pub(super) fn u8(&mut self) -> Result<u8, InstallationError> {
        Ok(self.array::<1>()?[0])
    }

    pub(super) fn u16(&mut self) -> Result<u16, InstallationError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(super) fn u32(&mut self) -> Result<u32, InstallationError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(super) fn u64(&mut self) -> Result<u64, InstallationError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub(super) fn u128(&mut self) -> Result<u128, InstallationError> {
        Ok(u128::from_le_bytes(self.array()?))
    }
}
