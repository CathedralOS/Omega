//! A whole array crosses an owned state edge without changing its storage shape.
use super::block_home_layout;
use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation as Operation,
    AbstractOperationPlan, AbstractStructuralBinding,
};
use calling_conventions::ValueShape;
use semantic_vocabulary::{FuelScheduleIdentity, PsiSemanticId, ScalarType};
use target::NativeTarget;
use target_operations::{
    TargetControlTerminator, TargetStructuralHomeLayout, TargetStructuralReturnSource,
};
use terminal_psi::{
    BindingRelevance, ByteSequenceCarrier, SemanticFingerprint, StructuralAccess,
    StructuralArgument, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralResultDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalPsiIdentity, VocabularyMarker,
};

fn id<T: PsiSemanticId>(value: u64) -> T {
    T::new(value).unwrap()
}

fn arrival() -> StructuralParameterDeclaration {
    StructuralParameterDeclaration {
        place: id(2),
        position: 0,
        is_self: false,
        structural_type: id(1),
        access: StructuralAccess::Owned,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }
}

fn source(length: u64) -> AbstractOperationPlan {
    let result = StructuralOperationResult {
        place: id(1),
        structural_type: id(1),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        claims: Vec::new(),
    };
    let mut operations = Vec::new();
    if length != 0 {
        operations.push(Operation::BooleanConstant {
            psi_operation: id(1),
            result: id(1),
            value: true,
        });
    }
    operations.push(Operation::EstablishScalarArray {
        psi_operation: id(2),
        result,
        elements: vec![id(1); length as usize],
    });
    operations.push(Operation::Jump {
        psi_edge: id(1),
        target: id(2),
        bindings: Vec::new(),
        structural_bindings: vec![AbstractStructuralBinding {
            parameter: id(2),
            argument: StructuralArgument {
                place: id(1),
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
        }],
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    });
    let arrival_offset = operations.len();
    operations.push(Operation::ReturnStructural {
        psi_edge: id(2),
        source: id(2),
        returned_claims: Vec::new(),
        trivial_affine_locals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    });
    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x71; 32]),
        },
        entry: id(1),
        structural_types: vec![
            StructuralTypeDeclaration {
                id: id(1),
                identity: "test::Array".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: id(2),
                    length,
                },
            },
            StructuralTypeDeclaration {
                id: id(2),
                identity: "test::Boolean".into(),
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            },
        ]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine: id(1),
            attachment: None,
            entry: id(1),
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Structural(StructuralResultDeclaration {
                reference_sources: Vec::new(),
                place: id(3),
                structural_type: id(1),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![
                AbstractBlockEntry {
                    block: id(1),
                    parameters: Vec::new(),
                    structural_parameters: Vec::new(),
                    operation_offset: 0,
                },
                AbstractBlockEntry {
                    block: id(2),
                    parameters: Vec::new(),
                    structural_parameters: vec![arrival()],
                    operation_offset: arrival_offset,
                },
            ],
            operations,
        }],
    }
}

