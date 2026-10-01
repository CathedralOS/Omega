//! Selection replay must preserve two runtime spans and the owned result layout.
use super::{
    LegalizedScalarInstructionKind, LegalizedScalarReturnValue, SelectedInstructionKind,
    SelectedSelectionConstraints, build, returned,
};
use semantic_vocabulary::{OperationId, PlaceId, StructuralTypeId, ValueId};
use terminal_psi::control_flow::ByteSequenceOperand;

#[test]
fn qualified_bounded_bytes_return_through_the_exact_hidden_result_home() {
    for target in [
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let mut source = owned_fixture(target);
        let operation = OperationId::new(5).unwrap();
        let place = PlaceId::new(4).unwrap();
        let structural_type = StructuralTypeId::new(2).unwrap();
        let qualification = semantic_vocabulary::StructuralDomainId::new(1).unwrap();
        let signature = source.structural.as_mut().unwrap();
        let declaration = signature
            .structural_types
            .make_mut()
            .iter_mut()
            .find(|declaration| declaration.id == structural_type)
            .unwrap();
        declaration.shape = terminal_psi::StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 16 },
        );
        let row = source.blocks[0]
            .instructions
            .iter_mut()
            .find(|row| row.operation == operation)
            .unwrap();
        let LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation { result, .. } =
            &mut row.kind
        else {
            panic!("owned byte constructor");
        };
        result.qualifications = vec![qualification];
        let returned_result = result.clone();
        signature.result = Some(terminal_psi::StructuralResultDeclaration {
            reference_sources: Vec::new(),
            place,
            structural_type,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            qualifications: vec![qualification],
            projected_qualifications: Vec::new(),
        });
        let shape = calling_conventions::ValueShape::integer(24, 8);
        source.call_plan = super::evaluate_call_plan(
            source.call_plan.policy,
            &super::CallSignature {
                parameters: source
                    .call_plan
                    .parameters
                    .iter()
                    .map(|parameter| parameter.shape)
                    .collect(),
                result: Some(shape),
            },
        )
        .unwrap();
        returned(&mut source.blocks[0]).value = LegalizedScalarReturnValue::Structural {
            source: legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                operation,
                result: returned_result,
            },
        };
        assert!(
            crate::selection::aggregate_result_input::indirect_result(
                source.call_plan.result.as_ref().unwrap(),
                source.call_plan.policy,
            )
            .is_some()
        );

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
        let validate = |candidate| {
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
        assert!(selected.local_storage_slots.iter().any(|slot| {
            slot.id == selected_instructions::LocalStorageSlotId::Structural { operation, place }
                && slot.byte_size == 24
                && slot.alignment == 8
        }));

        let mut wrong_home = selected.clone();
        wrong_home
            .local_storage_slots
            .iter_mut()
            .find(|slot| {
                slot.id
                    == selected_instructions::LocalStorageSlotId::Structural { operation, place }
            })
            .unwrap()
            .byte_size = 16;
        assert!(validate(&wrong_home).is_err());

        let mut wrong_qualification = source.clone();
        wrong_qualification
            .structural
            .as_mut()
            .unwrap()
            .result
            .as_mut()
            .unwrap()
            .qualifications
            .clear();
        let value = returned(&mut wrong_qualification.blocks[0]).value.clone();
        assert!(
            crate::selection::aggregate_result_input::returned(&wrong_qualification, &value)
                .is_none()
        );

        let mut borrowed_descriptor = source.clone();
        borrowed_descriptor
            .structural
            .as_mut()
            .unwrap()
            .structural_types
            .make_mut()
            .iter_mut()
            .find(|declaration| declaration.id == structural_type)
            .unwrap()
            .shape = terminal_psi::StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            },
        );
        let value = returned(&mut borrowed_descriptor.blocks[0]).value.clone();
        assert!(
            crate::selection::aggregate_result_input::returned(&borrowed_descriptor, &value)
                .is_none()
        );
    }
}

