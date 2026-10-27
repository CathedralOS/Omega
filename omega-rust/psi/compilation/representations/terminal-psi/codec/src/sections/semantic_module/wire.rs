//! Private canonical byte cursor primitives for the terminal module codec.

use super::{CodecError, MAX_CONTENT_IDENTITY_BYTES};
use semantic_vocabulary::PsiSemanticId;

pub(crate) fn encode_invocation_site(
    writer: &mut Writer,
    site: semantic_vocabulary::InvocationSite,
) {
    let (tag, identity) = match site {
        semantic_vocabulary::InvocationSite::Operation(operation) => (0, operation.get()),
        semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => (1, edge.get()),
        semantic_vocabulary::InvocationSite::InvokeNoNormal(edge) => (2, edge.get()),
    };
    writer.u8(tag);
    writer.u64(identity);
}

pub(crate) fn decode_invocation_site(
    reader: &mut Reader<'_>,
) -> Result<semantic_vocabulary::InvocationSite, CodecError> {
    let tag = reader.u8()?;
    let identity = reader.u64()?;
    semantic_vocabulary::InvocationSite::from_tagged_identity(tag, identity).ok_or(
        CodecError::MalformedStructuralFoundation("invalid invocation identity"),
    )
}

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
        // Every string carries at least its own four-byte length prefix, so a
        // roster is never sized from a count the remaining input cannot hold.
        let count = bounded_count(self, 4)?;
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(self.string(label)?);
        }
        Ok(values)
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

/// Read a `u32` collection count and refuse one the remaining input cannot
/// hold before any caller reserves capacity for it: every wire element
/// occupies at least one byte, so `count > remaining` cannot parse and a
/// collection is never sized from an unvalidated count. `min_element_bytes`
/// is the family's own encoded minimum where it is larger than one byte
/// (`u32` roots, fixed 40-byte fragments), which only tightens the same
/// refusal. This is the one place the bound is decided; callers that must
/// build a sibling index table while materializing their rows read their
/// count here rather than re-deriving the arithmetic.
pub(crate) fn bounded_count(
    reader: &mut Reader<'_>,
    min_element_bytes: usize,
) -> Result<usize, CodecError> {
    debug_assert!(
        min_element_bytes > 0,
        "every encoded element occupies at least one byte"
    );
    let count = reader.count()?;
    let count = usize::try_from(count).map_err(|_| CodecError::UnexpectedEnd)?;
    if count > reader.remaining() / min_element_bytes {
        return Err(CodecError::UnexpectedEnd);
    }
    Ok(count)
}

pub(crate) fn decode_counted<T>(
    reader: &mut Reader<'_>,
    mut decode: impl FnMut(&mut Reader<'_>) -> Result<T, CodecError>,
) -> Result<Vec<T>, CodecError> {
    let count = bounded_count(reader, 1)?;
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

/// Read a length-prefixed byte payload: a `u32` length the remaining input
/// must hold, then exactly those bytes. A length and a count share the one
/// bound above; this is its materializing form for an owned payload.
pub(crate) fn decode_bytes(reader: &mut Reader<'_>) -> Result<Vec<u8>, CodecError> {
    let len = bounded_count(reader, 1)?;
    Ok(reader.take(len)?.to_vec())
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

/// Decode one standalone leaf transport through the canonical envelope:
/// consume every byte of the input, then require the decoded value to
/// re-encode to those exact bytes, so alternate spellings, unreachable rows
/// or reordered tables cannot alias one artifact. This is the one place the
/// envelope is decided; callers pass their byte-exact leaf pair.
pub(crate) fn decode_canonical<T>(
    bytes: &[u8],
    decode: impl FnOnce(&mut Reader<'_>) -> Result<T, CodecError>,
    encode: impl FnOnce(&T) -> Result<Vec<u8>, CodecError>,
) -> Result<T, CodecError> {
    let mut reader = Reader::new(bytes);
    let value = decode(&mut reader)?;
    if reader.remaining() != 0 {
        return Err(CodecError::TrailingBytes(reader.remaining()));
    }
    if encode(&value)? != bytes {
        return Err(CodecError::NonCanonicalEncoding);
    }
    Ok(value)
}
