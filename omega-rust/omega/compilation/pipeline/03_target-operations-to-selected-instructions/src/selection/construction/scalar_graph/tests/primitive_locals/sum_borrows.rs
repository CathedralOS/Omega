//! Local sums lend their established storage without a record-shaped surrogate.
use super::{
    CallSignature, CallingPolicy, LegalizedScalarArgument, LegalizedScalarFunction,
    LegalizedScalarInstructionKind, OperationId, PlaceId, SelectedFunction,
    SelectedInstructionKind, SelectedSelectionConstraints, StructuralAccess, StructuralPlaceKind,
    StructuralTypeId, StructuralTypeShape, ValueShape, build, evaluate_call_plan, fixture,
};
use crate::selection::construction::scalar_graph::tests::primitive_locals::local_fixture;

fn sum_borrow_fixture(native: target::NativeTarget) -> LegalizedScalarFunction {
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
    source.blocks[0].instructions[1].kind = LegalizedScalarInstructionKind::EstablishCase {
        result,
        result_case: case,
        layout: layout.clone(),
        fields: vec![terminal_psi::RecordFieldInitializer {
            field,
            value: terminal_psi::RecordFieldValue::Scalar {
                value: value.value,
                range_obligation: None,
            },
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
    let LegalizedScalarArgument::Structural {
        semantic, target, ..
    } = &mut call.arguments[0]
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
    source
}

#[test]
fn shared_sum_call_replays_original_home_layout_and_consumed_scalar_result() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let source = sum_borrow_fixture(native);
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
            let LegalizedScalarArgument::Structural {
                semantic, target, ..
            } = &mut call.arguments[0]
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

fn nested_sum_borrow_fixture(native: target::NativeTarget) -> LegalizedScalarFunction {
    let mut source = sum_borrow_fixture(native);
    let child = PlaceId::new(1).unwrap();
    let outer = PlaceId::new(2).unwrap();
    let outer_type = StructuralTypeId::new(2).unwrap();
    let operation = OperationId::new(20).unwrap();
    let case = semantic_vocabulary::StructuralCaseId::new(3).unwrap();
    let field = semantic_vocabulary::StructuralFieldId::new(3).unwrap();
    let LegalizedScalarInstructionKind::EstablishCase {
        result,
        layout: child_layout,
        ..
    } = source.blocks[0].instructions[1].kind.clone()
    else {
        panic!("child case");
    };
    let layout = calling_conventions::evaluate_conventional_sum_layout(
        &[],
        &[vec![child_layout.shape], Vec::new()],
    )
    .unwrap();
    let signature = source.structural.as_mut().unwrap();
    signature
        .structural_types
        .make_mut()
        .push(terminal_psi::StructuralTypeDeclaration {
            id: outer_type,
            identity: "Outer".into(),
            shape: StructuralTypeShape::Sum {
                cases: vec![
                    terminal_psi::StructuralCaseDeclaration {
                        id: case,
                        identity: "Present".into(),
                        fields: vec![terminal_psi::StructuralFieldDeclaration {
                            id: field,
                            identity: "child".into(),
                            relevance: terminal_psi::BindingRelevance::Relevant,
                            field_type: terminal_psi::StructuralFieldType::Structural(
                                result.structural_type,
                            ),
                        }],
                    },
                    terminal_psi::StructuralCaseDeclaration {
                        id: semantic_vocabulary::StructuralCaseId::new(4).unwrap(),
                        identity: "Absent".into(),
                        fields: Vec::new(),
                    },
                ],
            },
        });
    signature
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: outer,
            kind: StructuralPlaceKind::OperationResult {
                producer: operation,
                structural_type: outer_type,
            },
        });
    let mut parent = source.blocks[0].instructions[1].clone();
    parent.operation = operation;
    parent.fuel.clear();
    parent.kind = LegalizedScalarInstructionKind::EstablishCase {
        result: terminal_psi::StructuralOperationResult {
            place: outer,
            structural_type: outer_type,
            ..result
        },
        result_case: case,
        fields: vec![terminal_psi::RecordFieldInitializer {
            field,
            value: terminal_psi::RecordFieldValue::Structural(terminal_psi::StructuralArgument {
                place: child,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            }),
        }],
        layout: layout.clone(),
    };
    source.blocks[0].instructions.insert(2, parent);
    let LegalizedScalarInstructionKind::Call(call) = &mut source.blocks[0].instructions[3].kind
    else {
        panic!("outer borrow");
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
    let LegalizedScalarArgument::Structural {
        semantic, target, ..
    } = &mut call.arguments[0]
    else {
        panic!("outer argument");
    };
    semantic.place = outer;
    target.place = outer;
    target.root_structural_type = outer_type;
    target.structural_type = outer_type;
    target.shape = call.call_plan.parameters[0].shape;
    target.destination = call.call_plan.parameters[0].clone();
    target.source = target_operations::TargetStructuralArgumentSource::StructuralHome {
        psi_operation: operation,
    };
    let observation = OperationId::new(21).unwrap();
    let block = &mut source.blocks[0];
    block
        .instructions
        .push(legalized_operations::LegalizedScalarInstruction {
            operation: observation,
            result: Some(legalized_operations::LegalizedValueDefinition {
                value: super::ValueId::new(21).unwrap(),
                scalar_type: super::ScalarType::Boolean,
                definition_site: optimization_unit::ValueDefinitionSite::Node {
                    block: block.id,
                    node: block.instructions.len() as u32,
                },
            }),
            kind: LegalizedScalarInstructionKind::StructuralCaseMembership {
                source: outer,
                path: Vec::new(),
                case,
                case_tag: 0,
                tag_byte_offset: 0,
                indices: Vec::new(),
            },
            fuel: vec![optimization_unit::FuelSettlement {
                site: optimization_unit::PsiProvenance::Operation(observation),
                units: 1,
            }],
            effect: optimization_unit::EffectLink {
                input: 20,
                output: 21,
            },
            ownership: Vec::new(),
        });
    source.provenance.operations.push(observation);
    source
}

#[test]
fn nested_case_payload_copy_replays_exact_child_storage_and_layout() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let source = nested_sum_borrow_fixture(native);
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
        assert_eq!(selected.local_storage_slots.len(), 2);
        assert!(selected.blocks[0].instructions.iter().any(|instruction| {
            matches!(
                instruction.kind,
                SelectedInstructionKind::Load32 { byte_offset: 0 }
            ) && instruction
                .provenance
                .operations
                .contains(&OperationId::new(21).unwrap())
        }));
        for mutation in 0..3 {
            let mut changed = source.clone();
            let LegalizedScalarInstructionKind::StructuralCaseMembership {
                case,
                case_tag,
                tag_byte_offset,
                ..
            } = &mut changed.blocks[0].instructions.last_mut().unwrap().kind
            else {
                panic!("membership");
            };
            match mutation {
                0 => *case = semantic_vocabulary::StructuralCaseId::new(99).unwrap(),
                1 => *case_tag = 1,
                _ => *tag_byte_offset = 4,
            }
            assert!(
                construct(&changed).is_err(),
                "membership source mutation {mutation}"
            );
            assert!(
                validate(&changed, &selected).is_err(),
                "membership replay mutation {mutation}"
            );
        }
        let mut changed_tag_read = selected.clone();
        changed_tag_read.blocks[0]
            .instructions
            .iter_mut()
            .find(|instruction| {
                matches!(instruction.kind, SelectedInstructionKind::Load32 { .. })
                    && instruction
                        .provenance
                        .operations
                        .contains(&OperationId::new(21).unwrap())
            })
            .unwrap()
            .kind = SelectedInstructionKind::Load32 { byte_offset: 4 };
        assert!(validate(&source, &changed_tag_read).is_err());
        for mutation in 0..6 {
            let mut changed = source.clone();
            let LegalizedScalarInstructionKind::EstablishCase {
                fields,
                layout,
                result_case,
                ..
            } = &mut changed.blocks[0].instructions[2].kind
            else {
                panic!("parent");
            };
            match mutation {
                0 => layout.cases[0].fields[0].byte_offset += 1,
                1 => fields[0].field = semantic_vocabulary::StructuralFieldId::new(99).unwrap(),
                2 => *result_case = semantic_vocabulary::StructuralCaseId::new(99).unwrap(),
                3 | 4 => {
                    let terminal_psi::RecordFieldValue::Structural(argument) = &mut fields[0].value
                    else {
                        panic!("child");
                    };
                    if mutation == 3 {
                        argument.access = StructuralAccess::SharedBorrow;
                    } else {
                        argument.place = PlaceId::new(99).unwrap();
                    }
                }
                _ => changed.blocks[0].instructions.swap(1, 2),
            }
            assert!(construct(&changed).is_err(), "source mutation {mutation}");
            assert!(
                validate(&changed, &selected).is_err(),
                "source replay mutation {mutation}"
            );
        }
        // The child copy must use its original address, exact offsets, and every store.
        let mut wrong_offset = selected.clone();
        let load = wrong_offset.blocks[0]
            .instructions
            .iter_mut()
            .find(|instruction| matches!(instruction.kind, SelectedInstructionKind::Load64 { .. }))
            .unwrap();
        let SelectedInstructionKind::Load64 { byte_offset } = &mut load.kind else {
            unreachable!()
        };
        *byte_offset += 1;
        assert!(validate(&source, &wrong_offset).is_err());
        let mut missing_store = selected.clone();
        let stores = &mut missing_store.blocks[0].instructions;
        let position = stores
            .iter()
            .rposition(|instruction| {
                matches!(instruction.kind, SelectedInstructionKind::Store { .. })
            })
            .unwrap();
        stores.remove(position);
        assert!(validate(&source, &missing_store).is_err());
    }
}

