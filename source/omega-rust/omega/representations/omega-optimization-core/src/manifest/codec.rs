//! Shared bounded cursor for manifest wire decoders.

use super::OptimizationManifestDecodeError;

pub(super) struct ManifestCursor<'a> {
    encoded: &'a [u8],
    position: usize,
}

impl<'a> ManifestCursor<'a> {
    pub(super) const fn new(encoded: &'a [u8]) -> Self {
        Self {
            encoded,
            position: 0,
        }
    }

    pub(super) fn take(
        &mut self,
        length: usize,
    ) -> Result<&'a [u8], OptimizationManifestDecodeError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(OptimizationManifestDecodeError::Truncated)?;
        let value = self
            .encoded
            .get(self.position..end)
            .ok_or(OptimizationManifestDecodeError::Truncated)?;
        self.position = end;
        Ok(value)
    }

    pub(super) fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], OptimizationManifestDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| OptimizationManifestDecodeError::Truncated)
    }

    pub(super) fn remaining(&self) -> usize {
        self.encoded.len() - self.position
    }
}
