//! Candidate geometry and the mandatory exact object-custody join are separate.
use super::*;
use crate::installation::{
    internal_unit_calls_match_object, resource_tests::installed_function_with_unit_call,
};
use machine_code::{
    InternalUnitCallArgumentRecord, InternalUnitCallRecord, UnitParameterHomeRecord,
    UnitParameterRecord,
};
use semantic_vocabulary::{MachineId, OperationId, PlaceId, StructuralTypeId, ValueId};
use target::NativeTarget;

fn fixture(
    target: NativeTarget,
    scalar_count: usize,
) -> (InstalledFunction, InternalUnitCallRecord) {
    let mut function = installed_function_with_unit_call();
    let shapes = [
        ValueShape::integer(24, 8),
        ValueShape::borrowed_reference(8, 8),
    ];
    let mut parameters = vec![ValueShape::integer(1, 1); scalar_count];
    parameters.extend(shapes);
    let plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters,
            result: Some(ValueShape::integer(1, 1)),
        },
    )
    .unwrap();
    let structural: Vec<_> = shapes
        .iter()
        .enumerate()
        .map(|(position, shape)| UnitParameterRecord {
            place: PlaceId::new(position as u64 + 1).unwrap(),
            structural_type: StructuralTypeId::new(position as u64 + 1).unwrap(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: if position == 0 {
                StructuralAccess::Owned
            } else {
                StructuralAccess::MutableBorrow
            },
            shape: *shape,
        })
        .collect();
    let homes: Vec<_> = structural
        .iter()
        .enumerate()
        .map(|(position, parameter)| {
            let source = plan.parameters[scalar_count + position].clone();
            let location = if position == 0 {
                let [ValueLocation::Indirect { pointer, .. }] = source.locations.as_slice() else {
                    panic!("indirect owned parameter")
                };
                match pointer {
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
                }
            } else {
                StructuralSourceLocation::IncomingBorrowedPointer {
                    location: pointer_location(&source).unwrap(),
                }
            };
            UnitParameterHomeRecord {
                place: parameter.place,
                structural_type: parameter.structural_type,
                multiplicity: parameter.multiplicity,
                access: parameter.access,
                shape: parameter.shape,
                source,
                indirect: true,
                location,
            }
        })
        .collect();
    function.mixed_structural_scalar_abi =
        Some(target_operations::MixedStructuralScalarFunctionAbi {
            scalar_parameters: (0..scalar_count)
                .map(|position| target_operations::ScalarAbiValue {
                    value: ValueId::new(position as u64 + 1).unwrap(),
                    scalar_type: ScalarType::Boolean,
                    placement: plan.parameters[position].clone(),
                })
                .collect(),
            structural_parameters: structural
                .iter()
                .zip(&homes)
                .map(
                    |(parameter, home)| target_operations::TargetStructuralParameter {
                        place: parameter.place,
                        structural_type: parameter.structural_type,
                        multiplicity: parameter.multiplicity,
                        access: parameter.access,
                        projected_qualifications: Vec::new(),
                        shape: parameter.shape,
                        placement: home.source.clone(),
                    },
                )
                .collect(),
            result: target_operations::ScalarAbiValue {
                value: ValueId::new(20).unwrap(),
                scalar_type: ScalarType::Boolean,
                placement: plan.result.clone().unwrap(),
            },
            call_plan: plan,
        });
    function.scalar_structural_parameters = structural;
    function.scalar_structural_parameter_homes = homes;
    let instruction_bytes = if target.architecture == target::Architecture::X86_64 {
        5
    } else {
        4
    };
    let arguments = parameter_homes(&function)
        .iter()
        .enumerate()
        .map(|(position, home)| {
            let (code_offset, byte_count) = if position == 0 {
                (16, 24)
            } else {
                (64, instruction_bytes)
            };
            InternalUnitCallArgumentRecord {
                place: home.place,
                access: home.access,
                path: Vec::new(),
                root_structural_type: home.structural_type,
                structural_type: home.structural_type,
                shape: home.shape,
                source_byte_offset: 0,
                source_location: home.location,
                call_stack_bytes: 256,
                fixed_array_length: None,
                element_stride: None,
                source: InternalUnitStructuralArgumentSourceRecord::Placement(home.source.clone()),
                destination: home.source.clone(),
                code_offset,
                byte_count,
                bytes: vec![position as u8 + 1; byte_count],
            }
        })
        .collect();
    let call = InternalUnitCallRecord {
        source: InternalUnitCallSource::Authored,
        owner: target_operations::CallSiteOwner::Operation(OperationId::new(1).unwrap()),
        target: MachineId::new(2).unwrap(),
        result: Some(ScalarType::Boolean),
        semantic_result: Some(abstract_operations::AbstractResult {
            value: ValueId::new(30).unwrap(),
            scalar_type: ScalarType::Boolean,
        }),
        structural_result: None,
        scalar_arguments: Vec::new(),
        arguments,
        claim_transfers: Vec::new(),
        operation_ordinal: 0,
        code_offset: 64,
        byte_count: instruction_bytes,
    };
    (function, call)
}

