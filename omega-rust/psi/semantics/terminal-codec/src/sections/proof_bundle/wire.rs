//! Private canonical byte cursor primitives for proof bundles.

use super::{MAX_CONTENT_IDENTITY_BYTES, ProofCodecError};
use semantic_vocabulary::PsiSemanticId;

#[derive(Default)]
pub(super) struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    pub(super) fn finish(self) -> Vec<u8> {
        self.bytes
    }

    pub(super) fn bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    pub(super) fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub(super) fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    pub(super) fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    pub(super) fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    pub(super) fn id(&mut self, id: impl PsiSemanticId) {
        self.bytes(&id.get().to_le_bytes());
    }

    pub(super) fn len(&mut self, label: &'static str, len: usize) -> Result<(), ProofCodecError> {
        self.u32(u32::try_from(len).map_err(|_| ProofCodecError::CollectionTooLong(label))?);
        Ok(())
    }

    pub(super) fn index(
        &mut self,
        label: &'static str,
        index: usize,
    ) -> Result<(), ProofCodecError> {
        self.u32(u32::try_from(index).map_err(|_| ProofCodecError::IndexTooLarge(label))?);
        Ok(())
    }

    pub(super) fn string(
        &mut self,
        label: &'static str,
        value: &str,
    ) -> Result<(), ProofCodecError> {
        if value.len() > MAX_CONTENT_IDENTITY_BYTES {
            return Err(ProofCodecError::StringTooLong(label));
        }
        self.len(label, value.len())?;
        self.bytes(value.as_bytes());
        Ok(())
    }
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

    pub(super) fn take(&mut self, len: usize) -> Result<&'bytes [u8], ProofCodecError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(ProofCodecError::UnexpectedEnd)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProofCodecError::UnexpectedEnd)?;
        self.offset = end;
        Ok(bytes)
    }

    pub(super) fn array<const N: usize>(&mut self) -> Result<[u8; N], ProofCodecError> {
        self.take(N)?
            .try_into()
            .map_err(|_| ProofCodecError::UnexpectedEnd)
    }

    pub(super) fn u8(&mut self) -> Result<u8, ProofCodecError> {
        Ok(self.array::<1>()?[0])
    }

    pub(super) fn u16(&mut self) -> Result<u16, ProofCodecError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(super) fn u32(&mut self) -> Result<u32, ProofCodecError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(super) fn u64(&mut self) -> Result<u64, ProofCodecError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub(super) fn count(&mut self) -> Result<u32, ProofCodecError> {
        self.u32()
    }

    pub(super) fn index(&mut self) -> Result<usize, ProofCodecError> {
        usize::try_from(self.u32()?).map_err(|_| ProofCodecError::IndexOutsideHost)
    }

    pub(super) fn boolean(&mut self) -> Result<bool, ProofCodecError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(ProofCodecError::InvalidBoolean(value)),
        }
    }

    pub(super) fn string(&mut self, label: &'static str) -> Result<String, ProofCodecError> {
        let len =
            usize::try_from(self.count()?).map_err(|_| ProofCodecError::StringTooLong(label))?;
        if len > MAX_CONTENT_IDENTITY_BYTES {
            return Err(ProofCodecError::StringTooLong(label));
        }
        std::str::from_utf8(self.take(len)?)
            .map(str::to_owned)
            .map_err(|_| ProofCodecError::InvalidUtf8(label))
    }

    pub(super) fn id<T: PsiSemanticId>(
        &mut self,
        label: &'static str,
    ) -> Result<T, ProofCodecError> {
        T::new(self.u64()?).ok_or(ProofCodecError::ZeroIdentity(label))
    }
}
