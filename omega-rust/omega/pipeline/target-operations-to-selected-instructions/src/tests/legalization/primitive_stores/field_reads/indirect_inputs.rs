//! Owned ABI backing is reconstructed independently of pointer-bit transport.
use super::*;
use calling_conventions::{
    IndirectPointerLocation, MachineRegister, ValueLocation, ValuePlacement,
};
use selected_instructions::{
    FrameStorageSlotId, SelectedInstructionKind as Instruction, VirtualRegisterOrigin,
};

fn input(
    native: NativeTarget,
    scalar_count: usize,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let scalar = integer(IntegerSign::Unsigned, 64);
    let (mut source, _, _) = fixture(native, scalar, false);
    source.structural_types.make_mut()[0].shape = StructuralTypeShape::Record {
        fields: (1..=3)
            .map(|ordinal| StructuralFieldDeclaration {
                id: StructuralFieldId::new(ordinal).unwrap(),
                identity: format!("field{ordinal}"),
                relevance: BindingRelevance::Relevant,
                field_type: StructuralFieldType::Scalar(scalar),
            })
            .collect(),
    };
    let function = &mut source.functions[0];
    function.parameters = (0..scalar_count)
        .map(|position| AbstractParameter {
            value: ValueId::new(10 + position as u64).unwrap(),
            scalar_type: scalar,
        })
        .collect();
    function.structural_parameters[0].access = StructuralAccess::Owned;
    let place = function.structural_parameters[0].place;
    function.result = AbstractFunctionResult::Scalar(AbstractResult {
        value: ValueId::new(2).unwrap(),
        scalar_type: scalar,
    });
    function.operations = vec![
        AbstractOperation::IntegerStructuralField {
            psi_operation: OperationId::new(1).unwrap(),
            result: AbstractResult {
                value: ValueId::new(1).unwrap(),
                scalar_type: scalar,
            },
            source: place,
            field: StructuralFieldId::new(3).unwrap(),
        },
        AbstractOperation::Return {
            psi_edge: EdgeId::new(1).unwrap(),
            result: ValueId::new(2).unwrap(),
            value: ValueId::new(1).unwrap(),
            scalar_type: scalar,
            cleanup_actions: Vec::new(),
        },
    ];
    let target =
        abstract_operations_to_target_operations::lower_to_target_operations(&source, native)
            .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
    (source, target, unit)
}

fn corrupt_placement(placement: &mut ValuePlacement, mutation: &str) {
    let [
        ValueLocation::Indirect {
            pointer,
            copy_stack_byte_offset,
            byte_size,
            alignment,
        },
    ] = placement.locations.as_mut_slice()
    else {
        panic!("owned indirect placement");
    };
    match mutation {
        "pointer" => match pointer {
            IndirectPointerLocation::Register(register) => {
                *register = match register {
                    MachineRegister::Aarch64X(_) => MachineRegister::Aarch64X(7),
                    _ => MachineRegister::X86R11,
                }
            }
            IndirectPointerLocation::Stack {
                stack_byte_offset, ..
            } => *stack_byte_offset += 8,
        },
        "pointer alignment" => {
            *pointer = IndirectPointerLocation::Stack {
                stack_byte_offset: 0,
                alignment: 4,
            }
        }
        "extent" => *byte_size -= 8,
        "alignment" => *alignment = 16,
        "missing copy" => *copy_stack_byte_offset = None,
        "copy offset" => *copy_stack_byte_offset = Some(copy_stack_byte_offset.unwrap() + 16),
        _ => panic!("unknown placement corruption"),
    }
}

