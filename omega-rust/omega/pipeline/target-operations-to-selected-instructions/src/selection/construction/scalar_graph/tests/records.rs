//! Exact record stores survive independent replay across native targets.
use super::*;
fn record_fixture(target: target::NativeTarget, count: u16) -> LegalizedScalarFunction {
    let mut source = super::scalar_arrays::array_fixture(target, count);
    let scalar = source.blocks[0].instructions[0].result.unwrap().scalar_type;
    let signature = source.structural.as_mut().unwrap();
    let mut types = signature.structural_types.to_vec();
    types[0].shape = terminal_psi::StructuralTypeShape::Record {
        fields: (1..=u64::from(count))
            .map(|ordinal| terminal_psi::StructuralFieldDeclaration {
                id: semantic_vocabulary::StructuralFieldId::new(ordinal).unwrap(),
                identity: format!("field{ordinal}"),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: terminal_psi::StructuralFieldType::Scalar(scalar),
            })
            .collect(),
    };
    signature.structural_types = types.into();
    let row = &mut source.blocks[0].instructions[2];
    let LegalizedScalarInstructionKind::EstablishScalarArray {
        result,
        elements,
        shape,
    } = &row.kind
    else {
        panic!("fixture");
    };
    row.kind = LegalizedScalarInstructionKind::EstablishRecord {
        result: result.clone(),
        shape: *shape,
        fields: elements
            .iter()
            .enumerate()
            .map(|(ordinal, value)| terminal_psi::RecordFieldInitializer {
                field: semantic_vocabulary::StructuralFieldId::new(ordinal as u64 + 1).unwrap(),
                value: terminal_psi::RecordFieldValue::Scalar {
                    value: *value,
                    range_obligation: None,
                },
            })
            .collect(),
    };
    source
}

#[test]
fn record_selection_rejoins_fields_layout_and_store_occurrences() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let source = record_fixture(target, 2);
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
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
        let validate = |source: &LegalizedScalarFunction, selected: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                source,
                selected,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&source, &selected).unwrap();
        for mutation in 0..6 {
            let mut changed = source.clone();
            let LegalizedScalarInstructionKind::EstablishRecord {
                result,
                fields,
                shape,
            } = &mut changed.blocks[0].instructions[2].kind
            else {
                panic!("record");
            };
            match mutation {
                0 => fields.swap(0, 1),
                1 => {
                    fields.pop();
                }
                2 => shape.byte_size += 1,
                3 => result.structural_type = StructuralTypeId::new(2).unwrap(),
                4 => {
                    let first = fields[0].value.clone();
                    fields[0].value = fields[1].value.clone();
                    fields[1].value = first;
                }
                5 => result.multiplicity = terminal_psi::StructuralMultiplicity::Linear,
                _ => unreachable!(),
            }
            assert!(validate(&changed, &selected).is_err());
        }
        let mut changed = selected.clone();
        let store = changed.blocks[0]
            .instructions
            .iter_mut()
            .find(|row| matches!(row.kind, SelectedInstructionKind::Store { .. }))
            .unwrap();
        store.kind = SelectedInstructionKind::Store {
            byte_offset: 1,
            byte_size: 1,
        };
        assert!(validate(&source, &changed).is_err());
    }
}

#[test]
fn nested_record_copy_rejoins_child_identity_and_narrow_tail() {
    use semantic_vocabulary::PlaceId;
    use terminal_psi::{RecordFieldValue, StructuralAccess};
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let source = nested_fixture(target, 3);
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        let selected = build(
            0,
            &source,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap_or_else(|error| panic!("nested {target:?}: {error:?}"));
        let validate = |source: &LegalizedScalarFunction, selected: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                source,
                selected,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&source, &selected).unwrap();
        assert!(
            selected.blocks[0]
                .instructions
                .iter()
                .any(|row| matches!(row.kind, SelectedInstructionKind::Load16 { byte_offset: 0 }))
        );
        assert!(
            selected.blocks[0]
                .instructions
                .iter()
                .any(|row| matches!(row.kind, SelectedInstructionKind::Load8 { byte_offset: 2 }))
        );
        for mutation in 0..3 {
            let mut changed = source.clone();
            let LegalizedScalarInstructionKind::EstablishRecord { fields, .. } =
                &mut changed.blocks[0].instructions[3].kind
            else {
                panic!("parent");
            };
            let RecordFieldValue::Structural(argument) = &mut fields[0].value else {
                panic!("child argument");
            };
            match mutation {
                0 => argument.place = PlaceId::new(3).unwrap(),
                1 => argument.access = StructuralAccess::SharedBorrow,
                2 => argument
                    .path
                    .push(terminal_psi::StructuralPathSegment::Field("field1".into())),
                _ => unreachable!(),
            }
            assert!(validate(&changed, &selected).is_err());
        }
        let mut changed = selected.clone();
        let tail = changed.blocks[0]
            .instructions
            .iter_mut()
            .find(|row| matches!(row.kind, SelectedInstructionKind::Load8 { byte_offset: 2 }))
            .unwrap();
        tail.kind = SelectedInstructionKind::Load16 { byte_offset: 2 };
        assert!(validate(&source, &changed).is_err());
    }
}

