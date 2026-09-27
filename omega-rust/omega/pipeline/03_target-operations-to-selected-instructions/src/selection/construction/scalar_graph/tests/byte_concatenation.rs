//! Selection replay must preserve two runtime spans and the owned result layout.
use super::{
    LegalizedScalarInstructionKind, LegalizedScalarReturnValue, SelectedInstructionKind,
    SelectedSelectionConstraints, build, returned,
};
use semantic_vocabulary::{OperationId, PlaceId, StructuralTypeId, ValueId};
use terminal_psi::control_flow::ByteSequenceOperand;

#[test]
fn owned_concatenation_replays_live_copy_counts_and_inline_length() {
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
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
