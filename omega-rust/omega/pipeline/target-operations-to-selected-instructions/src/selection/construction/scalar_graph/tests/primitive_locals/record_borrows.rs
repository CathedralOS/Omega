use super::*;

#[test]
fn shared_record_field_load_replays_field_type_access_and_offset() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        for scalar in [
            ScalarType::Boolean,
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).unwrap()),
        ] {
            let mut source = fixture(native, 0);
            let place = PlaceId::new(1).unwrap();
            let identity = StructuralTypeId::new(1).unwrap();
            let field = semantic_vocabulary::StructuralFieldId::new(2).unwrap();
            source.call_plan = evaluate_call_plan(
                CallingPolicy::native_for_target(native),
                &CallSignature {
                    parameters: vec![ValueShape::borrowed_reference(16, 8)],
                    result: Some(crate::selection::scalar_call_abi::scalar_shape(scalar).unwrap()),
                },
            )
            .unwrap();
            let parameter = terminal_psi::StructuralParameterDeclaration {
                place,
                position: 0,
                is_self: true,
                structural_type: identity,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            };
            source.structural = Some(legalized_operations::LegalizedStructuralContract {
                result: None,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                structural_types: vec![StructuralTypeDeclaration {
                    id: identity,
                    identity: "record".into(),
                    shape: StructuralTypeShape::Record {
                        fields: vec![
                            terminal_psi::StructuralFieldDeclaration {
                                id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                                identity: "prefix".into(),
                                relevance: terminal_psi::BindingRelevance::Relevant,
                                field_type: terminal_psi::StructuralFieldType::Scalar(
                                    ScalarType::Integer(
                                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                                    ),
                                ),
                            },
                            terminal_psi::StructuralFieldDeclaration {
                                id: field,
                                identity: "value".into(),
                                relevance: terminal_psi::BindingRelevance::Relevant,
                                field_type: terminal_psi::StructuralFieldType::Scalar(scalar),
                            },
                        ],
                    },
                }]
                .into(),
                structural_places: vec![StructuralPlaceDeclaration {
                    id: place,
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: true,
                    },
                }],
                parameters: vec![legalized_operations::LegalizedCallUnitParameter {
                    semantic: parameter,
                    target: target_operations::TargetStructuralParameter {
                        place,
                        structural_type: identity,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        access: StructuralAccess::SharedBorrow,
                        projected_qualifications: Vec::new(),
                        shape: ValueShape::borrowed_reference(16, 8),
                        placement: source.call_plan.parameters[0].clone(),
                    },
                }],
            });
            source.blocks[0].instructions.truncate(1);
            source.provenance.operations.truncate(1);
            source.blocks[0].instructions[0]
                .result
                .as_mut()
                .unwrap()
                .scalar_type = scalar;
            source.blocks[0].instructions[0].kind =
                LegalizedScalarInstructionKind::StructuralScalarFieldRead {
                    source: terminal_psi::StructuralArgument {
                        place,
                        access: StructuralAccess::SharedBorrow,
                        path: Vec::new(),
                    },
                    field,
                };
            let LegalizedScalarTerminator::Return(returned) = &mut source.blocks[0].terminator
            else {
                panic!("return");
            };
            returned.value = LegalizedScalarReturnValue::Value {
                value: ValueId::new(1).unwrap(),
                scalar_type: scalar,
            };
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
            assert!(
                selected
                    .memory_accesses
                    .iter()
                    .any(|access| access.place == place
                        && access.byte_offset == 8
                        && access.role
                            == selected_instructions::SelectedMemoryAccessRole::ReadPlace)
            );
            for mutation in 0..4 {
                let mut changed = source.clone();
                let row = &mut changed.blocks[0].instructions[0];
                let LegalizedScalarInstructionKind::StructuralScalarFieldRead { source, field } =
                    &mut row.kind
                else {
                    panic!("read");
                };
                match mutation {
                    0 => *field = semantic_vocabulary::StructuralFieldId::new(99).unwrap(),
                    1 => source.place = PlaceId::new(99).unwrap(),
                    2 => source.access = StructuralAccess::WriteOnlyBorrow,
                    _ => {
                        row.result.as_mut().unwrap().scalar_type = ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                        )
                    }
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
            let mut changed = selected.clone();
            let load = changed.blocks[0]
                .instructions
                .iter_mut()
                .find(|row| {
                    matches!(
                        row.kind,
                        SelectedInstructionKind::Load8 { .. }
                            | SelectedInstructionKind::Load16 { .. }
                    )
                })
                .unwrap();
            match &mut load.kind {
                SelectedInstructionKind::Load8 { byte_offset }
                | SelectedInstructionKind::Load16 { byte_offset } => *byte_offset = 0,
                _ => panic!("load"),
            }
            assert!(validate(&source, &changed).is_err());
        }
    }
}

#[test]
fn shared_record_call_replays_original_home_and_consumed_scalar_result() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let mut source = local_fixture(native, false);
        let field = semantic_vocabulary::StructuralFieldId::new(1).unwrap();
        let scalar = source.blocks[0].instructions[0].result.unwrap().scalar_type;
        source
            .structural
            .as_mut()
            .unwrap()
            .structural_types
            .make_mut()[0]
            .shape = StructuralTypeShape::Record {
            fields: vec![terminal_psi::StructuralFieldDeclaration {
                id: field,
                identity: "value".into(),
                relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: terminal_psi::StructuralFieldType::Scalar(scalar),
            }],
        };
        let LegalizedScalarInstructionKind::EstablishPrimitiveLocal {
            result,
            value,
            shape,
        } = source.blocks[0].instructions[1].kind.clone()
        else {
            panic!("local");
        };
        source.blocks[0].instructions[1].kind =
            LegalizedScalarInstructionKind::EstablishScalarRecord {
                result,
                shape,
                fields: vec![terminal_psi::ScalarRecordFieldValue {
                    field,
                    value: value.value,
                }],
            };
        let LegalizedScalarInstructionKind::Call(call) = &mut source.blocks[0].instructions[2].kind
        else {
            panic!("call");
        };
        let LegalizedScalarArgument::Structural { semantic, target } = &mut call.arguments[0]
        else {
            panic!("argument");
        };
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
        for mutation in 0..4 {
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
                    target.access = semantic.access;
                }
                _ => {
                    semantic.place = PlaceId::new(99).unwrap();
                    target.place = semantic.place;
                }
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