#[test]
fn indirect_record_return_replays_hidden_entry_and_exact_writes() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let source = record_fixture(target, 24);
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        let selected = build(
            0,
            &source,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap_or_else(|error| panic!("indirect return {target:?}: {error:?}"));
        let validate = |source: &LegalizedScalarFunction, selected: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                source,
                selected,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&source, &selected).unwrap();
        if matches!(
            source.call_plan.policy,
            calling_conventions::CallingPolicy::MicrosoftX64
                | calling_conventions::CallingPolicy::SystemVAMD64
        ) {
            let SelectedTerminator::Return { instruction, .. } = &selected.blocks[0].terminator
            else {
                panic!("x64 returns the hidden result pointer");
            };
            assert!(matches!(
                instruction.kind,
                SelectedInstructionKind::ReturnScalar
            ));
            let copy = selected.blocks[0]
                .instructions
                .last()
                .expect("return ABI copy");
            assert!(matches!(copy.kind, SelectedInstructionKind::CopyI64));
            let retained = copy.operands[0].virtual_register;
            let outgoing = copy.operands[1].virtual_register;
            assert_ne!(retained, outgoing);
            assert_eq!(instruction.operands[0].virtual_register, outgoing);
            let mut changed = selected.clone();
            let SelectedTerminator::Return { instruction, .. } = &mut changed.blocks[0].terminator
            else {
                unreachable!();
            };
            instruction.operands[0].virtual_register = retained;
            assert!(validate(&source, &changed).is_err());
        }
        let mut changed = selected.clone();
        let input = changed
            .virtual_registers
            .iter_mut()
            .find(|register| register.entry_fixed_view.is_some())
            .unwrap();
        input.entry_fixed_view = None;
        assert!(validate(&source, &changed).is_err());
        let mut changed = selected.clone();
        let store = changed.blocks[0]
            .instructions
            .iter_mut()
            .rfind(|row| matches!(row.kind, SelectedInstructionKind::Store { .. }))
            .unwrap();
        store.kind = SelectedInstructionKind::Store {
            byte_offset: 24,
            byte_size: 8,
        };
        assert!(validate(&source, &changed).is_err());
        let mut changed = source.clone();
        let ValueLocation::Indirect { byte_size, .. } =
            &mut changed.call_plan.result.as_mut().unwrap().locations[0]
        else {
            panic!("hidden result");
        };
        *byte_size -= 1;
        assert!(validate(&changed, &selected).is_err());
    }
}

#[test]
fn indirect_record_call_keeps_hidden_input_separate_from_shifted_arguments() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let mut source = record_fixture(target, 24);
        let row = &mut source.blocks[0].instructions[2];
        let LegalizedScalarInstructionKind::EstablishRecord { result, shape, .. } = &row.kind
        else {
            panic!("record");
        };
        let call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![ValueShape::integer(1, 1); 2],
                result: Some(*shape),
            },
        )
        .unwrap();
        row.kind = LegalizedScalarInstructionKind::Call(LegalizedScalarCall {
            source: LegalizedCallUnitSource::AuthoredCallUnit,
            callee: MachineId::new(99).unwrap(),
            arguments: call_plan
                .parameters
                .iter()
                .enumerate()
                .map(|(position, placement)| LegalizedScalarArgument::Scalar {
                    source: ValueId::new(position as u64 + 1).unwrap(),
                    placement: placement.clone(),
                })
                .collect(),
            result_placement: call_plan.result.clone(),
            structural_result: Some(result.clone()),
            call_plan,
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        });
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        let selected = build(
            0,
            &source,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap_or_else(|error| panic!("indirect call {target:?}: {error:?}"));
        let validate = |source: &LegalizedScalarFunction, selected: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                source,
                selected,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&source, &selected).unwrap();
        let call = selected.blocks[0]
            .instructions
            .iter()
            .find(|row| matches!(row.kind, SelectedInstructionKind::CallAggregate { .. }))
            .unwrap();
        assert_eq!(call.operands.len(), 3);
        assert!(
            call.operands
                .iter()
                .all(|operand| operand.access == RegisterOperandAccess::Use)
        );
        let mut changed = selected.clone();
        let call = changed.blocks[0]
            .instructions
            .iter_mut()
            .find(|row| matches!(row.kind, SelectedInstructionKind::CallAggregate { .. }))
            .unwrap();
        let pointer = call.operands[2].virtual_register;
        call.operands[2].virtual_register = call.operands[0].virtual_register;
        assert_ne!(pointer, call.operands[2].virtual_register);
        assert!(validate(&source, &changed).is_err());
        for missing in [false, true] {
            let mut changed = selected.clone();
            let call = changed.blocks[0]
                .instructions
                .iter_mut()
                .find(|row| matches!(row.kind, SelectedInstructionKind::CallAggregate { .. }))
                .unwrap();
            if missing {
                call.operands.pop();
            } else {
                call.operands[2].access = RegisterOperandAccess::Def;
            }
            assert!(validate(&source, &changed).is_err());
        }
        let mut changed = source.clone();
        let LegalizedScalarInstructionKind::Call(call) =
            &mut changed.blocks[0].instructions[2].kind
        else {
            panic!("call");
        };
        call.arguments.swap(0, 1);
        assert!(validate(&changed, &selected).is_err());
    }
}

