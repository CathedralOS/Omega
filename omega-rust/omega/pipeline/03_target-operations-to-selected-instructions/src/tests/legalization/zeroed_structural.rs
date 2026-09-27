//! Zero initialization retains every field and array leaf through selection.
use crate::tests::fixtures::plain_unit::plain_unit_fixture;
use crate::{
    legalize_target_operations, select_instructions, selection_constraints,
    validate_selected_instructions,
};
use abstract_operations::{AbstractFunctionResult, AbstractOperation, AbstractResult};
use semantic_vocabulary::{
    FuelScheduleIdentity, IntegerSign, IntegerType, OperationId, PlaceId, ScalarType,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BindingRelevance, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalAffineCleanupAction,
};

#[test]
fn zeroed_records_and_array_leaves_select_exact_stores_and_reads() {
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
            let read = ValueId::new(1).unwrap();
            let returned = ValueId::new(2).unwrap();
            let AbstractOperation::ReturnUnit { psi_edge, .. } = source.functions[0].operations[1]
            else {
                panic!("unit return");
            };
            source.functions[0].result = AbstractFunctionResult::Scalar(AbstractResult {
                value: returned,
                scalar_type: scalar(64),
            });
            source.functions[0].operations[1] = AbstractOperation::IntegerStructuralField {
                psi_operation: OperationId::new(2).unwrap(),
                result: AbstractResult {
                    value: read,
                    scalar_type: scalar(64),
                },
                source: place,
                path: Vec::new(),
                field: StructuralFieldId::new(2).unwrap(),
            };
            source.functions[0]
                .operations
                .push(AbstractOperation::Return {
                    psi_edge,
                    result: returned,
                    value: read,
                    scalar_type: scalar(64),
                    cleanup_actions: if multiplicity == StructuralMultiplicity::Affine {
                        vec![TerminalAffineCleanupAction::DiscardRoot(place)]
                    } else {
                        Vec::new()
                    },
                });
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
            assert!(
                selected.plan().functions[0]
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .any(|instruction| matches!(
                        instruction.kind,
                        selected_instructions::SelectedInstructionKind::Load64 { byte_offset: 8 }
                    ))
            );
            let mut changed_read = selected.plan().clone();
            let read = changed_read.functions[0]
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find(|instruction| {
                    matches!(
                        instruction.kind,
                        selected_instructions::SelectedInstructionKind::Load64 { .. }
                    )
                })
                .unwrap();
            let selected_instructions::SelectedInstructionKind::Load64 { byte_offset } =
                &mut read.kind
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
                    changed_read
                )
                .is_err()
            );
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
