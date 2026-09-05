use super::super::model::OfflinePolicyReferenceError;

pub(in crate::reference_policy) struct Cursor<'encoded> {
    encoded: &'encoded [u8],
    offset: usize,
}

impl<'encoded> Cursor<'encoded> {
    pub(in crate::reference_policy) const fn new(encoded: &'encoded [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    pub(in crate::reference_policy) fn take(
        &mut self,
        length: usize,
    ) -> Result<&'encoded [u8], OfflinePolicyReferenceError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(OfflinePolicyReferenceError::Truncated)?;
        let bytes = self
            .encoded
            .get(self.offset..end)
            .ok_or(OfflinePolicyReferenceError::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }

    pub(in crate::reference_policy) fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], OfflinePolicyReferenceError> {
        self.take(N)?
            .try_into()
            .map_err(|_| OfflinePolicyReferenceError::Truncated)
    }

    pub(in crate::reference_policy) fn byte(&mut self) -> Result<u8, OfflinePolicyReferenceError> {
        Ok(self.array::<1>()?[0])
    }

    pub(in crate::reference_policy) fn remaining(&self) -> usize {
        self.encoded.len() - self.offset
    }
}
