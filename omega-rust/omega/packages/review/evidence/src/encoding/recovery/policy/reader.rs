use super::{Error, PackagePolicyRecoveryLimits, PackagePolicyRecoveryUsage};

pub(super) struct Reader<'a> {
    remaining: &'a [u8],
    limits: PackagePolicyRecoveryLimits,
    remaining_elements: usize,
    remaining_owned_bytes: usize,
    depth: usize,
}

impl<'a> Reader<'a> {
    pub(super) fn new(bytes: &'a [u8], limits: PackagePolicyRecoveryLimits) -> Result<Self, Error> {
        let limits = limits.bounded();
        if bytes.len() > limits.maximum_bytes {
            return Err(Error::InputTooLarge);
        }
        Ok(Self {
            remaining: bytes,
            remaining_elements: limits.maximum_sequence_elements,
            remaining_owned_bytes: limits.maximum_owned_bytes,
            limits,
            depth: 0,
        })
    }

    pub(super) fn finish(&self) -> Result<(), Error> {
        if self.remaining.is_empty() {
            Ok(())
        } else {
            Err(Error::TrailingBytes)
        }
    }

    pub(super) fn usage(&self) -> PackagePolicyRecoveryUsage {
        PackagePolicyRecoveryUsage {
            owned_bytes: self.limits.maximum_owned_bytes - self.remaining_owned_bytes,
            sequence_elements: self.limits.maximum_sequence_elements - self.remaining_elements,
        }
    }

    /// The format check holds canonical output alongside the recovered value.
    /// Count its requested bytes in the same caller-owned storage budget.
    pub(super) fn canonical_scratch(&mut self, bytes: usize) -> Result<(), Error> {
        self.allocate(bytes)
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let (value, remaining) = self
            .remaining
            .split_at_checked(length)
            .ok_or(Error::UnexpectedEnd)?;
        self.remaining = remaining;
        Ok(value)
    }

    pub(super) fn literal(&mut self, expected: &[u8]) -> Result<(), Error> {
        if self.take(expected.len())? == expected {
            Ok(())
        } else {
            Err(Error::UnsupportedVersion)
        }
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        Ok(self.take(N)?.try_into().expect("exact fixed-size slice"))
    }

    pub(super) fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.array::<1>()?[0])
    }
    pub(super) fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    pub(super) fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    pub(super) fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    pub(super) fn i64(&mut self) -> Result<i64, Error> {
        Ok(i64::from_le_bytes(self.array()?))
    }
    pub(super) fn digest(&mut self) -> Result<[u8; 32], Error> {
        self.array()
    }
    pub(super) fn usize(&mut self) -> Result<usize, Error> {
        usize::try_from(self.u64()?).map_err(|_| Error::LengthOverflow)
    }

    pub(super) fn boolean(&mut self) -> Result<bool, Error> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::InvalidTag),
        }
    }

    pub(super) fn bytes(&mut self) -> Result<Vec<u8>, Error> {
        let length = self.usize()?;
        if length > self.limits.maximum_field_bytes {
            return Err(Error::FieldTooLarge);
        }
        let bytes = self.take(length)?;
        self.allocate(length)?;
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(length)
            .map_err(|_| Error::AllocationFailed)?;
        owned.extend_from_slice(bytes);
        Ok(owned)
    }

    pub(super) fn string(&mut self) -> Result<String, Error> {
        String::from_utf8(self.bytes()?).map_err(|_| Error::InvalidUtf8)
    }

    pub(super) fn sequence<T>(
        &mut self,
        minimum_encoded_bytes: usize,
        mut decode: impl FnMut(&mut Self) -> Result<T, Error>,
    ) -> Result<Vec<T>, Error> {
        let count = self.usize()?;
        self.elements(count)?;
        if minimum_encoded_bytes == 0 || count > self.remaining.len() / minimum_encoded_bytes {
            return Err(Error::UnexpectedEnd);
        }
        self.allocate(
            count
                .checked_mul(std::mem::size_of::<T>())
                .ok_or(Error::LengthOverflow)?,
        )?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| Error::AllocationFailed)?;
        for _ in 0..count {
            values.push(decode(self)?);
        }
        Ok(values)
    }

    pub(super) fn option<T>(
        &mut self,
        decode: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<Option<T>, Error> {
        if self.boolean()? {
            decode(self).map(Some)
        } else {
            Ok(None)
        }
    }

    pub(super) fn nested<T>(
        &mut self,
        decode: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        if self.depth >= self.limits.maximum_depth {
            return Err(Error::NestingLimitExceeded);
        }
        self.elements(1)?;
        self.depth += 1;
        let result = decode(self);
        self.depth -= 1;
        result
    }

    pub(super) fn boxed<T>(
        &mut self,
        decode: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<Box<T>, Error> {
        self.allocate(std::mem::size_of::<T>())?;
        Ok(Box::new(decode(self)?))
    }

    fn elements(&mut self, count: usize) -> Result<(), Error> {
        self.remaining_elements = self
            .remaining_elements
            .checked_sub(count)
            .ok_or(Error::ElementLimitExceeded)?;
        Ok(())
    }

    fn allocate(&mut self, bytes: usize) -> Result<(), Error> {
        self.remaining_owned_bytes = self
            .remaining_owned_bytes
            .checked_sub(bytes)
            .ok_or(Error::AllocationLimitExceeded)?;
        Ok(())
    }
}
