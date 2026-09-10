//! Owned argument transport retains exact homes, scalar carriers, and borrowed referents.
use super::*;

#[test]
fn owned_array_arguments_replay_exact_home_type_and_fragments() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::macos_arm64(),
        target::NativeTarget::windows_x64(),
    ] {
        for length in [1, 2, 8, 16] {
            if target == target::NativeTarget::windows_x64() && length == 16 {
                continue;
            }
            let mut source = array_fixture(target, length);
            let mut row = source.blocks[0].instructions[2].clone();
            let LegalizedScalarInstructionKind::EstablishScalarArray {
                result: input,
                shape,
                ..
            } = row.kind.clone()
            else {
                panic!("array");
            };
            let producer = row.operation;
            row.operation = OperationId::new(4).unwrap();
            row.fuel = vec![FuelSettlement {
                site: PsiProvenance::Operation(row.operation),
                units: 1,
            }];
            let mut output = input.clone();
            output.place = PlaceId::new(3).unwrap();
            let call_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: vec![shape],
                    result: Some(shape),
                },
            )
            .unwrap();
            row.ownership.clear();
            row.kind =
                LegalizedScalarInstructionKind::Call(legalized_operations::LegalizedScalarCall {
                    source: legalized_operations::LegalizedCallUnitSource::AuthoredCallUnit,
                    callee: semantic_vocabulary::MachineId::new(2).unwrap(),
                    arguments: vec![legalized_operations::LegalizedScalarArgument::Structural {
                        semantic: terminal_psi::StructuralArgument {
                            place: input.place,
                            access: StructuralAccess::Owned,
                            path: Vec::new(),
                        },
                        target: target_operations::TargetStructuralArgument {
                            place: input.place,
                            access: StructuralAccess::Owned,
                            path: Vec::new(),
                            root_structural_type: input.structural_type,
                            structural_type: input.structural_type,
                            shape,
                            source_byte_offset: 0,
                            fixed_array_length: None,
                            element_stride: None,
                            source:
                                target_operations::TargetStructuralArgumentSource::StructuralHome {
                                    psi_operation: producer,
                                },
                            destination: call_plan.parameters[0].clone(),
                        },
                    }],
                    structural_result: Some(output.clone()),
                    result_placement: call_plan.result.clone(),
                    call_plan,
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                });
            source.provenance.operations.push(row.operation);
            let LegalizedScalarTerminator::Return(returned) = &mut source.blocks[0].terminator
            else {
                panic!("return");
            };
            returned.value = LegalizedScalarReturnValue::Structural {
                defining_operation: row.operation,
                result: output,
            };
            source.blocks[0].instructions.push(row);
            let environment =
                register_environment::baseline_target_register_environment(target).unwrap();
            let constraints = SelectedSelectionConstraints {
                keys: environment.selected_keys(),
                projected_structural_call: None,
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
            .unwrap_or_else(|error| panic!("owned array {target:?}/{length}: {error:?}"));
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
            let narrow = with_narrow_scalar_result(source.clone());
            let selected_narrow = build(
                0,
                &narrow,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap_or_else(|error| panic!("narrow owned array {target:?}/{length}: {error:?}"));
            validate(&narrow, &selected_narrow).unwrap();
            let mut unnormalized = selected_narrow.clone();
            let normalization = unnormalized.blocks[0]
                .instructions
                .iter_mut()
                .find(|row| row.kind == SelectedInstructionKind::ZeroExtendU8)
                .expect("normalize returned byte");
            normalization.kind = SelectedInstructionKind::CopyI64;
            assert!(validate(&narrow, &unnormalized).is_err());
            let mut wrong_carrier = narrow.clone();
            wrong_carrier.blocks[0].instructions[3]
                .result
                .as_mut()
                .unwrap()
                .scalar_type =
                ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap());
            assert!(validate(&wrong_carrier, &selected_narrow).is_err());
            for (sign, bits) in [
                (IntegerSign::Unsigned, 16),
                (IntegerSign::Signed, 8),
                (IntegerSign::Signed, 16),
                (IntegerSign::Signed, 32),
            ] {
                let mut supported = narrow.clone();
                let integer = IntegerType::new(sign, bits).unwrap();
                let result_shape = ValueShape::integer(bits / 8, bits / 8);
                supported.call_plan = evaluate_call_plan(
                    supported.call_plan.policy,
                    &CallSignature {
                        parameters: Vec::new(),
                        result: Some(result_shape),
                    },
                )
                .unwrap();
                let row = &mut supported.blocks[0].instructions[3];
                row.result.as_mut().unwrap().scalar_type = ScalarType::Integer(integer);
                let LegalizedScalarInstructionKind::Call(call) = &mut row.kind else {
                    panic!("call");
                };
                call.call_plan = evaluate_call_plan(
                    call.call_plan.policy,
                    &CallSignature {
                        parameters: call
                            .arguments
                            .iter()
                            .map(|argument| argument.placement().shape)
                            .collect(),
                        result: Some(result_shape),
                    },
                )
                .unwrap();
                call.result_placement = call.call_plan.result.clone();
                let LegalizedScalarTerminator::Return(returned) =
                    &mut supported.blocks[0].terminator
                else {
                    panic!("return");
                };
                returned.value = LegalizedScalarReturnValue::Value {
                    value: ValueId::new(3).unwrap(),
                    scalar_type: semantic_vocabulary::ScalarType::Integer(integer),
                };
                let selected = build(
                    0,
                    &supported,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
                .unwrap();
                validate(&supported, &selected).unwrap();
                let expected = match (sign, bits) {
                    (IntegerSign::Unsigned, 16) => SelectedInstructionKind::ZeroExtendU16,
                    (IntegerSign::Signed, 8) => SelectedInstructionKind::SignExtendI8,
                    (IntegerSign::Signed, 16) => SelectedInstructionKind::SignExtendI16,
                    (IntegerSign::Signed, 32) => SelectedInstructionKind::SignExtendI32,
                    _ => panic!("test normalization"),
                };
                assert!(
                    selected.blocks[0]
                        .instructions
                        .iter()
                        .any(|row| row.kind == expected)
                );
                for replacement in [
                    SelectedInstructionKind::CopyI64,
                    SelectedInstructionKind::ZeroExtendU8,
                    SelectedInstructionKind::ZeroExtendU32,
                ] {
                    let mut changed = selected.clone();
                    changed.blocks[0]
                        .instructions
                        .iter_mut()
                        .find(|row| row.kind == expected)
                        .unwrap()
                        .kind = replacement;
                    assert!(validate(&supported, &changed).is_err());
                }
                assert!(validate(&supported, &selected_narrow).is_err());
            }
            for mutation in 0..3 {
                let mut changed = source.clone();
                let LegalizedScalarInstructionKind::Call(call) =
                    &mut changed.blocks[0].instructions[3].kind
                else {
                    panic!("call");
                };
                let legalized_operations::LegalizedScalarArgument::Structural { target, .. } =
                    &mut call.arguments[0]
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
                    1 => target.structural_type = StructuralTypeId::new(2).unwrap(),
                    2 => target.source_byte_offset = 1,
                    _ => unreachable!(),
                }
                assert!(validate(&changed, &selected).is_err());
            }
            let mut changed = selected.clone();
            let load = changed.blocks[0]
                .instructions
                .iter_mut()
                .find(|row| {
                    matches!(
                        row.kind,
                        SelectedInstructionKind::Load8 { .. }
                            | SelectedInstructionKind::Load16 { .. }
                            | SelectedInstructionKind::Load64 { .. }
                    )
                })
                .unwrap();
            load.kind = SelectedInstructionKind::Load32 { byte_offset: 1 };
            assert!(validate(&source, &changed).is_err());
            for borrowed_first in [false, true] {
                let mixed = with_borrowed_argument(source.clone(), borrowed_first);
                let selected = build(
                    0,
                    &mixed,
                    target,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                )
                .unwrap_or_else(|error| {
                    panic!("mixed owned array {target:?}/{length}/{borrowed_first}: {error:?}")
                });
                validate(&mixed, &selected).unwrap();
                let mut corrupted = mixed.clone();
                let LegalizedScalarInstructionKind::Call(call) =
                    &mut corrupted.blocks[0].instructions[3].kind
                else {
                    panic!("call");
                };
                let LegalizedScalarArgument::Structural { target, .. } =
                    &mut call.arguments[usize::from(!borrowed_first)]
                else {
                    panic!("borrow");
                };
                target.source_byte_offset = 1;
                assert!(validate(&corrupted, &selected).is_err());
            }
        }
    }
}