#[test]
fn owned_indirect_physical_shape_preserves_qualification_and_multiplicity_rows() {
    use semantic_vocabulary::StructuralDomainId;
    use terminal_psi::{StructuralPathQualification, StructuralPathSegment};

    for native in [
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        for scalar_count in [0, 8] {
            let (source, target, _) = input(native, scalar_count);
            let graph = &target.functions[0].graph;
            let accepts =
                |semantic: &StructuralParameterDeclaration,
                 target: &target_operations::TargetStructuralParameter| {
                    crate::structural_unit_input::accepts_graph(
                        &graph.call_plan,
                        &[crate::structural_unit_input::Parameter { semantic, target }],
                        &source.structural_types,
                    )
                };
            assert!(matches!(graph.parameters[0].placement.locations.as_slice(),
                [ValueLocation::Indirect { pointer, byte_size: 24, copy_stack_byte_offset: Some(_), .. }]
                    if matches!(pointer, IndirectPointerLocation::Stack { .. }) == (scalar_count == 8)));
            for multiplicity in [
                StructuralMultiplicity::Unrestricted,
                StructuralMultiplicity::Affine,
                StructuralMultiplicity::Linear,
            ] {
                let mut semantic = source.functions[0].structural_parameters[0].clone();
                let mut retained = graph.parameters[0].clone();
                // This tests physical ABI placement, not semantic-operation
                // permission. These qualification identities are opaque retained
                // payload here; domain/path validity and linear claim discharge
                // still require source checking before this body may execute.
                semantic.multiplicity = multiplicity;
                semantic.qualifications = vec![StructuralDomainId::new(100).unwrap()];
                semantic.projected_qualifications = vec![StructuralPathQualification {
                    path: vec![StructuralPathSegment::Field("field1".into())],
                    domain: StructuralDomainId::new(101).unwrap(),
                }];
                retained.multiplicity = multiplicity;
                retained.projected_qualifications = semantic.projected_qualifications.clone();
                let declared = semantic.clone();
                assert!(
                    accepts(&semantic, &retained),
                    "{native:?} {scalar_count} {multiplicity:?}"
                );
                assert_eq!(
                    semantic, declared,
                    "ABI reconstruction cannot strip declarations"
                );
                assert_eq!(retained.placement, graph.call_plan.parameters[scalar_count]);

                for mutation in [
                    "missing projected qualification",
                    "projected domain",
                    "projected path",
                    "access",
                    "multiplicity",
                    "shape",
                    "place",
                    "position",
                ] {
                    let mut changed_semantic = semantic.clone();
                    let mut changed_target = retained.clone();
                    match mutation {
                        "missing projected qualification" => {
                            changed_target.projected_qualifications.clear()
                        }
                        "projected domain" => {
                            changed_target.projected_qualifications[0].domain =
                                StructuralDomainId::new(102).unwrap()
                        }
                        "projected path" => {
                            changed_target.projected_qualifications[0].path =
                                vec![StructuralPathSegment::Field("field2".into())]
                        }
                        "access" => changed_target.access = StructuralAccess::SharedBorrow,
                        "multiplicity" => {
                            changed_target.multiplicity =
                                if multiplicity == StructuralMultiplicity::Linear {
                                    StructuralMultiplicity::Unrestricted
                                } else {
                                    StructuralMultiplicity::Linear
                                }
                        }
                        "shape" => changed_target.shape.byte_size = 16,
                        "place" => changed_target.place = PlaceId::new(99).unwrap(),
                        // Native position is carried by the ordered semantic
                        // row; TargetStructuralParameter has no position field.
                        "position" => changed_semantic.position = 1,
                        _ => unreachable!(),
                    }
                    assert!(
                        !accepts(&changed_semantic, &changed_target),
                        "{native:?} {scalar_count} {multiplicity:?} {mutation}"
                    );
                }
            }
        }
    }
}

#[test]
fn owned_indirect_inputs_reject_substituted_abi_and_receiving_access() {
    for native in [
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        for scalar_count in [0, 8] {
            let (source, target, unit) = input(native, scalar_count);
            let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
            validate_legalized_operations(&target, &source, &unit, legalized.plan().clone())
                .unwrap();
            let placement =
                &legalized.plan().scalar_functions[0].call_plan.parameters[scalar_count];
            assert_eq!(
                placement.shape,
                calling_conventions::ValueShape::integer(24, 8)
            );
            assert!(
                matches!(placement.locations.as_slice(), [ValueLocation::Indirect { pointer, copy_stack_byte_offset: Some(_), byte_size: 24, alignment: 8 }]
                if matches!(pointer, IndirectPointerLocation::Stack { .. }) == (scalar_count == 8))
            );
            for mutation in [
                "pointer",
                "pointer alignment",
                "extent",
                "alignment",
                "missing copy",
                "copy offset",
                "access",
                "shape",
            ] {
                let mut changed = target.clone();
                let function = &mut changed.functions[0];
                let parameter = &mut function.graph.parameters[0];
                match mutation {
                    "access" => parameter.access = StructuralAccess::SharedBorrow,
                    "shape" => {
                        parameter.shape.byte_size = 16;
                        parameter.placement.shape = parameter.shape;
                    }
                    _ => corrupt_placement(&mut parameter.placement, mutation),
                }
                // Keep duplicate retained ABI rows consistent: replay must rejoin
                // the source declaration and normalized plan, not just compare copies.
                function.graph.call_plan.parameters[scalar_count] = parameter.placement.clone();
                let abi = function.mixed_structural_scalar_abi.as_mut().unwrap();
                abi.structural_parameters = function.graph.parameters.clone();
                abi.call_plan = function.graph.call_plan.clone();
                assert!(
                    legalize_target_operations(&changed, &source, &unit).is_err(),
                    "{native:?} {scalar_count} target {mutation}"
                );
                assert!(
                    validate_legalized_operations(
                        &changed,
                        &source,
                        &unit,
                        legalized.plan().clone()
                    )
                    .is_err(),
                    "{native:?} {scalar_count} target replay {mutation}"
                );

                let mut changed = legalized.plan().clone();
                let function = &mut changed.scalar_functions[0];
                let parameter = &mut function.structural.as_mut().unwrap().parameters[0];
                match mutation {
                    "access" => {
                        parameter.semantic.access = StructuralAccess::SharedBorrow;
                        parameter.target.access = StructuralAccess::SharedBorrow;
                    }
                    "shape" => {
                        parameter.target.shape.byte_size = 16;
                        parameter.target.placement.shape = parameter.target.shape;
                    }
                    _ => corrupt_placement(&mut parameter.target.placement, mutation),
                }
                function.call_plan.parameters[scalar_count] = parameter.target.placement.clone();
                assert!(
                    validate_legalized_operations(&target, &source, &unit, changed).is_err(),
                    "{native:?} {scalar_count} legalized replay {mutation}"
                );
            }
        }
    }
}

