//! Zero initialization retains every field and array leaf through selection.
use crate::tests::fixtures::plain_unit::plain_unit_fixture;
use crate::{
    legalize_target_operations, select_instructions, selection_constraints,
    validate_selected_instructions,
};
use abstract_operations::AbstractOperation;
use semantic_vocabulary::{
    FuelScheduleIdentity, IntegerSign, IntegerType, OperationId, PlaceId, ScalarType,
    StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction,
};

#[test]
fn zeroed_records_and_array_leaves_select_exact_stores_and_reject_substitution() {
    for native in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        for multiplicity in [
            StructuralMultiplicity::Unrestricted,
            StructuralMultiplicity::Affine,
        ] {
            let (mut source, _, _) = plain_unit_fixture();
            let scalar = |width| {
                ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, width).unwrap())
            };
            let root = StructuralTypeId::new(1).unwrap();
            let array = StructuralTypeId::new(2).unwrap();
            let element = StructuralTypeId::new(3).unwrap();
            let field = |identity, name: &str, field_type| StructuralFieldDeclaration {
                id: StructuralFieldId::new(identity).unwrap(),
                identity: name.into(),
                relevance: BindingRelevance::Relevant,
                field_type,
            };
            source.structural_types = vec![
                StructuralTypeDeclaration {
                    id: root,
                    identity: "test::Zeroed".into(),
                    shape: StructuralTypeShape::Record {
                        fields: vec![
                            field(1, "tag", StructuralFieldType::Scalar(ScalarType::Boolean)),
                            field(2, "wide", StructuralFieldType::Scalar(scalar(64))),
                            field(3, "items", StructuralFieldType::Structural(array)),
                        ],
                    },
                },
                StructuralTypeDeclaration {
                    id: array,
                    identity: "test::Array".into(),
                    shape: StructuralTypeShape::FixedArray { element, length: 3 },
                },
                StructuralTypeDeclaration {
                    id: element,
                    identity: "test::Element".into(),
                    shape: StructuralTypeShape::PrimitiveScalar(scalar(16)),
                },
            ]
            .into();
            let place = PlaceId::new(1).unwrap();
            source.functions[0].operations.insert(
                0,
                AbstractOperation::EstablishZeroedStructural {
                    psi_operation: OperationId::new(1).unwrap(),
                    result: StructuralOperationResult {
                        place,
                        structural_type: root,
                        multiplicity,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        qualification_establishments: Vec::new(),
                        claims: Vec::new(),
                    },
                },
            );
            if multiplicity == StructuralMultiplicity::Affine {
                let AbstractOperation::ReturnUnit {
                    cleanup_actions, ..
                } = &mut source.functions[0].operations[1]
                else {
                    panic!("unit return")
                };
                cleanup_actions.push(TerminalAffineCleanupAction::DiscardRoot(place));
            }
            let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap();
            optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &source,
                abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
            )
            .unwrap();
            let legal = legalize_target_operations(&target, &source, &unit).unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = selection_constraints(&legal, &environment);
            let selected = select_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .expect("zeroed structural storage selects");
            validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .unwrap();
            let mut stores = selected.plan().functions[0]
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter_map(|instruction| {
                    if let selected_instructions::SelectedInstructionKind::Store {
                        byte_offset,
                        byte_size,
                    } = instruction.kind
                    {
                        Some((byte_offset, byte_size))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            stores.sort();
            assert_eq!(stores, [(0, 1), (8, 8), (16, 2), (18, 2), (20, 2)]);
            let mut changed = selected.plan().clone();
            let store = changed.functions[0]
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find(|instruction| {
                    matches!(
                        instruction.kind,
                        selected_instructions::SelectedInstructionKind::Store {
                            byte_offset: 8,
                            ..
                        }
                    )
                })
                .unwrap();
            let selected_instructions::SelectedInstructionKind::Store { byte_offset, .. } =
                &mut store.kind
            else {
                unreachable!()
            };
            *byte_offset = 0;
            assert!(
                validate_selected_instructions(
                    &legal,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    changed
                )
                .is_err()
            );
        }
    }
}
