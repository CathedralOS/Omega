//! Owned values in a private record. Sharing is local to one writer/reader;
//! pointers never enter the bytes, and decoding cannot create forward cycles.
use crate::{Record, RecordError, RecordReader, RecordWriter};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{BuildHasher, Hash};
use std::marker::PhantomData;
use std::sync::Arc;

macro_rules! integer {
    ($($kind:ty),*) => {$ (
        impl Record for $kind {
            fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> { output.extend(&self.to_le_bytes()) }
            fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
                let mut bytes = [0u8; std::mem::size_of::<Self>()];
                bytes.copy_from_slice(input.take(std::mem::size_of::<Self>())?);
                Ok(Self::from_le_bytes(bytes))
            }
        }
    )*};
}
integer!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, f32, f64);
impl Record for usize {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        u64::try_from(*self)
            .map_err(|_| output.error("size exceeds record width"))?
            .write(output)
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        usize::try_from(u64::read(input)?).map_err(|_| input.error("size exceeds host width"))
    }
}
impl Record for isize {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        i64::try_from(*self)
            .map_err(|_| output.error("signed size exceeds record width"))?
            .write(output)
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        isize::try_from(i64::read(input)?)
            .map_err(|_| input.error("signed size exceeds host width"))
    }
}
impl Record for bool {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        u8::from(*self).write(output)
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        match u8::read(input)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(input.error("invalid Boolean")),
        }
    }
}
impl Record for char {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        (*self as u32).write(output)
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        char::from_u32(u32::read(input)?).ok_or_else(|| input.error("invalid Unicode scalar"))
    }
}
impl Record for () {
    fn write(&self, _: &mut RecordWriter) -> Result<(), RecordError> {
        Ok(())
    }
    fn read<Context>(_: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        Ok(())
    }
}
impl<T> Record for PhantomData<T> {
    fn write(&self, _: &mut RecordWriter) -> Result<(), RecordError> {
        Ok(())
    }
    fn read<Context>(_: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        Ok(PhantomData)
    }
}
impl<T: Record> Record for Option<T> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.record(|output| match self {
            None => 0u8.write(output),
            Some(value) => {
                1u8.write(output)?;
                value.write(output)
            }
        })
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        input.record(|input| match u8::read(input)? {
            0 => Ok(None),
            1 => T::read(input).map(Some),
            _ => Err(input.error("invalid optional value")),
        })
    }
}
impl<T: Record, E: Record> Record for Result<T, E> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.record(|output| match self {
            Ok(value) => {
                0u8.write(output)?;
                value.write(output)
            }
            Err(error) => {
                1u8.write(output)?;
                error.write(output)
            }
        })
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        input.record(|input| match u8::read(input)? {
            0 => T::read(input).map(Ok),
            1 => E::read(input).map(Err),
            _ => Err(input.error("invalid result value")),
        })
    }
}
impl<T: Record> Record for Vec<T> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.record(|output| {
            output.sequence(self.len())?;
            for value in self {
                value.write(output)?;
            }
            Ok(())
        })
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        input.record(|input| {
            let length = input.sequence::<T>()?;
            let mut values = Vec::with_capacity(length);
            for _ in 0..length {
                values.push(T::read(input)?);
            }
            Ok(values)
        })
    }
}
impl<T: Record, const N: usize> Record for [T; N] {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.sequence(N)?;
        for value in self {
            value.write(output)?;
        }
        Ok(())
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        let length = input.sequence::<T>()?;
        if length != N {
            return Err(input.error("fixed array has another length"));
        }
        let mut values = Vec::with_capacity(N);
        for _ in 0..N {
            values.push(T::read(input)?);
        }
        values
            .try_into()
            .map_err(|_| input.error("fixed array construction failed"))
    }
}
impl Record for String {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.sequence(self.len())?;
        output.extend(self.as_bytes())
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        let length = input.sequence::<u8>()?;
        String::from_utf8(input.take(length)?.to_vec()).map_err(|_| input.error("invalid UTF-8"))
    }
}
impl Record for Cow<'_, str> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.sequence(self.len())?;
        output.extend(self.as_bytes())
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        String::read(input).map(Cow::Owned)
    }
}
impl<T: Record> Record for Box<T> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.record(|output| self.as_ref().write(output))
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        input.record(|input| {
            input.allocate(std::mem::size_of::<T>())?;
            T::read(input).map(Box::new)
        })
    }
}
impl<T: Record> Record for Box<[T]> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.record(|output| {
            output.sequence(self.len())?;
            for value in self {
                value.write(output)?;
            }
            Ok(())
        })
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        Vec::<T>::read(input).map(Vec::into_boxed_slice)
    }
}
impl<T: Record + 'static> Record for Arc<T> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.record(|output| {
            let (identity, first) = output.shared_identity(Arc::as_ptr(self), 0)?;
            identity.write(output)?;
            first.write(output)?;
            if first {
                self.as_ref().write(output)?;
            }
            Ok(())
        })
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        input.record(|input| {
            let identity = u64::read(input)?;
            if !bool::read(input)? {
                return input.shared::<Self>(identity);
            }
            input.begin_shared(identity)?;
            input.allocate(std::mem::size_of::<T>() + 2 * std::mem::size_of::<usize>())?;
            let value = Arc::new(T::read(input)?);
            input.finish_shared(identity, value)
        })
    }
}
impl Record for Arc<str> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        let (identity, first) = output.shared_identity(Arc::as_ptr(self), self.len())?;
        identity.write(output)?;
        first.write(output)?;
        if first {
            output.sequence(self.len())?;
            output.extend(self.as_bytes())?;
        }
        Ok(())
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        let identity = u64::read(input)?;
        if !bool::read(input)? {
            return input.shared::<Self>(identity);
        }
        input.begin_shared(identity)?;
        let value = String::read(input)?;
        input.allocate(value.len() + 2 * std::mem::size_of::<usize>())?;
        input.finish_shared(identity, Arc::<str>::from(value))
    }
}
impl<T: Record + 'static> Record for Arc<[T]> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        let (identity, first) = output.shared_identity(Arc::as_ptr(self), self.len())?;
        identity.write(output)?;
        first.write(output)?;
        if first {
            output.record(|output| {
                output.sequence(self.len())?;
                for value in self.iter() {
                    value.write(output)?;
                }
                Ok(())
            })?;
        }
        Ok(())
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        let identity = u64::read(input)?;
        if !bool::read(input)? {
            return input.shared::<Self>(identity);
        }
        input.begin_shared(identity)?;
        let values = Vec::<T>::read(input)?;
        input.allocate(
            values
                .len()
                .checked_mul(std::mem::size_of::<T>())
                .and_then(|bytes| bytes.checked_add(2 * std::mem::size_of::<usize>()))
                .ok_or_else(|| input.error("shared slice allocation overflow"))?,
        )?;
        input.finish_shared(identity, Arc::<[T]>::from(values))
    }
}
macro_rules! tuple {
    ($($name:ident:$position:tt),*) => {
        impl<$($name: Record),*> Record for ($($name,)*) {
            fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> { $(self.$position.write(output)?;)* Ok(()) }
            fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> { Ok(($(<$name as Record>::read(input)?,)*)) }
        }
    };
}
tuple!(A:0);
tuple!(A:0,B:1);
tuple!(A:0,B:1,C:2);
tuple!(A:0,B:1,C:2,D:3);
tuple!(A:0,B:1,C:2,D:3,E:4);
tuple!(A:0,B:1,C:2,D:3,E:4,F:5);
tuple!(A:0,B:1,C:2,D:3,E:4,F:5,G:6);
tuple!(A:0,B:1,C:2,D:3,E:4,F:5,G:6,H:7);
impl<K: Record + Ord, V: Record> Record for BTreeMap<K, V> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.sequence(self.len())?;
        for (key, value) in self {
            key.write(output)?;
            value.write(output)?;
        }
        Ok(())
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        let length = input.sequence::<(K, V)>()?;
        let mut values = Self::new();
        for _ in 0..length {
            let key = K::read(input)?;
            let value = V::read(input)?;
            if values.insert(key, value).is_some() {
                return Err(input.error("duplicate map key"));
            }
        }
        Ok(values)
    }
}
impl<T: Record + Ord> Record for BTreeSet<T> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.sequence(self.len())?;
        for value in self {
            value.write(output)?;
        }
        Ok(())
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        let length = input.sequence::<T>()?;
        let mut values = Self::new();
        for _ in 0..length {
            if !values.insert(T::read(input)?) {
                return Err(input.error("duplicate set entry"));
            }
        }
        Ok(values)
    }
}
impl<K: Record + Hash + Eq, V: Record, S: BuildHasher + Default> Record for HashMap<K, V, S> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.sequence(self.len())?;
        for (key, value) in self {
            key.write(output)?;
            value.write(output)?;
        }
        Ok(())
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        let length = input.sequence::<(K, V)>()?;
        let mut values = Self::with_capacity_and_hasher(length, S::default());
        for _ in 0..length {
            let key = K::read(input)?;
            let value = V::read(input)?;
            if values.insert(key, value).is_some() {
                return Err(input.error("duplicate map key"));
            }
        }
        Ok(values)
    }
}
impl<T: Record + Hash + Eq, S: BuildHasher + Default> Record for HashSet<T, S> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.sequence(self.len())?;
        for value in self {
            value.write(output)?;
        }
        Ok(())
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        let length = input.sequence::<T>()?;
        let mut values = Self::with_capacity_and_hasher(length, S::default());
        for _ in 0..length {
            if !values.insert(T::read(input)?) {
                return Err(input.error("duplicate set entry"));
            }
        }
        Ok(values)
    }
}
impl<T: Record> Record for std::ops::Range<T> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.start.write(output)?;
        self.end.write(output)
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        Ok(T::read(input)?..T::read(input)?)
    }
}
impl Record for std::time::Duration {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.as_secs().write(output)?;
        self.subsec_nanos().write(output)
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        let seconds = u64::read(input)?;
        let nanos = u32::read(input)?;
        if nanos >= 1_000_000_000 {
            return Err(input.error("invalid duration fraction"));
        }
        Ok(Self::new(seconds, nanos))
    }
}
impl Record for std::ffi::OsString {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            0u8.write(output)?;
            let bytes = self.as_os_str().as_bytes();
            output.sequence(bytes.len())?;
            output.extend(bytes)
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            1u8.write(output)?;
            self.encode_wide().collect::<Vec<_>>().write(output)
        }
        #[cfg(not(any(unix, windows)))]
        {
            Err(output.error("host path encoding is unsupported"))
        }
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        let tag = u8::read(input)?;
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            if tag != 0 {
                return Err(input.error("foreign host path encoding"));
            }
            Vec::<u8>::read(input).map(Self::from_vec)
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStringExt;
            if tag != 1 {
                return Err(input.error("foreign host path encoding"));
            }
            Vec::<u16>::read(input).map(|words| Self::from_wide(&words))
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = tag;
            Err(input.error("host path encoding is unsupported"))
        }
    }
}
impl Record for std::path::PathBuf {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.as_os_str().to_os_string().write(output)
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        std::ffi::OsString::read(input).map(Self::from)
    }
}