/// The local sum becomes a mixed shape with one runtime common field whose id
/// reuses the payload's: the establishment binds the common field, then the
/// payload, and a membership observation reads the result. The tag stays the
/// carrier's first word while the common field takes offset 8 and the
/// payload overlay moves to 16.
fn mixed_membership_fixture(native: target::NativeTarget) -> LegalizedScalarFunction {
    let mut source = sum_borrow_fixture(native);
    let place = PlaceId::new(1).unwrap();
    let declaration = &mut source
        .structural
        .as_mut()
        .unwrap()
        .structural_types
        .make_mut()[0];
    let StructuralTypeShape::Sum { cases } = declaration.shape.clone() else {
        panic!("sum");
    };
    let common = terminal_psi::StructuralFieldDeclaration {
        identity: "count".into(),
        ..cases[0].fields[0].clone()
    };
    let case = cases[0].id;
    declaration.shape = StructuralTypeShape::Mixed {
        fields: vec![common.clone()],
        cases,
    };
    let layout = calling_conventions::evaluate_conventional_sum_layout(
        &[ValueShape::integer(8, 8)],
        &[vec![ValueShape::integer(8, 8)], Vec::new()],
    )
    .unwrap();
    assert_eq!(
        (
            layout.common_fields[0].byte_offset,
            layout.payload_byte_offset
        ),
        (8, 16)
    );
    let LegalizedScalarInstructionKind::EstablishCase {
        fields,
        layout: established,
        ..
    } = &mut source.blocks[0].instructions[1].kind
    else {
        panic!("case");
    };
    let payload = fields[0].clone();
    fields.insert(
        0,
        terminal_psi::RecordFieldInitializer {
            field: common.id,
            value: payload.value,
        },
    );
    *established = layout.clone();
    // Borrowing a mixed local into a call is a separate route that selection
    // still refuses, so the getter becomes a constant the scalar call reads.
    let getter = &mut source.blocks[0].instructions[2];
    getter.kind = LegalizedScalarInstructionKind::Constant(super::super::IntegerValue::Unsigned(3));
    getter.ownership.clear();
    let observation = OperationId::new(21).unwrap();
    let block = &mut source.blocks[0];
    block
        .instructions
        .push(legalized_operations::LegalizedScalarInstruction {
            operation: observation,
            result: Some(legalized_operations::LegalizedValueDefinition {
                value: super::ValueId::new(21).unwrap(),
                scalar_type: super::ScalarType::Boolean,
                definition_site: optimization_unit::ValueDefinitionSite::Node {
                    block: block.id,
                    node: block.instructions.len() as u32,
                },
            }),
            kind: LegalizedScalarInstructionKind::StructuralCaseMembership {
                source: place,
                path: Vec::new(),
                case,
                case_tag: 0,
                tag_byte_offset: 0,
                indices: Vec::new(),
            },
            fuel: vec![optimization_unit::FuelSettlement {
                site: optimization_unit::PsiProvenance::Operation(observation),
                units: 1,
            }],
            effect: optimization_unit::EffectLink {
                input: 20,
                output: 21,
            },
            ownership: Vec::new(),
        });
    source.provenance.operations.push(observation);
    source
}

