//! Little-endian format-36 byte primitives.
//!
//! All higher-level installation codecs share this bounds-checked cursor and
//! these canonical scalar writers, preserving their existing error order.

use super::TerminalInstallationError;

pub(super) fn decode_boolean(value: u8) -> Result<bool, TerminalInstallationError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(TerminalInstallationError::InvalidBoolean(value)),
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

    pub(super) fn take(&mut self, len: usize) -> Result<&'bytes [u8], TerminalInstallationError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(TerminalInstallationError::UnexpectedEnd)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(TerminalInstallationError::UnexpectedEnd)?;
        self.offset = end;
        Ok(bytes)
    }

    pub(super) fn array<const N: usize>(&mut self) -> Result<[u8; N], TerminalInstallationError> {
        self.take(N)?
            .try_into()
            .map_err(|_| TerminalInstallationError::UnexpectedEnd)
    }

    pub(super) fn u8(&mut self) -> Result<u8, TerminalInstallationError> {
        Ok(self.array::<1>()?[0])
    }

    pub(super) fn u16(&mut self) -> Result<u16, TerminalInstallationError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(super) fn u32(&mut self) -> Result<u32, TerminalInstallationError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(super) fn u64(&mut self) -> Result<u64, TerminalInstallationError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
}
