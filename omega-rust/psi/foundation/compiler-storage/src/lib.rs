#![forbid(unsafe_code)]
//! Compiler-private graph storage with bounded decoding and shared-object identity.
//! Decoding produces candidates. Source, checking and realization owners must
//! bind the exact producer/inputs and rejoin current admission before reuse.
extern crate self as compiler_storage;
mod buffer;
mod values;
pub use buffer::{RecordError, RecordLimits, RecordReader, RecordWriter};
pub use compiler_storage_derive::Record;

pub trait Record: Sized {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError>;
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError>;
}

pub fn encode<T: Record>(value: &T, limits: RecordLimits) -> Result<Vec<u8>, RecordError> {
    let mut output = RecordWriter::new(limits);
    value.write(&mut output)?;
    Ok(output.finish())
}

pub fn decode<T: Record>(bytes: &[u8], limits: RecordLimits) -> Result<T, RecordError> {
    let mut input = RecordReader::<()>::new(bytes, limits)?;
    let value = T::read(&mut input)?;
    input.finish()?;
    Ok(value)
}

#[cfg(test)]
mod tests;
