use super::super::StackSlotColoringDecodeError;

pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    pub(super) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(super) fn take(&mut self, length: usize) -> Result<&'a [u8], StackSlotColoringDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(StackSlotColoringDecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(StackSlotColoringDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    pub(super) fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], StackSlotColoringDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| StackSlotColoringDecodeError::Truncated)
    }

    pub(super) fn byte(&mut self) -> Result<u8, StackSlotColoringDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    pub(super) fn length(&mut self) -> Result<usize, StackSlotColoringDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| StackSlotColoringDecodeError::LengthOverflow)
    }

    pub(super) fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
}
