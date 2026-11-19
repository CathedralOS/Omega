// Named-text recovery restoring bounded typed policy meaning.
// `PackagePolicyTextRecoveryLimits` pairs a text-expansion byte ceiling with the underlying policy owned-storage budget (shared by reconstructed binary storage, typed allocations, and canonical verification scratch while input and text verification stay borrowed).

pub(in crate::encoding) mod framing;
pub(in crate::encoding::recovery) mod tokens;

use crate::encoding::encode::encoder::text::{MAXIMUM_TEXT_BYTES, Writer, render};
use crate::encoding::{
    PackagePolicyRecoveryError, PackagePolicyRecoveryLimits, PackagePolicyRecoveryUsage,
};
use crate::record::PackagePolicyBaseline;

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
