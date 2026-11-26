use crate::CodecError;
use arena::NumericId;

const MAX_CONTENT_IDENTITY_BYTES: usize = 1 << 20;

#[derive(Clone, Copy, Default)]
enum EncodingRole {
    #[default]
    ClosedPublication,
    InvocationPrefix,
    CheckingIdentity,
}

#[derive(Default)]
pub struct Writer {
    bytes: Vec<u8>,
    role: EncodingRole,
    closed_types: std::rc::Rc<std::cell::RefCell<crate::EncodedIntegerTypes>>,
}

impl Writer {
    pub fn invocation_prefix() -> Self {
        Self {
            role: EncodingRole::InvocationPrefix,
            ..Self::default()
        }
    }

    pub fn is_invocation_prefix(&self) -> bool {
        matches!(self.role, EncodingRole::InvocationPrefix)
    }

    /// Checking keys identify rigid widths; they do not publish executable code.
    pub fn checking_identity() -> Self {
        Self {
            role: EncodingRole::CheckingIdentity,
            ..Self::default()
        }
    }

    pub(crate) fn is_checking_identity(&self) -> bool {
        matches!(self.role, EncodingRole::CheckingIdentity)
    }

    /// Local canonical keys keep their real bytes and share the closure
    /// traversal with the enclosing module or record.
    pub fn nested(&self) -> Self {
        Self {
            bytes: Vec::new(),
            role: self.role,
            closed_types: std::rc::Rc::clone(&self.closed_types),
        }
    }

    pub fn require_integer_width(
        &mut self,
        integer: semantic_vocabulary::IntegerType,
    ) -> Result<(), CodecError> {
        if self.is_checking_identity()
            && matches!(
                integer.width(),
                semantic_vocabulary::IntegerWidth::Parameter(_)
            )
        {
            return Ok(());
        }
        self.closed_types.borrow_mut().record(integer).map(|_| ())
    }

    /// Returns the bytes and the fixed integer types recorded while writing them.
    pub fn finish_with_integer_types(self) -> (Vec<u8>, crate::EncodedIntegerTypes) {
        let types = std::rc::Rc::try_unwrap(self.closed_types)
            .map(std::cell::RefCell::into_inner)
            .unwrap_or_else(|types| types.borrow().clone());
        (self.bytes, types)
    }

    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }

    pub fn bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    pub fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub fn boolean(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    pub fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    pub fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    pub fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    pub fn id(&mut self, id: impl NumericId) {
        self.bytes(&id.get().to_le_bytes());
    }

    pub fn len(&mut self, label: &'static str, len: usize) -> Result<(), CodecError> {
        self.u32(u32::try_from(len).map_err(|_| CodecError::CollectionTooLong(label))?);
        Ok(())
    }

    pub fn string(&mut self, label: &'static str, value: &str) -> Result<(), CodecError> {
        if value.len() > MAX_CONTENT_IDENTITY_BYTES {
            return Err(CodecError::StringTooLong(label));
        }
        self.len(label, value.len())?;
        self.bytes(value.as_bytes());
        Ok(())
    }

    pub fn strings(&mut self, label: &'static str, values: &[String]) -> Result<(), CodecError> {
        self.len(label, values.len())?;
        for value in values {
            self.string(label, value)?;
        }
        Ok(())
    }

    pub fn ids<I: NumericId>(
        &mut self,
        label: &'static str,
        values: &[I],
    ) -> Result<(), CodecError> {
        self.len(label, values.len())?;
        for value in values {
            self.id(*value);
        }
        Ok(())
    }

    pub fn counted<T>(
        &mut self,
        label: &'static str,
        values: &[T],
        encode: fn(&mut Writer, &T) -> Result<(), CodecError>,
    ) -> Result<(), CodecError> {
        self.len(label, values.len())?;
        for value in values {
            encode(self, value)?;
        }
        Ok(())
    }
}

pub struct Reader<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
    closed_types: crate::EncodedIntegerTypes,
}

impl<'bytes> Reader<'bytes> {
    pub fn new(bytes: &'bytes [u8]) -> Self {
        Self {
            bytes,
            offset: 0,
            closed_types: crate::EncodedIntegerTypes::default(),
        }
    }

    pub fn record_integer_type(
        &mut self,
        integer: semantic_vocabulary::IntegerType,
    ) -> Result<(), CodecError> {
        self.closed_types.record(integer).map(|_| ())
    }

    pub fn finish_integer_types(self) -> crate::EncodedIntegerTypes {
        self.closed_types
    }

    pub fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    pub fn take(&mut self, len: usize) -> Result<&'bytes [u8], CodecError> {
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

    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], CodecError> {
        self.take(N)?
            .try_into()
            .map_err(|_| CodecError::UnexpectedEnd)
    }

    pub fn u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.array::<1>()?[0])
    }

    pub fn u16(&mut self) -> Result<u16, CodecError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub fn u32(&mut self) -> Result<u32, CodecError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub fn u64(&mut self) -> Result<u64, CodecError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub fn count(&mut self) -> Result<u32, CodecError> {
        self.u32()
    }

    pub fn boolean(&mut self) -> Result<bool, CodecError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(CodecError::InvalidBoolean(value)),
        }
    }

    pub fn string(&mut self, label: &'static str) -> Result<String, CodecError> {
        let len = usize::try_from(self.count()?).map_err(|_| CodecError::StringTooLong(label))?;
        if len > MAX_CONTENT_IDENTITY_BYTES {
            return Err(CodecError::StringTooLong(label));
        }
        let bytes = self.take(len)?;
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| CodecError::InvalidUtf8(label))
    }

    pub fn strings(&mut self, label: &'static str) -> Result<Vec<String>, CodecError> {
        let count = bounded_count(self, 4)?;
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(self.string(label)?);
        }
        Ok(values)
    }

    pub fn id<T: NumericId>(&mut self, label: &'static str) -> Result<T, CodecError> {
        let raw = self.u64()?;
        T::new(raw).ok_or(CodecError::ZeroIdentity(label))
    }
}

pub fn encode_optional_id<I: NumericId>(writer: &mut Writer, id: Option<I>) {
    match id {
        None => writer.u8(0),
        Some(id) => {
            writer.u8(1);
            writer.id(id);
        }
    }
}

pub fn bounded_count(
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

pub fn decode_counted<T>(
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

pub fn decode_ids<I: NumericId>(
    reader: &mut Reader<'_>,
    label: &'static str,
) -> Result<Vec<I>, CodecError> {
    decode_counted(reader, |reader| reader.id(label))
}

pub fn strictly_increasing<T: Ord>(values: impl IntoIterator<Item = T>) -> bool {
    values.into_iter().is_sorted_by(|left, right| left < right)
}

pub fn require_increasing<T: Ord>(
    values: impl IntoIterator<Item = T>,
    label: &'static str,
) -> Result<(), CodecError> {
    if strictly_increasing(values) {
        Ok(())
    } else {
        Err(CodecError::NonCanonicalOrder(label))
    }
}

pub fn decode_bytes(reader: &mut Reader<'_>) -> Result<Vec<u8>, CodecError> {
    let len = bounded_count(reader, 1)?;
    Ok(reader.take(len)?.to_vec())
}

pub fn decode_optional_id<I: NumericId>(
    reader: &mut Reader<'_>,
    label: &'static str,
) -> Result<Option<I>, CodecError> {
    match reader.u8()? {
        0 => Ok(None),
        1 => Ok(Some(reader.id(label)?)),
        tag => Err(CodecError::InvalidTag("OptionalSemanticId", tag)),
    }
}

pub fn decode_canonical<T>(
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