impl<T: Clone + Record> Record for Cow<'_, [T]> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.record(|output| {
            output.sequence(self.len())?;
            for value in self.iter() {
                value.write(output)?;
            }
            Ok(())
        })
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        Vec::<T>::read(input).map(Cow::Owned)
    }
}
macro_rules! nonzero {
    ($($name:ident:$primitive:ty),*) => {$ (
        impl Record for std::num::$name {
            fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> { self.get().write(output) }
            fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
                Self::new(<$primitive>::read(input)?).ok_or_else(|| input.error("nonzero coordinate has the reserved zero value"))
            }
        }
    )*};
}
nonzero!(NonZeroU8:u8,NonZeroU16:u16,NonZeroU32:u32,NonZeroU64:u64,NonZeroU128:u128,NonZeroUsize:usize,NonZeroI8:i8,NonZeroI16:i16,NonZeroI32:i32,NonZeroI64:i64,NonZeroI128:i128,NonZeroIsize:isize);

impl Record for Box<str> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        output.sequence(self.len())?;
        output.extend(self.as_bytes())
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        String::read(input).map(String::into_boxed_str)
    }
}

impl<T: Record> Record for std::sync::Mutex<T> {
    fn write(&self, output: &mut RecordWriter) -> Result<(), RecordError> {
        self.lock()
            .map_err(|_| output.error("persistent graph owner is poisoned"))?
            .write(output)
    }
    fn read<Context>(input: &mut RecordReader<'_, Context>) -> Result<Self, RecordError> {
        T::read(input).map(Self::new)
    }
}
