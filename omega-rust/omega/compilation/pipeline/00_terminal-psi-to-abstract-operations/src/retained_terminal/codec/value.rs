//! Bounded values in a compiler-private native-product record.
//!
//! These bytes describe candidates, not admitted execution or live resources.
//! Recovery of the enclosing product belongs to its ordinary admission owners.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(super) struct RecordLimits {
    pub bytes: usize,
    pub allocation: usize,
    pub elements: usize,
    pub depth: usize,
}

impl Default for RecordLimits {
    fn default() -> Self {
        Self {
            bytes: 64 * 1024 * 1024,
            allocation: 512 * 1024 * 1024,
            elements: 4 * 1024 * 1024,
            depth: 128,
        }
    }
}

#[derive(Debug)]
pub(super) struct RecordError {
    pub position: usize,
    pub reason: Cow<'static, str>,
}

impl fmt::Display for RecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "native product record at byte {}: {}",
            self.position, self.reason
        )
    }
}

impl std::error::Error for RecordError {}

pub(super) struct RecordWriter {
    bytes: Vec<u8>,
    limits: RecordLimits,
    remaining_elements: usize,
}

impl RecordWriter {
    pub fn new(limits: RecordLimits) -> Self {
        Self {
            bytes: Vec::new(),
            remaining_elements: limits.elements,
            limits,
        }
    }

    pub fn extend(&mut self, bytes: &[u8]) -> Result<(), RecordError> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|length| length > self.limits.bytes)
        {
            return Err(self.error("encoded byte limit exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    pub fn sequence(&mut self, length: usize) -> Result<(), RecordError> {
        self.remaining_elements = self
            .remaining_elements
            .checked_sub(length)
            .ok_or_else(|| self.error("sequence element limit exceeded"))?;
        u64::try_from(length)
            .map_err(|_| self.error("sequence length does not fit the record"))?
            .write(self)
    }

    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }

    pub fn error(&self, reason: impl Into<Cow<'static, str>>) -> RecordError {
        RecordError {
            position: self.bytes.len(),
            reason: reason.into(),
        }
    }
}

pub(super) struct RecordReader<'bytes> {
    bytes: &'bytes [u8],
    position: usize,
    remaining_allocation: usize,
    remaining_elements: usize,
    remaining_depth: usize,
    binding_identities: Option<Arc<terminal_psi::TerminalIdentityTable>>,
}

impl<'bytes> RecordReader<'bytes> {
    pub fn new(bytes: &'bytes [u8], limits: RecordLimits) -> Result<Self, RecordError> {
        if bytes.len() > limits.bytes {
            return Err(RecordError {
                position: 0,
                reason: "encoded byte limit exceeded".into(),
            });
        }
        Ok(Self {
            bytes,
            position: 0,
            remaining_allocation: limits.allocation,
            remaining_elements: limits.elements,
            remaining_depth: limits.depth,
            binding_identities: None,
        })
    }

    pub fn take(&mut self, length: usize) -> Result<&'bytes [u8], RecordError> {
        let end = self
            .position
            .checked_add(length)
            .ok_or_else(|| self.error("byte length overflow"))?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| self.error("truncated value"))?;
        self.position = end;
        Ok(bytes)
    }

    pub fn set_binding_identities(&mut self, identities: Arc<terminal_psi::TerminalIdentityTable>) {
        self.binding_identities = Some(identities);
    }

    pub fn binding_identities(
        &self,
    ) -> Result<Arc<terminal_psi::TerminalIdentityTable>, RecordError> {
        self.binding_identities.clone().ok_or_else(|| {
            self.error("evaluated binding receipt is missing its producer identity namespace")
        })
    }

    pub fn allocate(&mut self, bytes: usize) -> Result<(), RecordError> {
        self.remaining_allocation = self
            .remaining_allocation
            .checked_sub(bytes)
            .ok_or_else(|| self.error("decoded allocation limit exceeded"))?;
        Ok(())
    }

    pub fn sequence<T>(&mut self) -> Result<usize, RecordError> {
        let length = usize::try_from(u64::read(self)?)
            .map_err(|_| self.error("sequence length exceeds this host"))?;
        self.remaining_elements = self
            .remaining_elements
            .checked_sub(length)
            .ok_or_else(|| self.error("sequence element limit exceeded"))?;
        self.allocate(
            length
                .checked_mul(std::mem::size_of::<T>())
                .ok_or_else(|| self.error("sequence allocation overflow"))?,
        )?;
        Ok(length)
    }

    pub fn record<T>(
        &mut self,
        read: impl FnOnce(&mut Self) -> Result<T, RecordError>,
    ) -> Result<T, RecordError> {
        self.remaining_depth = self
            .remaining_depth
            .checked_sub(1)
            .ok_or_else(|| self.error("record nesting limit exceeded"))?;
        let result = read(self);
        self.remaining_depth += 1;
        result
    }

    pub fn finish(self) -> Result<(), RecordError> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(self.error("trailing bytes"))
        }
    }

    pub fn error(&self, reason: impl Into<Cow<'static, str>>) -> RecordError {
        RecordError {
            position: self.position,
            reason: reason.into(),
        }
    }
}

