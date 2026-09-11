//! Exact IEEE source identity survives field writes and ordinary Unit-call transport.
use abstract_operations::{AbstractOperation, AbstractParameter, AbstractResult};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IeeeFloatValue, IntegerValue, MachineId, OperationId,
    PlaceId, ScalarType, StructuralFieldId, StructuralTypeId, ValueId,
};
use target::NativeTarget;
use target_operations::{TargetUnitOperation, TargetUnitScalarArgumentSource as Source};
use terminal_psi::{
    BindingRelevance, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape,
};

use crate::{legalize_target_operations, validate_legalized_operations};

fn fixture(
    native: NativeTarget,
    literal: IeeeFloatValue,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let scalar_type = ScalarType::IeeeFloat(literal.format());
    let mut callee = source.functions[0].clone();
    callee.machine = MachineId::new(2).unwrap();
    callee.entry = BlockId::new(2).unwrap();
    callee.block_entries[0].block = callee.entry;
    callee.parameters = vec![AbstractParameter {
        value: ValueId::new(200).unwrap(),
        scalar_type,
    }];
    callee.operations = vec![AbstractOperation::ReturnUnit {
        psi_edge: EdgeId::new(2).unwrap(),
        cleanup_actions: Vec::new(),
    }];
    let structural_type = StructuralTypeId::new(1).unwrap();
    let field = StructuralFieldId::new(1).unwrap();
    source
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: structural_type,
            identity: "FloatField".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: field,
                    identity: "value".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::IeeeFloat(literal.format()),
                }],
            },
        });
    let destination = StructuralParameterDeclaration {
        place: PlaceId::new(1).unwrap(),
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::WriteOnlyBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let caller = &mut source.functions[0];
    caller.structural_parameters.push(destination.clone());
    let value = ValueId::new(100).unwrap();
    caller.operations.splice(
        0..0,
        [
            AbstractOperation::IeeeFloatConstant {
                psi_operation: OperationId::new(1).unwrap(),
                result: value,
                value: literal,
            },
            AbstractOperation::StructuralScalarFieldStore {
                psi_operation: OperationId::new(2).unwrap(),
                destination,
                path: Vec::new(),
                field,
                value: AbstractResult { value, scalar_type },
            },
            AbstractOperation::CallUnit {
                psi_operation: OperationId::new(3).unwrap(),
                callee: callee.machine,
                arguments: vec![value],
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        ],
    );
    source.functions.push(callee);
    let target =
        abstract_operations_to_target_operations::lower_to_target_operations(&source, native)
            .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (source, target, unit)
}

fn changed_bits(literal: IeeeFloatValue) -> IeeeFloatValue {
    match literal {
        IeeeFloatValue::Binary32(bits) => IeeeFloatValue::Binary32(bits ^ 1),
        IeeeFloatValue::Binary64(bits) => IeeeFloatValue::Binary64(bits ^ 1),
    }
}

fn changed_format(literal: IeeeFloatValue) -> IeeeFloatValue {
    match literal {
        IeeeFloatValue::Binary32(bits) => IeeeFloatValue::Binary64(u64::from(bits)),
        IeeeFloatValue::Binary64(bits) => IeeeFloatValue::Binary32(bits as u32),
    }
}

#[test]
fn ieee_literal_field_and_call_receiving_binds_definition_bits_format_and_order() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        for literal in [
            IeeeFloatValue::Binary32(0x7fc01234),
            IeeeFloatValue::Binary64(0x8000000000000000),
        ] {
            let (source, target, unit) = fixture(native, literal);
            let legal = legalize_target_operations(&target, &source, &unit).unwrap();
            validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
            for source_position in [1, 2] {
                for mutation in 0..7 {
                    let mut changed = target.clone();
                    let body = &mut changed.functions[0].graph;
                    if mutation == 5 {
                        body.blocks[0].operations.swap(0, source_position);
                    } else if mutation == 6 {
                        let TargetUnitOperation::IeeeFloatConstant { value, .. } =
                            &mut body.blocks[0].operations[0]
                        else {
                            panic!("literal");
                        };
                        *value = changed_bits(*value);
                    } else {
                        let supplied = match &mut body.blocks[0].operations[source_position] {
                            TargetUnitOperation::StructuralScalarFieldStore { source, .. } => {
                                source
                            }
                            TargetUnitOperation::Call {
                                scalar_arguments, ..
                            } => &mut scalar_arguments[0].source,
                            _ => panic!("field or Unit call"),
                        };
                        let Source::IeeeFloatImmediate {
                            defining_operation,
                            source_value,
                            value,
                        } = supplied
                        else {
                            panic!("IEEE source");
                        };
                        match mutation {
                            0 => *defining_operation = OperationId::new(99).unwrap(),
                            1 => *source_value = ValueId::new(99).unwrap(),
                            2 => *value = changed_bits(*value),
                            3 => *value = changed_format(*value),
                            _ => {
                                *supplied = Source::BooleanImmediate {
                                    defining_operation: *defining_operation,
                                    source_value: *source_value,
                                    value: true,
                                }
                            }
                        }
                    }
                    assert!(
                        legalize_target_operations(&changed, &source, &unit).is_err(),
                        "{native:?} source {source_position} mutation {mutation}"
                    );
                    assert!(
                        validate_legalized_operations(
                            &changed,
                            &source,
                            &unit,
                            legal.plan().clone()
                        )
                        .is_err()
                    );
                }
            }
            for mutation in 0..5 {
                let mut changed = legal.plan().clone();
                let rows = &mut changed.scalar_functions[0].blocks[0].instructions;
                match mutation {
                    0 => {
                        rows[0].kind =
                            legalized_operations::LegalizedScalarInstructionKind::Constant(
                                IntegerValue::Unsigned(0),
                            )
                    }
                    1 => {
                        rows[0].result.as_mut().unwrap().scalar_type =
                            ScalarType::IeeeFloat(changed_format(literal).format())
                    }
                    2 => rows[0].result.as_mut().unwrap().value = ValueId::new(99).unwrap(),
                    3 => rows[0].operation = OperationId::new(99).unwrap(),
                    _ => rows.swap(0, 1),
                }
                assert!(
                    validate_legalized_operations(&target, &source, &unit, changed).is_err(),
                    "legalized mutation {mutation}"
                );
            }
        }
    }
}
