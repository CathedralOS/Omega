//! Raw selection/replay controls, not source admission or native execution claims.
use super::*;
mod shared_records;
use terminal_psi::{
    StructuralAccess, StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
};

#[test]
fn borrowed_unit_calls_preserve_fixed_integer_and_boolean_parameter_types() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        for scalar_type in
            [ScalarType::Boolean]
                .into_iter()
                .chain([8, 16, 32, 64].into_iter().flat_map(|bits| {
                    [IntegerSign::Signed, IntegerSign::Unsigned]
                        .into_iter()
                        .map(move |sign| ScalarType::Integer(IntegerType::new(sign, bits).unwrap()))
                }))
        {
            let mut source = projected_call(target);
            let scalar_shape =
                crate::selection::scalar_call_abi::scalar_shape(scalar_type).unwrap();
            let root_shape = source.call_plan.parameters[0].shape;
            source.call_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: vec![scalar_shape, root_shape],
                    result: None,
                },
            )
            .unwrap();
            let value = ValueId::new(100).unwrap();
            source.parameters = vec![LegalizedScalarParameter {
                value,
                scalar_type,
                definition_site: ValueDefinitionSite::FunctionParameter(0),
                placement: source.call_plan.parameters[0].clone(),
            }];
            source.structural.as_mut().unwrap().parameters[0]
                .target
                .placement = source.call_plan.parameters[1].clone();
            let LegalizedScalarInstructionKind::Call(call) =
                &mut source.blocks[0].instructions[0].kind
            else {
                panic!("call");
            };
            let mut argument = call.arguments[0].clone();
            let leaf_shape = argument.placement().shape;
            call.call_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: vec![scalar_shape, leaf_shape],
                    result: None,
                },
            )
            .unwrap();
            let LegalizedScalarArgument::Structural {
                target: target_argument,
                ..
            } = &mut argument
            else {
                panic!("reference");
            };
            target_argument.source = source.call_plan.parameters[1].clone().into();
            target_argument.destination = call.call_plan.parameters[1].clone();
            call.arguments = vec![
                LegalizedScalarArgument::Scalar {
                    source: value,
                    placement: call.call_plan.parameters[0].clone(),
                },
                argument,
            ];
            let ValueLocation::Register { register, .. } =
                source.parameters[0].placement.locations[0]
            else {
                panic!("scalar register parameter");
            };
            let constraints = SelectedSelectionConstraints {
                keys: environment.selected_keys(),
                fixed_inputs: vec![SelectedFixedInputConstraint {
                    machine: source.machine,
                    source_value: value,
                    parameter_index: 0,
                    register,
                    fixed_view: environment.fixed_register_view(register).unwrap(),
                }],
            };
            let selected = build(
                0,
                &source,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            crate::selection::validation::scalar_graph::validate(
                0,
                &source,
                &selected,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            assert_eq!(
                selected.calls[0].call.arguments[0].placement().shape,
                scalar_shape
            );
            assert_eq!(selected.virtual_registers.iter().find(|register|
                matches!(register.origin, VirtualRegisterOrigin::EntryParameter { source_value, .. }
                    if source_value == value)).unwrap().scalar_type, scalar_type);
        }
    }
}