fn with_narrow_scalar_result(mut source: LegalizedScalarFunction) -> LegalizedScalarFunction {
    let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    let shape = ValueShape::integer(1, 1);
    source.structural.as_mut().unwrap().result = None;
    source.call_plan = evaluate_call_plan(
        source.call_plan.policy,
        &CallSignature {
            parameters: Vec::new(),
            result: Some(shape),
        },
    )
    .unwrap();
    let block = source.blocks[0].id;
    let row = &mut source.blocks[0].instructions[3];
    row.result = Some(legalized_operations::LegalizedValueDefinition {
        value: ValueId::new(3).unwrap(),
        scalar_type: ScalarType::Integer(integer),
        definition_site: ValueDefinitionSite::Node { block, node: 3 },
    });
    let LegalizedScalarInstructionKind::Call(call) = &mut row.kind else {
        panic!("call");
    };
    call.structural_result = None;
    call.call_plan = evaluate_call_plan(
        call.call_plan.policy,
        &CallSignature {
            parameters: vec![call.arguments[0].placement().shape, shape],
            result: Some(shape),
        },
    )
    .unwrap();
    call.result_placement = call.call_plan.result.clone();
    call.arguments.push(LegalizedScalarArgument::Scalar {
        source: ValueId::new(1).unwrap(),
        placement: call.call_plan.parameters[1].clone(),
    });
    let LegalizedScalarTerminator::Return(returned) = &mut source.blocks[0].terminator else {
        panic!("return");
    };
    returned.value = LegalizedScalarReturnValue::Value {
        value: ValueId::new(3).unwrap(),
        scalar_type: semantic_vocabulary::ScalarType::Integer(integer),
    };
    source
}

