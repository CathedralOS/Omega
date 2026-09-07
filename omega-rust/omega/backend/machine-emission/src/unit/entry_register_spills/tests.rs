use super::*;
use semantic_vocabulary::{BlockId, EdgeId, IntegerSign, IntegerType, ScalarType, ValueId};

fn fixture(target: NativeTarget) -> AssignedUnitBody {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let call_plan = calling_conventions::evaluate_call_plan(
        calling_conventions::CallingPolicy::native_for_target(target),
        &calling_conventions::CallSignature {
            parameters: vec![ValueShape::integer(8, 8); 10],
            result: None,
        },
    )
    .unwrap();
    let scalar_parameters = call_plan
        .parameters
        .iter()
        .enumerate()
        .map(|(index, placement)| target_operations::ScalarAbiValue {
            value: ValueId::new(u64::try_from(index).unwrap() + 1).unwrap(),
            scalar_type,
            placement: placement.clone(),
        })
        .collect::<Vec<_>>();
    let entry_register_spills = scalar_parameters
        .iter()
        .enumerate()
        .filter_map(|(parameter_index, parameter)| {
            let [ValueLocation::Register { register, .. }] =
                parameter.placement.locations.as_slice()
            else {
                return None;
            };
            Some((parameter_index, parameter.value, *register))
        })
        .enumerate()
        .map(|(slot, (parameter_index, source_value, register))| {
            assigned_target_operations::EntryRegisterSpill {
                source_value,
                parameter_index,
                register,
                byte_offset: u32::try_from(slot).unwrap() * 8,
            }
        })
        .collect();
    AssignedUnitBody {
        structural_types: Vec::new(),
        call_plan,
        scalar_parameters,
        entry_register_spills,
        parameters: Vec::new(),
        operations: vec![AssignedUnitOperation::Continue {
            psi_edge: EdgeId::new(1).unwrap(),
            source_block: BlockId::new(1).unwrap(),
            target_block: BlockId::new(2).unwrap(),
            bindings: Vec::new(),
            cleanup_actions: Vec::new(),
        }],
    }
}

#[test]
fn entry_spills_replay_original_order_and_exact_store_intervals() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let body = fixture(target);
        let mut cursor = 0;
        validate(&mut cursor, &body, target).unwrap();
        assert_eq!(
            cursor,
            u32::try_from(body.entry_register_spills.len()).unwrap() * 8
        );
        assert!(body.entry_register_spills.len() < body.scalar_parameters.len());
        let mut bytes = vec![0; 8];
        let records = emit(&mut bytes, target, &body.entry_register_spills).unwrap();
        let mut next_code_offset = 8;
        for (record, spill) in records.iter().zip(&body.entry_register_spills) {
            assert_eq!(record.source_value, spill.source_value);
            assert_eq!(record.parameter_index, spill.parameter_index);
            assert_eq!(record.register, spill.register);
            assert_eq!(record.byte_offset, spill.byte_offset);
            assert_eq!(record.code_offset, next_code_offset);
            let mut expected = Vec::new();
            match target.architecture {
                Architecture::X86_64 => emit_x86_64_stack_store_width(
                    &mut expected,
                    x86_unit_register(spill.register).unwrap(),
                    spill.byte_offset,
                    8,
                )
                .unwrap(),
                Architecture::Aarch64 => expected.extend_from_slice(
                    &aarch64_unit_stack_access(
                        0xf900_0000,
                        aarch64_unit_register(spill.register).unwrap(),
                        spill.byte_offset,
                        8,
                    )
                    .unwrap()
                    .to_le_bytes(),
                ),
            }
            assert_eq!(record.byte_count, expected.len());
            assert_eq!(
                bytes[record.code_offset..record.code_offset + record.byte_count],
                expected
            );
            next_code_offset += record.byte_count;
        }
        assert_eq!(next_code_offset, bytes.len());
    }
}

#[test]
fn entry_spills_reject_missing_duplicate_wrong_source_slot_and_register() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        for mutation in 0..7 {
            let mut body = fixture(target);
            match mutation {
                0 => {
                    body.entry_register_spills.pop();
                }
                1 => body
                    .entry_register_spills
                    .push(body.entry_register_spills[0].clone()),
                2 => body.entry_register_spills[0].source_value = ValueId::new(99).unwrap(),
                3 => body.entry_register_spills[0].byte_offset += 8,
                4 => {
                    body.entry_register_spills[0].register = body.entry_register_spills[1].register
                }
                5 => body.entry_register_spills.swap(0, 1),
                6 => body.operations.clear(),
                _ => unreachable!(),
            }
            assert!(
                validate(&mut 0, &body, target).is_err(),
                "{target:?} mutation={mutation}"
            );
        }
    }
}

#[test]
fn entry_scalar_arguments_reject_original_clobberable_register_and_wrong_home() {
    let target = NativeTarget::linux_x64();
    let body = fixture(target);
    let spill = &body.entry_register_spills[0];
    let parameter = &body.scalar_parameters[0];
    let mut argument = assigned_target_operations::AssignedUnitScalarCallArgument {
        parameter_index: 0,
        destination: assigned_target_operations::AssignedCallDestination::Register(spill.register),
        source: AssignedUnitScalarArgumentSource::Parameter {
            parameter_index: 0,
            source_value: parameter.value,
            scalar_type: parameter.scalar_type,
            location: AssignedScalarLocation::FrameSpill {
                byte_offset: spill.byte_offset,
            },
        },
    };
    let validate = |argument: &assigned_target_operations::AssignedUnitScalarCallArgument| {
        super::super::scalar_call::validate_unit_scalar_argument(
            OperationId::new(1).unwrap(),
            0,
            argument,
            &body.call_plan,
            &body.scalar_parameters,
            &body.entry_register_spills,
            &[],
        )
    };
    validate(&argument).unwrap();
    let AssignedUnitScalarArgumentSource::Parameter { location, .. } = &mut argument.source else {
        unreachable!();
    };
    *location = AssignedScalarLocation::Register(spill.register);
    assert!(validate(&argument).is_err());
    let AssignedUnitScalarArgumentSource::Parameter { location, .. } = &mut argument.source else {
        unreachable!();
    };
    *location = AssignedScalarLocation::FrameSpill {
        byte_offset: spill.byte_offset + 8,
    };
    assert!(validate(&argument).is_err());
}