#[test]
fn outgoing_projected_pointer_stack_slot_replays_exact_bits_and_call_registers() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        let mut source = projected_call(target);
        let prefix = match source.call_plan.policy {
            CallingPolicy::MicrosoftX64 => 4,
            CallingPolicy::SystemVAMD64 => 6,
            _ => 8,
        };
        let value = ValueId::new(100).unwrap();
        let operation = OperationId::new(100).unwrap();
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let mut literal = source.blocks[0].instructions[0].clone();
        literal.operation = operation;
        literal.result = Some(LegalizedValueDefinition {
            value,
            scalar_type,
            definition_site: ValueDefinitionSite::Node {
                block: source.entry_block,
                node: 0,
            },
        });
        literal.kind = LegalizedScalarInstructionKind::Constant(IntegerValue::Unsigned(17));
        literal.ownership.clear();
        literal.fuel = vec![FuelSettlement {
            site: PsiProvenance::Operation(operation),
            units: 1,
        }];
        source.blocks[0].instructions.insert(0, literal);
        source.provenance.operations.insert(0, operation);
        let LegalizedScalarInstructionKind::Call(call) = &mut source.blocks[0].instructions[1].kind
        else {
            panic!("call fixture");
        };
        let mut argument = call.arguments[0].clone();
        let leaf_shape = argument.placement().shape;
        call.call_plan = evaluate_call_plan(
            source.call_plan.policy,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8); prefix]
                    .into_iter()
                    .chain([leaf_shape])
                    .collect(),
                result: None,
            },
        )
        .unwrap();
        let LegalizedScalarArgument::Structural {
            target: structural, ..
        } = &mut argument
        else {
            panic!("borrowed argument");
        };
        structural.destination = call.call_plan.parameters[prefix].clone();
        let expected_offset =
            crate::structural_reference_input::stack_pointer_offset(&structural.destination)
                .unwrap();
        call.arguments = call.call_plan.parameters[..prefix]
            .iter()
            .map(|placement| LegalizedScalarArgument::Scalar {
                source: value,
                placement: placement.clone(),
            })
            .chain([argument])
            .collect();
        let selected = build(
            0,
            &source,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        let validate = |candidate: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                &source,
                candidate,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&selected).unwrap();
        assert_eq!(selected.outgoing_arguments.len(), 1);
        assert_eq!(selected.outgoing_arguments[0].byte_size, 8);
        assert_eq!(
            selected.outgoing_arguments[0].abi_stack_byte_offset,
            expected_offset
        );
        assert_eq!(selected.calls[0].call.arguments.len(), prefix + 1);
        let call_index = selected.blocks[0]
            .instructions
            .iter()
            .position(|instruction| {
                matches!(instruction.kind, SelectedInstructionKind::CallUnit { .. })
            })
            .unwrap();
        assert_eq!(
            selected.blocks[0].instructions[call_index].operands.len(),
            prefix
        );
        let store_index = selected.blocks[0]
            .instructions
            .iter()
            .position(|instruction| {
                matches!(instruction.kind, SelectedInstructionKind::Store64 { .. })
            })
            .unwrap();
        assert_eq!(selected.memory_accesses.len(), 1);
        let mut scalar_stack = source.clone();
        let LegalizedScalarInstructionKind::Call(call) =
            &mut scalar_stack.blocks[0].instructions[1].kind
        else {
            panic!("call fixture");
        };
        let mut argument = call.arguments.last().unwrap().clone();
        call.call_plan = evaluate_call_plan(
            source.call_plan.policy,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8); prefix + 1]
                    .into_iter()
                    .chain([leaf_shape])
                    .collect(),
                result: None,
            },
        )
        .unwrap();
        let LegalizedScalarArgument::Structural {
            target: structural, ..
        } = &mut argument
        else {
            panic!("borrowed argument");
        };
        structural.destination = call.call_plan.parameters[prefix + 1].clone();
        call.arguments = call.call_plan.parameters[..prefix + 1]
            .iter()
            .map(|placement| LegalizedScalarArgument::Scalar {
                source: value,
                placement: placement.clone(),
            })
            .chain([argument])
            .collect();
        let with_scalar_stack = build(
            0,
            &scalar_stack,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .expect("exact scalar and borrowed-pointer stack fragments coexist");
        crate::selection::validation::scalar_graph::validate(
            0,
            &scalar_stack,
            &with_scalar_stack,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        assert_eq!(with_scalar_stack.outgoing_arguments.len(), 2);
        assert_eq!(with_scalar_stack.memory_accesses.len(), 1);
        assert!(
            with_scalar_stack.blocks[0]
                .instructions
                .iter()
                .any(|instruction| {
                    matches!(
                        instruction.kind,
                        SelectedInstructionKind::Store { byte_size: 8, .. }
                    )
                })
        );
        assert!(
            validate(&with_scalar_stack).is_err(),
            "a new stack actual changes source custody"
        );
        for mutation in 0..6 {
            let mut changed = selected.clone();
            match mutation {
                0 => changed.outgoing_arguments[0].abi_stack_byte_offset += 8,
                1 => changed.outgoing_arguments[0].byte_size = 2,
                2 => changed.outgoing_arguments[0].id.argument_index = 0,
                3 => {
                    changed.memory_accesses[0].role =
                        selected_instructions::SelectedMemoryAccessRole::ReadPlace
                }
                4 => {
                    changed.blocks[0].instructions[store_index].operands[0].virtual_register =
                        changed.blocks[0].instructions[call_index].operands[0].virtual_register
                }
                _ => {
                    changed.blocks[0].instructions[store_index].provenance.fuel =
                        source.blocks[0].instructions[1].fuel.clone()
                }
            }
            assert!(
                validate(&changed).is_err(),
                "mutation {mutation} on {target:?}"
            );
        }
    }
}

