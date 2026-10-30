//! Native-product vocabulary over the shared bounded record buffers.
//! The identity context belongs to this decoder invocation, never the store.
pub(super) use compiler_storage::{RecordError, RecordLimits, RecordWriter};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
pub(super) type RecordReader<'bytes> = compiler_storage::RecordReader<'bytes, NativeRecordContext>;

#[derive(Default)]
pub(super) struct NativeRecordContext {
    pub(super) binding_identities: Option<Arc<terminal_psi::TerminalIdentityTable>>,
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
