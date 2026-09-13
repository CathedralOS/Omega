//! Owned return custody is independent of input and result ABI placement.
use super::*;
use semantic_vocabulary::{PlaceId, StructuralFieldId, StructuralPlaceKind};
use terminal_psi::{
    BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeShape,
};

fn parameter_return(
    target: target::NativeTarget,
    bytes: u16,
    multiplicity: StructuralMultiplicity,
) -> LegalizedScalarFunction {
    let mut source = super::scalar_arrays::array_fixture(target, bytes);
    source.blocks[0].instructions.clear();
    source.provenance.operations.clear();
    let shape = ValueShape::integer(bytes, 1);
    source.call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![shape],
            result: Some(shape),
        },
    )
    .unwrap();
    let signature = source.structural.as_mut().unwrap();
    signature.structural_types.make_mut()[0].shape = StructuralTypeShape::Record {
        fields: (1..=bytes)
            .map(|ordinal| StructuralFieldDeclaration {
                id: StructuralFieldId::new(u64::from(ordinal)).unwrap(),
                identity: format!("byte{ordinal}"),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                )),
            })
            .collect(),
    };
    let place = PlaceId::new(1).unwrap();
    let structural_type = signature.structural_types[0].id;
    signature.parameters = vec![legalized_operations::LegalizedCallUnitParameter {
        semantic: StructuralParameterDeclaration {
            place,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
        target: target_operations::TargetStructuralParameter {
            place,
            structural_type,
            multiplicity,
            access: StructuralAccess::Owned,
            projected_qualifications: Vec::new(),
            shape,
            placement: source.call_plan.parameters[0].clone(),
        },
    }];
    signature.structural_places = vec![StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];
    signature.result.as_mut().unwrap().multiplicity = multiplicity;
    let LegalizedScalarTerminator::Return(returned) = &mut source.blocks[0].terminator else {
        panic!("return fixture");
    };
    returned.value = LegalizedScalarReturnValue::StructuralParameter { place };
    source
}

#[test]
fn owned_parameter_returns_replay_the_current_source_and_exact_result_transfer() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        for (bytes, multiplicity) in [
            (24, StructuralMultiplicity::Affine),
            (9, StructuralMultiplicity::Unrestricted),
        ] {
            let source = parameter_return(target, bytes, multiplicity);
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let constraints = SelectedSelectionConstraints {
                keys: environment.selected_keys(),
                fixed_inputs: Vec::new(),
            };
            let select = |source: &LegalizedScalarFunction| {
                build(
                    0,
                    source,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
            };
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
            let selected = select(&source).unwrap();
            validate(&source, &selected).unwrap();
            assert!(
                selected.local_storage_slots.is_empty(),
                "return needs no extra value copy"
            );

            let mut wrong_edge = selected.clone();
            let SelectedTerminator::Return { instruction, .. } =
                &mut wrong_edge.blocks[0].terminator
            else {
                panic!("selected return");
            };
            instruction.provenance.edges.clear();
            assert!(validate(&source, &wrong_edge).is_err());

            let mut wrong_owner = source.clone();
            let signature = wrong_owner.structural.as_mut().unwrap();
            signature.parameters[0].semantic.access = StructuralAccess::SharedBorrow;
            signature.parameters[0].target.access = StructuralAccess::SharedBorrow;
            assert!(select(&wrong_owner).is_err());
            assert!(validate(&wrong_owner, &selected).is_err());

            if crate::selection::aggregate_result_input::indirect_result(
                source.call_plan.result.as_ref().unwrap(),
                source.call_plan.policy,
            )
            .is_none()
            {
                continue;
            }
            for mutation in ["pointer", "offset", "width", "memory"] {
                let mut changed = selected.clone();
                if mutation == "memory" {
                    changed.memory_accesses.pop().unwrap();
                } else {
                    let store = changed.blocks[0]
                        .instructions
                        .iter_mut()
                        .find(|row| matches!(row.kind, SelectedInstructionKind::Store { .. }))
                        .unwrap();
                    match mutation {
                        "pointer" => store.operands.swap(0, 1),
                        "offset" => {
                            store.kind = SelectedInstructionKind::Store {
                                byte_offset: 1,
                                byte_size: 8,
                            }
                        }
                        "width" => {
                            store.kind = SelectedInstructionKind::Store {
                                byte_offset: 0,
                                byte_size: 4,
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                assert!(
                    validate(&source, &changed).is_err(),
                    "{target:?}: {mutation}"
                );
            }
        }
    }
}
