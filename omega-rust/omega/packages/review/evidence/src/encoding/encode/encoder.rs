use super::PackageReviewEncodingError;
use psi_core::PackageKeyIdentity;

pub(crate) struct Encoder {
    output: Vec<u8>,
    maximum_bytes: usize,
    exceeded: bool,
    policy_elements: Option<usize>,
    policy_depth: usize,
    maximum_policy_depth: usize,
}

impl Encoder {
    pub(crate) fn bounded(maximum_bytes: usize) -> Self {
        Self {
            output: Vec::new(),
            maximum_bytes,
            exceeded: false,
            policy_elements: None,
            policy_depth: 0,
            maximum_policy_depth: 0,
        }
    }

    pub(crate) fn policy_bounded(maximum_bytes: usize) -> Self {
        let limits = crate::encoding::PackagePolicyRecoveryLimits::default();
        Self {
            policy_elements: Some(limits.maximum_sequence_elements),
            maximum_policy_depth: limits.maximum_depth,
            ..Self::bounded(maximum_bytes.min(limits.maximum_bytes))
        }
    }

    /// Only policy components use these structural limits. Ordinary review
    /// serialization keeps its existing behavior and byte representation.
    pub(crate) fn nested<T>(
        &mut self,
        encode: impl FnOnce(&mut Self) -> Result<T, PackageReviewEncodingError>,
    ) -> Result<T, PackageReviewEncodingError> {
        if self.policy_elements.is_none() {
            return encode(self);
        }
        self.policy_elements(1)?;
        if self.policy_depth >= self.maximum_policy_depth {
            return Err(PackageReviewEncodingError::new(
                "package policy exceeds its nesting ceiling",
            ));
        }
        self.policy_depth += 1;
        let result = encode(self);
        self.policy_depth -= 1;
        result
    }

    fn policy_elements(&mut self, count: usize) -> Result<(), PackageReviewEncodingError> {
        if let Some(remaining) = self.policy_elements {
            self.policy_elements = Some(remaining.checked_sub(count).ok_or_else(|| {
                PackageReviewEncodingError::new(
                    "package policy exceeds its aggregate element ceiling",
                )
            })?);
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<Vec<u8>, PackageReviewEncodingError> {
        if self.exceeded {
            Err(PackageReviewEncodingError::new(
                "package review exceeds its canonical encoding byte ceiling",
            ))
        } else {
            Ok(self.output)
        }
    }

    pub(crate) fn append(&mut self, bytes: &[u8]) {
        if self.exceeded {
            return;
        }
        let Some(required) = self.output.len().checked_add(bytes.len()) else {
            self.exceeded = true;
            return;
        };
        if required > self.maximum_bytes || self.output.try_reserve(bytes.len()).is_err() {
            self.exceeded = true;
            return;
        }
        self.output.extend_from_slice(bytes);
    }

    pub(crate) fn fixed_bytes(&mut self, value: &[u8]) {
        self.append(value);
    }

    pub(crate) fn byte(&mut self, value: u8) {
        self.append(&[value]);
    }

    pub(crate) fn boolean(&mut self, value: bool) {
        self.byte(u8::from(value));
    }

    pub(crate) fn u16(&mut self, value: u16) {
        self.append(&value.to_le_bytes());
    }

    pub(crate) fn u32(&mut self, value: u32) {
        self.append(&value.to_le_bytes());
    }

    pub(crate) fn u64(&mut self, value: u64) {
        self.append(&value.to_le_bytes());
    }

    pub(crate) fn i64(&mut self, value: i64) {
        self.append(&value.to_le_bytes());
    }

    pub(crate) fn i128(&mut self, value: i128) {
        self.append(&value.to_le_bytes());
    }

    pub(crate) fn u128(&mut self, value: u128) {
        self.append(&value.to_le_bytes());
    }

    pub(crate) fn usize(&mut self, value: usize) -> Result<(), PackageReviewEncodingError> {
        self.u64(u64::try_from(value).map_err(|_| {
            PackageReviewEncodingError::new(
                "package review value exceeds the portable encoding range",
            )
        })?);
        Ok(())
    }

    pub(crate) fn bytes(&mut self, value: &[u8]) -> Result<(), PackageReviewEncodingError> {
        self.usize(value.len())?;
        self.append(value);
        self.check()
    }

    pub(crate) fn string(&mut self, value: &str) -> Result<(), PackageReviewEncodingError> {
        self.bytes(value.as_bytes())
    }

    pub(crate) fn sequence<T>(
        &mut self,
        values: &[T],
        encode_value: impl Fn(&mut Self, &T) -> Result<(), PackageReviewEncodingError>,
    ) -> Result<(), PackageReviewEncodingError> {
        self.policy_elements(values.len())?;
        self.usize(values.len())?;
        for value in values {
            encode_value(self, value)?;
        }
        Ok(())
    }

    pub(crate) fn option<T: ?Sized>(
        &mut self,
        value: Option<&T>,
        encode_value: impl Fn(&mut Self, &T) -> Result<(), PackageReviewEncodingError>,
    ) -> Result<(), PackageReviewEncodingError> {
        match value {
            None => self.byte(0),
            Some(value) => {
                self.byte(1);
                encode_value(self, value)?;
            }
        }
        Ok(())
    }

    pub(crate) fn package_identity(&mut self, identity: PackageKeyIdentity) {
        self.append(&identity.digest());
    }

    pub(crate) fn optional_package_identity(&mut self, identity: Option<PackageKeyIdentity>) {
        match identity {
            None => self.byte(0),
            Some(identity) => {
                self.byte(1);
                self.package_identity(identity);
            }
        }
    }

    pub(crate) fn check(&self) -> Result<(), PackageReviewEncodingError> {
        if self.exceeded {
            Err(PackageReviewEncodingError::new(
                "package review exceeds its canonical encoding byte ceiling",
            ))
        } else {
            Ok(())
        }
    }
}
