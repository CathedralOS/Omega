//! A case payload observation reads its declared case's overlay at the sum's
//! own place. `Mode` stores `Idle { level }` and `Run { distance, speed }`
//! under one tag; both cases declare a field 2 of the same scalar type, so a
//! substituted case still names a well-typed leaf, only at another offset.
//! Every replay must therefore bind the case itself, not merely a
//! plausible field.
use super::super::reject_target;
use super::{
    AbstractFunctionResult, AbstractOperation, AbstractResult, BindingRelevance, EdgeId,
    FuelScheduleIdentity, IntegerSign, LegalizedScalarInstructionKind, NativeTarget, OperationId,
    ScalarType, StructuralAccess, StructuralFieldDeclaration, StructuralFieldId,
    StructuralFieldType, StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape,
    TargetUnitOperation, ValueId, fixture,
};
use crate::legalize_target_operations;
use crate::tests::legalization::primitive_stores::integer;
use crate::validate_legalized_operations;
use selected_instructions::SelectedInstructionKind as Instruction;
use semantic_vocabulary::{CanonicalStructuralPathSegment as Segment, StructuralCaseId};

const PREFIX: u64 = 1;
const MODE: u64 = 2;
const DISTANCE: u64 = 1;
const SPEED: u64 = 2;

fn field(id: u64, identity: &str, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
    StructuralFieldDeclaration {
        id: StructuralFieldId::new(id).unwrap(),
        identity: identity.into(),
        relevance: BindingRelevance::Relevant,
        field_type,
    }
}

fn case(id: u64) -> StructuralCaseId {
    StructuralCaseId::new(id).unwrap()
}

/// `Owner { prefix: u64, mode: Mode }` behind a borrowed parameter, with one
/// read of `mode`'s `Run` payload field `speed`. The tag occupies bytes
/// 0..4 of `Mode` and the 8-aligned payload base is byte 8, so `Idle.level`
/// sits at 8 and `Run.speed` at 16 within `Mode`; `mode` itself starts at
/// byte 8 of `Owner`. The read therefore loads byte 24; `Idle.level` would
/// be byte 16.
fn case_payload_read(
    native: NativeTarget,
    scalar: ScalarType,
    access: StructuralAccess,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut source, _, _) = fixture(native, scalar, false);
    let owner = StructuralTypeId::new(1).unwrap();
    let mode = StructuralTypeId::new(2).unwrap();
    let wide = integer(IntegerSign::Unsigned, 64);
    let mut types = source.structural_types.to_vec();
    types[0] = StructuralTypeDeclaration {
        id: owner,
        identity: "Owner".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![
                field(PREFIX, "prefix", StructuralFieldType::Scalar(wide)),
                field(MODE, "mode", StructuralFieldType::Structural(mode)),
            ],
        },
    };
    types.push(StructuralTypeDeclaration {
        id: mode,
        identity: "Mode".into(),
        shape: StructuralTypeShape::Sum {
            cases: vec![
                terminal_psi::StructuralCaseDeclaration {
                    id: case(1),
                    identity: "Idle".into(),
                    fields: vec![field(SPEED, "level", StructuralFieldType::Scalar(scalar))],
                },
                terminal_psi::StructuralCaseDeclaration {
                    id: case(2),
                    identity: "Run".into(),
                    fields: vec![
                        field(DISTANCE, "distance", StructuralFieldType::Scalar(wide)),
                        field(SPEED, "speed", StructuralFieldType::Scalar(scalar)),
                    ],
                },
            ],
        },
    });
    source.structural_types = types.into();
    let function = &mut source.functions[0];
    function.parameters.clear();
    function.structural_parameters[0].access = access;
    let parameter = function.structural_parameters[0].clone();
    let read = ValueId::new(7).unwrap();
    let result = AbstractResult {
        value: ValueId::new(8).unwrap(),
        scalar_type: scalar,
    };
    let path = vec![
        Segment::Field(StructuralFieldId::new(MODE).unwrap()),
        Segment::Case(case(2)),
    ];
    let speed = StructuralFieldId::new(SPEED).unwrap();
    function.result = AbstractFunctionResult::Scalar(result);
    function.operations = vec![
        AbstractOperation::StructuralScalarFieldRead {
            psi_operation: OperationId::new(1).unwrap(),
            result: AbstractResult {
                value: read,
                scalar_type: scalar,
            },
            source: parameter.place,
            path,
            field: speed,
        },
        AbstractOperation::Return {
            psi_edge: EdgeId::new(1).unwrap(),
            result: result.value,
            value: read,
            scalar_type: scalar,
            cleanup_actions: Vec::new(),
        },
    ];
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
    )
    .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (source, target, unit)
}

fn load_offset(instruction: &Instruction) -> Option<u32> {
    match instruction {
        Instruction::Load8 { byte_offset } | Instruction::Load16 { byte_offset } => {
            Some(*byte_offset)
        }
        _ => None,
    }
}

