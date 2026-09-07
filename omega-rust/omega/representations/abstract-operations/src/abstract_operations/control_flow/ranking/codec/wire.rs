//! Checked primitive transport for ranked custody only.
use super::RankedCustodyCodecError as Error;
pub(super) struct Reader<'a>(pub &'a [u8]);
impl<'a> Reader<'a> {
    pub fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let value = self.0.get(..count).ok_or(Error::InvalidEncoding)?;
        self.0 = &self.0[count..];
        Ok(value)
    }
    pub fn tag(&mut self, expected: u8) -> Result<(), Error> {
        if u8::read(self)? != expected {
            return Err(Error::InvalidEncoding);
        }
        Ok(())
    }
}
pub(super) trait Wire: Sized {
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), Error>;
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error>;
}
macro_rules! number {
    ($($name:ty),*) => { $(impl Wire for $name {
        fn write(&self, bytes: &mut Vec<u8>) -> Result<(), Error> { bytes.extend_from_slice(&self.to_le_bytes()); Ok(()) }
        fn read(reader: &mut Reader<'_>) -> Result<Self, Error> { Ok(Self::from_le_bytes(reader.take(size_of::<Self>())?.try_into().map_err(|_| Error::InvalidEncoding)?)) }
    })* };
}
number!(u8, u16, u32, u64, u128, i128);
impl<T: Wire> Wire for Vec<T> {
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), Error> {
        u64::try_from(self.len())
            .map_err(|_| Error::InvalidEncoding)?
            .write(bytes)?;
        for value in self {
            value.write(bytes)?;
        }
        Ok(())
    }
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        let count = usize::try_from(u64::read(reader)?).map_err(|_| Error::InvalidEncoding)?;
        if count > reader.0.len() {
            return Err(Error::InvalidEncoding);
        }
        (0..count).map(|_| T::read(reader)).collect()
    }
}
impl<T: Wire> Wire for Option<T> {
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), Error> {
        match self {
            None => 0u8.write(bytes),
            Some(value) => {
                1u8.write(bytes)?;
                value.write(bytes)
            }
        }
    }
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        match u8::read(reader)? {
            0 => Ok(None),
            1 => Ok(Some(T::read(reader)?)),
            _ => Err(Error::InvalidEncoding),
        }
    }
}
impl Wire for String {
    fn write(&self, bytes: &mut Vec<u8>) -> Result<(), Error> {
        self.as_bytes().to_vec().write(bytes)
    }
    fn read(reader: &mut Reader<'_>) -> Result<Self, Error> {
        String::from_utf8(Vec::<u8>::read(reader)?).map_err(|_| Error::InvalidEncoding)
    }
}
