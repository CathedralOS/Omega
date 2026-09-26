use super::FunctionRelativeOptimizationRealizationManifestDecodeError;

pub(super) struct Cursor<'encoded> {
    encoded: &'encoded [u8],
    offset: usize,
}

impl<'encoded> Cursor<'encoded> {
    pub(super) const fn new(encoded: &'encoded [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    pub(super) fn take(
        &mut self,
        length: usize,
    ) -> Result<&'encoded [u8], FunctionRelativeOptimizationRealizationManifestDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(FunctionRelativeOptimizationRealizationManifestDecodeError::Truncated)?;
        let value = self
            .encoded
            .get(self.offset..end)
            .ok_or(FunctionRelativeOptimizationRealizationManifestDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    pub(super) fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], FunctionRelativeOptimizationRealizationManifestDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| FunctionRelativeOptimizationRealizationManifestDecodeError::Truncated)
    }

    pub(super) fn byte(
        &mut self,
    ) -> Result<u8, FunctionRelativeOptimizationRealizationManifestDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    pub(super) fn remaining(&self) -> usize {
        self.encoded.len() - self.offset
    }
}
