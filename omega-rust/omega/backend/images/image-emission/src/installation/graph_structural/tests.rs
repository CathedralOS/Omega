//! Canonical structural ABI and pointer-home rosters at installation.
use super::super::borrowed_structural;
use super::super::resource_tests::installed_function_with_unit_call;
use super::*;
use calling_conventions::{MachineRegister, ValueShape};
use semantic_vocabulary::{PlaceId, StructuralTypeId, ValueId};
use target::NativeTarget;

#[test]
fn direct_structural_roster_retains_exact_borrowed_home_subset() {
    use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
    let target = target::NativeTarget::macos_arm64();
    for owned_bytes in [0, 2] {
        let mut function = installed_function_with_unit_call();
        let shapes = [
            ValueShape::integer(owned_bytes, 1),
            ValueShape::borrowed_reference(8, 8),
        ];
        let plan = evaluate_call_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: shapes.to_vec(),
                result: Some(shapes[0]),
            },
        )
        .unwrap();
        function.unit_parameters = shapes
            .iter()
            .enumerate()
            .map(|(position, shape)| machine_code::UnitParameterRecord {
                place: PlaceId::new(position as u64 + 1).unwrap(),
                structural_type: StructuralTypeId::new(position as u64 + 1).unwrap(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: if position == 0 {
                    terminal_psi::StructuralAccess::Owned
                } else {
                    terminal_psi::StructuralAccess::SharedBorrow
                },
                shape: *shape,
            })
            .collect();
        let borrowed = &function.unit_parameters[1];
        function
            .unit_parameter_homes
            .push(machine_code::UnitParameterHomeRecord {
                place: borrowed.place,
                structural_type: borrowed.structural_type,
                multiplicity: borrowed.multiplicity,
                access: borrowed.access,
                shape: borrowed.shape,
                source: plan.parameters[1].clone(),
                indirect: true,
                location: machine_code::StructuralSourceLocation::IncomingBorrowedPointer {
                    location: borrowed_structural::pointer_location(&plan.parameters[1]).unwrap(),
                },
            });
        function.parameter_abi = Some(machine_code::ParameterFunctionAbiRecord {
            call_plan: plan,
            parameters: Vec::new(),
            entry_register_spills: Vec::new(),
        });
        assert!(super::function_is_exact(&function, target));
        for mutation in 0..4 {
            let mut changed = function.clone();
            match mutation {
                0 => changed.unit_parameter_homes.clear(),
                1 => changed.unit_parameter_homes[0].place = changed.unit_parameters[0].place,
                2 => changed.unit_parameter_homes[0].multiplicity = StructuralMultiplicity::Linear,
                _ => changed
                    .parameter_abi
                    .as_mut()
                    .unwrap()
                    .call_plan
                    .parameters
                    .swap(0, 1),
            }
            assert!(!super::function_is_exact(&changed, target));
        }
    }
}

