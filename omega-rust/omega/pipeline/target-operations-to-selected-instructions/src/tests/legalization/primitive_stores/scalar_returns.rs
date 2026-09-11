//! Scalar-returning stores retain independent ABI, source, and physical custody.
use super::*;
use abstract_operations::AbstractFunctionResult;
use selected_instructions::{SelectedInstructionKind, SelectedMemoryAccessRole};
use target_operations::{TargetControlGraph, TargetControlTerminator};

fn fixture(
    native: NativeTarget,
    runtime: bool,
    access: StructuralAccess,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    fixture_with_scalar(native, runtime, access, integer(IntegerSign::Unsigned, 64))
}

fn fixture_with_scalar(
    native: NativeTarget,
    runtime: bool,
    access: StructuralAccess,
    scalar: ScalarType,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let (mut source, _, _) = super::fixture(native, scalar, !runtime);
    let function = &mut source.functions[0];
    function.structural_parameters[0].access = access;
    for operation in &mut function.operations {
        if let AbstractOperation::WriteOnlyPrimitiveStore { destination, .. } = operation {
            destination.access = access;
        }
    }
    let AbstractOperation::ReturnUnit { psi_edge, .. } = function.operations.pop().unwrap() else {
        panic!("Unit fixture exit")
    };
    let returned = ValueId::new(7).unwrap();
    let result = ValueId::new(8).unwrap();
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: result,
        scalar_type: scalar,
    });
    function.operations.extend([
        if scalar == ScalarType::Boolean {
            AbstractOperation::BooleanConstant {
                psi_operation: OperationId::new(3).unwrap(),
                result: returned,
                value: true,
            }
        } else { AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(3).unwrap(),
            result: returned,
            scalar_type: scalar,
            value: if matches!(scalar, ScalarType::Integer(integer) if integer.sign() == IntegerSign::Signed) {
                IntegerValue::Signed(0)
            } else { IntegerValue::Unsigned(0) },
        } },
        AbstractOperation::Return {
            psi_edge,
            result,
            value: returned,
            scalar_type: scalar,
            cleanup_actions: Vec::new(),
        },
    ]);
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

fn graph(target: &mut TargetOperationPlan) -> &mut TargetControlGraph {
    &mut target.functions[0].graph
}

#[test]
fn borrowed_primitive_writes_retain_boolean_and_fixed_integer_results() {
    let scalars = std::iter::once(ScalarType::Boolean).chain(
        [IntegerSign::Unsigned, IntegerSign::Signed]
            .into_iter()
            .flat_map(|sign| [8, 16, 32, 64].map(|bits| integer(sign, bits))),
    );
    for scalar in scalars {
        for native in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let (source, target, unit) =
                fixture_with_scalar(native, true, StructuralAccess::MutableBorrow, scalar);
            let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
            validate_legalized_operations(&target, &source, &unit, legalized.plan().clone())
                .unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = crate::selection_constraints(&legalized, &environment);
            crate::select_instructions(
                &legalized,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            for mutation in ["type", "shape", "result identity"] {
                let mut changed = target.clone();
                let result = &mut changed.functions[0]
                    .mixed_structural_scalar_abi
                    .as_mut()
                    .unwrap()
                    .result;
                match mutation {
                    "type" => {
                        result.scalar_type = if scalar == ScalarType::Boolean {
                            integer(IntegerSign::Unsigned, 8)
                        } else {
                            ScalarType::Boolean
                        }
                    }
                    "shape" => result.placement.shape.byte_size = 3,
                    "result identity" => result.value = ValueId::new(99).unwrap(),
                    _ => unreachable!(),
                }
                assert!(
                    legalize_target_operations(&changed, &source, &unit).is_err(),
                    "{scalar:?} {mutation}"
                );
                assert!(
                    validate_legalized_operations(
                        &changed,
                        &source,
                        &unit,
                        legalized.plan().clone()
                    )
                    .is_err(),
                    "{scalar:?} {mutation}"
                );
            }
        }
    }
}

fn store(target: &mut TargetOperationPlan) -> &mut TargetUnitOperation {
    graph(target).blocks[0]
        .operations
        .iter_mut()
        .find(|operation| {
            matches!(
                operation,
                TargetUnitOperation::WriteOnlyPrimitiveStore { .. }
            )
        })
        .unwrap()
}

