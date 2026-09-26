use super::super::LogicalSpillOperationDecodeError;

pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(super) fn take(
        &mut self,
        length: usize,
    ) -> Result<&'a [u8], LogicalSpillOperationDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(LogicalSpillOperationDecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(LogicalSpillOperationDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    pub(super) fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], LogicalSpillOperationDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| LogicalSpillOperationDecodeError::Truncated)
    }

    pub(super) fn byte(&mut self) -> Result<u8, LogicalSpillOperationDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    pub(super) fn length(&mut self) -> Result<usize, LogicalSpillOperationDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| LogicalSpillOperationDecodeError::LengthOverflow)
    }

    pub(super) fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
}
