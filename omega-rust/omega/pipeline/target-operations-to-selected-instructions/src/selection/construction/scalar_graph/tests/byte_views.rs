//! Descriptor reads retain exact source, displacement, and replay custody.
use super::*;

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
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        };
        source.structural = Some(legalized_operations::LegalizedStructuralContract {
            structural_types: vec![terminal_psi::StructuralTypeDeclaration {
                id: structural_type,
                identity: "bytes".into(),
                shape: terminal_psi::StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView,
                ),
            }],
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
        assert!(
            build(
                0,
                &mutable_view,
                target,
                &constraints,
                environment.physical(),
                environment.constraints()
            )
            .is_err()
        );
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
