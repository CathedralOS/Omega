//! Named-text recovery restores the existing bounded typed policy meaning.

pub(in crate::encoding) mod framing;
#[cfg(test)]
mod tests;
mod tokens;

use crate::encoding::encode::encoder::text::{MAXIMUM_TEXT_BYTES, Writer, render};
use crate::encoding::{
    PackagePolicyRecoveryError, PackagePolicyRecoveryLimits, PackagePolicyRecoveryUsage,
};
use crate::record::PackagePolicyBaseline;

/// Text expansion has its own byte ceiling. All reconstructed binary storage,
/// typed recovery allocations, and canonical verification scratch share the
/// underlying policy owned-storage budget. Input text remains borrowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackagePolicyTextRecoveryLimits {
    maximum_text_bytes: usize,
    policy: PackagePolicyRecoveryLimits,
}

impl PackagePolicyTextRecoveryLimits {
    pub const fn new(maximum_text_bytes: usize, policy: PackagePolicyRecoveryLimits) -> Self {
        Self {
            maximum_text_bytes,
            policy,
        }
    }
}

impl Default for PackagePolicyTextRecoveryLimits {
    fn default() -> Self {
        Self::new(MAXIMUM_TEXT_BYTES, PackagePolicyRecoveryLimits::default())
    }
}

impl PackagePolicyBaseline {
    /// Recover without source or compiler execution. Every field and variant
    /// label must match the canonical typed rerender, not just the scalar bytes.
    pub fn recover_text(
        text: &str,
        limits: PackagePolicyTextRecoveryLimits,
    ) -> Result<Self, PackagePolicyRecoveryError> {
        Self::recover_text_with_usage(text, limits).map(|(policy, _)| policy)
    }

    /// Recover the same inert meaning and report all charged storage and typed
    /// elements so an enclosing document can lower its next child's ceilings.
    /// Storage includes the requested reconstructed-binary reserve, typed data,
    /// and canonical binary scratch. Input and text verification stay borrowed.
    pub fn recover_text_with_usage(
        text: &str,
        limits: PackagePolicyTextRecoveryLimits,
    ) -> Result<(Self, PackagePolicyRecoveryUsage), PackagePolicyRecoveryError> {
        let maximum = limits.maximum_text_bytes.min(MAXIMUM_TEXT_BYTES);
        if text.len() > maximum {
            return Err(PackagePolicyRecoveryError::InputTooLarge);
        }
        let mut policy_limits = limits.policy.bounded();
        let (binary, reserved) = framing::binary(text, policy_limits)?;
        policy_limits.maximum_owned_bytes = policy_limits
            .maximum_owned_bytes
            .checked_sub(reserved)
            .ok_or(PackagePolicyRecoveryError::AllocationLimitExceeded)?;
        let (policy, mut usage) = Self::recover_canonical_with_usage(&binary, policy_limits)?;
        render(&policy, Writer::new(maximum, Some(text)))
            .map_err(|_| PackagePolicyRecoveryError::NonCanonicalEncoding)?;
        usage.owned_bytes = usage
            .owned_bytes
            .checked_add(reserved)
            .ok_or(PackagePolicyRecoveryError::LengthOverflow)?;
        Ok((policy, usage))
    }
}
