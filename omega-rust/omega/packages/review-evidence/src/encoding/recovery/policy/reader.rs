use super::{Error, PackagePolicyRecoveryLimits, PackagePolicyRecoveryUsage};

pub(in crate::encoding::recovery) struct Reader<'a> {
    remaining: &'a [u8],
    limits: PackagePolicyRecoveryLimits,
    remaining_elements: usize,
    remaining_owned_bytes: usize,
    depth: usize,
}

impl<'a> Reader<'a> {
    pub(in crate::encoding::recovery) fn new(
        bytes: &'a [u8],
        limits: PackagePolicyRecoveryLimits,
    ) -> Result<Self, Error> {
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

    pub(in crate::encoding::recovery) fn finish(&self) -> Result<(), Error> {
        if self.remaining.is_empty() {
            Ok(())
        } else {
            Err(Error::TrailingBytes)
        }
    }

    pub(in crate::encoding::recovery) fn usage(&self) -> PackagePolicyRecoveryUsage {
        PackagePolicyRecoveryUsage {
            owned_bytes: self.limits.maximum_owned_bytes - self.remaining_owned_bytes,
            sequence_elements: self.limits.maximum_sequence_elements - self.remaining_elements,
        }
    }

    /// The format check holds canonical output alongside the recovered value.
    /// Count its requested bytes in the same caller-owned storage budget.
    pub(in crate::encoding::recovery) fn canonical_scratch(
        &mut self,
        bytes: usize,
    ) -> Result<(), Error> {
        self.allocate(bytes)
    }

    pub(in crate::encoding::recovery) fn take(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let (value, remaining) = self
            .remaining
            .split_at_checked(length)
            .ok_or(Error::UnexpectedEnd)?;
        self.remaining = remaining;
        Ok(value)
    }

    pub(in crate::encoding::recovery) fn literal(&mut self, expected: &[u8]) -> Result<(), Error> {
        if self.take(expected.len())? == expected {
            Ok(())
        } else {
            Err(Error::UnsupportedVersion)
        }
    }

    pub(in crate::encoding::recovery) fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], Error> {
        Ok(self.take(N)?.try_into().expect("exact fixed-size slice"))
    }

    pub(in crate::encoding::recovery) fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.array::<1>()?[0])
    }
    pub(in crate::encoding::recovery) fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    pub(in crate::encoding::recovery) fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    pub(in crate::encoding::recovery) fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    pub(in crate::encoding::recovery) fn i64(&mut self) -> Result<i64, Error> {
        Ok(i64::from_le_bytes(self.array()?))
    }
    pub(in crate::encoding::recovery) fn digest(&mut self) -> Result<[u8; 32], Error> {
        self.array()
    }
    pub(in crate::encoding::recovery) fn usize(&mut self) -> Result<usize, Error> {
        usize::try_from(self.u64()?).map_err(|_| Error::LengthOverflow)
    }

    pub(in crate::encoding::recovery) fn boolean(&mut self) -> Result<bool, Error> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::InvalidTag),
        }
    }

    pub(in crate::encoding::recovery) fn bytes(&mut self) -> Result<Vec<u8>, Error> {
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

    pub(in crate::encoding::recovery) fn string(&mut self) -> Result<String, Error> {
        String::from_utf8(self.bytes()?).map_err(|_| Error::InvalidUtf8)
    }

    pub(in crate::encoding::recovery) fn sequence<T>(
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

    pub(in crate::encoding::recovery) fn option<T>(
        &mut self,
        decode: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<Option<T>, Error> {
        if self.boolean()? {
            decode(self).map(Some)
        } else {
            Ok(None)
        }
    }

    pub(in crate::encoding::recovery) fn nested<T>(
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

    pub(in crate::encoding::recovery) fn boxed<T>(
        &mut self,
        decode: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<Box<T>, Error> {
        self.allocate(std::mem::size_of::<T>())?;
        Ok(Box::new(decode(self)?))
    }

    pub(in crate::encoding::recovery) fn elements(&mut self, count: usize) -> Result<(), Error> {
        self.remaining_elements = self
            .remaining_elements
            .checked_sub(count)
            .ok_or(Error::ElementLimitExceeded)?;
        Ok(())
    }

    pub(in crate::encoding::recovery) fn allocate(&mut self, bytes: usize) -> Result<(), Error> {
        self.remaining_owned_bytes = self
            .remaining_owned_bytes
            .checked_sub(bytes)
            .ok_or(Error::AllocationLimitExceeded)?;
        Ok(())
    }
}
