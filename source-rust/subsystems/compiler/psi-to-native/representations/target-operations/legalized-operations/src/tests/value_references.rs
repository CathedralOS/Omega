// Value-reference and plan-query tests for structural edges, access lifecycles, primitive
// stores, custody-only decomposition and function ABI lookup.

use super::super::LegalizedScalarSuccessor;
use super::{
    EffectLink, IntegerSign, IntegerType, LegalizedOperationPlan, LegalizedScalarInstruction,
    LegalizedScalarInstructionKind, LegalizedScalarTerminator, ScalarType, StructuralAccess,
    StructuralMultiplicity, StructuralParameterDeclaration, id, scalar_call_unit_plan,
};
use optimization_core::AcceptedObligationFactIdentity;

#[test]
fn structural_edge_liveness_retains_selector_evidence() {
    let mut plan = scalar_call_unit_plan();
    let scalar_type = semantic_vocabulary::ClosedScalarType::Integer(
        semantic_vocabulary::ClosedIntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
    );
    plan.scalar_functions[0].blocks[0].terminator = LegalizedScalarTerminator::Jump {
        successor: LegalizedScalarSuccessor {
            edge: id(502),
            target: id(503),
            bindings: Vec::new(),
            work: Vec::new(),
            cleanup_calls: Vec::new(),
            structural_bindings: vec![crate::LegalizedStructuralBinding {
                semantic: operation_operands::values::AbstractStructuralBinding {
                    parameter: id(500),
                    argument: terminal_psi::StructuralArgument {
                        place: id(504),
                        path: Vec::new(),
                        access: terminal_psi::StructuralAccess::SharedBorrow,
                    },
                },
                indices: vec![crate::LegalizedRuntimeIndexOperand {
                    operand: operation_operands::values::AbstractResult {
                        value: id(510),
                        scalar_type: scalar_type.as_checking(),
                    },
                    stride: 16,
                    extent: Some(4),
                    obligation: id(511),
                    accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes(
                        [3; 32],
                    ),
                }],
            }],
        },
        effect: EffectLink {
            input: 0,
            output: 1,
        },
        ownership: Vec::new(),
    };
    assert!(plan.scalar_functions[0].references_value(id(510)));
    assert!(!plan.scalar_functions[0].references_value(id(512)));
}

#[test]
fn lifecycle_liveness_retains_original_owner_and_projection_evidence() {
    let scalar_type = semantic_vocabulary::ClosedScalarType::Integer(
        semantic_vocabulary::ClosedIntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
    );
    let instruction = LegalizedScalarInstruction {
        operation: id(400),
        result: None,
        kind: LegalizedScalarInstructionKind::AccessLifecycle {
            boundary: id(401),
            operands: operation_operands::access_lifecycles::AccessLifecycleOperands::MoveOut {
                storage: terminal_psi::StructuralArgument {
                    place: id(402),
                    path: vec![terminal_psi::StructuralPathSegment::RuntimeIndex {
                        index: id(403),
                        obligation: id(404),
                    }],
                    access: terminal_psi::StructuralAccess::Owned,
                },
                profile: terminal_psi::StructuralArgument {
                    place: id(412),
                    path: vec![],
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                },
            },
            indices: vec![crate::LegalizedRuntimeIndexOperand {
                operand: operation_operands::values::AbstractResult {
                    value: id(403),
                    scalar_type: scalar_type.as_checking(),
                },
                stride: 16,
                extent: Some(4),
                obligation: id(404),
                accepted_fact: AcceptedObligationFactIdentity::from_bytes([3; 32]),
            }],
            result: terminal_psi::StructuralOperationResult {
                place: id(405),
                structural_type: id(406),
                multiplicity: terminal_psi::StructuralMultiplicity::Affine,
                qualifications: vec![],
                projected_qualifications: vec![],
                additional_input_qualifications: Vec::new(),
                qualification_establishments: vec![],
                claims: vec![],
            },
        },
        work: vec![],
        effect: EffectLink {
            input: 0,
            output: 1,
        },
        ownership: vec![],
    };
    assert!(instruction.references_value(id(403)));
    assert!(!instruction.references_value(id(402)));
    assert!(instruction.result.is_none());
}

fn primitive_store_plan() -> LegalizedOperationPlan {
    let mut plan = scalar_call_unit_plan();
    let row = &mut plan.scalar_functions[0].blocks[0].instructions[0];
    row.result = None;
    row.kind = LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
        path: Vec::new(),
        byte_offset: 0,
        destination: StructuralParameterDeclaration {
            place: id(501),
            position: 0,
            structural_type: id(502),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::WriteOnlyBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
        }
        .into(),
        value: operation_operands::values::AbstractResult {
            value: id(503),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
        },
        byte_size: 4,
        indices: Vec::new(),
    };
    plan
}

#[test]
fn primitive_store_references_its_input() {
    let plan = primitive_store_plan();
    assert!(plan.scalar_functions[0].references_value(id(503)));
    assert!(!plan.scalar_functions[0].references_value(id(999)));
}

#[test]
fn decomposition_keeps_no_result_or_scalar_use() {
    let mut plan = scalar_call_unit_plan();
    let row = &mut plan.scalar_functions[0].blocks[0].instructions[0];
    row.result = None;
    row.kind = LegalizedScalarInstructionKind::DecomposeStructuralValue {
        subject: terminal_psi::StructuralArgument {
            place: id(201),
            path: vec![],
            access: StructuralAccess::Owned,
        },
    };
    assert!(row.kind.get_structural_result().is_none());
    let mut operands = vec![];
    row.visit_scalar_operands(|value| operands.push(value));
    assert!(operands.is_empty());
}

#[test]
fn function_abi_finds_the_described_machine() {
    let plan = scalar_call_unit_plan();
    let function = &plan.scalar_functions[0];
    let abi = crate::LegalizedFunctionAbi {
        assembly_return: None,
        assembly_register_versions: Vec::new(),
        machine: function.machine,
        call_plan: function.call_plan.clone(),
        entry_scalar_codecs: vec![Some(0); function.call_plan.parameters.len()],
        scalar_parameters: Vec::new(),
        structural_parameters: Vec::new(),
        scalar: None,
        mixed_structural_scalar: None,
    };
    let mut described = plan.clone();
    described.function_abis = std::sync::Arc::from([abi.clone()]);
    assert_eq!(described.function_abi(function.machine), Some(&abi));
}
