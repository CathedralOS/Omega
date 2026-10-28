//! Canonical byte framing, hexadecimal conversion, and subject fingerprinting.

use super::super::SOURCE_CLOSURE_SUBJECT_FINGERPRINT_DOMAIN;
use super::super::usage::Budget;
use super::super::{CanonicalSourceClosureSubjectError, CanonicalSourceClosureSubjectFingerprint};

pub(in super::super) fn fingerprint(bytes: &[u8]) -> CanonicalSourceClosureSubjectFingerprint {
    let mut digest =
        crate::digest_encoder::DomainDigestEncoder::new(SOURCE_CLOSURE_SUBJECT_FINGERPRINT_DOMAIN);
    digest.bytes(bytes);
    CanonicalSourceClosureSubjectFingerprint(digest.finish())
}

pub(crate) fn encode_hex(bytes: &[u8]) -> String {
    crate::Hex(bytes).to_string()
}

pub(super) struct Encoder<'budget> {
    bytes: Vec<u8>,
    budget: &'budget mut Budget,
    maximum: usize,
    reserved: usize,
    failed: bool,
}

impl<'budget> Encoder<'budget> {
    pub(super) fn new(budget: &'budget mut Budget, maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            budget,
            maximum,
            reserved: 0,
            failed: false,
        }
    }

    pub(super) fn scratch(
        &mut self,
        bytes: usize,
    ) -> Result<(), CanonicalSourceClosureSubjectError> {
        self.budget.charge(bytes)
    }

    pub(super) fn check(&self) -> Result<(), CanonicalSourceClosureSubjectError> {
        if self.failed {
            Err(CanonicalSourceClosureSubjectError::new(
                "source-closure encoding exceeds its allocation or record-byte limit",
            ))
        } else {
            Ok(())
        }
    }

    pub(super) fn fixed(&mut self, bytes: &[u8]) {
        if self.failed {
            return;
        }
        let Some(required) = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .filter(|length| *length <= self.maximum)
        else {
            self.failed = true;
            return;
        };
        if required > self.reserved {
            let capacity = self
                .reserved
                .saturating_mul(2)
                .max(required)
                .min(self.maximum);
            if self.scratch(capacity - self.reserved).is_err()
                || self
                    .bytes
                    .try_reserve_exact(capacity - self.bytes.len())
                    .is_err()
            {
                self.failed = true;
                return;
            }
            self.reserved = capacity;
        }
        self.bytes.extend_from_slice(bytes);
    }

    pub(super) fn byte(&mut self, value: u8) {
        self.fixed(&[value]);
    }

    pub(super) fn u16(&mut self, value: u16) {
        self.fixed(&value.to_le_bytes());
    }

    pub(super) fn u32(&mut self, value: u32) {
        self.fixed(&value.to_le_bytes());
    }

    pub(super) fn count(&mut self, value: usize) -> Result<(), CanonicalSourceClosureSubjectError> {
        self.u32(u32::try_from(value).map_err(|_| {
            CanonicalSourceClosureSubjectError::new("canonical sequence count exceeds u32")
        })?);
        self.check()
    }

    pub(super) fn bytes_bounded(
        &mut self,
        value: &[u8],
        maximum_bytes: usize,
    ) -> Result<(), CanonicalSourceClosureSubjectError> {
        if value.len() > maximum_bytes {
            return Err(CanonicalSourceClosureSubjectError::new(
                "canonical field exceeds its byte limit",
            ));
        }
        self.count(value.len())?;
        self.fixed(value);
        Ok(())
    }

    pub(super) fn finish(self) -> Vec<u8> {
        self.bytes
    }
}
