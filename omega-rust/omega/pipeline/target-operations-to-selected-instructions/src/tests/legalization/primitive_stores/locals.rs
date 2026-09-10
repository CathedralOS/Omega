//! Receiving replay binds primitive local operations to exact source identities.
use super::*;
mod boolean_transport;

fn fixture(scalar: ScalarType) -> AbstractOperationPlan {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let identity = StructuralTypeId::new(1).unwrap();
    let place = PlaceId::new(1).unwrap();
    source.structural_types.push(StructuralTypeDeclaration {
        id: identity,
        identity: "primitive".into(),
        shape: StructuralTypeShape::PrimitiveScalar(scalar),
    });
    let returned = source.functions[0].operations.pop().unwrap();
    source.functions[0].operations = vec![
        if scalar == ScalarType::Boolean {
            AbstractOperation::BooleanConstant {
                psi_operation: OperationId::new(1).unwrap(),
                result: ValueId::new(1).unwrap(),
                value: true,
            }
        } else {
            AbstractOperation::IntegerConstant {
                psi_operation: OperationId::new(1).unwrap(),
                result: ValueId::new(1).unwrap(),
                scalar_type: scalar,
                value: IntegerValue::Unsigned(91),
            }
        },
        AbstractOperation::EstablishPrimitiveLocal {
            psi_operation: OperationId::new(2).unwrap(),
            result: terminal_psi::StructuralOperationResult {
                place,
                structural_type: identity,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
            value: AbstractResult {
                value: ValueId::new(1).unwrap(),
                scalar_type: scalar,
            },
        },
        AbstractOperation::PrimitiveScalarRead {
            psi_operation: OperationId::new(3).unwrap(),
            source: place,
            result: AbstractResult {
                value: ValueId::new(2).unwrap(),
                scalar_type: scalar,
            },
        },
        AbstractOperation::PrimitiveLocalStore {
            psi_operation: OperationId::new(4).unwrap(),
            destination: place,
            value: AbstractResult {
                value: ValueId::new(2).unwrap(),
                scalar_type: scalar,
            },
        },
        returned,
    ];
    source
}

#[test]
fn local_establishment_store_and_read_reject_target_and_legalized_corruption() {
    let source = fixture(integer(IntegerSign::Unsigned, 64));
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let target =
            abstract_operations_to_target_operations::lower_to_target_operations(&source, native)
                .unwrap();
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legalized.plan().clone()).unwrap();
        for mutation in 0..5 {
            let mut changed = target.clone();
            let TargetOperation::ControlGraph(graph) = &mut changed.functions[0].operation else {
                panic!("common graph");
            };
            match mutation {
                0 => {
                    let TargetUnitOperation::EstablishPrimitiveLocal { shape, .. } =
                        &mut graph.blocks[0].operations[1]
                    else {
                        panic!("local");
                    };
                    shape.byte_size = 4;
                }
                1 => {
                    let TargetUnitOperation::PrimitiveScalarRead { source, .. } =
                        &mut graph.blocks[0].operations[2]
                    else {
                        panic!("read");
                    };
                    *source = PlaceId::new(99).unwrap();
                }
                2 => {
                    let TargetUnitOperation::PrimitiveLocalStore { value, .. } =
                        &mut graph.blocks[0].operations[3]
                    else {
                        panic!("store");
                    };
                    value.value = ValueId::new(1).unwrap();
                }
                3 => graph.blocks[0].operations.swap(1, 2),
                _ => {
                    let TargetUnitOperation::PrimitiveScalarRead { result, .. } =
                        &mut graph.blocks[0].operations[2]
                    else {
                        panic!("read");
                    };
                    result.value = ValueId::new(1).unwrap();
                }
            }
            assert!(
                legalize_target_operations(&changed, &source, &unit).is_err(),
                "target mutation {mutation}"
            );
            assert!(
                validate_legalized_operations(&changed, &source, &unit, legalized.plan().clone())
                    .is_err(),
                "receiving mutation {mutation}"
            );
        }
        for mutation in 0..4 {
            let mut changed = legalized.plan().clone();
            let rows = &mut changed.scalar_functions[0].blocks[0].instructions;
            match mutation {
                0 => rows[1].fuel.clear(),
                1 => rows[1].effect.output += 1,
                2 => {
                    let LegalizedScalarInstructionKind::PrimitiveLocalStore { value, .. } =
                        &mut rows[3].kind
                    else {
                        panic!("store");
                    };
                    value.value = ValueId::new(1).unwrap();
                }
                _ => rows[2].result.as_mut().unwrap().value = ValueId::new(1).unwrap(),
            }
            assert!(
                validate_legalized_operations(&target, &source, &unit, changed).is_err(),
                "legalized mutation {mutation}"
            );
        }
    }
}

#[test]
fn boolean_branch_replay_rejects_substituted_or_unavailable_read_homes() {
    use abstract_operations::{AbstractBlockEntry, AbstractSuccessor};
    use semantic_vocabulary::{BlockId, EdgeId};
    use target_operations::{TargetBooleanExpression, TargetControlTerminator};

    let mut source = fixture(ScalarType::Boolean);
    let function = &mut source.functions[0];
    function.operations.pop().unwrap();
    // The intervening store must not turn the earlier read into another load or
    // make Boolean predicate availability depend on an adjacent source suffix.
    let successor = |ordinal| AbstractSuccessor {
        psi_edge: EdgeId::new(ordinal).unwrap(),
        target: BlockId::new(ordinal).unwrap(),
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    function.operations.push(AbstractOperation::Conditional {
        condition: ValueId::new(2).unwrap(),
        when_true: successor(2),
        when_false: successor(3),
    });
    for ordinal in [2, 3] {
        function.block_entries.push(AbstractBlockEntry {
            block: BlockId::new(ordinal).unwrap(),
            operation_offset: function.operations.len(),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
        });
        function.operations.push(AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(ordinal + 2).unwrap(),
            cleanup_actions: Vec::new(),
        });
    }
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let target =
            abstract_operations_to_target_operations::lower_to_target_operations(&source, native)
                .unwrap();
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
        for mutation in 0..5 {
            let mut changed = target.clone();
            let TargetOperation::ControlGraph(graph) = &mut changed.functions[0].operation else {
                panic!("ordinary local graph");
            };
            let TargetControlTerminator::Conditional {
                condition_source,
                condition: TargetBooleanExpression::ScalarHome(home),
                ..
            } = &mut graph.blocks[0].terminator
            else {
                panic!("branch on fresh read home");
            };
            match mutation {
                0 => *condition_source = ValueId::new(1).unwrap(),
                1 => home.source_value = ValueId::new(1).unwrap(),
                2 => home.source_value = ValueId::new(99).unwrap(),
                3 => home.defining_operation = OperationId::new(1).unwrap(),
                _ => home.scalar_type = integer(IntegerSign::Unsigned, 8),
            }
            assert!(legalize_target_operations(&changed, &source, &unit).is_err());
            assert!(
                validate_legalized_operations(&changed, &source, &unit, legalized.plan().clone())
                    .is_err(),
                "branch read home mutation {mutation}"
            );
        }
        let mut changed = legalized.plan().clone();
        let legalized_operations::LegalizedScalarTerminator::Conditional { condition, .. } =
            &mut changed.scalar_functions[0].blocks[0].terminator
        else {
            panic!("legalized conditional");
        };
        *condition = ValueId::new(1).unwrap();
        assert!(validate_legalized_operations(&target, &source, &unit, changed).is_err());
    }
}
