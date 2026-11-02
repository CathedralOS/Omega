//! Claim names for parameters that retain invocation storage.
//!
//! Checking owns move legality and publishes each state's entry claims and
//! successor transfers. Ordinary block parameters bind destination-local claim
//! names to the obligations of their arriving values; different arms may carry
//! different original identities. Lowering consumes those checked transfers,
//! rather than searching permission events to authenticate them a second time.
//!
//! A whole parameter proven to keep invocation storage has no block argument.
//! Only these physical aliases need their destination claim names bound back to
//! the invocation claims. Matching the retained typed paths supplies that
//! emission namespace; it neither creates claims nor decides move legality.
use super::super::LoweringError;
use super::CheckedComposedUnitControlMachinePlan;
use language_semantics::PermissionClaimIdentity;

/// Invocation-storage aliases. All other state entries own binding names.
pub(super) struct ClaimTransport {
    pub(super) aliases: Vec<(PermissionClaimIdentity, PermissionClaimIdentity)>,
}

pub(super) fn bind_invocation_claims(
    plan: &CheckedComposedUnitControlMachinePlan,
    live: &[bool],
    entry_aliases: &[std::collections::BTreeMap<u32, u32>],
) -> Result<ClaimTransport, LoweringError> {
    let mut aliases = Vec::new();
    for (position, state) in plan.states.iter().enumerate() {
        if !live[position] {
            continue;
        }
        for claim in &state.entry_claims {
            let Some(root_parameter) = entry_aliases
                .get(position)
                .and_then(|parameters| parameters.get(&claim.parameter_index))
            else {
                continue;
            };
            let root = plan.states[0]
                .entry_claims
                .iter()
                .find(|entry| entry.parameter_index == *root_parameter && entry.path == claim.path)
                .ok_or(LoweringError::Unsupported(
                    "Unit graph invocation alias has no matching claim path",
                ))?;
            if claim.claim_identity != root.claim_identity {
                aliases.push((claim.claim_identity, root.claim_identity));
            }
        }
    }
    Ok(ClaimTransport { aliases })
}