#[test]
fn mixed_owned_copy_retains_canonical_pointer_homes_and_separate_spans() {
    for target in [
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        for scalar_count in [0, 8] {
            let (function, call) = fixture(target, scalar_count);
            let homes = parameter_homes(&function);
            assert!(super::super::graph_structural::function_is_exact(
                &function, target
            ));
            assert!(scalar_result_is_exact(&call, &function));
            assert!(owned_copy_is_exact(
                &call.arguments[0],
                homes,
                &homes[0],
                call.code_offset,
                256
            ));
            assert!(outgoing_pointer_fits(&call.arguments[1].destination, 256));
            assert_ne!(call.arguments[0].code_offset, call.arguments[1].code_offset);
            assert_ne!(call.arguments[0].bytes, call.arguments[1].bytes);
            if scalar_count == 8 {
                assert!(matches!(
                    homes[0].location,
                    StructuralSourceLocation::IncomingIndirectStackPointer { .. }
                ));
            } else {
                assert!(matches!(
                    homes[0].location,
                    StructuralSourceLocation::IncomingIndirectPointer { .. }
                ));
            }
            for mutation in 0..15 {
                let mut changed = call.arguments[0].clone();
                match mutation {
                    0 => changed.source_byte_offset = 8,
                    1 => changed.source_location = homes[1].location,
                    2 => changed.source = call.arguments[1].source.clone(),
                    3 => changed.destination = call.arguments[1].destination.clone(),
                    4 => changed.code_offset = call.code_offset,
                    5 => changed.code_offset = usize::MAX,
                    6 => {
                        changed.byte_count = 0;
                        changed.bytes.clear();
                    }
                    7 => {
                        changed.bytes.pop();
                    }
                    8 => changed.path.push(StructuralPathSegment::FixedIndex(0)),
                    9 => changed.structural_type = homes[1].structural_type,
                    10 => changed.root_structural_type = homes[1].structural_type,
                    11 => changed.shape.byte_size = 16,
                    12 => changed.access = StructuralAccess::SharedBorrow,
                    13 => changed.call_stack_bytes = 255,
                    _ => changed.fixed_array_length = Some(24),
                }
                assert!(
                    !owned_copy_is_exact(&changed, homes, &homes[0], call.code_offset, 256),
                    "{target:?} {scalar_count} mutation {mutation}"
                );
            }
            assert!(!owned_copy_is_exact(
                &call.arguments[0],
                homes,
                &homes[0],
                call.code_offset,
                0
            ));
            // Coordinated changes to retained homes and ABI placements still
            // must reconstruct the canonical native plan, not merely agree.
            let mut changed = function.clone();
            let home = &mut changed.scalar_structural_parameter_homes[0];
            let ValueLocation::Indirect {
                copy_stack_byte_offset,
                ..
            } = &mut home.source.locations[0]
            else {
                panic!("copy")
            };
            *copy_stack_byte_offset = Some(200);
            let abi = changed.mixed_structural_scalar_abi.as_mut().unwrap();
            abi.call_plan.parameters[scalar_count] = home.source.clone();
            abi.structural_parameters[0].placement = home.source.clone();
            assert!(!super::super::graph_structural::function_is_exact(
                &changed, target
            ));
        }
    }
}

#[test]
fn owned_copy_image_join_rejects_even_shape_valid_span_and_byte_substitutions() {
    let (function, call) = fixture(NativeTarget::macos_arm64(), 8);
    let installed = InstalledInternalUnitCall {
        machine: function.machine,
        text_offset: function.text_offset + call.code_offset,
        custody: call.clone(),
    };
    let emitted = [(function.machine, function.text_offset, &call)];
    assert!(internal_unit_calls_match_object(
        std::slice::from_ref(&installed),
        emitted
    ));
    for mutation in 0..8 {
        let mut changed = installed.clone();
        match mutation {
            0 => changed.custody.arguments[0].code_offset += 1,
            1 => changed.custody.arguments[0].bytes[0] ^= 1,
            2 => changed.custody.arguments[0].source_location = call.arguments[1].source_location,
            3 => changed.custody.arguments[0].source = call.arguments[1].source.clone(),
            4 => changed.custody.arguments[0].destination = call.arguments[1].destination.clone(),
            5 => {
                changed.custody.arguments[0].byte_count -= 1;
                changed.custody.arguments[0].bytes.pop();
            }
            6 => changed.text_offset += 1,
            _ => changed.machine = call.target,
        }
        if matches!(mutation, 0 | 1 | 5) {
            let homes = parameter_homes(&function);
            assert!(owned_copy_is_exact(
                &changed.custody.arguments[0],
                homes,
                &homes[0],
                call.code_offset,
                256
            ));
        }
        assert!(
            !internal_unit_calls_match_object(std::slice::from_ref(&changed), emitted),
            "mutation {mutation}"
        );
    }
    assert!(!internal_unit_calls_match_object(&[], emitted));
    assert!(!internal_unit_calls_match_object(
        &[installed.clone(), installed.clone()],
        emitted
    ));
    assert!(!internal_unit_calls_match_object(
        std::slice::from_ref(&installed),
        [(function.machine, usize::MAX, &call)]
    ));
}