#[test]
fn mixed_case_membership_reads_tag_ahead_of_common_fields() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let source = mixed_membership_fixture(native);
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
        let selected = construct(&source).unwrap();
        validate(&source, &selected).unwrap();
        let observes = |instruction: &selected_instructions::SelectedInstruction| {
            instruction
                .provenance
                .operations
                .contains(&OperationId::new(21).unwrap())
        };
        assert!(selected.blocks[0].instructions.iter().any(|instruction| {
            matches!(
                instruction.kind,
                SelectedInstructionKind::Load32 { byte_offset: 0 }
            ) && observes(instruction)
        }));
        // A substituted case, a tag that names the other case, or a tag read
        // at the common field's offset each refuse in both directions.
        for mutation in 0..3 {
            let mut changed = source.clone();
            let LegalizedScalarInstructionKind::StructuralCaseMembership {
                case,
                case_tag,
                tag_byte_offset,
                ..
            } = &mut changed.blocks[0].instructions.last_mut().unwrap().kind
            else {
                panic!("membership");
            };
            match mutation {
                0 => *case = semantic_vocabulary::StructuralCaseId::new(99).unwrap(),
                1 => *case_tag = 1,
                _ => *tag_byte_offset = 8,
            }
            assert!(
                construct(&changed).is_err(),
                "mixed membership source mutation {mutation}"
            );
            assert!(
                validate(&changed, &selected).is_err(),
                "mixed membership replay mutation {mutation}"
            );
        }
        let mut changed_tag_read = selected.clone();
        changed_tag_read.blocks[0]
            .instructions
            .iter_mut()
            .find(|instruction| {
                matches!(instruction.kind, SelectedInstructionKind::Load32 { .. })
                    && observes(instruction)
            })
            .unwrap()
            .kind = SelectedInstructionKind::Load32 { byte_offset: 8 };
        assert!(validate(&source, &changed_tag_read).is_err());
    }
}
