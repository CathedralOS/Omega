//! Focused regression coverage: each crafted vocabulary fragment must reject
//! with its exact owning `LoweringError` variant — never a sibling variant, a
//! panic, or silent acceptance.

use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, MachineId, ObligationId, OperationId,
    PlaceId, ScalarQualificationSetId, ScalarType, StructuralCaseId, StructuralFieldId,
    StructuralPlaceKind, StructuralTypeId, ValueId,
};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralArgument, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};

use super::{LoweringError, lower_decoded_module};

fn scalar(id: u64, scalar_type: ScalarType) -> ValueDeclaration {
    ValueDeclaration {
        id: ValueId::new(id).unwrap(),
        scalar_type,
        qualifications: ScalarQualificationSetId::ZERO,
    }
}

fn u64_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}

fn machine(id: u64, result: TerminalMachineResult, blocks: Vec<Block>) -> TerminalMachine {
    TerminalMachine {
        id: MachineId::new(id).unwrap(),
        attachment: None,
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: BlockId::new(1).unwrap(),
        blocks,
        contract: MachineContract {
            erased_proof_formals: Vec::new(),
            erased_scalar_formals: Vec::new(),
            id: ContractId::new(1).unwrap(),
            crash_routes: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    }
}

fn block(id: u64, operations: Vec<Operation>, terminator: Terminator) -> Block {
    Block {
        erased_proof_formals: Vec::new(),
        id: BlockId::new(id).unwrap(),
        parameters: Vec::new(),
        erased_scalar_formals: Vec::new(),
        structural_parameters: Vec::new(),
        operations,
        terminator,
    }
}

fn unit_block(id: u64, operations: Vec<Operation>) -> Block {
    block(
        id,
        operations,
        Terminator::ReturnUnit {
            edge: EdgeId::new(1).unwrap(),
            trivial_affine_discards: Vec::new(),
        },
    )
}

fn op(id: u64, result: OperationResult, kind: OperationKind) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(id).unwrap(),
        result,
        kind,
    }
}

fn structural_result(id: u64) -> OperationResult {
    OperationResult::Structural(StructuralOperationResult {
        qualification_establishments: Vec::new(),
        place: PlaceId::new(id).unwrap(),
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    })
}

fn module(machines: Vec<TerminalMachine>) -> TerminalModule {
    TerminalModule {
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(1).unwrap(),
        scalar_qualifications: Default::default(),
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
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines,
    }
}

fn reject(module: &TerminalModule) -> LoweringError {
    lower_decoded_module(module).expect_err("crafted module must reject")
}

#[test]
fn minimal_unit_module_lowers() {
    let module = module(vec![machine(
        1,
        TerminalMachineResult::Unit,
        vec![unit_block(1, Vec::new())],
    )]);
    assert!(lower_decoded_module(&module).is_ok());
}

#[test]
fn missing_entry_machine_rejects_as_entry_missing() {
    let mut module = module(Vec::new());
    module.entry = MachineId::new(9).unwrap();
    assert_eq!(
        reject(&module),
        LoweringError::VerifiedEntryMachineMissing(MachineId::new(9).unwrap())
    );
}

#[test]
fn structural_entry_block_parameter_rejects() {
    let entry = Block {
        structural_parameters: vec![StructuralParameterDeclaration {
            place: PlaceId::new(1).unwrap(),
            position: 0,
            is_self: false,
            structural_type: StructuralTypeId::new(1).unwrap(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }],
        ..unit_block(1, Vec::new())
    };
    let module = module(vec![machine(1, TerminalMachineResult::Unit, vec![entry])]);
    assert_eq!(
        reject(&module),
        LoweringError::UnsupportedStructuralBlockParameters {
            machine: MachineId::new(1).unwrap(),
            block: BlockId::new(1).unwrap(),
        }
    );
}

#[test]
fn projected_structural_successor_argument_rejects() {
    let target = unit_block(2, Vec::new());
    let entry = block(
        1,
        Vec::new(),
        Terminator::Jump {
            erased_proof_arguments: Vec::new(),
            edge: EdgeId::new(1).unwrap(),
            target: BlockId::new(2).unwrap(),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: PlaceId::new(1).unwrap(),
                path: vec!["field".into()],
                access: StructuralAccess::Owned,
            }],
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
    );
    let module = module(vec![machine(
        1,
        TerminalMachineResult::Unit,
        vec![entry, target],
    )]);
    assert_eq!(
        reject(&module),
        LoweringError::UnsupportedStructuralSuccessorArguments {
            machine: MachineId::new(1).unwrap(),
            edge: EdgeId::new(1).unwrap(),
        }
    );
}

