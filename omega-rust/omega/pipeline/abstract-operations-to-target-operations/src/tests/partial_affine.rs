//! Projected owned edge arguments and their residual cleanup admission.

use super::support;
use super::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
    PlaceId, ScalarType, StructuralAccess, StructuralFieldDeclaration, StructuralFieldId,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, ValueId,
};
use terminal_psi::{
    RecordFieldInitializer, RecordFieldValue, StructuralAffineDiscard, StructuralArgument,
    StructuralOperationResult, StructuralPathSegment, TerminalAffineCleanupAction,
};

fn block(raw: u64) -> BlockId {
    BlockId::new(raw).unwrap()
}
fn edge(raw: u64) -> EdgeId {
    EdgeId::new(raw).unwrap()
}
fn place(raw: u64) -> PlaceId {
    PlaceId::new(raw).unwrap()
}
fn operation(raw: u64) -> semantic_vocabulary::OperationId {
    semantic_vocabulary::OperationId::new(raw).unwrap()
}
fn value(raw: u64) -> ValueId {
    ValueId::new(raw).unwrap()
}
fn field(raw: u64) -> StructuralFieldId {
    StructuralFieldId::new(raw).unwrap()
}
fn structural_type(raw: u64) -> StructuralTypeId {
    StructuralTypeId::new(raw).unwrap()
}
fn segment(name: &str) -> StructuralPathSegment {
    StructuralPathSegment::Field(name.to_owned())
}

const TOKEN: u64 = 1;
const PAIR: u64 = 2;

fn scalar() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}

fn pair_catalog() -> abstract_operations::StructuralTypeCatalog {
    vec![
        StructuralTypeDeclaration {
            id: structural_type(TOKEN),
            identity: "Token".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: field(1),
                    identity: "value".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(scalar()),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: structural_type(PAIR),
            identity: "Pair".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: field(2),
                        identity: "left".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(structural_type(TOKEN)),
                    },
                    StructuralFieldDeclaration {
                        id: field(3),
                        identity: "right".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(structural_type(TOKEN)),
                    },
                ],
            },
        },
    ]
    .into()
}

fn token_result(
    place: PlaceId,
    psi_operation: semantic_vocabulary::OperationId,
) -> AbstractOperation {
    AbstractOperation::EstablishRecord {
        psi_operation,
        result: StructuralOperationResult {
            place,
            structural_type: structural_type(TOKEN),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        },
        fields: vec![RecordFieldInitializer {
            field: field(1),
            value: RecordFieldValue::Scalar {
                value: value(10),
                range_obligation: None,
            },
        }],
    }
}

fn pair_result(place: PlaceId, left: PlaceId, right: PlaceId) -> AbstractOperation {
    AbstractOperation::EstablishRecord {
        psi_operation: operation(13),
        result: StructuralOperationResult {
            place,
            structural_type: structural_type(PAIR),
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        },
        fields: vec![
            RecordFieldInitializer {
                field: field(2),
                value: RecordFieldValue::Structural(StructuralArgument {
                    place: left,
                    access: StructuralAccess::Owned,
                    path: Vec::new(),
                }),
            },
            RecordFieldInitializer {
                field: field(3),
                value: RecordFieldValue::Structural(StructuralArgument {
                    place: right,
                    access: StructuralAccess::Owned,
                    path: Vec::new(),
                }),
            },
        ],
    }
}