#[test]
fn owned_array_arrival_replays_exact_home_and_rejects_substitution() {
    for native in [NativeTarget::linux_x64(), NativeTarget::macos_arm64()] {
        for length in [0, 2] {
            let source = source(length);
            let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap();
            optimization_unit_semantics::validate_psi_optimization_unit(&unit)
                .unwrap_or_else(|error| panic!("{native:?}, length {length}: {error:?}"));
            let targeted = abstract_operations_to_target_operations::lower_to_target_operations(
                &source,
                abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
            )
            .unwrap();
            let legalized = crate::legalize_target_operations(&targeted, &source, &unit).unwrap();
            crate::validate_legalized_operations(
                &targeted,
                &source,
                &unit,
                legalized.plan().clone(),
            )
            .unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = crate::selection_constraints(&legalized, &environment);
            let selected = crate::select_instructions(
                &legalized,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap_or_else(|error| panic!("{native:?}, length {length}: {error:?}"));
            crate::validate_selected_instructions(
                &legalized,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .unwrap_or_else(|error| panic!("{native:?}, length {length}: {error:?}"));
            if length == 0 {
                assert!(selected.plan().functions[0].local_storage_slots.is_empty());
                assert!(selected.plan().functions[0].memory_accesses.is_empty());
                assert!(selected.plan().functions[0].virtual_registers.is_empty());
            }
            for mutation in 0..3 {
                let mut changed = selected.plan().clone();
                let function = &mut changed.functions[0];
                let binding = function
                    .blocks
                    .iter_mut()
                    .find_map(|block| {
                        let selected_instructions::SelectedTerminator::Jump { successor, .. } =
                            &mut block.terminator
                        else {
                            return None;
                        };
                        successor.structural_bindings.iter_mut().find(|binding| {
                            mutation != 0
                                || length == 0
                                || matches!(
                                    binding.transport,
                                    selected_instructions::SelectedStructuralTransport::WholeValue {
                                        ..
                                    }
                                )
                        })
                    })
                    .expect("the owned value retains its semantic or physical edge binding");
                match mutation {
                    0 => {
                        binding.transport = if length == 0 {
                            selected_instructions::SelectedStructuralTransport::WholeValue {
                            argument: selected_instructions::VirtualRegisterId(99),
                            destination: selected_instructions::LocalStorageSlotId::StructuralBlockParameter { block: id(2), place: id(2) },
                            byte_size: 0,
                            alignment: 1,
                        }
                        } else {
                            selected_instructions::SelectedStructuralTransport::Unused
                        }
                    }
                    1 => binding.semantic.argument.place = id(99),
                    2 => {
                        if length == 0 {
                            function.local_storage_slots.push(selected_instructions::SelectedLocalStorageSlot {
                            id: selected_instructions::LocalStorageSlotId::StructuralBlockParameter { block: id(2), place: id(2) },
                            byte_size: 0,
                            alignment: 1,
                        });
                        } else {
                            function.local_storage_slots.clear();
                        }
                    }
                    _ => unreachable!(),
                }
                assert!(
                    crate::validate_selected_instructions(
                        &legalized,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                        changed,
                    )
                    .is_err(),
                    "{native:?}, length {length}, selected mutation {mutation}"
                );
            }
            for mutation in 0..3 {
                let mut changed = targeted.clone();
                let block = changed.functions[0]
                    .graph
                    .blocks
                    .iter_mut()
                    .find(|block| block.block == id(2))
                    .unwrap();
                let TargetControlTerminator::ReturnStructural {
                    source: TargetStructuralReturnSource::Home(home),
                    ..
                } = &mut block.terminator
                else {
                    panic!("owned arrival return");
                };
                match mutation {
                    0 => {
                        home.layout =
                            TargetStructuralHomeLayout::Aggregate(ValueShape::integer(17, 1))
                    }
                    1 => {
                        let target_operations::TargetStructuralHomeOrigin::BlockParameter {
                            declaration,
                            ..
                        } = &mut home.origin
                        else {
                            panic!("block home");
                        };
                        declaration.access = StructuralAccess::SharedBorrow;
                    }
                    2 => block.structural_parameters[0].structural_type = id(2),
                    _ => unreachable!(),
                }
                assert!(
                    crate::legalize_target_operations(&changed, &source, &unit).is_err(),
                    "{native:?}, length {length}, mutation {mutation}"
                );
            }
        }
    }
}

#[test]
fn owned_arrival_layout_excludes_scalar_and_descriptor_roots() {
    let mut plan = source(2);
    for shape in [
        StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        StructuralTypeShape::Reference {
            referent: id(2),
            access: StructuralAccess::SharedBorrow,
        },
        StructuralTypeShape::ElementView { element: id(2) },
        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { access: None }),
    ] {
        plan.structural_types = vec![
            StructuralTypeDeclaration {
                id: id(1),
                identity: "test::NonOwner".into(),
                shape,
            },
            StructuralTypeDeclaration {
                id: id(2),
                identity: "test::Boolean".into(),
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            },
        ]
        .into();
        assert!(block_home_layout(&arrival(), &plan).is_err());
    }
    let plan = source(2);
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut declaration = arrival();
        declaration.access = access;
        assert!(block_home_layout(&declaration, &plan).is_err());
    }
    let mut declaration = arrival();
    declaration.multiplicity = StructuralMultiplicity::Linear;
    assert!(block_home_layout(&declaration, &plan).is_err());
}

#[test]
fn owned_arrival_storage_preserves_erasure_and_nested_descriptor_geometry() {
    let mut plan = source(2);
    for (relevance, field_type, expected, copied) in [
        (
            BindingRelevance::Erased,
            StructuralFieldType::Scalar(ScalarType::Boolean),
            ValueShape::integer(0, 1),
            None,
        ),
        (
            BindingRelevance::Relevant,
            StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::SharedBorrow),
            }),
            ValueShape::integer(16, 8),
            Some(ValueShape::integer(16, 8)),
        ),
        (
            BindingRelevance::Relevant,
            StructuralFieldType::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::MutableBorrow),
            }),
            ValueShape::integer(16, 8),
            None,
        ),
    ] {
        plan.structural_types = vec![StructuralTypeDeclaration {
            id: id(1),
            identity: "test::EstablishedRecord".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: id(1),
                    identity: "value".into(),
                    relevance,
                    field_type,
                }],
            },
        }]
        .into();
        assert_eq!(
            block_home_layout(&arrival(), &plan).unwrap().shape(),
            expected
        );
        // Shared descriptors can travel in a whole-value copy; exclusive
        // descriptors still have storage without permission to duplicate it.
        assert_eq!(
            crate::structural_inputs::structural_reference_input::owned_aggregate_shape(
                id(1),
                &plan.structural_types
            ),
            copied
        );
    }
}

