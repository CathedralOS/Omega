//! Optional invariant inference must preserve already supplied source evidence.

use proof_admission::{AdmissionProfile, verify_obligation_with_machine_parameters};
use terminal_psi::{ProofBundle, TerminalModule};
use terminal_verifier::ReconstructedTerminalObligationSet;

use crate::LoweringError;

pub(super) fn replays(
    module: &TerminalModule,
    original: &ReconstructedTerminalObligationSet,
    proposed: &ReconstructedTerminalObligationSet,
    proofs: &ProofBundle,
) -> Result<bool, LoweringError> {
    let validated =
        terminal_verifier::validate_module(module).map_err(LoweringError::InvalidTerminalModule)?;
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
        if verify_obligation_with_machine_parameters(
            &context,
            &after.obligation,
            &after.requirements,
            &after.semantic_axioms,
            &parameters,
            proof.route.clone(),
            &AdmissionProfile::default(),
        )
        .is_err()
        {
            return Ok(false);
        }
    }
    Ok(true)
}
