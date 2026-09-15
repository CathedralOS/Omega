//! Private canonical byte cursor primitives for the terminal module codec.

use super::{CodecError, MAX_CONTENT_IDENTITY_BYTES};
use semantic_vocabulary::PsiSemanticId;

#[derive(Default)]
pub(crate) struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    pub(crate) fn finish(self) -> Vec<u8> {
        self.bytes
    }

    pub(crate) fn bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    pub(crate) fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub(crate) fn boolean(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    pub(crate) fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    pub(crate) fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    pub(crate) fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    pub(crate) fn id(&mut self, id: impl PsiSemanticId) {
        self.bytes(&id.get().to_le_bytes());
    }

    pub(crate) fn len(&mut self, label: &'static str, len: usize) -> Result<(), CodecError> {
        self.u32(u32::try_from(len).map_err(|_| CodecError::CollectionTooLong(label))?);
        Ok(())
    }

    pub(crate) fn string(&mut self, label: &'static str, value: &str) -> Result<(), CodecError> {
        if value.len() > MAX_CONTENT_IDENTITY_BYTES {
            return Err(CodecError::StringTooLong(label));
        }
        self.len(label, value.len())?;
        self.bytes(value.as_bytes());
        Ok(())
    }

    pub(crate) fn strings(
        &mut self,
        label: &'static str,
        values: &[String],
    ) -> Result<(), CodecError> {
        self.len(label, values.len())?;
        for value in values {
            self.string(label, value)?;
        }
        Ok(())
    }
}

pub(crate) struct Reader<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}

impl<'bytes> Reader<'bytes> {
    pub(crate) const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    pub(crate) fn take(&mut self, len: usize) -> Result<&'bytes [u8], CodecError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(CodecError::UnexpectedEnd)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(CodecError::UnexpectedEnd)?;
        self.offset = end;
        Ok(bytes)
    }

    pub(crate) fn array<const N: usize>(&mut self) -> Result<[u8; N], CodecError> {
        self.take(N)?
            .try_into()
            .map_err(|_| CodecError::UnexpectedEnd)
    }

    pub(crate) fn u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.array::<1>()?[0])
    }

    pub(crate) fn u16(&mut self) -> Result<u16, CodecError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, CodecError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, CodecError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub(crate) fn count(&mut self) -> Result<u32, CodecError> {
        self.u32()
    }

    pub(crate) fn boolean(&mut self) -> Result<bool, CodecError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(CodecError::InvalidBoolean(value)),
        }
    }

    pub(crate) fn string(&mut self, label: &'static str) -> Result<String, CodecError> {
        let len = usize::try_from(self.count()?).map_err(|_| CodecError::StringTooLong(label))?;
        if len > MAX_CONTENT_IDENTITY_BYTES {
            return Err(CodecError::StringTooLong(label));
        }
        let bytes = self.take(len)?;
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| CodecError::InvalidUtf8(label))
    }

    pub(crate) fn strings(&mut self, label: &'static str) -> Result<Vec<String>, CodecError> {
        let count = self.count()?;
        (0..count).map(|_| self.string(label)).collect()
    }

    pub(crate) fn id<T: PsiSemanticId>(&mut self, label: &'static str) -> Result<T, CodecError> {
        let raw = self.u64()?;
        T::new(raw).ok_or(CodecError::ZeroIdentity(label))
    }
}

pub(crate) fn encode_optional_id<I: PsiSemanticId>(writer: &mut Writer, id: Option<I>) {
    match id {
        None => writer.u8(0),
        Some(id) => {
            writer.u8(1);
            writer.id(id);
        }
    }
}

pub(crate) fn decode_counted<T>(
    reader: &mut Reader<'_>,
    mut decode: impl FnMut(&mut Reader<'_>) -> Result<T, CodecError>,
) -> Result<Vec<T>, CodecError> {
    let count = reader.count()?;
    let count = usize::try_from(count).map_err(|_| CodecError::UnexpectedEnd)?;
    if count > reader.remaining() {
        return Err(CodecError::UnexpectedEnd);
    }
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(decode(reader)?);
    }
    Ok(values)
}

pub(crate) fn decode_ids<I: PsiSemanticId>(
    reader: &mut Reader<'_>,
    label: &'static str,
) -> Result<Vec<I>, CodecError> {
    decode_counted(reader, |reader| reader.id(label))
}

pub(crate) fn decode_optional_id<I: PsiSemanticId>(
    reader: &mut Reader<'_>,
    label: &'static str,
) -> Result<Option<I>, CodecError> {
    match reader.u8()? {
        0 => Ok(None),
        1 => Ok(Some(reader.id(label)?)),
        tag => Err(CodecError::InvalidTag("OptionalSemanticId", tag)),
    }
}
