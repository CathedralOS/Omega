/*
Resource accounting, limits, and fixed diagnostics for untrusted policy recovery.

`PackagePolicyRecoveryUsage` tracks resource charges incurred by one successful
policy recovery (requested owned allocations, verification scratch, and
aggregate sequence/expression entries), not evidence of acceptance or compiler
validation; callers subtract both charges from an enclosing budget before
recovering its next child. `PackagePolicyRecoveryLimits` defines independent
ceilings clamped to format defaults (64 MiB for input, field, and owned bytes
so large per-package `semantic_dependencies` products fit within the candidate
pipeline's retained policy budget). `PackagePolicyRecoveryError` uses fixed
diagnostics that never echo untrusted record fields.
*/

use std::fmt;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PackagePolicyRecoveryUsage {
    pub(in crate::encoding) owned_bytes: usize,
    pub(in crate::encoding) sequence_elements: usize,
}

impl PackagePolicyRecoveryUsage {
    pub const fn owned_bytes(self) -> usize {
        self.owned_bytes
    }

    pub const fn sequence_elements(self) -> usize {
        self.sequence_elements
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackagePolicyRecoveryLimits {
    pub(in crate::encoding) maximum_bytes: usize,
    pub(in crate::encoding) maximum_field_bytes: usize,
    pub(in crate::encoding) maximum_sequence_elements: usize,
    pub(in crate::encoding) maximum_owned_bytes: usize,
    pub(in crate::encoding) maximum_depth: usize,
}

impl PackagePolicyRecoveryLimits {
    pub const fn new(
        maximum_bytes: usize,
        maximum_field_bytes: usize,
        maximum_sequence_elements: usize,
        maximum_owned_bytes: usize,
        maximum_depth: usize,
    ) -> Self {
        Self {
            maximum_bytes,
            maximum_field_bytes,
            maximum_sequence_elements,
            maximum_owned_bytes,
            maximum_depth,
        }
    }

    pub(in crate::encoding) fn bounded(self) -> Self {
        let default = Self::default();
        Self::new(
            self.maximum_bytes.min(default.maximum_bytes),
            self.maximum_field_bytes.min(default.maximum_field_bytes),
            self.maximum_sequence_elements
                .min(default.maximum_sequence_elements),
            self.maximum_owned_bytes.min(default.maximum_owned_bytes),
            self.maximum_depth.min(default.maximum_depth),
        )
    }
}

impl Default for PackagePolicyRecoveryLimits {
    fn default() -> Self {
        Self::new(
            64 * 1024 * 1024,
            64 * 1024 * 1024,
            65_536,
            64 * 1024 * 1024,
            128,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackagePolicyRecoveryError {
    InputTooLarge,
    FieldTooLarge,
    ElementLimitExceeded,
    AllocationLimitExceeded,
    NestingLimitExceeded,
    AllocationFailed,
    UnexpectedEnd,
    InvalidTag,
    RetiredVocabulary, // Closed-vocabulary tag retired without reuse in the current schema.
    InvalidUtf8,
    InvalidIdentity,
    InvalidValue,
    LengthOverflow,
    UnsupportedVersion,
    TrailingBytes,
    NonCanonicalEncoding,
}

impl fmt::Display for PackagePolicyRecoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InputTooLarge => "package policy exceeds the input-byte limit",
            Self::FieldTooLarge => "package policy exceeds the field-byte limit",
            Self::ElementLimitExceeded => "package policy exceeds the aggregate element limit",
            Self::AllocationLimitExceeded => "package policy exceeds the owned-storage limit",
            Self::NestingLimitExceeded => "package policy exceeds the nesting limit",
            Self::AllocationFailed => "package policy allocation failed",
            Self::UnexpectedEnd => "package policy ends before its declared fields",
            Self::InvalidTag => "package policy contains an unknown closed-vocabulary tag",
            Self::RetiredVocabulary => "package policy contains a retired closed-vocabulary tag; recapture the review with the current toolchain",
            Self::InvalidUtf8 => "package policy contains an invalid UTF-8 text field",
            Self::InvalidIdentity => "package policy contains an invalid nominal owner",
            Self::InvalidValue => "package policy contains an invalid field value",
            Self::LengthOverflow => "package policy contains an unrepresentable length",
            Self::UnsupportedVersion => "unsupported package policy version; preserve source pins and recover with a compatible toolchain",
            Self::TrailingBytes => "package policy has trailing fields",
            Self::NonCanonicalEncoding => "package policy is not canonically encoded",
        })
    }
}

impl std::error::Error for PackagePolicyRecoveryError {}
