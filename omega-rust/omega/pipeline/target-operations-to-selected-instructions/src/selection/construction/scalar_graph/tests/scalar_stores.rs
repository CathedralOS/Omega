//! Store replay binds the original pointer, exact write width, SSA source and charge.
use super::*;

#[test]
fn borrowed_scalar_store_replay_rejects_changed_footprint_source_and_fuel() {
    use selected_instructions::SelectedMemoryAccessRole;
    use terminal_psi::{
        StructuralAccess, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
    };
    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
        target::NativeTarget::windows_x64(),
        target::NativeTarget::macos_arm64(),
    ] {
        for (bytes, scalar_type) in
            [(1_u8, ScalarType::Boolean)]
                .into_iter()
                .chain([1_u8, 2, 4, 8].into_iter().map(|bytes| {
                    (
                        bytes,
                        ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, u16::from(bytes) * 8).unwrap(),
                        ),
                    )
                }))
        {
            for access in [
                StructuralAccess::MutableBorrow,
                StructuralAccess::WriteOnlyBorrow,
            ] {
                let environment =
                    register_environment::baseline_target_register_environment(target).unwrap();
                let mut source = fixture(target, 0);
                let structural_type = source.attachment.unwrap();
                let place = semantic_vocabulary::PlaceId::new(1).unwrap();
                let field = semantic_vocabulary::StructuralFieldId::new(1).unwrap();
                source.call_plan = evaluate_call_plan(
                    CallingPolicy::native_for_target(target),
                    &CallSignature {
                        parameters: vec![ValueShape::borrowed_reference(
                            u16::from(bytes),
                            u16::from(bytes),
                        )],
                        result: None,
                    },
                )
                .unwrap();
                let destination = StructuralParameterDeclaration {
                    place,
                    position: 0,
                    is_self: true,
                    structural_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                };
                source.structural = Some(legalized_operations::LegalizedStructuralContract {
                    structural_types: vec![StructuralTypeDeclaration {
                        id: structural_type,
                        identity: "Record".into(),
                        shape: StructuralTypeShape::Record {
                            fields: vec![StructuralFieldDeclaration {
                                id: field,
                                identity: "value".into(),
                                relevance: terminal_psi::BindingRelevance::Relevant,
                                field_type: StructuralFieldType::Scalar(scalar_type),
                            }],
                        },
                    }],
                    parameters: vec![legalized_operations::LegalizedCallUnitParameter {
                        semantic: destination.clone(),
                        target: target_operations::TargetStructuralParameter {
                            place,
                            structural_type,
                            multiplicity: destination.multiplicity,
                            access,
                            projected_qualifications: Vec::new(),
                            shape: source.call_plan.parameters[0].shape,
                            placement: source.call_plan.parameters[0].clone(),
                        },
                    }],
                    structural_places: vec![terminal_psi::StructuralPlaceDeclaration {
                        id: place,
                        kind: semantic_vocabulary::StructuralPlaceKind::Parameter {
                            position: 0,
                            is_self: true,
                        },
                    }],
                    entry_claims: Vec::new(),
                    published_service_ceiling: Vec::new(),
                });
                source.blocks[0].instructions.truncate(2);
                source.provenance.operations.truncate(2);
                source.blocks[0].instructions[0]
                    .result
                    .as_mut()
                    .unwrap()
                    .scalar_type = scalar_type;
                source.blocks[0].instructions[1].result = None;
                source.blocks[0].instructions[1].kind =
                    LegalizedScalarInstructionKind::StructuralScalarFieldStore {
                        destination,
                        path: Vec::new(),
                        field,
                        value: abstract_operations::AbstractResult {
                            value: ValueId::new(1).unwrap(),
                            scalar_type,
                        },
                        byte_offset: 0,
                        byte_size: bytes,
                    };
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
                assert_eq!(selected.memory_accesses.len(), 1);
                assert_eq!(
                    selected.memory_accesses[0].role,
                    SelectedMemoryAccessRole::WritePlace
                );
                for mutation in 0..5 {
                    let mut candidate = selected.clone();
                    let instruction = candidate.blocks[0]
                        .instructions
                        .iter_mut()
                        .find(|instruction| {
                            matches!(instruction.kind, SelectedInstructionKind::Store { .. })
                        })
                        .unwrap();
                    match mutation {
                        0 => {
                            instruction.kind = SelectedInstructionKind::Store {
                                byte_offset: 1,
                                byte_size: bytes,
                            }
                        }
                        1 => {
                            instruction.kind = SelectedInstructionKind::Store {
                                byte_offset: 0,
                                byte_size: if bytes == 8 { 4 } else { 8 },
                            }
                        }
                        2 => instruction.provenance.fuel.clear(),
                        3 => {
                            candidate.memory_accesses[0].role = SelectedMemoryAccessRole::ReadPlace
                        }
                        _ => instruction.operands.swap(0, 1),
                    }
                    assert!(
                        validate(&candidate).is_err(),
                        "mutation {mutation}, width {bytes}"
                    );
                }
            }
        }
    }
}
