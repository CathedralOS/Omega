//! Nested shared receivers retain the root pointer and exact field displacement.
use super::*;

fn shared_record_call(native: target::NativeTarget, projected: bool) -> LegalizedScalarFunction {
    let mut source = borrowed_calls::borrowed_call(native);
    let outer = StructuralTypeId::new(1).unwrap();
    let inner = StructuralTypeId::new(2).unwrap();
    let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let scalar_field = |id, name: &str| StructuralFieldDeclaration {
        id: semantic_vocabulary::StructuralFieldId::new(id).unwrap(),
        identity: name.into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(scalar),
    };
    let record_field = |id, name: &str| StructuralFieldDeclaration {
        id: semantic_vocabulary::StructuralFieldId::new(id).unwrap(),
        identity: name.into(),
        relevance: terminal_psi::BindingRelevance::Relevant,
        field_type: StructuralFieldType::Structural(inner),
    };
    let root_shape = ValueShape::borrowed_reference(40, 8);
    let referent_shape = if projected {
        ValueShape::borrowed_reference(16, 8)
    } else {
        root_shape
    };
    source.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(native),
        &CallSignature {
            parameters: vec![root_shape],
            result: Some(ValueShape::integer(8, 8)),
        },
    )
    .unwrap();
    source.attachment = Some(outer);
    let contract = source.structural.as_mut().unwrap();
    contract.structural_types = vec![
        StructuralTypeDeclaration {
            id: outer,
            identity: "Outer".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    scalar_field(1, "prefix"),
                    record_field(2, "inner"),
                    record_field(3, "other"),
                ],
            },
        },
        StructuralTypeDeclaration {
            id: inner,
            identity: "Inner".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![scalar_field(1, "left"), scalar_field(2, "right")],
            },
        },
    ]
    .into();
    contract.parameters[0].semantic.is_self = true;
    contract.parameters[0].target.shape = root_shape;
    contract.parameters[0].target.placement = source.call_plan.parameters[0].clone();
    contract.structural_places[0].kind = semantic_vocabulary::StructuralPlaceKind::Parameter {
        position: 0,
        is_self: true,
    };
    let LegalizedScalarInstructionKind::Call(call) = &mut source.blocks[0].instructions[0].kind
    else {
        panic!("call");
    };
    call.call_plan = evaluate_call_plan(
        source.call_plan.policy,
        &CallSignature {
            parameters: vec![referent_shape],
            result: Some(ValueShape::integer(8, 8)),
        },
    )
    .unwrap();
    call.result_placement = call.call_plan.result.clone();
    let LegalizedScalarArgument::Structural { semantic, target } = &mut call.arguments[0] else {
        panic!("receiver");
    };
    semantic.path = if projected {
        vec![StructuralPathSegment::Field("inner".into())]
    } else {
        Vec::new()
    };
    target.path = semantic.path.clone();
    target.structural_type = if projected { inner } else { outer };
    target.shape = referent_shape;
    target.source_byte_offset = if projected { 8 } else { 0 };
    target.source = source.call_plan.parameters[0].clone().into();
    target.destination = call.call_plan.parameters[0].clone();
    source
}

#[test]
fn shared_nested_record_call_replays_original_root_and_same_typed_sibling() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
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
        for projected in [false, true] {
            let source = shared_record_call(native, projected);
            let selected = construct(&source).unwrap();
            validate(&source, &selected).unwrap();
            assert!(
                selected.local_storage_slots.is_empty(),
                "a borrowed receiver has no copied referent home"
            );
            if !projected {
                continue;
            }
            for mutation in 0..5 {
                let mut changed = source.clone();
                let LegalizedScalarInstructionKind::Call(call) =
                    &mut changed.blocks[0].instructions[0].kind
                else {
                    panic!("call");
                };
                let LegalizedScalarArgument::Structural { semantic, target } =
                    &mut call.arguments[0]
                else {
                    panic!("receiver");
                };
                match mutation {
                    0 => semantic.path = vec![StructuralPathSegment::Field("other".into())],
                    1 => {
                        semantic.path = vec![StructuralPathSegment::Field("other".into())];
                        target.path = semantic.path.clone();
                    }
                    2 => target.source_byte_offset = 24,
                    3 => target.root_structural_type = target.structural_type,
                    _ => {
                        semantic.access = StructuralAccess::WriteOnlyBorrow;
                        target.access = semantic.access;
                    }
                }
                assert!(
                    construct(&changed).is_err(),
                    "invalid receiver mutation {mutation}"
                );
                assert!(validate(&changed, &selected).is_err());
            }
            let mut sibling = source.clone();
            let LegalizedScalarInstructionKind::Call(call) =
                &mut sibling.blocks[0].instructions[0].kind
            else {
                panic!("call");
            };
            let LegalizedScalarArgument::Structural { semantic, target } = &mut call.arguments[0]
            else {
                panic!("receiver");
            };
            semantic.path = vec![StructuralPathSegment::Field("other".into())];
            target.path = semantic.path.clone();
            target.source_byte_offset = 24;
            let sibling_selected = construct(&sibling).unwrap();
            validate(&sibling, &sibling_selected).unwrap();
            assert!(
                validate(&source, &sibling_selected).is_err(),
                "same-type sibling is not the authored receiver"
            );
            let mut changed = selected.clone();
            let address = changed.blocks[0]
                .instructions
                .iter_mut()
                .find(|instruction| {
                    matches!(
                        instruction.kind,
                        SelectedInstructionKind::AddressOffset { byte_offset: 8 }
                    )
                })
                .unwrap();
            address.kind = SelectedInstructionKind::AddressOffset { byte_offset: 24 };
            assert!(
                validate(&source, &changed).is_err(),
                "physical sibling displacement rejects"
            );
        }
    }
}
