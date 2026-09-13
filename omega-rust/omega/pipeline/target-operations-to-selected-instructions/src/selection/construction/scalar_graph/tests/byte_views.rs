//! Descriptor reads retain exact source, displacement, and replay custody.
use super::*;

#[test]
fn indexed_byte_read_replay_binds_dynamic_subject_and_proof() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let mut source = fixture(target, 0);
        source.attachment = None;
        source.blocks[0].instructions.truncate(3);
        source.provenance.operations.truncate(3);
        let place = semantic_vocabulary::PlaceId::new(1).unwrap();
        let structural_type = StructuralTypeId::new(1).unwrap();
        let index = ValueId::new(100).unwrap();
        let length = ValueId::new(1).unwrap();
        let obligation = semantic_vocabulary::ObligationId::new(1).unwrap();
        let accepted_fact = optimization_core::AcceptedObligationFactIdentity::from_bytes([1; 32]);
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        source.call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![
                    ValueShape::integer(8, 8),
                    ValueShape::borrowed_reference(16, 8),
                ],
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .unwrap();
        source.parameters = vec![LegalizedScalarParameter {
            value: index,
            scalar_type: ScalarType::Integer(integer),
            definition_site: ValueDefinitionSite::FunctionParameter(0),
            placement: source.call_plan.parameters[0].clone(),
        }];
        source.structural = Some(legalized_operations::LegalizedStructuralContract {
            result: None,
            structural_types: vec![terminal_psi::StructuralTypeDeclaration {
                id: structural_type,
                identity: "bytes".into(),
                shape: terminal_psi::StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView,
                ),
            }]
            .into(),
            parameters: vec![legalized_operations::LegalizedCallUnitParameter {
                semantic: terminal_psi::StructuralParameterDeclaration {
                    place,
                    position: 0,
                    is_self: false,
                    structural_type,
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                },
                target: target_operations::TargetStructuralParameter {
                    place,
                    structural_type,
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                    projected_qualifications: Vec::new(),
                    shape: ValueShape::borrowed_reference(16, 8),
                    placement: source.call_plan.parameters[1].clone(),
                },
            }],
            structural_places: vec![terminal_psi::StructuralPlaceDeclaration {
                id: place,
                kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            }],
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
        source.blocks[0].instructions[0].kind =
            LegalizedScalarInstructionKind::ByteSequenceLength {
                source: place,
                length_byte_offset: 8,
            };
        source.blocks[0].instructions[1].kind = LegalizedScalarInstructionKind::ByteSequenceRead {
            source: place,
            index,
            length,
            obligation,
            accepted_fact,
        };
        source.blocks[0].instructions[1]
            .result
            .as_mut()
            .unwrap()
            .scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).unwrap());
        source.blocks[0].instructions[2].kind = LegalizedScalarInstructionKind::IntegerWiden {
            operand: ValueId::new(2).unwrap(),
            source_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        };
        returned(&mut source.blocks[0]).value = LegalizedScalarReturnValue::Value {
            value: ValueId::new(3).unwrap(),
            scalar_type: semantic_vocabulary::ScalarType::Integer(integer),
        };
        let [ValueLocation::Register { register, .. }] =
            source.parameters[0].placement.locations.as_slice()
        else {
            panic!("scalar index register");
        };
        let constraints = SelectedSelectionConstraints {
            keys: environment.selected_keys(),
            fixed_inputs: vec![SelectedFixedInputConstraint {
                machine: source.machine,
                source_value: index,
                parameter_index: 0,
                register: *register,
                fixed_view: environment.fixed_register_view(*register).unwrap(),
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
        for mutation in 0..6 {
            let mut candidate = selected.clone();
            let access = candidate
                .memory_accesses
                .iter_mut()
                .find(|access| {
                    matches!(
                        access.role,
                        selected_instructions::SelectedMemoryAccessRole::ReadByteSequence { .. }
                    )
                })
                .unwrap();
            let selected_instructions::SelectedMemoryAccessRole::ReadByteSequence {
                index,
                length,
                obligation,
                accepted_fact,
            } = &mut access.role
            else {
                unreachable!()
            };
            match mutation {
                0 => *index = ValueId::new(999).unwrap(),
                1 => *length = ValueId::new(999).unwrap(),
                2 => *obligation = semantic_vocabulary::ObligationId::new(2).unwrap(),
                3 => {
                    *accepted_fact =
                        optimization_core::AcceptedObligationFactIdentity::from_bytes([2; 32])
                }
                4 => access.place = semantic_vocabulary::PlaceId::new(2).unwrap(),
                _ => access.byte_count = 8,
            }
            assert!(validate(&candidate).is_err());
        }
        let mut candidate = selected.clone();
        let read = candidate
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find(|instruction| instruction.kind == SelectedInstructionKind::Load8Indexed)
            .unwrap();
        read.provenance.fuel.clear();
        assert!(validate(&candidate).is_err());
    }
}

#[test]
fn byte_view_length_uses_descriptor_read_and_rejects_changed_projection() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let environment =
            register_environment::baseline_target_register_environment(target).unwrap();
        let mut source = fixture(target, 0);
        source.attachment = None;
        source.blocks[0].instructions.truncate(1);
        source.provenance.operations.truncate(1);
        let place = semantic_vocabulary::PlaceId::new(1).unwrap();
        let structural_type = StructuralTypeId::new(1).unwrap();
        source.call_plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: vec![ValueShape::borrowed_reference(16, 8)],
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .unwrap();
        source.blocks[0].instructions[0].kind =
            LegalizedScalarInstructionKind::ByteSequenceLength {
                source: place,
                length_byte_offset: 8,
            };
        returned(&mut source.blocks[0]).value = LegalizedScalarReturnValue::Value {
            value: ValueId::new(1).unwrap(),
            scalar_type: semantic_vocabulary::ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            ),
        };
        source.structural = Some(legalized_operations::LegalizedStructuralContract {
            result: None,
            structural_types: vec![terminal_psi::StructuralTypeDeclaration {
                id: structural_type,
                identity: "bytes".into(),
                shape: terminal_psi::StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView,
                ),
            }]
            .into(),
            parameters: vec![legalized_operations::LegalizedCallUnitParameter {
                semantic: terminal_psi::StructuralParameterDeclaration {
                    place,
                    position: 0,
                    is_self: false,
                    structural_type,
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                },
                target: target_operations::TargetStructuralParameter {
                    place,
                    structural_type,
                    multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                    projected_qualifications: Vec::new(),
                    shape: ValueShape::borrowed_reference(16, 8),
                    placement: source.call_plan.parameters[0].clone(),
                },
            }],
            structural_places: vec![terminal_psi::StructuralPlaceDeclaration {
                id: place,
                kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            }],
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
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
        // Keep every ABI and projected source row consistent: only the semantic
        // carrier changes. Equal descriptor-sized record storage grants no byte
        // observation authority, even when the selected loads still match.
        let mut record_source = source.clone();
        record_source
            .structural
            .as_mut()
            .unwrap()
            .structural_types
            .make_mut()[0]
            .shape = terminal_psi::StructuralTypeShape::Record {
            fields: (1..=2)
                .map(|ordinal| terminal_psi::StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(ordinal).unwrap(),
                    identity: format!("field{ordinal}"),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Scalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    )),
                })
                .collect(),
        };
        let signature = record_source.structural.as_ref().unwrap();
        assert!(crate::structural_unit_input::accepts_graph(
            &record_source.call_plan,
            &[crate::structural_unit_input::Parameter {
                semantic: &signature.parameters[0].semantic,
                target: &signature.parameters[0].target,
            }],
            &signature.structural_types,
        ));
        let mut record_selected = selected.clone();
        record_selected.structural = record_source.structural.clone();
        assert!(validate(&record_source, &record_selected).is_err());
        assert!(
            build(
                0,
                &record_source,
                target,
                &constraints,
                environment.physical(),
                environment.constraints()
            )
            .is_err()
        );
        assert!(
            selected
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| matches!(
                    instruction.kind,
                    SelectedInstructionKind::Load64 { byte_offset: 8 }
                ))
        );
        let mut corrupted = selected.clone();
        let load = corrupted
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find(|instruction| matches!(instruction.kind, SelectedInstructionKind::Load64 { .. }))
            .unwrap();
        load.kind = SelectedInstructionKind::Load64 { byte_offset: 0 };
        assert!(validate(&source, &corrupted).is_err());
        let mut missing_read = selected.clone();
        missing_read.memory_accesses.clear();
        assert!(validate(&source, &missing_read).is_err());
        let mut missing_fuel = selected.clone();
        let load = missing_fuel
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find(|instruction| matches!(instruction.kind, SelectedInstructionKind::Load64 { .. }))
            .unwrap();
        load.provenance.fuel.clear();
        assert!(validate(&source, &missing_fuel).is_err());
        let mut mutable_view = source.clone();
        let signature = mutable_view.structural.as_mut().unwrap();
        signature.parameters[0].semantic.access = terminal_psi::StructuralAccess::MutableBorrow;
        signature.parameters[0].target.access = terminal_psi::StructuralAccess::MutableBorrow;
        let mutable_selected = build(
            0,
            &mutable_view,
            target,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .expect("incoming mutable descriptor is independent of the scalar result");
        validate(&mutable_view, &mutable_selected).unwrap();
        assert!(validate(&source, &mutable_selected).is_err());
        let mut wrong_source = source.clone();
        wrong_source.blocks[0].instructions[0].kind =
            LegalizedScalarInstructionKind::ByteSequenceLength {
                source: semantic_vocabulary::PlaceId::new(2).unwrap(),
                length_byte_offset: 8,
            };
        assert!(validate(&wrong_source, &selected).is_err());
        wrong_source.blocks[0].instructions[0].kind =
            LegalizedScalarInstructionKind::ByteSequenceLength {
                source: place,
                length_byte_offset: 0,
            };
        assert!(
            build(
                0,
                &wrong_source,
                target,
                &constraints,
                environment.physical(),
                environment.constraints()
            )
            .is_err()
        );
    }
}
