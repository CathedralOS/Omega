//! Replay the canonical semantic and proof sections of one Terminal artifact
//! under the request admission profile.

use diagnostics::Diagnostic;

pub(crate) fn verify_terminal_artifact(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    profile: &proof_admission::AdmissionProfile,
) -> Result<(), Vec<Diagnostic>> {
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).map_err(|error| {
        vec![Diagnostic::error(format!(
            "terminal-artifact verification could not decode canonical semantics: {error}"
        ))]
    })?;
    // Verification consumes only the subject-sealed proof section: the seal
    // must name the identity reconstructed from this artifact's own semantic
    // section, so a proof sealed for another subject cannot be replayed here.
    let proof = terminal_codec::decode_proof_section_for(&module, artifact.proof_bytes()).map_err(
        |error| {
            vec![Diagnostic::error(format!(
                "terminal-artifact verification could not decode canonical proof: {error}"
            ))]
        },
    )?;
    terminal_verifier::verify_module(&module, &proof, profile)
        .map(|_| ())
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "terminal-artifact verification failed: {error}"
            ))]
        })
}

#[cfg(test)]
mod tests {
    use super::verify_terminal_artifact;
    use semantic_vocabulary::{BlockId, ContractId, EdgeId, MachineId, ObligationId, Proposition};
    use terminal_psi::{
        Block, ContractClause, MachineContract, TerminalMachine, TerminalMachineResult,
        TerminalModule, Terminator, VocabularyMarker,
    };
    use terminal_verifier::ProofBundle;

    #[test]
    fn terminal_product_verification_rejects_a_canonical_unproved_contract() {
        let machine = MachineId::new(900).expect("machine");
        let block = BlockId::new(900).expect("block");
        let obligation = ObligationId::new(900).expect("obligation");
        let module = TerminalModule {
            scalar_qualifications: Default::default(),
            scalar_block_invariants: Vec::new(),
            operation_crash_contracts: Vec::new(),
            vocabulary_marker: VocabularyMarker::CURRENT,
            entry: machine,
            structural_types: Vec::new(),
            structural_domains: Vec::new(),
            services: Vec::new(),
            root_service_reach: Default::default(),
            placed_view_inputs: Vec::new(),
            reborrow_root_handoffs: Vec::new(),
            reborrow_restored_call_uses: Vec::new(),
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            float_meaning_projections: Vec::new(),
            float_meaning_equalities: Vec::new(),
            proposition_declarations: Vec::new(),
            proposition_applications: Vec::new(),
            evidence_terms: Vec::new(),
            evidence_contract_lanes: Vec::new(),
            proof_output_calls: Vec::new(),
            proof_recursive_components: Vec::new(),
            closed_conformance_applications: Vec::new(),
            dynamic_dispatch: Default::default(),
            suspension_call_plan_count: 0,
            suspension_call_sites: Vec::new(),
            suspension_call_plans: Vec::new(),
            quotient_correspondences: Vec::new(),
            machines: vec![TerminalMachine {
                closed_reach_application: None,
                declared_service_reach: Vec::new(),
                id: machine,
                attachment: None,
                structural_parameters: Vec::new(),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                parameters: Vec::new(),
                ranked_scc: None,
                result: TerminalMachineResult::Unit,
                structural_places: Vec::new(),
                content_entry_claims: Vec::new(),
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                entry: block,
                blocks: vec![Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block,
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: EdgeId::new(900).expect("edge"),
                        trivial_affine_discards: Vec::new(),
                    },
                }],
                contract: MachineContract {
                    erased_scalar_formals: Vec::new(),
                    id: ContractId::new(900).expect("contract"),
                    crash_routes: Vec::new(),
                    requires: Vec::new(),
                    ensures: vec![ContractClause {
                        obligation,
                        proposition: Proposition::Truth,
                    }],
                    outcome_specific_ensures: Vec::new(),
                },
            }],
        };
        let proof = ProofBundle::default();
        let optimization =
            terminal_codec::build_identity_optimization_execution_record(&module, &proof)
                .expect("identity optimization execution");
        let artifact = terminal_codec::CanonicalTerminalArtifact::from_parts(
            &module,
            &proof,
            &optimization,
            None,
        )
        .expect("canonical framing does not prove contract evidence");

        let diagnostics =
            verify_terminal_artifact(&artifact, &proof_admission::AdmissionProfile::default())
                .expect_err("Terminal product verification must reconstruct proof obligations");
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("terminal-artifact verification failed: MissingEvidence"),
            "unexpected diagnostic: {}",
            diagnostics[0].message
        );
    }
}
