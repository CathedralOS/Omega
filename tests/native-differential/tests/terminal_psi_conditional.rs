use proof_admission::AdmissionProfile;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, ScalarType,
    ValueId,
};
use terminal_fixed_fuel::{derive_fixed_entry_fuel, derive_fixed_safe_point_segments};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, SuccessorEdge, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};
use terminal_verifier::{ProofBundle, verify_module};

#[test]
fn conditional_fixed_bound_uses_the_maximum_path_not_the_sum() {
    let mut module = conditional_module(VocabularyMarker::CURRENT);
    module.machines[0].blocks[1].operations.push(Operation {
        id: OperationId::new(1).unwrap(),
        result: terminal_psi::OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(7).unwrap(),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanConstant { value: true },
    });
    let verified = verify_module(
        &module,
        &ProofBundle::default(),
        &AdmissionProfile::default(),
    )
    .expect("unequal acyclic branch costs verify");

    let fixed = derive_fixed_entry_fuel(&verified, MachineId::new(1).unwrap())
        .expect("maximum branch cost derives");
    assert_eq!(fixed.ceiling_units(), 3);
    let segments = derive_fixed_safe_point_segments(&verified, MachineId::new(1).unwrap())
        .expect("unequal branch segments derive");
    assert_eq!(segments[2].ceiling_units(), 2);
    assert_eq!(segments[3].ceiling_units(), 1);
}

#[test]
fn conditional_requires_boolean_condition_and_dominating_values() {
    let mut wrong_condition = conditional_module(VocabularyMarker::CURRENT);
    let integer = wrong_condition.machines[0].parameters[1].scalar_type;
    wrong_condition.machines[0].parameters[0].scalar_type = integer;
    assert!(matches!(
        verify_module(
            &wrong_condition,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::Module(
            terminal_verifier::ModuleError::ConditionalConditionTypeMismatch { .. }
        ))
    ));

    let mut branch_local_leak = conditional_module(VocabularyMarker::CURRENT);
    let true_parameter = branch_local_leak.machines[0].blocks[1].parameters[0].id;
    let Terminator::Return { value, .. } = &mut branch_local_leak.machines[0].blocks[2].terminator
    else {
        unreachable!("fixture's false block returns")
    };
    *value = true_parameter;
    assert!(matches!(
        verify_module(
            &branch_local_leak,
            &ProofBundle::default(),
            &AdmissionProfile::default(),
        ),
        Err(terminal_verifier::VerificationError::Module(
            terminal_verifier::ModuleError::ValueUsedBeforeDefinition(value)
        )) if value == true_parameter
    ));
}

fn conditional_module(vocabulary_marker: VocabularyMarker) -> TerminalModule {
    let integer =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 terminal type"));
    let declaration = |raw, scalar_type| ValueDeclaration {
        id: ValueId::new(raw).expect("nonzero value"),
        scalar_type,
    };
    TerminalModule {
        scalar_range_invariants: Vec::new(),
        vocabulary_marker,
        entry: MachineId::new(1).unwrap(),
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
            id: MachineId::new(1).unwrap(),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: vec![
                declaration(1, ScalarType::Boolean),
                declaration(2, integer),
                declaration(3, integer),
            ],
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(4, integer)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).unwrap(),
            blocks: vec![
                Block {
                    structural_parameters: Vec::new(),
                    id: BlockId::new(1).unwrap(),
                    parameters: Vec::new(),
                    operations: Vec::new(),
                    terminator: Terminator::Conditional {
                        condition: ValueId::new(1).unwrap(),
                        when_true: SuccessorEdge {
                            structural_arguments: Vec::new(),
                            edge: EdgeId::new(1).unwrap(),
                            target: BlockId::new(2).unwrap(),
                            arguments: vec![ValueId::new(2).unwrap()],
                            trivial_affine_discards: Vec::new(),
                        },
                        when_false: SuccessorEdge {
                            structural_arguments: Vec::new(),
                            edge: EdgeId::new(2).unwrap(),
                            target: BlockId::new(3).unwrap(),
                            arguments: vec![ValueId::new(3).unwrap()],
                            trivial_affine_discards: Vec::new(),
                        },
                    },
                },
                Block {
                    structural_parameters: Vec::new(),
                    id: BlockId::new(2).unwrap(),
                    parameters: vec![declaration(5, integer)],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(3).unwrap(),
                        value: ValueId::new(5).unwrap(),
                    },
                },
                Block {
                    structural_parameters: Vec::new(),
                    id: BlockId::new(3).unwrap(),
                    parameters: vec![declaration(6, integer)],
                    operations: Vec::new(),
                    terminator: Terminator::Return {
                        cleanup_actions: Vec::new(),
                        edge: EdgeId::new(4).unwrap(),
                        value: ValueId::new(6).unwrap(),
                    },
                },
            ],
            contract: MachineContract {
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

#[cfg(unix)]
static NEXT_SCRATCH_DIRECTORY: AtomicU64 = AtomicU64::new(0);

#[cfg(unix)]
struct ScratchDirectory(PathBuf);

#[cfg(unix)]
impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use std::sync::atomic::AtomicU64;