#[test]
fn scalar_primitive_stores_select_and_replay_on_four_targets() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        for runtime in [false, true] {
            for access in [
                StructuralAccess::MutableBorrow,
                StructuralAccess::WriteOnlyBorrow,
            ] {
                let (source, target, unit) = fixture(native, runtime, access);
                let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
                validate_legalized_operations(&target, &source, &unit, legalized.plan().clone())
                    .unwrap();
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
                let function = &selected.plan().functions[0];
                assert_eq!(function.memory_accesses.len(), 1);
                assert_eq!(
                    function.memory_accesses[0].role,
                    SelectedMemoryAccessRole::WritePlace
                );
                assert!(function.calls.is_empty());
                let instructions = &function.blocks[0].instructions;
                assert_eq!(
                    instructions
                        .iter()
                        .filter(|row| matches!(
                            row.kind,
                            SelectedInstructionKind::Store {
                                byte_offset: 0,
                                byte_size: 8
                            }
                        ))
                        .count(),
                    1
                );
                for mutation in [
                    "pointer",
                    "value",
                    "width",
                    "offset",
                    "fuel",
                    "footprint",
                    "return",
                ] {
                    let mut changed = selected.plan().clone();
                    let function = &mut changed.functions[0];
                    if mutation == "footprint" {
                        function.memory_accesses[0].place = PlaceId::new(99).unwrap();
                    } else if mutation == "return" {
                        function.blocks[0].instructions.pop();
                    } else {
                        let store = function.blocks[0]
                            .instructions
                            .iter_mut()
                            .find(|row| matches!(row.kind, SelectedInstructionKind::Store { .. }))
                            .unwrap();
                        match mutation {
                            "pointer" => {
                                store.operands[0].virtual_register =
                                    store.operands[1].virtual_register
                            }
                            "value" => {
                                store.operands[1].virtual_register =
                                    store.operands[0].virtual_register
                            }
                            "width" => {
                                store.kind = SelectedInstructionKind::Store {
                                    byte_offset: 0,
                                    byte_size: 4,
                                }
                            }
                            "offset" => {
                                store.kind = SelectedInstructionKind::Store {
                                    byte_offset: 8,
                                    byte_size: 8,
                                }
                            }
                            _ => store.provenance.fuel.clear(),
                        }
                    }
                    assert!(
                        crate::validate_selected_instructions(
                            &legalized,
                            &constraints,
                            environment.physical(),
                            environment.constraints(),
                            changed
                        )
                        .is_err(),
                        "accepted {mutation}"
                    );
                }
            }
        }
    }
}

