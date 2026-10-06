//! Optional proof-fact inference must preserve already supplied source evidence.

use terminal_psi::ProofBundle;
use terminal_verifier::{ReconstructedTerminalObligationSet, ValidatedTerminalModule};

use crate::lowering_error::LoweringError;

/// `validated` covers the same module state `proposed` was reconstructed
/// from; its callers already hold it, so this check does not repeat
/// whole-module validation.
pub(super) fn replays(
    validated: ValidatedTerminalModule<'_>,
    original: &ReconstructedTerminalObligationSet,
    proposed: &ReconstructedTerminalObligationSet,
    proofs: &ProofBundle,
) -> Result<bool, LoweringError> {
    for proof in &proofs.evidence {
        let Some(after) = proposed
            .obligations()
            .iter()
            .find(|site| site.obligation.id == proof.obligation)
        else {
            continue;
        };
        if original.obligations().iter().any(|before| before == after) {
            continue;
        }
        // A checker decision answers the goal it judged. New premises leave it
        // valid. A changed goal (inference conjoining a member into an authored
        // arrival contract) is a question the checker never saw: the pass may
        // still keep the proposal, and `retain_provable_bounded` strips the
        // decision afterwards so certificate search proves the extended goal.
        if proof.route == proof_admission::EvidenceRoute::SourceChecked {
            continue;
        }
        let machine =
            validated
                .machine(after.owner.machine())
                .ok_or(LoweringError::Unsupported(
                    "retained proof owner disappeared",
                ))?;
        let context = validated
            .value_context(machine)
            .map_err(LoweringError::InvalidTerminalModule)?;
        let parameters = machine
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect();
        if terminal_verifier::verify_reconstructed_obligation(
            &context,
            after,
            &parameters,
            proof.route.clone(),
        )
        .is_err()
        {
            return Ok(false);
        }
    }
    Ok(true)
}
