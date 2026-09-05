use std::fmt;

/// Resource charges incurred by one successful policy recovery. These are
/// accounting values, not evidence of acceptance or compiler validation.
/// Subtract both charges from an enclosing budget before recovering its next
/// child. Storage counts requested allocations, including verification scratch,
/// not allocator overhead or the retained value's exact heap size.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PackagePolicyRecoveryUsage {
    pub(in crate::encoding) owned_bytes: usize,
    pub(in crate::encoding) sequence_elements: usize,
}

impl PackagePolicyRecoveryUsage {
    pub const fn owned_bytes(self) -> usize {
        self.owned_bytes
    }

    /// Aggregate sequence entries and recursive expression, static-argument,
    /// and machine-contract entries charged by the typed reader.
    pub const fn sequence_elements(self) -> usize {
        self.sequence_elements
    }
}

/// Independent ceilings for untrusted policy input. Raised caller ceilings
/// are clamped to these format limits; lowering any ceiling is supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackagePolicyRecoveryLimits {
    pub(in crate::encoding) maximum_bytes: usize,
    pub(in crate::encoding) maximum_field_bytes: usize,
    pub(in crate::encoding) maximum_sequence_elements: usize,
    pub(in crate::encoding) maximum_owned_bytes: usize,
    pub(in crate::encoding) maximum_depth: usize,
}

impl PackagePolicyRecoveryLimits {
    /// `maximum_sequence_elements` counts aggregate list elements and recursive
    /// expression, static-argument, and machine-contract entries. Owned storage
    /// counts requested vector/string/box allocations and canonical comparison
    /// scratch, not allocator overhead.
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
        Self::new(
            self.maximum_bytes.min(4 * 1024 * 1024),
            self.maximum_field_bytes.min(4 * 1024 * 1024),
            self.maximum_sequence_elements.min(65_536),
            self.maximum_owned_bytes.min(64 * 1024 * 1024),
            self.maximum_depth.min(128),
        )
    }
}

impl Default for PackagePolicyRecoveryLimits {
    fn default() -> Self {
        Self::new(
            4 * 1024 * 1024,
            4 * 1024 * 1024,
            65_536,
            64 * 1024 * 1024,
            128,
        )
    }
}

/// Fixed diagnostics do not echo untrusted record fields.
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
