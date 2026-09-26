use super::{Encoder, PackageReviewEncodingError};

impl Encoder<'_> {
    pub(crate) fn fixed_bytes(&mut self, value: &[u8]) {
        if let Some(text) = &mut self.text {
            text.bytes("fixed", value);
        }
        self.append(value);
    }

    pub(crate) fn byte(&mut self, value: u8) {
        if let Some(text) = &mut self.text {
            text.number("u8", value);
        }
        self.append(&[value]);
    }

    pub(crate) fn tag(&mut self, name: &'static str, value: u8) {
        if let Some(text) = &mut self.text {
            text.tag(name, value);
        }
        self.append(&[value]);
    }

    pub(crate) fn boolean(&mut self, value: bool) {
        if let Some(text) = &mut self.text {
            text.boolean(value);
        }
        self.append(&[u8::from(value)]);
    }

    pub(crate) fn u16(&mut self, value: u16) {
        if let Some(text) = &mut self.text {
            text.number("u16", value);
        }
        self.append(&value.to_le_bytes());
    }

    pub(crate) fn u32(&mut self, value: u32) {
        if let Some(text) = &mut self.text {
            text.number("u32", value);
        }
        self.append(&value.to_le_bytes());
    }

    pub(crate) fn u64(&mut self, value: u64) {
        if let Some(text) = &mut self.text {
            text.number("u64", value);
        }
        self.append(&value.to_le_bytes());
    }

    pub(crate) fn i64(&mut self, value: i64) {
        if let Some(text) = &mut self.text {
            text.number("i64", value);
        }
        self.append(&value.to_le_bytes());
    }

    pub(crate) fn i128(&mut self, value: i128) {
        if let Some(text) = &mut self.text {
            text.number("i128", value);
        }
        self.append(&value.to_le_bytes());
    }

    pub(crate) fn u128(&mut self, value: u128) {
        if let Some(text) = &mut self.text {
            text.number("u128", value);
        }
        self.append(&value.to_le_bytes());
    }

    pub(crate) fn usize(&mut self, value: usize) -> Result<(), PackageReviewEncodingError> {
        self.u64(u64::try_from(value).map_err(|_| {
            PackageReviewEncodingError::new(
                "package review value exceeds the portable encoding range",
            )
        })?);
        self.check()
    }

    pub(crate) fn bytes(&mut self, value: &[u8]) -> Result<(), PackageReviewEncodingError> {
        self.byte_string("bytes", value)
    }

    pub(crate) fn string(&mut self, value: &str) -> Result<(), PackageReviewEncodingError> {
        self.byte_string("string", value.as_bytes())
    }

    fn byte_string(
        &mut self,
        kind: &'static str,
        value: &[u8],
    ) -> Result<(), PackageReviewEncodingError> {
        let count = u64::try_from(value.len()).map_err(|_| {
            PackageReviewEncodingError::new(
                "package review value exceeds the portable encoding range",
            )
        })?;
        if let Some(text) = &mut self.text {
            text.bytes(kind, value);
        }
        self.append(&count.to_le_bytes());
        self.append(value);
        self.check()
    }
}