/// An established `Pair` moves `left` into the next block's owned parameter
/// while the `right` complement dies as a plain residual on the same edge.
fn residual_jump_plan() -> AbstractOperationPlan {
    let pair = place(12);
    let token_parameter = place(30);
    let jump = AbstractOperation::Jump {
        psi_edge: edge(1),
        target: block(2),
        bindings: Vec::new(),
        structural_bindings: vec![abstract_operations::AbstractStructuralBinding {
            parameter: token_parameter,
            argument: StructuralArgument {
                place: pair,
                access: StructuralAccess::Owned,
                path: vec![segment("left")],
            },
        }],
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: vec![StructuralAffineDiscard {
            place: pair,
            path: vec![segment("right")],
            structural_type: structural_type(TOKEN),
        }],
    };
    let function = AbstractFunction {
        machine: MachineId::new(1).unwrap(),
        attachment: None,
        entry: block(1),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![
            AbstractBlockEntry {
                block: block(1),
                operation_offset: 0,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
            },
            AbstractBlockEntry {
                block: block(2),
                operation_offset: 5,
                parameters: Vec::new(),
                structural_parameters: vec![StructuralParameterDeclaration {
                    place: token_parameter,
                    position: 0,
                    is_self: false,
                    structural_type: structural_type(TOKEN),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                }],
            },
        ],
        operations: vec![
            AbstractOperation::IntegerConstant {
                psi_operation: operation(10),
                result: value(10),
                scalar_type: scalar(),
                value: IntegerValue::Unsigned(7),
            },
            token_result(place(10), operation(11)),
            token_result(place(11), operation(12)),
            pair_result(pair, place(10), place(11)),
            jump,
            AbstractOperation::ReturnUnit {
                psi_edge: edge(2),
                cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(token_parameter)],
            },
        ],
    };
    AbstractOperationPlan {
        psi: support::identity(),
        entry: function.machine,
        structural_types: pair_catalog(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![function],
    }
}

fn lower(
    plan: &AbstractOperationPlan,
) -> Result<target_operations::TargetOperationPlan, crate::LoweringError> {
    crate::lower_to_target_operations(
        plan,
        crate::TargetLoweringRequest::new(target::NativeTarget::linux_x64()),
    )
}

#[test]
fn residual_jump_discards_the_sibling_subtree_on_the_edge() {
    let plan = residual_jump_plan();
    let lowered = lower(&plan).expect("projected owned edge with residual cleanup lowers");
    let target_operations::TargetControlTerminator::Jump { successor } =
        &lowered.functions[0].graph.blocks[0].terminator
    else {
        panic!("entry block terminator");
    };
    assert_eq!(
        successor.cleanup_actions,
        vec![TerminalAffineCleanupAction::DiscardResidual(
            StructuralAffineDiscard {
                place: place(12),
                path: vec![segment("right")],
                structural_type: structural_type(TOKEN),
            }
        )]
    );
    assert_eq!(
        successor.structural_bindings[0].argument.path,
        vec![segment("left")]
    );
    crate::validate_abstract_to_target_translation(
        &plan,
        target::NativeTarget::linux_x64(),
        &lowered,
    )
    .expect("the retained residual edge replays exactly");
}

#[test]
fn residual_jump_rejects_malformed_boundaries() {
    // A residual path outside the root's declared shape cannot be replayed.
    let mut plan = residual_jump_plan();
    let AbstractOperation::Jump {
        residual_affine_discards,
        ..
    } = &mut plan.functions[0].operations[4]
    else {
        panic!("jump")
    };
    residual_affine_discards[0].path = vec![segment("missing")];
    assert!(lower(&plan).is_err());

    // A residual on a root already whole-discarded on the same edge double-disposes.
    let mut plan = residual_jump_plan();
    let AbstractOperation::Jump {
        trivial_affine_discards,
        ..
    } = &mut plan.functions[0].operations[4]
    else {
        panic!("jump")
    };
    trivial_affine_discards.push(place(12));
    assert!(lower(&plan).is_err());

    // A residual subtree typed as its own root is not a projection.
    let mut plan = residual_jump_plan();
    let AbstractOperation::Jump {
        residual_affine_discards,
        ..
    } = &mut plan.functions[0].operations[4]
    else {
        panic!("jump")
    };
    residual_affine_discards[0].structural_type = structural_type(PAIR);
    assert!(lower(&plan).is_err());

    // A residual overlapping the moved subtree re-disposes moved storage.
    let mut plan = residual_jump_plan();
    let AbstractOperation::Jump {
        residual_affine_discards,
        ..
    } = &mut plan.functions[0].operations[4]
    else {
        panic!("jump")
    };
    residual_affine_discards[0].path = vec![segment("left")];
    assert!(lower(&plan).is_err());
}