#[test]
fn owned_indirect_entry_replay_checks_pointer_capture_without_a_value_copy() {
    for native in [
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        for scalar_count in [0, 8] {
            let (source, target, unit) = input(native, scalar_count);
            let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
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
            let retained = &selected.plan().functions[0];
            assert!(retained.local_storage_slots.is_empty());
            assert_eq!(retained.memory_accesses.len(), 1);
            assert!(!retained.blocks[0].instructions.iter().any(|row| matches!(
                row.kind,
                Instruction::Store { .. } | Instruction::Store64 { .. }
            )));
            crate::validate_selected_instructions(
                &legalized,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .unwrap();
            if scalar_count == 0 {
                let incoming = retained
                    .virtual_registers
                    .iter()
                    .find(|register| {
                        matches!(
                            register.origin,
                            VirtualRegisterOrigin::StructuralParameter { .. }
                        )
                    })
                    .unwrap();
                for mutation in ["register", "owner", "ordinal"] {
                    let mut changed = selected.plan().clone();
                    let register =
                        &mut changed.functions[0].virtual_registers[incoming.id.0 as usize];
                    match mutation {
                        "register" => register.entry_fixed_view = None,
                        "owner" => {
                            register.origin = VirtualRegisterOrigin::StructuralParameter {
                                place: PlaceId::new(99).unwrap(),
                                parameter_index: 0,
                            }
                        }
                        _ => {
                            register.origin = VirtualRegisterOrigin::StructuralParameter {
                                place: PlaceId::new(1).unwrap(),
                                parameter_index: 8,
                            }
                        }
                    }
                    assert!(
                        crate::validate_selected_instructions(
                            &legalized,
                            &constraints,
                            environment.physical(),
                            environment.constraints(),
                            changed
                        )
                        .is_err(),
                        "{native:?} register {mutation}"
                    );
                }
                continue;
            }
            let entry = retained.blocks[0]
                .instructions
                .iter()
                .position(|row| {
                    matches!(
                        row.kind,
                        Instruction::FrameAddress {
                            slot: FrameStorageSlotId::Incoming { .. },
                            ..
                        }
                    )
                })
                .unwrap();
            let Instruction::FrameAddress {
                slot:
                    FrameStorageSlotId::Incoming {
                        parameter_index,
                        abi_stack_byte_offset,
                    },
                byte_offset: 0,
            } = retained.blocks[0].instructions[entry].kind
            else {
                panic!("incoming pointer address")
            };
            assert_eq!(parameter_index, 8);
            assert!(matches!(
                retained.blocks[0].instructions[entry + 1].kind,
                Instruction::Load64 { byte_offset: 0 }
            ));
            let [
                ValueLocation::Indirect {
                    copy_stack_byte_offset: Some(copy_offset),
                    ..
                },
            ] = legalized.plan().scalar_functions[0].call_plan.parameters[scalar_count]
                .locations
                .as_slice()
            else {
                panic!("owned copy")
            };
            assert_ne!(abi_stack_byte_offset, *copy_offset);
            for mutation in [
                "ordinal",
                "offset",
                "copy as pointer slot",
                "address offset",
                "load offset",
                "load width",
                "missing dereference",
                "operand",
                "fuel",
            ] {
                let mut changed = selected.plan().clone();
                let rows = &mut changed.functions[0].blocks[0].instructions;
                match mutation {
                    "ordinal" | "offset" | "copy as pointer slot" | "address offset" => {
                        rows[entry].kind = Instruction::FrameAddress {
                            slot: FrameStorageSlotId::Incoming {
                                parameter_index: if mutation == "ordinal" {
                                    0
                                } else {
                                    parameter_index
                                },
                                abi_stack_byte_offset: match mutation {
                                    "offset" => abi_stack_byte_offset + 8,
                                    "copy as pointer slot" => *copy_offset,
                                    _ => abi_stack_byte_offset,
                                },
                            },
                            byte_offset: if mutation == "address offset" { 8 } else { 0 },
                        }
                    }
                    "load offset" => rows[entry + 1].kind = Instruction::Load64 { byte_offset: 8 },
                    "load width" => rows[entry + 1].kind = Instruction::Load32 { byte_offset: 0 },
                    "missing dereference" => rows[entry + 1].kind = Instruction::CopyI64,
                    "operand" => rows[entry + 1].operands.swap(0, 1),
                    _ => {
                        rows[entry + 1].provenance.fuel =
                            legalized.plan().scalar_functions[0].blocks[0].instructions[0]
                                .fuel
                                .clone()
                    }
                }
                assert!(
                    crate::validate_selected_instructions(
                        &legalized,
                        &constraints,
                        environment.physical(),
                        environment.constraints(),
                        changed
                    )
                    .is_err(),
                    "{native:?} stack {mutation}"
                );
            }
        }
    }
}