#[test]
fn indirect_owned_roster_retains_exact_value_pointer_home() {
    use calling_conventions::{IndirectPointerLocation, ValueLocation};
    use machine_code::StructuralSourceLocation;
    for target in [
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        for scalar_count in [0, 8] {
            let mut function = installed_function_with_unit_call();
            let owned_shape = ValueShape::integer(24, 8);
            let mut shapes = vec![ValueShape::integer(8, 8); scalar_count];
            shapes.push(owned_shape);
            let plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: shapes,
                    result: None,
                },
            )
            .unwrap();
            let placement = &plan.parameters[scalar_count];
            let [ValueLocation::Indirect { pointer, .. }] = placement.locations.as_slice() else {
                panic!("indirect value ABI");
            };
            let location = match pointer {
                IndirectPointerLocation::Register(register) => {
                    StructuralSourceLocation::IncomingIndirectPointer {
                        register: *register,
                    }
                }
                IndirectPointerLocation::Stack {
                    stack_byte_offset,
                    alignment,
                } => StructuralSourceLocation::IncomingIndirectStackPointer {
                    stack_byte_offset: *stack_byte_offset,
                    alignment: *alignment,
                },
            };
            let parameter = machine_code::UnitParameterRecord {
                place: PlaceId::new(1).unwrap(),
                structural_type: StructuralTypeId::new(1).unwrap(),
                multiplicity: StructuralMultiplicity::Affine,
                access: terminal_psi::StructuralAccess::Owned,
                shape: owned_shape,
            };
            function.unit_parameters = vec![parameter];
            function.unit_parameter_homes = vec![machine_code::UnitParameterHomeRecord {
                place: parameter.place,
                structural_type: parameter.structural_type,
                multiplicity: parameter.multiplicity,
                access: parameter.access,
                shape: owned_shape,
                source: placement.clone(),
                location,
                indirect: true,
            }];
            function.parameter_abi = Some(machine_code::ParameterFunctionAbiRecord {
                parameters: (0..scalar_count)
                    .map(|position| target_operations::ScalarAbiValue {
                        value: ValueId::new(position as u64 + 1).unwrap(),
                        scalar_type: semantic_vocabulary::ScalarType::Integer(
                            semantic_vocabulary::IntegerType::new(
                                semantic_vocabulary::IntegerSign::Unsigned,
                                64,
                            )
                            .unwrap(),
                        ),
                        placement: plan.parameters[position].clone(),
                    })
                    .collect(),
                call_plan: plan,
                entry_register_spills: Vec::new(),
            });
            assert!(super::function_is_exact(&function, target));
            for mutation in 0..9 {
                let mut changed = function.clone();
                match mutation {
                    0 => changed.unit_parameter_homes.clear(),
                    1 => changed
                        .unit_parameter_homes
                        .push(changed.unit_parameter_homes[0].clone()),
                    2 => changed.unit_parameter_homes[0].shape.byte_size = 16,
                    3 => {
                        changed.unit_parameter_homes[0].structural_type =
                            StructuralTypeId::new(2).unwrap()
                    }
                    4 => changed.unit_parameter_homes[0].indirect = false,
                    5 => {
                        changed.unit_parameter_homes[0].location =
                            StructuralSourceLocation::Stack { byte_offset: 0 }
                    }
                    6 => {
                        changed.unit_parameter_homes[0].access =
                            terminal_psi::StructuralAccess::SharedBorrow
                    }
                    7 => {
                        changed.unit_parameter_homes[0].location =
                            StructuralSourceLocation::IncomingBorrowedPointer {
                                location: IndirectPointerLocation::Register(
                                    MachineRegister::Aarch64X(0),
                                ),
                            }
                    }
                    _ => {
                        let home = &mut changed.unit_parameter_homes[0];
                        let ValueLocation::Indirect {
                            copy_stack_byte_offset,
                            ..
                        } = &mut home.source.locations[0]
                        else {
                            panic!("owned indirect source");
                        };
                        *copy_stack_byte_offset = None;
                        changed.parameter_abi.as_mut().unwrap().call_plan.parameters
                            [scalar_count] = home.source.clone();
                    }
                }
                assert!(
                    !super::function_is_exact(&changed, target),
                    "{target:?} {scalar_count} mutation {mutation}"
                );
            }
        }
    }
}

#[test]
fn direct_owned_roster_preserves_ieee_format_width_and_register_bank() {
    use semantic_vocabulary::{IeeeFloatFormat, ScalarType};
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for (format, bytes) in [
            (IeeeFloatFormat::Binary32, 4),
            (IeeeFloatFormat::Binary64, 8),
        ] {
            let scalar_shape = ValueShape::float(bytes);
            let owned_shape = ValueShape::integer(16, 8);
            let plan = evaluate_call_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature {
                    parameters: vec![scalar_shape, scalar_shape, owned_shape],
                    result: Some(owned_shape),
                },
            )
            .unwrap();
            let mut function = installed_function_with_unit_call();
            function.unit_parameters = vec![machine_code::UnitParameterRecord {
                place: PlaceId::new(1).unwrap(),
                structural_type: StructuralTypeId::new(1).unwrap(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: terminal_psi::StructuralAccess::Owned,
                shape: owned_shape,
            }];
            function.parameter_abi = Some(machine_code::ParameterFunctionAbiRecord {
                parameters: (0..2)
                    .map(|position| target_operations::ScalarAbiValue {
                        value: ValueId::new(position as u64 + 1).unwrap(),
                        scalar_type: ScalarType::IeeeFloat(format),
                        placement: plan.parameters[position].clone(),
                    })
                    .collect(),
                call_plan: plan,
                entry_register_spills: Vec::new(),
            });
            assert!(super::function_is_exact(&function, target));
            for mutation in 0..4 {
                let mut changed = function.clone();
                let abi = changed.parameter_abi.as_mut().unwrap();
                match mutation {
                    0 => {
                        abi.parameters[0].scalar_type =
                            ScalarType::IeeeFloat(if format == IeeeFloatFormat::Binary32 {
                                IeeeFloatFormat::Binary64
                            } else {
                                IeeeFloatFormat::Binary32
                            })
                    }
                    1 => abi.parameters[0].placement.shape.byte_size = 1,
                    2 => {
                        abi.parameters[0].placement.locations =
                            abi.call_plan.parameters[2].locations.clone()
                    }
                    _ => abi.parameters.swap(0, 1),
                }
                assert!(
                    !super::function_is_exact(&changed, target),
                    "{target:?} mutation {mutation}"
                );
            }
        }
    }
}
