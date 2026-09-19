//! Verified unit with one unranked cyclic component.
//!
//! The machine is an eligible unranked self-loop: a unique-entry preheader
//! feeds a header whose conditional either re-enters itself with the
//! loop-carried Boolean parameter or exits to a `ReturnUnit` tail. The header
//! owns one dead integer constant so scalar-leaf consumers have an admissible
//! relocation target. `ranked_scc` stays `None`, so the component enters the
//! validated Terminal SCC roster without any countdown certificate.

use super::super::VerifiedPsiOptimizationUnit;

pub(in crate::pass_manager::tests) fn verified_unranked_cycle_unit() -> VerifiedPsiOptimizationUnit
{
    use semantic_vocabulary::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
        OperationId, ScalarType, ValueId,
    };
    use terminal_psi::{
        Block, MachineContract, Operation, OperationKind, OperationResult, SuccessorEdge,
        TerminalMachine, TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
        VocabularyMarker,
    };

    let value = |raw| ValueId::new(raw).unwrap();
    let edge = |raw| EdgeId::new(raw).unwrap();
    let block = |raw| BlockId::new(raw).unwrap();
    let successor = |edge, target, arguments| SuccessorEdge {
        erased_arguments: Vec::new(),
        edge,
        target,
        arguments,
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    let unsigned_64 = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();

    let module = TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(501).unwrap(),
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
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(501).unwrap(),
            attachment: None,
            parameters: vec![ValueDeclaration {
                qualifications: Default::default(),
                id: value(502),
                scalar_type: ScalarType::Boolean,
            }],
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: block(503),
            blocks: vec![
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block(503),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Jump {
                        erased_arguments: Vec::new(),
                        edge: edge(504),
                        target: block(505),
                        arguments: vec![value(502)],
                        structural_arguments: Vec::new(),
                        trivial_affine_discards: Vec::new(),
                        residual_affine_discards: Vec::new(),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block(505),
                    parameters: vec![ValueDeclaration {
                        qualifications: Default::default(),
                        id: value(506),
                        scalar_type: ScalarType::Boolean,
                    }],
                    operations: vec![Operation {
                        static_reach_binding: None,
                        id: OperationId::new(507).unwrap(),
                        result: OperationResult::Scalar(ValueDeclaration {
                            qualifications: Default::default(),
                            id: value(508),
                            scalar_type: ScalarType::Integer(unsigned_64),
                        }),
                        kind: OperationKind::IntegerConstant {
                            value: IntegerValue::Unsigned(7),
                        },
                    }],
                    terminator: Terminator::Conditional {
                        condition: value(506),
                        when_true: successor(edge(509), block(505), vec![value(506)]),
                        when_false: successor(edge(510), block(511), Vec::new()),
                    },
                },
                Block {
                    erased_scalar_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    id: block(511),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::ReturnUnit {
                        edge: edge(512),
                        trivial_affine_discards: Vec::new(),
                    },
                },
            ],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(513).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    };
    let semantic = terminal_codec::encode_module(&module).unwrap();
    let proof =
        terminal_codec::encode_proof_section(&module, &terminal_verifier::ProofBundle::default())
            .unwrap();
    let input = terminal_psi_to_abstract_operations::lower_artifact_for_optimization(
        terminal_psi_to_abstract_operations::ArtifactSections {
            semantic_bytes: &semantic,
            proof_bytes: &proof,
            obligation_ledger_bytes: None,
        },
        &proof_admission::AdmissionProfile::default(),
    )
    .and_then(|admitted| admitted.try_into_optimization_input())
    .unwrap();
    terminal_psi_to_abstract_operations::build_verified_psi_optimization_unit(
        input,
        terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .unwrap()
}
