//! Exact-add fixtures with proof-certified constant operands.

use super::super::VerifiedPsiOptimizationUnit;

use super::proof_certificates::exact_unsigned_add_certificate;

pub(in crate::pass_manager::tests) fn verified_exact_add_unit() -> VerifiedPsiOptimizationUnit {
    verified_exact_add_unit_with_right(psi_core::IntegerValue::Unsigned(8))
}

pub(in crate::pass_manager::tests) fn verified_exact_add_zero_unit() -> VerifiedPsiOptimizationUnit
{
    verified_exact_add_unit_with_right(psi_core::IntegerValue::Unsigned(0))
}

pub(super) fn verified_exact_add_unit_with_right(
    right_constant: psi_core::IntegerValue,
) -> VerifiedPsiOptimizationUnit {
    use psi_core::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        ObligationId, OperationId, ScalarType, ValueId,
    };
    use psi_terminal::{
        Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
        TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
    };
    use psi_terminal_verifier::{ObligationEvidence, ProofBundle};

    let machine = MachineId::new(411).unwrap();
    let block = BlockId::new(412).unwrap();
    let left = ValueId::new(413).unwrap();
    let right = ValueId::new(414).unwrap();
    let computed = ValueId::new(415).unwrap();
    let result = ValueId::new(422).unwrap();
    let obligation = ObligationId::new(419).unwrap();
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let declaration = |id| ValueDeclaration { id, scalar_type };
    let module = TerminalModule {
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
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            id: machine,
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(result)),
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block,
            blocks: vec![Block {
                id: block,
                parameters: Vec::new(),
                operations: vec![
                    Operation {
                        id: OperationId::new(416).unwrap(),
                        result: OperationResult::Scalar(declaration(left)),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(7),
                        },
                    },
                    Operation {
                        id: OperationId::new(417).unwrap(),
                        result: OperationResult::Scalar(declaration(right)),
                        kind: OperationKind::IntegerConstant {
                            value: right_constant,
                        },
                    },
                    Operation {
                        id: OperationId::new(418).unwrap(),
                        result: OperationResult::Scalar(declaration(computed)),
                        kind: OperationKind::ExactIntegerAdd {
                            left,
                            right,
                            obligation,
                        },
                    },
                ],
                terminator: Terminator::Return {
                    cleanup_actions: Vec::new(),
                    edge: EdgeId::new(420).unwrap(),
                    value: computed,
                },
            }],
            contract: MachineContract {
                id: ContractId::new(421).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let proof = ProofBundle {
        recursive_components: Vec::new(),
        evidence_producers: Vec::new(),
        evidence: vec![ObligationEvidence {
            obligation,
            route: exact_unsigned_add_certificate(
                integer,
                left,
                right,
                IntegerValue::Unsigned(7),
                right_constant,
                0,
                1,
                419,
            ),
        }],
    };
    let semantic = psi_terminal_codec::encode_module(&module).unwrap();
    let proof = psi_terminal_codec::encode_proof_bundle(&proof).unwrap();
    let input = omega_psi_to_abstract_operations::lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    omega_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}