#[test]
fn completed_structural_array_replays_element_storage_and_rejects_substitution() {
    for native in [NativeTarget::linux_x64(), NativeTarget::macos_arm64()] {
        for (length, padded_bytes) in [(0, false), (2, false), (2, true)] {
            let mut source = source(length);
            let mut declarations = source.structural_types.to_vec();
            declarations[1].shape = StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: id(1),
                    identity: "value".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                }],
            };
            if padded_bytes {
                declarations[1].shape =
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                        capacity: 1,
                    });
            }
            source.structural_types = declarations.into();
            let function = &mut source.functions[0];
            let position = function
                .operations
                .iter()
                .position(|operation| matches!(operation, Operation::EstablishScalarArray { .. }))
                .unwrap();
            let Operation::EstablishScalarArray {
                psi_operation,
                result,
                ..
            } = function.operations[position].clone()
            else {
                unreachable!()
            };
            let mut replacements = Vec::new();
            let mut elements = Vec::new();
            for ordinal in 0..length {
                let child = StructuralOperationResult {
                    place: id(100 + ordinal),
                    structural_type: id(2),
                    ..result.clone()
                };
                elements.push(StructuralArgument {
                    place: child.place,
                    path: Vec::new(),
                    access: StructuralAccess::Owned,
                });
                if padded_bytes {
                    function
                        .structural_parameters
                        .push(StructuralParameterDeclaration {
                            place: child.place,
                            position: ordinal as u32,
                            is_self: false,
                            structural_type: child.structural_type,
                            access: StructuralAccess::Owned,
                            multiplicity: StructuralMultiplicity::Unrestricted,
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                        });
                } else {
                    replacements.push(Operation::EstablishRecord {
                        psi_operation: id(100 + ordinal),
                        result: child,
                        fields: vec![terminal_psi::RecordFieldInitializer {
                            field: id(1),
                            value: terminal_psi::RecordFieldValue::Scalar {
                                value: id(1),
                                range_obligation: None,
                            },
                        }],
                    });
                }
            }
            replacements.push(Operation::EstablishStructuralArray {
                psi_operation,
                result,
                elements,
            });
            function
                .operations
                .splice(position..=position, replacements);
            function.block_entries[1].operation_offset +=
                if padded_bytes { 0 } else { length as usize };
            let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap();
            optimization_unit_semantics::validate_psi_optimization_unit(&unit)
                .unwrap_or_else(|error| panic!("{native:?}, length {length}: {error:?}"));
            let targeted = abstract_operations_to_target_operations::lower_to_target_operations(
                &source,
                abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
            )
            .unwrap();
            let legalized = crate::legalize_target_operations(&targeted, &source, &unit).unwrap();
            if padded_bytes {
                let function = &legalized.plan().scalar_functions[0];
                let row = function.blocks.iter().flat_map(|block| &block.instructions)
                    .find(|row| matches!(row.kind, legalized_operations::LegalizedScalarInstructionKind::EstablishStructuralArray { .. })).unwrap();
                let legalized_operations::LegalizedScalarInstructionKind::EstablishStructuralArray { shape, .. } = &row.kind else { unreachable!() };
                // Capacity one occupies nine live bytes but each aligned
                // element owns sixteen bytes in the completed array home.
                assert_eq!(shape.byte_size, 32);
                assert_eq!(shape.alignment, 8);
            }
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = crate::selection_constraints(&legalized, &environment);
            let selected = crate::select_instructions(
                &legalized,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            crate::validate_selected_instructions(
                &legalized,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .unwrap();
            let mut changed = legalized.plan().clone();
            let constructor = changed.scalar_functions[0].blocks.iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find(|row| matches!(row.kind, legalized_operations::LegalizedScalarInstructionKind::EstablishStructuralArray { .. })).unwrap();
            let legalized_operations::LegalizedScalarInstructionKind::EstablishStructuralArray {
                elements,
                shape,
                ..
            } = &mut constructor.kind
            else {
                unreachable!()
            };
            if let Some(element) = elements.first_mut() {
                element.place = id(999);
            } else {
                shape.byte_size = 1;
            }
            assert!(
                crate::validate_legalized_operations(&targeted, &source, &unit, changed).is_err()
            );
            let mut changed = selected.plan().clone();
            changed.functions[0].local_storage_slots.clear();
            assert!(
                crate::validate_selected_instructions(
                    &legalized,
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
