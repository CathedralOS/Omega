//! Bounds-checked transformation-ledger byte cursor.

use super::*;

pub(in crate::ledger) struct LedgerCursor<'encoded> {
    encoded: &'encoded [u8],
    pub(in crate::ledger) offset: usize,
}

impl<'encoded> LedgerCursor<'encoded> {
    pub(in crate::ledger) const fn new(encoded: &'encoded [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    pub(in crate::ledger) fn take(
        &mut self,
        length: usize,
    ) -> Result<&'encoded [u8], PsiTransformationLedgerDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(PsiTransformationLedgerDecodeError::Truncated)?;
        let value = self
            .encoded
            .get(self.offset..end)
            .ok_or(PsiTransformationLedgerDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    pub(in crate::ledger) fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], PsiTransformationLedgerDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| PsiTransformationLedgerDecodeError::Truncated)
    }

    pub(in crate::ledger) fn byte(&mut self) -> Result<u8, PsiTransformationLedgerDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    pub(in crate::ledger) fn length(
        &mut self,
    ) -> Result<usize, PsiTransformationLedgerDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| PsiTransformationLedgerDecodeError::LengthOverflow)
    }

    pub(in crate::ledger) fn remaining(&self) -> usize {
        self.encoded.len() - self.offset
    }
}
