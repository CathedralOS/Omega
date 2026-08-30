//! Bounded byte framing for canonical baseline fields.

use crate::review::baseline::ReviewOnlyBaselineError;

pub(in crate::review::baseline) struct Encoder {
    bytes: Vec<u8>,
    maximum_bytes: usize,
    exceeded: bool,
}

impl Encoder {
    pub(in crate::review::baseline) fn bounded(maximum_bytes: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum_bytes,
            exceeded: false,
        }
    }

    pub(in crate::review::baseline) fn append(&mut self, bytes: &[u8]) {
        if self.exceeded
            || self
                .bytes
                .len()
                .checked_add(bytes.len())
                .is_none_or(|length| length > self.maximum_bytes)
        {
            self.exceeded = true;
            return;
        }
        if self.bytes.try_reserve(bytes.len()).is_err() {
            self.exceeded = true;
            return;
        }
        self.bytes.extend_from_slice(bytes);
    }

    pub(in crate::review::baseline) fn fixed(&mut self, bytes: &[u8]) {
        self.append(bytes);
    }

    pub(in crate::review::baseline) fn byte(&mut self, value: u8) {
        self.append(&[value]);
    }

    pub(in crate::review::baseline) fn u16(&mut self, value: u16) {
        self.append(&value.to_le_bytes());
    }

    pub(in crate::review::baseline) fn u32(&mut self, value: u32) {
        self.append(&value.to_le_bytes());
    }

    pub(in crate::review::baseline) fn usize(
        &mut self,
        value: usize,
    ) -> Result<(), ReviewOnlyBaselineError> {
        self.append(
            &u64::try_from(value)
                .map_err(|_| ReviewOnlyBaselineError::new("baseline length exceeds u64"))?
                .to_le_bytes(),
        );
        Ok(())
    }

    pub(in crate::review::baseline) fn bytes(
        &mut self,
        bytes: &[u8],
    ) -> Result<(), ReviewOnlyBaselineError> {
        self.usize(bytes.len())?;
        self.append(bytes);
        Ok(())
    }

    pub(in crate::review::baseline) fn string(
        &mut self,
        value: &str,
    ) -> Result<(), ReviewOnlyBaselineError> {
        self.bytes(value.as_bytes())
    }

    pub(in crate::review::baseline) fn finish(self) -> Result<Vec<u8>, ReviewOnlyBaselineError> {
        if self.exceeded {
            Err(ReviewOnlyBaselineError::new(
                "review baseline encoding exceeds its byte ceiling",
            ))
        } else {
            Ok(self.bytes)
        }
    }
}

pub(in crate::review::baseline) struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    pub(in crate::review::baseline) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(in crate::review::baseline) fn take(
        &mut self,
        length: usize,
    ) -> Result<&'a [u8], ReviewOnlyBaselineError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| ReviewOnlyBaselineError::new("baseline length overflow"))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| ReviewOnlyBaselineError::new("truncated review baseline capsule"))?;
        self.offset = end;
        Ok(value)
    }

    pub(in crate::review::baseline) fn fixed(
        &mut self,
        expected: &[u8],
    ) -> Result<(), ReviewOnlyBaselineError> {
        if self.take(expected.len())? == expected {
            Ok(())
        } else {
            Err(ReviewOnlyBaselineError::new(
                "invalid review baseline capsule magic",
            ))
        }
    }

    pub(in crate::review::baseline) fn byte(&mut self) -> Result<u8, ReviewOnlyBaselineError> {
        Ok(self.take(1)?[0])
    }

    pub(in crate::review::baseline) fn u16(&mut self) -> Result<u16, ReviewOnlyBaselineError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("exact u16 width"),
        ))
    }

    pub(in crate::review::baseline) fn u32(&mut self) -> Result<u32, ReviewOnlyBaselineError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("exact u32 width"),
        ))
    }

    pub(in crate::review::baseline) fn usize(&mut self) -> Result<usize, ReviewOnlyBaselineError> {
        usize::try_from(u64::from_le_bytes(
            self.take(8)?.try_into().expect("exact u64 width"),
        ))
        .map_err(|_| ReviewOnlyBaselineError::new("baseline length exceeds usize"))
    }

    pub(in crate::review::baseline) fn bytes(
        &mut self,
        maximum: usize,
    ) -> Result<&'a [u8], ReviewOnlyBaselineError> {
        let length = self.usize()?;
        if length > maximum {
            return Err(ReviewOnlyBaselineError::new(
                "review baseline field exceeds its byte ceiling",
            ));
        }
        self.take(length)
    }

    pub(in crate::review::baseline) fn string(
        &mut self,
        maximum: usize,
    ) -> Result<&'a str, ReviewOnlyBaselineError> {
        std::str::from_utf8(self.bytes(maximum)?)
            .map_err(|_| ReviewOnlyBaselineError::new("review baseline string is not UTF-8"))
    }

    pub(in crate::review::baseline) fn array_32(
        &mut self,
    ) -> Result<[u8; 32], ReviewOnlyBaselineError> {
        Ok(self.take(32)?.try_into().expect("exact digest width"))
    }

    pub(in crate::review::baseline) fn finish(self) -> Result<(), ReviewOnlyBaselineError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(ReviewOnlyBaselineError::new(
                "review baseline capsule has trailing bytes",
            ))
        }
    }
}