pub(super) fn projected_call(target: target::NativeTarget) -> LegalizedScalarFunction {
    let mut source = borrowed_calls::borrowed_call(target);
    let record = StructuralTypeId::new(2).unwrap();
    let root_shape = ValueShape::borrowed_reference(4, 2);
    let leaf_shape = ValueShape::borrowed_reference(2, 2);
    source.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![root_shape],
            result: None,
        },
    )
    .unwrap();
    let signature = source.structural.as_mut().unwrap();
    signature.structural_types.make_mut()[0].shape = StructuralTypeShape::FixedArray {
        element: record,
        length: 2,
    };
    signature
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: record,
            identity: "Record".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                    identity: "value".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
                    )),
                }],
            },
        });
    let parameter = &mut signature.parameters[0];
    parameter.semantic.access = StructuralAccess::WriteOnlyBorrow;
    parameter.target.access = StructuralAccess::WriteOnlyBorrow;
    parameter.target.shape = root_shape;
    parameter.target.placement = source.call_plan.parameters[0].clone();
    let row = &mut source.blocks[0].instructions[0];
    row.result = None;
    let LegalizedScalarInstructionKind::Call(call) = &mut row.kind else {
        panic!("call fixture")
    };
    call.call_plan = evaluate_call_plan(
        source.call_plan.policy,
        &CallSignature {
            parameters: vec![leaf_shape],
            result: None,
        },
    )
    .unwrap();
    call.result_placement = None;
    let LegalizedScalarArgument::Structural { semantic, target } = &mut call.arguments[0] else {
        panic!("structural argument")
    };
    semantic.access = StructuralAccess::WriteOnlyBorrow;
    semantic.path = vec![StructuralPathSegment::FixedIndex(1)];
    target.access = semantic.access;
    target.path = semantic.path.clone();
    target.structural_type = record;
    target.shape = leaf_shape;
    target.source_byte_offset = 2;
    target.source = parameter.target.placement.clone().into();
    target.destination = call.call_plan.parameters[0].clone();
    returned(&mut source.blocks[0]).value = LegalizedScalarReturnValue::Unit;
    source
}

#[test]
fn projected_write_only_call_replays_original_pointer_offset_and_contract() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        let source = projected_call(target);
        let construct = |source: &LegalizedScalarFunction| {
            build(
                0,
                source,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        let selected = construct(&source).expect("exact projected write-only call selects");
        let validate = |source: &LegalizedScalarFunction, candidate: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                source,
                candidate,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&source, &selected).unwrap();
        assert!(
            selected.memory_accesses.is_empty(),
            "pointer adjustment never reads or copies the referent"
        );
        assert!(selected.outgoing_arguments.is_empty());
        let address = selected.blocks[0]
            .instructions
            .iter()
            .position(|instruction| {
                matches!(
                    instruction.kind,
                    SelectedInstructionKind::AddressOffset { byte_offset: 2 }
                )
            })
            .expect("literal index becomes its exact byte offset");
        assert!(
            selected.blocks[0].instructions[address]
                .provenance
                .fuel
                .is_empty()
        );
        assert_eq!(selected.calls.len(), 1);

        for mutation in 0..10 {
            let mut candidate = selected.clone();
            match mutation {
                0 => {
                    candidate.blocks[0].instructions[address].kind =
                        SelectedInstructionKind::AddressOffset { byte_offset: 0 }
                }
                1 => {
                    candidate.blocks[0].instructions[address].kind =
                        SelectedInstructionKind::CopyI64
                }
                2 => candidate.blocks[0].instructions[address]
                    .operands
                    .swap(0, 1),
                3 => {
                    candidate.blocks[0].instructions[address].provenance.fuel =
                        source.blocks[0].instructions[0].fuel.clone()
                }
                4 => candidate.calls[0].effect.output += 1,
                mutation => {
                    let LegalizedScalarArgument::Structural { semantic, target } =
                        &mut candidate.calls[0].call.arguments[0]
                    else {
                        panic!("structural call")
                    };
                    match mutation {
                        5 => target.source_byte_offset = 0,
                        6 => semantic.path = vec![StructuralPathSegment::FixedIndex(0)],
                        7 => target.source = target.destination.clone().into(),
                        8 => {
                            semantic.access = StructuralAccess::MutableBorrow;
                            target.access = semantic.access;
                        }
                        _ => target.path = vec![StructuralPathSegment::FixedIndex(0)],
                    }
                }
            }
            assert!(
                validate(&source, &candidate).is_err(),
                "selected mutation {mutation}"
            );
        }

        for mutation in 0..7 {
            let mut changed = source.clone();
            let LegalizedScalarInstructionKind::Call(call) =
                &mut changed.blocks[0].instructions[0].kind
            else {
                panic!("call fixture")
            };
            let LegalizedScalarArgument::Structural { semantic, target } = &mut call.arguments[0]
            else {
                panic!("structural call")
            };
            match mutation {
                0 => target.source_byte_offset = 0,
                1 => {
                    semantic.path = vec![StructuralPathSegment::FixedIndex(0)];
                    target.path = semantic.path.clone();
                }
                2 => target.source = target.destination.clone().into(),
                3 => {
                    semantic.access = StructuralAccess::MutableBorrow;
                    target.access = semantic.access;
                }
                4 => {
                    semantic.path = vec![StructuralPathSegment::FixedIndex(2)];
                    target.path = semantic.path.clone();
                    target.source_byte_offset = 4;
                }
                5 => target.structural_type = target.root_structural_type,
                _ => target.shape = ValueShape::integer(2, 2),
            }
            assert!(construct(&changed).is_err(), "source mutation {mutation}");
            assert!(
                validate(&changed, &selected).is_err(),
                "receiving mutation {mutation}"
            );
        }
    }
}