fn nested_fixture(target: target::NativeTarget, child_count: u16) -> LegalizedScalarFunction {
    use semantic_vocabulary::{PlaceId, StructuralFieldId, StructuralPlaceKind};
    use terminal_psi::{
        RecordFieldInitializer, RecordFieldValue, StructuralAccess, StructuralArgument,
        StructuralFieldDeclaration, StructuralFieldType, StructuralPlaceDeclaration,
        StructuralTypeDeclaration, StructuralTypeShape,
    };
    let mut source = record_fixture(target, child_count);
    let mut parent = source.blocks[0].instructions[2].clone();
    let LegalizedScalarInstructionKind::EstablishRecord { result: child, .. } = &parent.kind else {
        panic!("child");
    };
    let child = child.clone();
    let shape = ValueShape::integer(child_count + 1, 1);
    let scalar = source.blocks[0].instructions[0].result.unwrap().scalar_type;
    let operation = OperationId::new(4).unwrap();
    let parent_type = StructuralTypeId::new(3).unwrap();
    let mut result = child.clone();
    result.place = PlaceId::new(3).unwrap();
    result.structural_type = parent_type;
    parent.operation = operation;
    parent.fuel[0].site = PsiProvenance::Operation(operation);
    parent.kind = LegalizedScalarInstructionKind::EstablishRecord {
        result: result.clone(),
        shape,
        fields: vec![
            RecordFieldInitializer {
                field: StructuralFieldId::new(4).unwrap(),
                value: RecordFieldValue::Structural(StructuralArgument {
                    place: child.place,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                }),
            },
            RecordFieldInitializer {
                field: StructuralFieldId::new(5).unwrap(),
                value: RecordFieldValue::Scalar {
                    value: ValueId::new(2).unwrap(),
                    range_obligation: None,
                },
            },
        ],
    };
    source.blocks[0].instructions.push(parent);
    source.provenance.operations.push(operation);
    let signature = source.structural.as_mut().unwrap();
    let mut declarations = signature.structural_types.to_vec();
    declarations.push(StructuralTypeDeclaration {
        id: parent_type,
        identity: "parent".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![
                StructuralFieldDeclaration {
                    id: StructuralFieldId::new(4).unwrap(),
                    identity: "child".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(child.structural_type),
                },
                StructuralFieldDeclaration {
                    id: StructuralFieldId::new(5).unwrap(),
                    identity: "last".into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(scalar),
                },
            ],
        },
    });
    signature.structural_types = declarations.into();
    signature.result.as_mut().unwrap().structural_type = parent_type;
    signature
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: result.place,
            kind: StructuralPlaceKind::OperationResult {
                producer: operation,
                structural_type: parent_type,
            },
        });
    let LegalizedScalarTerminator::Return(returned) = &mut source.blocks[0].terminator else {
        panic!("return");
    };
    returned.value = LegalizedScalarReturnValue::Structural {
        source: legalized_operations::LegalizedStructuralCaseSource::OperationResult {
            operation,
            result,
        },
    };
    source.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: Vec::new(),
            result: Some(shape),
        },
    )
    .unwrap();
    source
}

#[path = "records/parameters.rs"]
mod parameters;