#[test]
fn scalar_primitive_store_target_rejects_source_and_return_substitutions() {
    for runtime in [false, true] {
        let (source, target, unit) = fixture(
            NativeTarget::macos_arm64(),
            runtime,
            StructuralAccess::MutableBorrow,
        );
        let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
        for mutation in [
            "access",
            "referent",
            "placement",
            "source value",
            "source type",
            "store order",
            "return operand",
            "return edge",
            "result identity",
            "result type",
            "result placement",
            "graph result",
            "graph parameter",
            "graph scalar",
        ] {
            let mut changed = target.clone();
            match mutation {
                "access" | "referent" | "placement" | "source value" | "source type" => {
                    let TargetUnitOperation::WriteOnlyPrimitiveStore {
                        destination,
                        destination_type,
                        destination_placement,
                        source: stored,
                        ..
                    } = store(&mut changed)
                    else {
                        panic!("store")
                    };
                    match mutation {
                        "access" => destination.access = StructuralAccess::SharedBorrow,
                        "referent" => {
                            destination_type.shape =
                                StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)
                        }
                        "placement" => destination_placement.shape.byte_size = 4,
                        "source value" | "source type" => match stored {
                            TargetUnitWriteOnlyPrimitiveStoreSource::Parameter {
                                source_value,
                                scalar_type,
                                ..
                            } => {
                                if mutation == "source value" {
                                    *source_value = ValueId::new(6).unwrap();
                                } else {
                                    *scalar_type = ScalarType::Boolean;
                                }
                            }
                            TargetUnitWriteOnlyPrimitiveStoreSource::IntegerImmediate {
                                source_value,
                                scalar_type,
                                ..
                            } => {
                                if mutation == "source value" {
                                    *source_value = ValueId::new(7).unwrap();
                                } else {
                                    *scalar_type =
                                        IntegerType::new(IntegerSign::Signed, 64).unwrap();
                                }
                            }
                            _ => panic!("parameter or literal"),
                        },
                        _ => unreachable!(),
                    }
                }
                "store order" => graph(&mut changed).blocks[0].operations.swap(0, 1),
                "graph result" => graph(&mut changed).call_plan.result = None,
                "graph parameter" => {
                    graph(&mut changed).parameters[0].access = StructuralAccess::Owned
                }
                "graph scalar" => graph(&mut changed).scalar_parameters.clear(),
                "return operand" | "return edge" => {
                    let TargetControlTerminator::ReturnScalar {
                        source_value,
                        psi_edge,
                        ..
                    } = &mut graph(&mut changed).blocks[0].terminator
                    else {
                        panic!("return")
                    };
                    if mutation == "return operand" {
                        *source_value = ValueId::new(5).unwrap();
                    } else {
                        *psi_edge = semantic_vocabulary::EdgeId::new(99).unwrap();
                    }
                }
                _ => {
                    let abi = changed.functions[0]
                        .mixed_structural_scalar_abi
                        .as_mut()
                        .unwrap();
                    match mutation {
                        "result identity" => abi.result.value = ValueId::new(7).unwrap(),
                        "result type" => abi.result.scalar_type = ScalarType::Boolean,
                        _ => {
                            abi.result.placement.shape =
                                calling_conventions::ValueShape::integer(4, 4)
                        }
                    }
                }
            }
            if mutation == "graph scalar" && !runtime {
                continue;
            }
            reject_target(&source, &changed, &unit, legalized.plan());
        }
    }
}

#[test]
fn scalar_primitive_store_legalized_replay_rejects_effect_and_result_drift() {
    let (source, target, unit) = fixture(
        NativeTarget::linux_x64(),
        true,
        StructuralAccess::MutableBorrow,
    );
    let legal = legalize_target_operations(&target, &source, &unit).unwrap();
    for mutation in [
        "width",
        "value",
        "destination",
        "access",
        "fuel",
        "effect",
        "result",
        "result ABI",
        "result type",
    ] {
        let mut changed = legal.plan().clone();
        if mutation == "result ABI" {
            changed.scalar_functions[0].call_plan.result = None;
        } else if mutation == "result type" {
            let legalized_operations::LegalizedScalarTerminator::Return(returned) =
                &mut changed.scalar_functions[0].blocks[0].terminator
            else {
                panic!("return")
            };
            returned.value = legalized_operations::LegalizedScalarReturnValue::Unit;
        } else {
            let row = legalized_store(&mut changed);
            let LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
                destination,
                value,
                byte_size,
            } = &mut row.kind
            else {
                panic!("store")
            };
            match mutation {
                "width" => *byte_size = 4,
                "value" => value.value = ValueId::new(6).unwrap(),
                "destination" => destination.place = PlaceId::new(99).unwrap(),
                "access" => destination.access = StructuralAccess::Owned,
                "fuel" => row.fuel.clear(),
                "effect" => row.effect.output += 1,
                _ => {
                    row.result = Some(legalized_operations::LegalizedValueDefinition {
                        value: ValueId::new(99).unwrap(),
                        scalar_type: integer(IntegerSign::Unsigned, 64),
                        definition_site: optimization_unit::ValueDefinitionSite::Node {
                            block: source.functions[0].entry,
                            node: 0,
                        },
                    })
                }
            }
        }
        assert!(
            validate_legalized_operations(&target, &source, &unit, changed).is_err(),
            "accepted {mutation}"
        );
    }
}