#[test]
fn case_payload_reads_load_the_named_overlay_and_reject_substitution() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        for scalar in [ScalarType::Boolean, integer(IntegerSign::Signed, 16)] {
            for access in [
                StructuralAccess::SharedBorrow,
                StructuralAccess::MutableBorrow,
            ] {
                let (source, target, unit) = case_payload_read(native, scalar, access);
                // Lowering keeps the case beside a runtime carrier that stops
                // at the sum: the `Case` step names no storage.
                let TargetUnitOperation::StructuralScalarFieldRead {
                    source: argument,
                    case: lowered_case,
                    ..
                } = &target.functions[0].graph.blocks[0].operations[0]
                else {
                    panic!("case payload read");
                };
                assert_eq!(*lowered_case, Some(case(2)));
                assert_eq!(
                    argument.path,
                    vec![terminal_psi::StructuralPathSegment::Field("mode".into())]
                );
                let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
                validate_legalized_operations(&target, &source, &unit, legalized.plan().clone())
                    .unwrap();
                let LegalizedScalarInstructionKind::StructuralScalarFieldRead {
                    case: legalized_case,
                    ..
                } = &legalized.plan().scalar_functions[0].blocks[0].instructions[0].kind
                else {
                    panic!("legalized case payload read");
                };
                assert_eq!(*legalized_case, Some(case(2)));

                let environment =
                    register_environment::baseline_target_register_environment(native).unwrap();
                let constraints = crate::selection_constraints(&legalized, &environment);
                let selected = crate::select_instructions(
                    &legalized,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
                .unwrap();
                crate::validate_selected_instructions(
                    &legalized,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    selected.plan().clone(),
                )
                .unwrap();
                let loads = selected.plan().functions[0].blocks[0]
                    .instructions
                    .iter()
                    .filter_map(|instruction| load_offset(&instruction.kind))
                    .collect::<Vec<_>>();
                assert_eq!(loads, vec![24], "{native:?} {scalar:?} {access:?}");

                // Selection replay rebuilds the offset from the declared
                // case: a load of the other case's overlay (16), of the tag
                // (8), or of the sum base is not this observation.
                for substituted in [16, 8, 0] {
                    let mut changed = selected.plan().clone();
                    let load = changed.functions[0].blocks[0]
                        .instructions
                        .iter_mut()
                        .find(|instruction| load_offset(&instruction.kind).is_some())
                        .unwrap();
                    match &mut load.kind {
                        Instruction::Load8 { byte_offset }
                        | Instruction::Load16 { byte_offset } => *byte_offset = substituted,
                        _ => unreachable!(),
                    }
                    assert!(
                        crate::validate_selected_instructions(
                            &legalized,
                            &constraints,
                            environment.physical(),
                            environment.constraints(),
                            changed,
                        )
                        .is_err(),
                        "{native:?} accepted a load at {substituted}"
                    );
                }

                // The raw target row must name exactly the replayed case and
                // field: a substituted or dropped case, or another field,
                // fails both legalization and replay of the proposed plan.
                for mutation in 0..4 {
                    let mut changed = target.clone();
                    let TargetUnitOperation::StructuralScalarFieldRead {
                        case: row_case,
                        field: row_field,
                        ..
                    } = &mut changed.functions[0].graph.blocks[0].operations[0]
                    else {
                        panic!("case payload read");
                    };
                    match mutation {
                        0 => *row_case = Some(case(1)),
                        1 => *row_case = None,
                        2 => *row_case = Some(case(3)),
                        _ => *row_field = StructuralFieldId::new(DISTANCE).unwrap(),
                    }
                    reject_target(&source, &changed, &unit, legalized.plan());
                }

                // The legalized row is replayed against the abstract carrier
                // the same way.
                for mutation in 0..3 {
                    let mut changed = legalized.plan().clone();
                    let LegalizedScalarInstructionKind::StructuralScalarFieldRead {
                        case: row_case,
                        field: row_field,
                        ..
                    } = &mut changed.scalar_functions[0].blocks[0].instructions[0].kind
                    else {
                        panic!("legalized case payload read");
                    };
                    match mutation {
                        0 => *row_case = Some(case(1)),
                        1 => *row_case = None,
                        _ => *row_field = StructuralFieldId::new(DISTANCE).unwrap(),
                    }
                    assert!(
                        validate_legalized_operations(&target, &source, &unit, changed).is_err(),
                        "{native:?} accepted legalized mutation {mutation}"
                    );
                }

                // Malformed canonical carriers stay closed at lowering: an
                // undeclared case, a field the named case does not declare, a
                // case step that is not final, and a case over a scalar field.
                for (path, field) in [
                    (
                        vec![
                            Segment::Field(StructuralFieldId::new(MODE).unwrap()),
                            Segment::Case(case(3)),
                        ],
                        SPEED,
                    ),
                    (
                        vec![
                            Segment::Field(StructuralFieldId::new(MODE).unwrap()),
                            Segment::Case(case(1)),
                        ],
                        DISTANCE,
                    ),
                    (
                        vec![
                            Segment::Field(StructuralFieldId::new(MODE).unwrap()),
                            Segment::Case(case(2)),
                            Segment::Field(StructuralFieldId::new(SPEED).unwrap()),
                        ],
                        SPEED,
                    ),
                    (
                        vec![
                            Segment::Field(StructuralFieldId::new(PREFIX).unwrap()),
                            Segment::Case(case(2)),
                        ],
                        SPEED,
                    ),
                ] {
                    let mut changed = source.clone();
                    let AbstractOperation::StructuralScalarFieldRead {
                        path: row_path,
                        field: row_field,
                        ..
                    } = &mut changed.functions[0].operations[0]
                    else {
                        panic!("case payload read");
                    };
                    *row_path = path;
                    *row_field = StructuralFieldId::new(field).unwrap();
                    assert!(
                        abstract_operations_to_target_operations::lower_to_target_operations(
                            &changed,
                            abstract_operations_to_target_operations::TargetLoweringRequest::new(
                                native
                            ),
                        )
                        .is_err()
                    );
                }
            }
        }
    }
}
