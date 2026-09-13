//! Local sums lend their established storage without a record-shaped surrogate.
use super::*;

#[test]
fn shared_sum_call_replays_original_home_layout_and_consumed_scalar_result() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let mut source = local_fixture(native, false);
        let field = semantic_vocabulary::StructuralFieldId::new(1).unwrap();
        let scalar = source.blocks[0].instructions[0].result.unwrap().scalar_type;
        let case = semantic_vocabulary::StructuralCaseId::new(1).unwrap();
        let layout = calling_conventions::evaluate_conventional_sum_layout(
            &[],
            &[vec![ValueShape::integer(8, 8)], Vec::new()],
        )
        .unwrap();
        source
            .structural
            .as_mut()
            .unwrap()
            .structural_types
            .make_mut()[0]
            .shape = StructuralTypeShape::Sum {
            cases: vec![
                terminal_psi::StructuralCaseDeclaration {
                    id: case,
                    identity: "Present".into(),
                    fields: vec![terminal_psi::StructuralFieldDeclaration {
                        id: field,
                        identity: "value".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: terminal_psi::StructuralFieldType::Scalar(scalar),
                    }],
                },
                terminal_psi::StructuralCaseDeclaration {
                    id: semantic_vocabulary::StructuralCaseId::new(2).unwrap(),
                    identity: "Absent".into(),
                    fields: Vec::new(),
                },
            ],
        };
        let LegalizedScalarInstructionKind::EstablishPrimitiveLocal { result, value, .. } =
            source.blocks[0].instructions[1].kind.clone()
        else {
            panic!("local");
        };
        source.blocks[0].instructions[1].kind =
            LegalizedScalarInstructionKind::EstablishScalarCase {
                result,
                result_case: case,
                layout: layout.clone(),
                fields: vec![terminal_psi::ScalarCaseField {
                    field,
                    value: value.value,
                    range_obligation: None,
                }],
            };
        let LegalizedScalarInstructionKind::Call(call) = &mut source.blocks[0].instructions[2].kind
        else {
            panic!("call");
        };
        call.call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(native),
            &CallSignature {
                parameters: vec![ValueShape::borrowed_reference(
                    layout.shape.byte_size,
                    layout.shape.alignment,
                )],
                result: call
                    .result_placement
                    .as_ref()
                    .map(|placement| placement.shape),
            },
        )
        .unwrap();
        let LegalizedScalarArgument::Structural { semantic, target } = &mut call.arguments[0]
        else {
            panic!("argument");
        };
        target.shape = call.call_plan.parameters[0].shape;
        target.destination = call.call_plan.parameters[0].clone();
        semantic.access = StructuralAccess::SharedBorrow;
        target.access = StructuralAccess::SharedBorrow;
        target.source = target_operations::TargetStructuralArgumentSource::StructuralHome {
            psi_operation: OperationId::new(2).unwrap(),
        };
        // The following scalar call consumes the getter result, not its input field.
        source.blocks[0].instructions[3] = fixture(native, 0).blocks[0].instructions[3].clone();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: Vec::new(),
        };
        let construct = |source: &LegalizedScalarFunction| {
            build(
                0,
                source,
                native,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        let selected = construct(&source).unwrap();
        let validate = |source: &LegalizedScalarFunction, selected: &SelectedFunction| {
            crate::selection::validation::scalar_graph::validate(
                0,
                source,
                selected,
                native,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&source, &selected).unwrap();
        assert_eq!(selected.local_storage_slots.len(), 1);
        for mutation in 0..7 {
            let mut changed = source.clone();
            let LegalizedScalarInstructionKind::Call(call) =
                &mut changed.blocks[0].instructions[2].kind
            else {
                panic!("call");
            };
            let LegalizedScalarArgument::Structural { semantic, target } = &mut call.arguments[0]
            else {
                panic!("argument");
            };
            match mutation {
                0 => {
                    target.source =
                        target_operations::TargetStructuralArgumentSource::StructuralHome {
                            psi_operation: OperationId::new(99).unwrap(),
                        }
                }
                1 => target.source_byte_offset = 1,
                2 => {
                    semantic.access = StructuralAccess::MutableBorrow;
                }
                3 => target.access = StructuralAccess::MutableBorrow,
                4 => {
                    semantic.place = PlaceId::new(99).unwrap();
                    target.place = semantic.place;
                }
                5 => target.shape.alignment = 4,
                _ => target.root_structural_type = StructuralTypeId::new(99).unwrap(),
            }
            assert!(
                construct(&changed).is_err(),
                "construction mutation {mutation}"
            );
            assert!(
                validate(&changed, &selected).is_err(),
                "replay mutation {mutation}"
            );
        }
        let mut changed_source = source.clone();
        let contract = changed_source.structural.as_mut().unwrap();
        contract.structural_places[0].kind = StructuralPlaceKind::OperationResult {
            producer: OperationId::new(99).unwrap(),
            structural_type: StructuralTypeId::new(1).unwrap(),
        };
        assert!(construct(&changed_source).is_err());
        assert!(validate(&changed_source, &selected).is_err());
        let mut changed = selected.clone();
        let address = changed.blocks[0]
            .instructions
            .iter_mut()
            .rev()
            .find(|instruction| {
                matches!(
                    instruction.kind,
                    SelectedInstructionKind::FrameAddress { .. }
                )
            })
            .unwrap();
        let SelectedInstructionKind::FrameAddress { byte_offset, .. } = &mut address.kind else {
            panic!("address");
        };
        *byte_offset = 1;
        assert!(validate(&source, &changed).is_err());
    }
}
