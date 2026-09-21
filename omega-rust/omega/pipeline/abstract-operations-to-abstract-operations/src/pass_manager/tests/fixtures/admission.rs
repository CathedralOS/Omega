//! Shared verified-artifact admission for entrance-level fixtures.

use super::super::VerifiedPsiOptimizationUnit;
use terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput;

/// Encode a Terminal module and its proof section, admit the artifact through
/// the real optimizer boundary, and return the verified input carrier.
pub(in crate::pass_manager::tests) fn verified_input(
    module: &terminal_psi::TerminalModule,
    proof: &terminal_verifier::ProofBundle,
) -> VerifiedPsiOptimizationInput {
    let semantic = terminal_codec::encode_module(module).unwrap();
    let proof = terminal_codec::encode_proof_section(module, proof).unwrap();
    terminal_psi_to_abstract_operations::lower_artifact(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .map(|admitted| {
        admitted
            .into_optimization_artifact()
            .into_optimization_input()
    })
    .unwrap()
}

/// The same admission plus the canonical unit build, matching every other
/// verified fixture in this suite.
pub(in crate::pass_manager::tests) fn verified_unit(
    module: &terminal_psi::TerminalModule,
    proof: &terminal_verifier::ProofBundle,
) -> VerifiedPsiOptimizationUnit {
    terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        verified_input(module, proof),
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}