#[test]
fn owned_concatenation_replays_live_copy_counts_and_inline_length() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let source = owned_fixture(target);
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
        let validate = |selected| {
            crate::selection::validation::scalar_graph::validate(
                0,
                &source,
                selected,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&selected).unwrap();
        assert_eq!(
            selected
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter(|row| row.kind == SelectedInstructionKind::CopyBytes)
                .count(),
            2
        );
        assert!(
            selected
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(
                    |row| row.kind == SelectedInstructionKind::Load64 { byte_offset: 0 }
                        && row
                            .provenance
                            .operations
                            .contains(&OperationId::new(6).unwrap())
                )
        );
        let mut wrong_count = selected.clone();
        let copy = wrong_count
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find(|row| row.kind == SelectedInstructionKind::CopyBytes)
            .unwrap();
        copy.operands.swap(1, 2);
        assert!(validate(&wrong_count).is_err());
        let mut wrong_length = selected.clone();
        let load = wrong_length
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find(|row| {
                row.kind == SelectedInstructionKind::Load64 { byte_offset: 0 }
                    && row
                        .provenance
                        .operations
                        .contains(&OperationId::new(6).unwrap())
            })
            .unwrap();
        load.kind = SelectedInstructionKind::Load64 { byte_offset: 8 };
        assert!(validate(&wrong_length).is_err());
        let mut wrong_order = selected.clone();
        let copies = wrong_order.blocks[0]
            .instructions
            .iter()
            .enumerate()
            .filter(|(_, row)| row.kind == SelectedInstructionKind::CopyBytes)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        wrong_order.blocks[0]
            .instructions
            .swap(copies[0], copies[1]);
        assert!(validate(&wrong_order).is_err());
    }
}

fn owned_fixture(target: target::NativeTarget) -> legalized_operations::LegalizedScalarFunction {
    let mut source = super::subslices::view_fixture(target, true);
    let place = PlaceId::new(4).unwrap();
    let structural_type = StructuralTypeId::new(2).unwrap();
    let operation = OperationId::new(5).unwrap();
    let contract = source.structural.as_mut().unwrap();
    contract
        .structural_types
        .make_mut()
        .push(terminal_psi::StructuralTypeDeclaration {
            id: structural_type,
            identity: "owned bytes".into(),
            shape: terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity: 64 },
            ),
        });
    contract
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: place,
            kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
                producer: operation,
                structural_type,
            },
        });
    let mut concatenate = source.blocks[0].instructions[2].clone();
    concatenate.operation = operation;
    concatenate.fuel = vec![optimization_unit::FuelSettlement {
        site: optimization_unit::PsiProvenance::Operation(operation),
        units: 1,
    }];
    concatenate.kind = LegalizedScalarInstructionKind::EstablishByteSequenceConcatenation {
        result: terminal_psi::StructuralOperationResult {
            place,
            structural_type,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        },
        left: ByteSequenceOperand::Whole {
            source: PlaceId::new(1).unwrap(),
            length: ValueId::new(1).unwrap(),
        },
        right: ByteSequenceOperand::Whole {
            source: PlaceId::new(2).unwrap(),
            length: ValueId::new(4).unwrap(),
        },
        capacity_obligation: semantic_vocabulary::ObligationId::new(2).unwrap(),
        accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([2; 32]),
    };
    source.blocks[0].instructions.push(concatenate);
    let mut length = source.blocks[0].instructions[3].clone();
    length.operation = OperationId::new(6).unwrap();
    length.fuel = vec![optimization_unit::FuelSettlement {
        site: optimization_unit::PsiProvenance::Operation(length.operation),
        units: 1,
    }];
    let definition = length.result.as_mut().unwrap();
    definition.value = ValueId::new(6).unwrap();
    definition.definition_site = optimization_unit::ValueDefinitionSite::Node {
        block: source.blocks[0].id,
        node: 5,
    };
    let scalar_type = definition.scalar_type;
    length.kind = LegalizedScalarInstructionKind::ByteSequenceLength {
        source: place,
        length_byte_offset: 0,
    };
    source.blocks[0].instructions.push(length);
    source
        .provenance
        .operations
        .extend([operation, OperationId::new(6).unwrap()]);
    returned(&mut source.blocks[0]).value = LegalizedScalarReturnValue::Value {
        value: ValueId::new(6).unwrap(),
        scalar_type,
    };
    source
}