#[test]
fn scalar_primitive_store_accepts_stack_input_and_reference_with_exact_return_location() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        for return_stack_input in [false, true] {
            let (mut source, _, _) = fixture(native, true, StructuralAccess::MutableBorrow);
            let scalar = integer(IntegerSign::Unsigned, 64);
            let function = &mut source.functions[0];
            function
                .parameters
                .extend((10..=16).map(|raw| AbstractParameter {
                    value: ValueId::new(raw).unwrap(),
                    scalar_type: scalar,
                }));
            let AbstractOperation::WriteOnlyPrimitiveStore { value, .. } =
                &mut function.operations[0]
            else {
                panic!("store")
            };
            value.value = ValueId::new(16).unwrap();
            let AbstractOperation::Return { value, .. } = function.operations.last_mut().unwrap()
            else {
                panic!("return")
            };
            *value = ValueId::new(if return_stack_input { 16 } else { 5 }).unwrap();
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &source, native,
            )
            .unwrap();
            let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap();
            let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
            validate_legalized_operations(&target, &source, &unit, legalized.plan().clone())
                .unwrap();
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
            for mutation in ["offset", "ordinal", "identity", "missing load"] {
                let mut changed = selected.plan().clone();
                let instructions = &mut changed.functions[0].blocks[0].instructions;
                let position = instructions
                    .iter()
                    .position(|instruction| {
                        matches!(
                            instruction.kind,
                            SelectedInstructionKind::FrameAddress {
                                slot: selected_instructions::FrameStorageSlotId::Incoming {
                                    parameter_index: 8,
                                    ..
                                },
                                ..
                            }
                        )
                    })
                    .unwrap();
                if mutation == "missing load" {
                    instructions.remove(position + 1);
                } else {
                    let instruction = &mut instructions[position];
                    let SelectedInstructionKind::FrameAddress {
                        slot:
                            selected_instructions::FrameStorageSlotId::Incoming {
                                parameter_index,
                                abi_stack_byte_offset,
                            },
                        ..
                    } = &mut instruction.kind
                    else {
                        panic!("incoming scalar address")
                    };
                    match mutation {
                        "offset" => *abi_stack_byte_offset += 8,
                        "ordinal" => *parameter_index = 9,
                        _ => instruction.provenance.values = vec![ValueId::new(5).unwrap()],
                    }
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
                    "accepted stack scalar {mutation}"
                );
            }
            let mut changed = target.clone();
            let TargetControlTerminator::ReturnScalar {
                expression:
                    target_operations::TargetScalarExpression::Integer {
                        expression:
                            target_operations::TargetIntegerExpression::Parameter { location, .. },
                        ..
                    },
                ..
            } = &mut graph(&mut changed).blocks[0].terminator
            else {
                panic!("parameter return")
            };
            *location =
                target_operations::ScalarParameterLocation::IncomingStack { byte_offset: 4096 };
            reject_target(&source, &changed, &unit, legalized.plan());
        }
    }
}

#[test]
fn boolean_store_scalar_result_requires_exact_mixed_header() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let (mut source, _, _) = fixture(native, true, StructuralAccess::MutableBorrow);
        source.structural_types.make_mut()[0].shape =
            StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean);
        source.functions[0].parameters[0].scalar_type = ScalarType::Boolean;
        let AbstractOperation::WriteOnlyPrimitiveStore { value, .. } =
            &mut source.functions[0].operations[0]
        else {
            panic!("primitive store");
        };
        value.scalar_type = ScalarType::Boolean;
        let target =
            abstract_operations_to_target_operations::lower_to_target_operations(&source, native)
                .unwrap();
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
        for mutation in 0..3 {
            let mut changed = target.clone();
            let header = &mut changed.functions[0].mixed_structural_scalar_abi;
            match mutation {
                0 => *header = None,
                1 => {
                    header.as_mut().unwrap().scalar_parameters[0].scalar_type =
                        integer(IntegerSign::Unsigned, 8)
                }
                _ => {
                    header.as_mut().unwrap().scalar_parameters[0].value = ValueId::new(99).unwrap()
                }
            }
            assert!(legalize_target_operations(&changed, &source, &unit).is_err());
            assert!(
                validate_legalized_operations(&changed, &source, &unit, legalized.plan().clone())
                    .is_err(),
                "Boolean mixed header mutation {mutation}"
            );
        }
    }
}
