use super::*;

pub(crate) struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    pub(crate) fn take(
        &mut self,
        count: usize,
    ) -> Result<&'a [u8], PreAllocationMachineEffectDecodeError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(PreAllocationMachineEffectDecodeError::Truncated)?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or(PreAllocationMachineEffectDecodeError::Truncated)?;
        self.position = end;
        Ok(bytes)
    }

    pub(crate) fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], PreAllocationMachineEffectDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| PreAllocationMachineEffectDecodeError::Truncated)
    }

    pub(crate) fn byte(&mut self) -> Result<u8, PreAllocationMachineEffectDecodeError> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16, PreAllocationMachineEffectDecodeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, PreAllocationMachineEffectDecodeError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, PreAllocationMachineEffectDecodeError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub(crate) fn length(&mut self) -> Result<usize, PreAllocationMachineEffectDecodeError> {
        usize::try_from(self.u64()?)
            .map_err(|_| PreAllocationMachineEffectDecodeError::InvalidField)
    }

    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }
}