#[test]
fn borrowed_owner_replacement_replays_original_storage_and_length_publication() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        let mut source = owned_fixture(target);
        let destination = PlaceId::new(5).unwrap();
        let operation = OperationId::new(7).unwrap();
        let owner_type = StructuralTypeId::new(2).unwrap();
        let owner_shape = calling_conventions::ValueShape::borrowed_reference(72, 8);
        source.call_plan = calling_conventions::evaluate_call_plan(
            source.call_plan.policy,
            &calling_conventions::CallSignature {
                parameters: vec![source.call_plan.parameters[0].shape, owner_shape],
                result: source
                    .call_plan
                    .result
                    .as_ref()
                    .map(|placement| placement.shape),
            },
        )
        .unwrap();
        let contract = source.structural.as_mut().unwrap();
        contract.parameters[0].target.placement = source.call_plan.parameters[0].clone();
        let mut owner = contract.parameters[0].clone();
        owner.semantic.place = destination;
        owner.semantic.position = 1;
        owner.semantic.structural_type = owner_type;
        owner.semantic.access = terminal_psi::StructuralAccess::MutableBorrow;
        owner.target.place = destination;
        owner.target.structural_type = owner_type;
        owner.target.access = owner.semantic.access;
        owner.target.shape = owner_shape;
        owner.target.placement = source.call_plan.parameters[1].clone();
        contract.parameters.push(owner);
        contract
            .structural_places
            .push(terminal_psi::StructuralPlaceDeclaration {
                id: destination,
                kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                    position: 1,
                    is_self: false,
                },
            });
        let mut store = source.blocks[0].instructions[4].clone();
        store.operation = operation;
        store.result = None;
        store.fuel = vec![optimization_unit::FuelSettlement {
            site: optimization_unit::PsiProvenance::Operation(operation),
            units: 1,
        }];
        store.kind = LegalizedScalarInstructionKind::ByteSequenceStore {
            destination,
            source: PlaceId::new(4).unwrap(),
            length: ValueId::new(6).unwrap(),
            obligation: semantic_vocabulary::ObligationId::new(3).unwrap(),
            accepted_fact: optimization_core::AcceptedObligationFactIdentity::from_bytes([3; 32]),
        };
        source.blocks[0].instructions.push(store);
        source.provenance.operations.push(operation);
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
        let validate = |selected| {
            crate::selection::validation::scalar_graph::validate(
                0,
                &source,
                selected,
                target,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
        };
        validate(&selected).unwrap();
        let instructions = &selected.blocks[0].instructions;
        let copy = instructions
            .iter()
            .position(|row| {
                row.kind == SelectedInstructionKind::CopyBytes
                    && row.provenance.operations.contains(&operation)
            })
            .unwrap();
        let publish = instructions
            .iter()
            .position(|row| {
                row.kind
                    == (SelectedInstructionKind::Store {
                        byte_offset: 0,
                        byte_size: 8,
                    })
                    && row.provenance.operations.contains(&operation)
            })
            .unwrap();
        assert!(copy < publish);
        let mut wrong_destination = selected.clone();
        wrong_destination.blocks[0].instructions[copy]
            .operands
            .swap(0, 1);
        assert!(validate(&wrong_destination).is_err());
        let mut premature_length = selected.clone();
        premature_length.blocks[0].instructions.swap(copy, publish);
        assert!(validate(&premature_length).is_err());
        let mut wrong_metadata = selected.clone();
        wrong_metadata.blocks[0].instructions[publish].kind = SelectedInstructionKind::Store {
            byte_offset: 8,
            byte_size: 8,
        };
        assert!(validate(&wrong_metadata).is_err());
    }
}