#[test]
fn jump_to_missing_block_rejects_as_block_missing() {
    let entry = block(
        1,
        Vec::new(),
        Terminator::Jump {
            erased_proof_arguments: Vec::new(),
            edge: EdgeId::new(1).unwrap(),
            target: BlockId::new(9).unwrap(),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
    );
    let module = module(vec![machine(1, TerminalMachineResult::Unit, vec![entry])]);
    assert_eq!(
        reject(&module),
        LoweringError::VerifiedBlockMissing {
            machine: MachineId::new(1).unwrap(),
            block: BlockId::new(9).unwrap(),
        }
    );
}

#[test]
fn jump_arity_mismatch_rejects() {
    let target = Block {
        parameters: vec![scalar(1, u64_type())],
        ..unit_block(2, Vec::new())
    };
    let entry = block(
        1,
        Vec::new(),
        Terminator::Jump {
            erased_proof_arguments: Vec::new(),
            edge: EdgeId::new(1).unwrap(),
            target: BlockId::new(2).unwrap(),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            structural_arguments: Vec::new(),
            trivial_affine_discards: Vec::new(),
            residual_affine_discards: Vec::new(),
        },
    );
    let module = module(vec![machine(
        1,
        TerminalMachineResult::Unit,
        vec![entry, target],
    )]);
    assert_eq!(
        reject(&module),
        LoweringError::VerifiedJumpArityMismatch {
            edge: EdgeId::new(1).unwrap(),
        }
    );
}

#[test]
fn scalar_return_on_unit_machine_rejects() {
    let mut m = machine(
        1,
        TerminalMachineResult::Unit,
        vec![block(
            1,
            Vec::new(),
            Terminator::Return {
                edge: EdgeId::new(1).unwrap(),
                value: ValueId::new(1).unwrap(),
                cleanup_actions: Vec::new(),
            },
        )],
    );
    m.parameters = vec![scalar(1, u64_type())];
    let module = module(vec![m]);
    assert_eq!(
        reject(&module),
        LoweringError::ScalarReturnFromUnitMachine(MachineId::new(1).unwrap())
    );
}

#[test]
fn unit_return_on_scalar_machine_rejects() {
    let m = machine(
        1,
        TerminalMachineResult::Scalar(scalar(9, u64_type())),
        vec![unit_block(1, Vec::new())],
    );
    let module = module(vec![m]);
    assert_eq!(
        reject(&module),
        LoweringError::UnitReturnFromScalarMachine(MachineId::new(1).unwrap())
    );
}

#[test]
fn structural_return_on_unit_machine_rejects() {
    let entry = block(
        1,
        Vec::new(),
        Terminator::ReturnStructural {
            edge: EdgeId::new(1).unwrap(),
            source: PlaceId::new(1).unwrap(),
            returned_claims: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    );
    let module = module(vec![machine(1, TerminalMachineResult::Unit, vec![entry])]);
    assert_eq!(
        reject(&module),
        LoweringError::UnsupportedStructuralReturn {
            machine: MachineId::new(1).unwrap(),
            edge: EdgeId::new(1).unwrap(),
        }
    );
}

#[test]
fn scalar_case_without_structural_result_rejects() {
    let operation = op(
        1,
        OperationResult::Unit,
        OperationKind::EstablishScalarCase {
            result_case: StructuralCaseId::new(1).unwrap(),
            fields: Vec::new(),
        },
    );
    let module = module(vec![machine(
        1,
        TerminalMachineResult::Unit,
        vec![unit_block(1, vec![operation])],
    )]);
    assert_eq!(
        reject(&module),
        LoweringError::UnsupportedPayloadlessCase(OperationId::new(1).unwrap())
    );
}

#[test]
fn scalar_array_without_structural_result_rejects() {
    let operation = op(
        1,
        OperationResult::Unit,
        OperationKind::EstablishScalarArray {
            elements: Vec::new(),
        },
    );
    let module = module(vec![machine(
        1,
        TerminalMachineResult::Unit,
        vec![unit_block(1, vec![operation])],
    )]);
    assert_eq!(
        reject(&module),
        LoweringError::UnsupportedScalarArray(OperationId::new(1).unwrap())
    );
}

#[test]
fn byte_sequence_literal_without_declared_place_rejects() {
    let operation = op(
        1,
        structural_result(1),
        OperationKind::EstablishByteSequenceLiteral {
            destination: PlaceId::new(1).unwrap(),
            bytes: vec![0xAA],
        },
    );
    let module = module(vec![machine(
        1,
        TerminalMachineResult::Unit,
        vec![unit_block(1, vec![operation])],
    )]);
    assert_eq!(
        reject(&module),
        LoweringError::UnsupportedByteSequenceLiteral(OperationId::new(1).unwrap())
    );
}

#[test]
fn trivial_affine_local_with_undeclared_type_rejects() {
    let mut m = machine(
        1,
        TerminalMachineResult::Unit,
        vec![unit_block(
            1,
            vec![op(
                1,
                structural_result(1),
                OperationKind::EstablishTrivialAffineLocal {
                    destination: PlaceId::new(1).unwrap(),
                },
            )],
        )],
    );
    m.structural_places = vec![StructuralPlaceDeclaration {
        id: PlaceId::new(1).unwrap(),
        kind: StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal: 0,
            structural_type: StructuralTypeId::new(2).unwrap(),
            construction: None,
        },
    }];
    let module = module(vec![m]);
    assert_eq!(
        reject(&module),
        LoweringError::UnsupportedStructuralReturn {
            machine: MachineId::new(1).unwrap(),
            edge: EdgeId::new(1).unwrap(),
        }
    );
}

#[test]
fn reference_without_structural_result_rejects() {
    let operation = op(
        1,
        OperationResult::Unit,
        OperationKind::EstablishReference {
            source: StructuralArgument {
                place: PlaceId::new(1).unwrap(),
                path: Vec::new(),
                access: StructuralAccess::SharedBorrow,
            },
        },
    );
    let module = module(vec![machine(
        1,
        TerminalMachineResult::Unit,
        vec![unit_block(1, vec![operation])],
    )]);
    assert_eq!(
        reject(&module),
        LoweringError::UnsupportedReferenceCustody(OperationId::new(1).unwrap())
    );
}

#[test]
fn case_membership_without_boolean_result_rejects() {
    let operation = op(
        1,
        OperationResult::Scalar(scalar(1, u64_type())),
        OperationKind::StructuralCaseMembership {
            source: PlaceId::new(1).unwrap(),
            path: Vec::new(),
            case: StructuralCaseId::new(1).unwrap(),
        },
    );
    let module = module(vec![machine(
        1,
        TerminalMachineResult::Unit,
        vec![unit_block(1, vec![operation])],
    )]);
    assert_eq!(
        reject(&module),
        LoweringError::InvalidStructuralCaseMembership(OperationId::new(1).unwrap())
    );
}

#[test]
fn wrapping_add_without_integer_result_rejects() {
    let operation = op(
        1,
        OperationResult::Scalar(scalar(1, ScalarType::Boolean)),
        OperationKind::WrappingIntegerAdd {
            left: ValueId::new(2).unwrap(),
            right: ValueId::new(3).unwrap(),
        },
    );
    let module = module(vec![machine(
        1,
        TerminalMachineResult::Unit,
        vec![unit_block(1, vec![operation])],
    )]);
    assert_eq!(
        reject(&module),
        LoweringError::VerifiedWrappingAddMalformed(OperationId::new(1).unwrap())
    );
}

#[test]
fn primitive_local_without_structural_result_rejects() {
    let operation = op(
        1,
        OperationResult::Unit,
        OperationKind::EstablishPrimitiveLocal {
            value: ValueId::new(1).unwrap(),
        },
    );
    let module = module(vec![machine(
        1,
        TerminalMachineResult::Unit,
        vec![unit_block(1, vec![operation])],
    )]);
    assert_eq!(
        reject(&module),
        LoweringError::InvalidPrimitiveLocalEstablishment(OperationId::new(1).unwrap())
    );
}

#[test]
fn structural_call_without_structural_result_rejects() {
    let operation = op(
        1,
        OperationResult::Unit,
        OperationKind::CallStructural {
            callee: MachineId::new(1).unwrap(),
            structural_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            returned_claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            selected_evidence: Vec::new(),
        },
    );
    let module = module(vec![machine(
        1,
        TerminalMachineResult::Unit,
        vec![unit_block(1, vec![operation])],
    )]);
    assert_eq!(
        reject(&module),
        LoweringError::UnsupportedStructuralResult(MachineId::new(1).unwrap())
    );
}

#[test]
fn write_only_store_without_declared_destination_rejects() {
    let operation = op(
        1,
        OperationResult::Unit,
        OperationKind::WriteOnlyPrimitiveStore {
            destination: PlaceId::new(9).unwrap(),
            path: Vec::new(),
            value: ValueId::new(1).unwrap(),
        },
    );
    let module = module(vec![machine(
        1,
        TerminalMachineResult::Unit,
        vec![unit_block(1, vec![operation])],
    )]);
    assert_eq!(
        reject(&module),
        LoweringError::InvalidWriteOnlyPrimitiveStore(OperationId::new(1).unwrap())
    );
}

#[test]
fn runtime_indexed_store_without_declared_destination_rejects() {
    let operation = op(
        1,
        OperationResult::Unit,
        OperationKind::WriteOnlyIndexedPrimitiveStore {
            destination: PlaceId::new(9).unwrap(),
            path: Vec::new(),
            index: ValueId::new(1).unwrap(),
            value: ValueId::new(2).unwrap(),
            obligation: ObligationId::new(1).unwrap(),
        },
    );
    let module = module(vec![machine(
        1,
        TerminalMachineResult::Unit,
        vec![unit_block(1, vec![operation])],
    )]);
    assert_eq!(
        reject(&module),
        LoweringError::InvalidWriteOnlyPrimitiveStore(OperationId::new(1).unwrap())
    );
}

#[test]
fn primitive_read_without_declared_source_rejects() {
    let operation = op(
        1,
        OperationResult::Scalar(scalar(1, u64_type())),
        OperationKind::PrimitiveScalarRead {
            source: PlaceId::new(9).unwrap(),
            path: Vec::new(),
        },
    );
    let module = module(vec![machine(
        1,
        TerminalMachineResult::Unit,
        vec![unit_block(1, vec![operation])],
    )]);
    assert_eq!(
        reject(&module),
        LoweringError::InvalidPrimitiveScalarRead(OperationId::new(1).unwrap())
    );
}

#[test]
fn byte_sequence_field_length_without_scalar_result_rejects() {
    let operation = op(
        1,
        OperationResult::Unit,
        OperationKind::StructuralByteSequenceFieldLength {
            source: PlaceId::new(1).unwrap(),
            path: Vec::new(),
            field: StructuralFieldId::new(1).unwrap(),
        },
    );
    let module = module(vec![machine(
        1,
        TerminalMachineResult::Unit,
        vec![unit_block(1, vec![operation])],
    )]);
    assert_eq!(
        reject(&module),
        LoweringError::InvalidByteSequenceLength(OperationId::new(1).unwrap())
    );
}

#[test]
fn bitwise_not_without_integer_result_rejects() {
    let operation = op(
        1,
        OperationResult::Scalar(scalar(1, ScalarType::Boolean)),
        OperationKind::IntegerBitwiseNot {
            operand: ValueId::new(1).unwrap(),
        },
    );
    let module = module(vec![machine(
        1,
        TerminalMachineResult::Unit,
        vec![unit_block(1, vec![operation])],
    )]);
    assert_eq!(
        reject(&module),
        LoweringError::VerifiedIntegerBitwiseMalformed(OperationId::new(1).unwrap())
    );
}
