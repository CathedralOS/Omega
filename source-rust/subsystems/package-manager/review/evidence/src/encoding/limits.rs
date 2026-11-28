//! Canonical policy encoding and recovery share the same format ceilings.

/// Recovery clamps each caller-supplied ceiling to its format default. Input,
/// field and owned storage each allow up to 64 MiB so retained package
/// policies can carry large semantic dependency lists.
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

    #[cfg(any(test, feature = "full-policy-recovery"))]
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