pub(super) trait RecordValue: Sized {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError>;
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError>;
}

macro_rules! integer_value {
    ($($value:ty),* $(,)?) => {$ (
        impl RecordValue for $value {
            fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
                output.extend(&self.to_le_bytes())
            }
            fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
                let bytes = input.take(std::mem::size_of::<Self>())?;
                let bytes = bytes.try_into().map_err(|_| input.error("integer width mismatch"))?;
                Ok(Self::from_le_bytes(bytes))
            }
        }
    )*};
}
integer_value!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128);

impl RecordValue for usize {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        u64::try_from(*self)
            .map_err(|_| output.error("integer exceeds record width"))?
            .write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        Self::try_from(u64::read(input)?).map_err(|_| input.error("integer exceeds this host"))
    }
}

impl RecordValue for bool {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        u8::from(*self).write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        match u8::read(input)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(input.error("invalid Boolean tag")),
        }
    }
}

impl RecordValue for String {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.sequence(self.len())?;
        output.extend(self.as_bytes())
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        let length = input.sequence::<u8>()?;
        let text =
            std::str::from_utf8(input.take(length)?).map_err(|_| input.error("invalid UTF-8"))?;
        Ok(text.to_owned())
    }
}

impl<T: RecordValue> RecordValue for Vec<T> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.sequence(self.len())?;
        for value in self {
            value.write(output)?;
        }
        Ok(())
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input.record(|input| {
            let length = input.sequence::<T>()?;
            let mut values = Vec::with_capacity(length.min(1024));
            for _ in 0..length {
                values.push(T::read(input)?);
            }
            Ok(values)
        })
    }
}

impl<T: RecordValue, const LENGTH: usize> RecordValue for [T; LENGTH] {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        for value in self {
            value.write(output)?;
        }
        Ok(())
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input.allocate(std::mem::size_of::<Self>())?;
        let mut values = Vec::with_capacity(LENGTH);
        for _ in 0..LENGTH {
            values.push(T::read(input)?);
        }
        values
            .try_into()
            .map_err(|_| input.error("array width mismatch"))
    }
}

impl<T: RecordValue> RecordValue for Option<T> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        match self {
            None => 0u8.write(output),
            Some(value) => {
                1u8.write(output)?;
                value.write(output)
            }
        }
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input.record(|input| match u8::read(input)? {
            0 => Ok(None),
            1 => T::read(input).map(Some),
            _ => Err(input.error("invalid optional-value tag")),
        })
    }
}

impl<T: RecordValue, Error: RecordValue> RecordValue for Result<T, Error> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        match self {
            Ok(value) => {
                0u8.write(output)?;
                value.write(output)
            }
            Err(error) => {
                1u8.write(output)?;
                error.write(output)
            }
        }
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input.record(|input| match u8::read(input)? {
            0 => T::read(input).map(Ok),
            1 => Error::read(input).map(Err),
            _ => Err(input.error("invalid result tag")),
        })
    }
}

impl<T: RecordValue> RecordValue for Arc<T> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.as_ref().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input
            .allocate(std::mem::size_of::<T>().saturating_add(2 * std::mem::size_of::<usize>()))?;
        input.record(T::read).map(Arc::new)
    }
}

impl<T: RecordValue + Ord> RecordValue for BTreeSet<T> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.sequence(self.len())?;
        for value in self {
            value.write(output)?;
        }
        Ok(())
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        let values = Vec::<T>::read(input)?;
        if values.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(input.error("set values are not strictly ordered"));
        }
        // Include tree storage as well as the already-accounted temporary vector.
        input.allocate(
            values
                .len()
                .saturating_mul(std::mem::size_of::<T>() + 4 * std::mem::size_of::<usize>()),
        )?;
        Ok(values.into_iter().collect())
    }
}

impl<Key: RecordValue + Ord, Value: RecordValue> RecordValue for BTreeMap<Key, Value> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.sequence(self.len())?;
        for (key, value) in self {
            key.write(output)?;
            value.write(output)?;
        }
        Ok(())
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input.record(|input| {
            let length = input.sequence::<(Key, Value)>()?;
            input.allocate(length.saturating_mul(4 * std::mem::size_of::<usize>()))?;
            let mut values = BTreeMap::new();
            for _ in 0..length {
                let key = Key::read(input)?;
                if values
                    .last_key_value()
                    .is_some_and(|(previous, _)| previous >= &key)
                {
                    return Err(input.error("map keys are not strictly ordered"));
                }
                values.insert(key, Value::read(input)?);
            }
            Ok(values)
        })
    }
}

