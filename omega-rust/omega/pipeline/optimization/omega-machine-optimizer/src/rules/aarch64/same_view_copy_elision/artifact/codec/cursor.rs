use super::super::Aarch64SameViewCopyElisionDecodeError;

pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(super) fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    pub(super) fn take(
        &mut self,
        count: usize,
    ) -> Result<&'a [u8], Aarch64SameViewCopyElisionDecodeError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(Aarch64SameViewCopyElisionDecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(Aarch64SameViewCopyElisionDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    pub(super) fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], Aarch64SameViewCopyElisionDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| Aarch64SameViewCopyElisionDecodeError::Truncated)
    }

    pub(super) fn byte(&mut self) -> Result<u8, Aarch64SameViewCopyElisionDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    pub(super) fn u16(&mut self) -> Result<u16, Aarch64SameViewCopyElisionDecodeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(super) fn u32(&mut self) -> Result<u32, Aarch64SameViewCopyElisionDecodeError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(super) fn u64(&mut self) -> Result<u64, Aarch64SameViewCopyElisionDecodeError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub(super) fn length(&mut self) -> Result<usize, Aarch64SameViewCopyElisionDecodeError> {
        usize::try_from(self.u64()?)
            .map_err(|_| Aarch64SameViewCopyElisionDecodeError::InvalidField)
    }
}
