use super::super::model::OfflinePolicyCorpusError;

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
    ) -> Result<&'encoded [u8], OfflinePolicyCorpusError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(OfflinePolicyCorpusError::Truncated)?;
        let bytes = self
            .encoded
            .get(self.offset..end)
            .ok_or(OfflinePolicyCorpusError::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }

    pub(super) fn array<const N: usize>(&mut self) -> Result<[u8; N], OfflinePolicyCorpusError> {
        self.take(N)?
            .try_into()
            .map_err(|_| OfflinePolicyCorpusError::Truncated)
    }

    pub(super) fn byte(&mut self) -> Result<u8, OfflinePolicyCorpusError> {
        Ok(self.array::<1>()?[0])
    }

    pub(super) fn remaining(&self) -> usize {
        self.encoded.len() - self.offset
    }
}