impl RecordValue for () {
    fn write(&self, _output: &mut RecordWriter) -> Result<(), RecordError> {
        Ok(())
    }
    fn read(_input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        Ok(())
    }
}
impl<T: RecordValue> RecordValue for Box<T> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.as_ref().write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input.allocate(std::mem::size_of::<T>())?;
        input.record(T::read).map(Box::new)
    }
}
impl<Left: RecordValue, Right: RecordValue> RecordValue for (Left, Right) {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.0.write(output)?;
        self.1.write(output)
    }
    fn read(input: &mut RecordReader<'_>) -> Result<Self, RecordError> {
        input.record(|input| Ok((Left::read(input)?, Right::read(input)?)))
    }
}

pub(super) fn write_values<T: RecordValue>(
    values: &[T],
    output: &mut RecordWriter,
) -> Result<(), RecordError> {
    output.sequence(values.len())?;
    for value in values {
        value.write(output)?;
    }
    Ok(())
}

pub(super) fn write_text(value: &str, output: &mut RecordWriter) -> Result<(), RecordError> {
    output.sequence(value.len())?;
    output.extend(value.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode<T: RecordValue>(value: &T) -> Vec<u8> {
        let mut writer = RecordWriter::new(RecordLimits::default());
        value.write(&mut writer).expect("bounded record");
        writer.finish()
    }

    #[test]
    fn record_values_preserve_wide_bits_and_deferred_outcomes() {
        let values = vec![
            None,
            Some(Ok(u128::MAX)),
            Some(Err("pending".to_owned())),
            Some(Ok(0)),
        ];
        let bytes = encode(&values);
        let mut reader = RecordReader::new(&bytes, RecordLimits::default()).expect("bounded bytes");
        assert_eq!(
            Vec::<Option<Result<u128, String>>>::read(&mut reader).expect("value"),
            values
        );
        reader.finish().expect("complete record");
    }

    #[test]
    fn record_values_reject_every_truncated_prefix_and_trailing_bytes() {
        let value = (i128::MIN, vec!["entry".to_owned(), "λ".to_owned()]);
        let mut bytes = encode(&value);
        for length in 0..bytes.len() {
            let mut reader = RecordReader::new(&bytes[..length], RecordLimits::default())
                .expect("bounded prefix");
            assert!(<(i128, Vec<String>)>::read(&mut reader).is_err());
        }
        bytes.push(0);
        let mut reader = RecordReader::new(&bytes, RecordLimits::default()).expect("bounded bytes");
        <(i128, Vec<String>)>::read(&mut reader).expect("value");
        assert!(reader.finish().is_err());
    }

    #[test]
    fn record_limits_precede_allocation_and_nested_decoding() {
        let enormous_length = u64::MAX.to_le_bytes();
        let mut reader =
            RecordReader::new(&enormous_length, RecordLimits::default()).expect("bounded bytes");
        assert!(Vec::<u128>::read(&mut reader).is_err());
        let bytes = [0; 32];
        let limits = RecordLimits {
            allocation: 31,
            ..RecordLimits::default()
        };
        let mut reader = RecordReader::new(&bytes, limits).expect("bounded bytes");
        assert!(<[u8; 32]>::read(&mut reader).is_err());
        fn nested(reader: &mut RecordReader<'_>) -> Result<(), RecordError> {
            reader.record(|reader| {
                if bool::read(reader)? {
                    nested(reader)
                } else {
                    Ok(())
                }
            })
        }
        let bytes = [1; 129];
        let mut reader = RecordReader::new(&bytes, RecordLimits::default()).expect("bounded bytes");
        assert!(
            nested(&mut reader)
                .expect_err("depth bound")
                .reason
                .contains("nesting")
        );
    }

    #[test]
    fn record_values_refuse_unknown_tags_and_duplicate_set_rows() {
        for tag in [2u8, 255] {
            let bytes = [tag];
            let mut reader =
                RecordReader::new(&bytes, RecordLimits::default()).expect("bounded bytes");
            assert!(Option::<u32>::read(&mut reader).is_err());
        }
        let bytes = encode(&vec![3u32, 3]);
        let mut reader = RecordReader::new(&bytes, RecordLimits::default()).expect("bounded bytes");
        assert!(BTreeSet::<u32>::read(&mut reader).is_err());
    }
}
