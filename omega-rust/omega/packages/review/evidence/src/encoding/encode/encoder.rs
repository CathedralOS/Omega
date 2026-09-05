use super::PackageReviewEncodingError;
use psi_core::PackageKeyIdentity;

mod membership;
mod scalars;
pub(in crate::encoding) mod text;

pub(crate) struct Encoder<'text> {
    output: Vec<u8>,
    text: Option<text::Writer<'text>>,
    membership: Option<&'text mut dyn super::membership::Observer>,
    membership_error: Option<crate::encoding::PackagePolicyMembershipError>,
    encoded_bytes: usize,
    maximum_bytes: usize,
    exceeded: bool,
    policy_elements: Option<usize>,
    policy_depth: usize,
    maximum_policy_depth: usize,
}

impl<'text> Encoder<'text> {
    pub(crate) fn bounded(maximum_bytes: usize) -> Self {
        Self {
            output: Vec::new(),
            text: None,
            membership: None,
            membership_error: None,
            encoded_bytes: 0,
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

    /// Recovery has already charged this exact verification buffer. Reserve it
    /// once, and cap every later append at that length so no growth can occur.
    pub(super) fn policy_preallocated(
        expected_length: usize,
    ) -> Result<Self, PackageReviewEncodingError> {
        let mut encoder = Self::policy_bounded(expected_length);
        if expected_length > encoder.maximum_bytes {
            return Err(PackageReviewEncodingError::new(
                "package policy verification exceeds its byte ceiling",
            ));
        }
        encoder
            .output
            .try_reserve_exact(expected_length)
            .map_err(|_| {
                PackageReviewEncodingError::new("package policy verification allocation failed")
            })?;
        Ok(encoder)
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
        if self.text.is_some() {
            return Err(PackageReviewEncodingError::new(
                "text encoder requires text completion",
            ));
        }
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
        let Some(required) = self.encoded_bytes.checked_add(bytes.len()) else {
            self.exceeded = true;
            return;
        };
        if required > self.maximum_bytes {
            self.exceeded = true;
            return;
        }
        self.encoded_bytes = required;
        if self.text.is_some() || self.membership.is_some() {
            return;
        }
        if self.output.try_reserve(bytes.len()).is_err() {
            self.exceeded = true;
            return;
        }
        self.output.extend_from_slice(bytes);
    }

    pub(crate) fn sequence<T>(
        &mut self,
        values: &[T],
        encode_value: impl Fn(&mut Self, &T) -> Result<(), PackageReviewEncodingError>,
    ) -> Result<(), PackageReviewEncodingError> {
        self.policy_elements(values.len())?;
        let count = u64::try_from(values.len()).map_err(|_| {
            PackageReviewEncodingError::new("package policy sequence count overflows")
        })?;
        self.append(&count.to_le_bytes());
        if let Some(text) = &mut self.text {
            text.sequence(count);
        }
        for value in values {
            if let Some(text) = &mut self.text {
                text.item();
            }
            let result = encode_value(self, value);
            if let Some(text) = &mut self.text {
                text.end();
            }
            result?;
        }
        if let Some(text) = &mut self.text {
            text.end();
        }
        self.check()
    }

    pub(crate) fn option<T: ?Sized>(
        &mut self,
        value: Option<&T>,
        encode_value: impl Fn(&mut Self, &T) -> Result<(), PackageReviewEncodingError>,
    ) -> Result<(), PackageReviewEncodingError> {
        self.append(&[u8::from(value.is_some())]);
        if let Some(text) = &mut self.text {
            text.option(value.is_some());
        }
        match value {
            None => {}
            Some(value) => {
                let result = encode_value(self, value);
                if let Some(text) = &mut self.text {
                    text.end();
                }
                result?;
            }
        }
        self.check()
    }

    pub(crate) fn package_identity(&mut self, identity: PackageKeyIdentity) {
        if let Some(observer) = &mut self.membership {
            let result = observer.package(identity);
            self.record_membership_result(result);
        }
        self.fixed_bytes(&identity.digest());
    }

    pub(crate) fn optional_package_identity(&mut self, identity: Option<PackageKeyIdentity>) {
        let _ = self.option(identity.as_ref(), |encoder, identity| {
            encoder.package_identity(*identity);
            Ok(())
        });
    }

    pub(crate) fn check(&self) -> Result<(), PackageReviewEncodingError> {
        if self.membership_error.is_some() {
            return Err(PackageReviewEncodingError::new(
                "package policy membership validation failed",
            ));
        }
        if let Some(text) = &self.text {
            text.check()?;
        }
        if self.exceeded {
            Err(PackageReviewEncodingError::new(
                "package review exceeds its canonical encoding byte ceiling",
            ))
        } else {
            Ok(())
        }
    }
}