fn with_borrowed_argument(
    mut source: LegalizedScalarFunction,
    borrowed_first: bool,
) -> LegalizedScalarFunction {
    let place = PlaceId::new(4).unwrap();
    let structural_type = StructuralTypeId::new(2).unwrap();
    let shape = ValueShape::borrowed_reference(1, 1);
    source.call_plan = evaluate_call_plan(
        source.call_plan.policy,
        &CallSignature {
            parameters: vec![shape],
            result: source
                .call_plan
                .result
                .as_ref()
                .map(|placement| placement.shape),
        },
    )
    .unwrap();
    let placement = source.call_plan.parameters[0].clone();
    source.structural.as_mut().unwrap().parameters.push(
        legalized_operations::LegalizedCallUnitParameter {
            semantic: terminal_psi::StructuralParameterDeclaration {
                place,
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            },
            target: target_operations::TargetStructuralParameter {
                place,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                projected_qualifications: Vec::new(),
                shape,
                placement: placement.clone(),
            },
        },
    );
    let row = &mut source.blocks[0].instructions[3];
    row.ownership = vec![optimization_unit::OwnershipEvent::ClaimTransfer(Vec::new())];
    let LegalizedScalarInstructionKind::Call(call) = &mut row.kind else {
        panic!("call");
    };
    let argument = LegalizedScalarArgument::Structural {
        semantic: terminal_psi::StructuralArgument {
            place,
            access: StructuralAccess::SharedBorrow,
            path: Vec::new(),
        },
        target: target_operations::TargetStructuralArgument {
            place,
            access: StructuralAccess::SharedBorrow,
            path: Vec::new(),
            root_structural_type: structural_type,
            structural_type,
            shape,
            source_byte_offset: 0,
            fixed_array_length: None,
            element_stride: None,
            source: placement.clone().into(),
            destination: placement,
        },
    };
    call.arguments
        .insert(usize::from(!borrowed_first), argument);
    call.call_plan = evaluate_call_plan(
        call.call_plan.policy,
        &CallSignature {
            parameters: call
                .arguments
                .iter()
                .map(|argument| argument.placement().shape)
                .collect(),
            result: call
                .result_placement
                .as_ref()
                .map(|placement| placement.shape),
        },
    )
    .unwrap();
    for (argument, placement) in call.arguments.iter_mut().zip(&call.call_plan.parameters) {
        let LegalizedScalarArgument::Structural { target, .. } = argument else {
            panic!("argument");
        };
        target.destination = placement.clone();
    }
    source
}
