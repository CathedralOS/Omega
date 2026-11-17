//! Compare supplied adapter bytes and effects with the exact canonical request.
use diagnostics::Diagnostic;
use native_encoding::native_entry_bridge::{NativeEntryBridge, encode_native_entry_bridge};
use native_encoding::native_foreign_bridge::{NativeForeignBridge, encode_native_foreign_bridge};
use physical_planning::native_entry_bridge::{
    NativeEntryBridgeError as Error, plan_installed_interrupt_entry_bridge,
    plan_native_entry_bridge,
};
use physical_planning::native_foreign_bridge::{
    NativeForeignBridgeError, plan_native_foreign_bridge,
};

pub fn validate_native_entry_bridge(bridge: &NativeEntryBridge) -> Result<(), Error> {
    verify_entry_translation(bridge, bridge.bytes())
}

fn verify_entry_translation(bridge: &NativeEntryBridge, bytes: &[u8]) -> Result<(), Error> {
    bridge.validate_published_code(bytes)?;
    let request = bridge.request().clone();
    let plan = match bridge.installed_interrupt() {
        Some(installed) => plan_installed_interrupt_entry_bridge(request, installed)?,
        None => plan_native_entry_bridge(request)?,
    };
    let canonical = encode_native_entry_bridge(plan)?;
    if canonical != *bridge || canonical.bytes() != bytes {
        return Err(Error::ReplayMismatch);
    }
    Ok(())
}

pub fn validate_native_foreign_bridge(
    bridge: &NativeForeignBridge,
) -> Result<(), NativeForeignBridgeError> {
    let plan = plan_native_foreign_bridge(bridge.request().clone())?;
    if encode_native_foreign_bridge(plan)? != *bridge {
        return Err(NativeForeignBridgeError::ReplayMismatch);
    }
    Ok(())
}

pub fn validate_stack_switch(
    bridge: &NativeEntryBridge,
    switch: &native_encoding::physical_stack_switch::StackSwitch,
) -> Result<(), Diagnostic> {
    let invalid = |reason: &str| Diagnostic::error(format!("physical image entry: {reason}"));
    let plan = plan_native_entry_bridge(bridge.request().clone())
        .map_err(|error| invalid(&error.to_string()))?;
    let switch_plan = physical_planning::physical_stack_switch::plan(plan)?;
    let (canonical, expected_switch) = native_encoding::physical_stack_switch::encode(switch_plan)?;
    if canonical != *bridge {
        return Err(invalid("physical switch replay changed its entry bridge"));
    }
    if expected_switch != *switch {
        return Err(invalid("private stack differs from its exact invocation"));
    }
    Ok(())
}
#[cfg(test)]
mod entry_tests {
    use super::*;
    use arena::Arena;
    use calling_conventions::{
        CallSignature, CallingPolicy, ValueLocation, evaluate_call_plan,
        evaluate_ordinary_boundary_entry_plan,
    };
    use calling_conventions::{
        EntryControl, MachineRegister, MachineState, MachineStateSet, RegisterSet,
        StateEffectsEvidence, StateFootprintEvidence, ValueShape,
    };
    use native_encoding::test_support::emit_native_entry_bridge;
    use physical_planning::native_entry_bridge::{NativeEntryBridgeRequest, NativeEntryParameter};
    use target::{Architecture, NativeTarget};

    fn request(
        target: NativeTarget,
        internal_shapes: Vec<ValueShape>,
        inbound_shapes: Vec<ValueShape>,
        ordinals: &[u32],
        codecs: &[u64],
        result: Option<ValueShape>,
    ) -> NativeEntryBridgeRequest {
        let policy = CallingPolicy::native_for_target(target);
        let internal = evaluate_call_plan(
            policy,
            &CallSignature {
                parameters: internal_shapes,
                result,
            },
        )
        .unwrap();
        let inbound = evaluate_ordinary_boundary_entry_plan(
            policy,
            &CallSignature {
                parameters: inbound_shapes,
                result,
            },
        )
        .unwrap();
        let mut parameters = Arena::with_capacity(ordinals.len());
        for (ordinal, codec) in ordinals.iter().zip(codecs) {
            parameters.insert(NativeEntryParameter {
                inbound_ordinal: *ordinal,
                scalar_codec: *codec,
            });
        }
        NativeEntryBridgeRequest {
            target,
            internal,
            inbound: inbound.plan().clone(),
            body_footprint: Some(full_convention_body()),
            parameters,
        }
    }

    // A body that overwrites both register classes, as the internal convention allows.
    fn full_convention_body() -> StateEffectsEvidence {
        let classes = MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::VectorRegisters,
        ]);
        StateEffectsEvidence::new(
            RegisterSet::default(),
            RegisterSet::default(),
            classes,
            classes,
        )
    }

    fn interrupt_request(shapes: Vec<ValueShape>) -> NativeEntryBridgeRequest {
        use calling_conventions::{
            EntryStack, MachineRegime, MachineState, MachineStateSet, Preemption, StatePlan,
        };
        let count = shapes.len();
        let mut request = request(
            NativeTarget::linux_x64(),
            shapes.clone(),
            shapes,
            &(0..count as u32).collect::<Vec<_>>(),
            &vec![0; count],
            None,
        );
        request.inbound.call.entry_control = EntryControl::InterruptReturn;
        let preserved = MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
        ]);
        request.inbound.state = StatePlan {
            initial_regime: MachineRegime::X86Long64,
            interrupted_state: preserved
                .union(MachineStateSet::new([MachineState::VectorRegisters])),
            saved_state: preserved,
            restored_state: preserved,
            permitted_transitive_use: preserved
                .union(MachineStateSet::new([MachineState::SegmentState])),
            stack: EntryStack::Dedicated { class: 4 },
            preemption: Preemption::Masked,
        };
        request.body_footprint = Some(
            StateFootprintEvidence::new(
                RegisterSet::new([
                    MachineRegister::X86Rax,
                    MachineRegister::X86Rdx,
                    MachineRegister::X86Rsp,
                ]),
                MachineStateSet::new([
                    MachineState::Flags,
                    MachineState::InstructionPointer,
                    MachineState::StackPointer,
                ]),
            )
            .into(),
        );
        request
    }

    #[test]
    fn interrupt_entry_preserves_registers_and_returns_through_cpu_frame() {
        use iced_x86::{Decoder, DecoderOptions, Mnemonic, Register};
        let bridge = emit_native_entry_bridge(interrupt_request(vec![])).unwrap();
        let instructions = Decoder::new(64, bridge.bytes(), DecoderOptions::NONE)
            .into_iter()
            .collect::<Vec<_>>();
        assert!(
            instructions
                .iter()
                .all(|instruction| !instruction.is_invalid())
        );
        assert_eq!(instructions.last().unwrap().mnemonic(), Mnemonic::Iretq);
        let call = instructions
            .iter()
            .position(|instruction| instruction.mnemonic() == Mnemonic::Call)
            .unwrap();
        assert!(
            instructions[..call]
                .iter()
                .any(|instruction| instruction.mnemonic() == Mnemonic::Cld)
        );
        let stores = instructions[..call]
            .iter()
            .filter(|instruction| {
                instruction.mnemonic() == Mnemonic::Mov
                    && instruction.memory_base() == Register::RSP
                    && instruction.op1_register() != Register::None
            })
            .count();
        let loads = instructions[call + 1..]
            .iter()
            .filter(|instruction| {
                instruction.mnemonic() == Mnemonic::Mov
                    && instruction.memory_base() == Register::RSP
                    && instruction.op0_register() != Register::None
            })
            .count();
        assert_eq!((stores, loads), (15, 15));
        assert_eq!(bridge.frame_bytes() % 16, 8);
        validate_native_entry_bridge(&bridge).unwrap();

        if let Some(path) = std::env::var_os("OMEGA_INTERRUPT_BRIDGE_PROBE") {
            let mut payload = bridge.bytes().to_vec();
            let body_offset = payload.len();
            payload.extend_from_slice(&[0xb0, b'T', 0xe6, 0xe9, 0xc3]);
            let field = bridge.call_field_offset();
            let displacement =
                i32::try_from(body_offset).unwrap() - i32::try_from(field + 4).unwrap();
            payload[field..field + 4].copy_from_slice(&displacement.to_le_bytes());
            std::fs::write(path, payload).unwrap();
        }
    }

    #[test]
    fn interrupt_stack_arguments_follow_the_complete_return_frame() {
        use calling_conventions::ValueLocation;
        use iced_x86::{Decoder, DecoderOptions, Register};
        let mut request = interrupt_request(vec![ValueShape::integer(8, 8)]);
        request.inbound.call.parameters[0].locations = vec![ValueLocation::Stack {
            stack_byte_offset: 0,
            value_byte_offset: 0,
            byte_size: 8,
            alignment: 8,
        }];
        let bridge = emit_native_entry_bridge(request).unwrap();
        let instructions = Decoder::new(64, bridge.bytes(), DecoderOptions::NONE)
            .into_iter()
            .collect::<Vec<_>>();
        assert!(
            instructions
                .iter()
                .any(|instruction| instruction.memory_base() == Register::RSP
                    && instruction.memory_displacement64() == u64::from(bridge.frame_bytes()) + 40)
        );
        assert!(!instructions.iter().any(|instruction| {
            instruction.memory_base() == Register::RSP
                && (u64::from(bridge.frame_bytes())..u64::from(bridge.frame_bytes()) + 40)
                    .contains(&instruction.memory_displacement64())
        }));
    }

    #[test]
    fn interrupt_entry_refuses_vector_argument_reads() {
        use calling_conventions::ValueLocation;
        let mut request = interrupt_request(vec![ValueShape::integer(8, 8)]);
        request.inbound.call.parameters[0].locations = vec![ValueLocation::Register {
            register: MachineRegister::X86Xmm(0),
            value_byte_offset: 0,
            byte_size: 8,
        }];
        assert_eq!(
            emit_native_entry_bridge(request),
            Err(Error::UnsupportedInterruptState)
        );
    }

    #[test]
    fn interrupt_entry_refuses_unpreserved_or_unaccounted_body_state() {
        use calling_conventions::{MachineState, MachineStateSet};
        let mut missing = interrupt_request(vec![]);
        missing.body_footprint = None;
        assert_eq!(
            emit_native_entry_bridge(missing),
            Err(Error::MissingInterruptBodyFootprint)
        );
        let mut vector = interrupt_request(vec![]);
        vector.body_footprint = Some(
            StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Xmm(0)]),
                MachineStateSet::empty(),
            )
            .into(),
        );
        assert_eq!(
            emit_native_entry_bridge(vector),
            Err(Error::InterruptBodyStateExceedsPlan)
        );
        let mut unsaved = interrupt_request(vec![]);
        unsaved.inbound.state.saved_state = MachineStateSet::new([MachineState::Flags]);
        assert_eq!(
            emit_native_entry_bridge(unsaved),
            Err(Error::UnsupportedInterruptState)
        );
    }

    fn narrow_inbound_clobbers(
        request: &mut NativeEntryBridgeRequest,
        keep: impl Fn(&MachineRegister) -> bool,
    ) {
        request.inbound.call.ordinary_clobbers = RegisterSet::new(
            request
                .internal
                .ordinary_clobbers
                .as_slice()
                .iter()
                .copied()
                .filter(|register| keep(register)),
        );
    }

    #[test]
    fn narrower_inbound_clobbers_save_integer_and_full_vector_state() {
        let mut request = request(NativeTarget::macos_arm64(), vec![], vec![], &[], &[], None);
        narrow_inbound_clobbers(&mut request, |register| {
            !matches!(
                register,
                MachineRegister::Aarch64X(9) | MachineRegister::Aarch64V(16)
            )
        });
        let bridge = emit_native_entry_bridge(request).unwrap();
        assert_eq!(bridge.frame_bytes(), 32);
        let before_call = &bridge.bytes()[..bridge.call_field_offset()];
        let after_call = &bridge.bytes()[bridge.call_field_offset() + 4..];
        for (save, restore) in [
            (0xf900_07e9_u32, 0xf940_07e9_u32),
            (0x3d80_07f0, 0x3dc0_07f0),
        ] {
            assert!(
                before_call
                    .windows(4)
                    .any(|bytes| bytes == save.to_le_bytes())
            );
            assert!(
                after_call
                    .windows(4)
                    .any(|bytes| bytes == restore.to_le_bytes())
            );
        }
        validate_native_entry_bridge(&bridge).unwrap();
    }

    #[test]
    fn body_that_overwrites_no_vector_register_gets_no_vector_saves() {
        let mut request = request(NativeTarget::macos_arm64(), vec![], vec![], &[], &[], None);
        narrow_inbound_clobbers(&mut request, |register| {
            !matches!(
                register,
                MachineRegister::Aarch64X(9) | MachineRegister::Aarch64V(16)
            )
        });
        let general = MachineStateSet::new([MachineState::GeneralRegisters]);
        request.body_footprint = Some(StateEffectsEvidence::new(
            RegisterSet::default(),
            RegisterSet::default(),
            general,
            general,
        ));
        let bridge = emit_native_entry_bridge(request).unwrap();
        let before_call = &bridge.bytes()[..bridge.call_field_offset()];
        assert!(
            before_call
                .windows(4)
                .any(|bytes| bytes == 0xf900_07e9_u32.to_le_bytes())
        );
        assert!(
            !bridge
                .complete_effects()
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::VectorRegisters]))
        );
        validate_native_entry_bridge(&bridge).unwrap();
    }

    #[test]
    fn returned_register_is_not_restored_over_the_result() {
        let mut request = request(
            NativeTarget::macos_arm64(),
            vec![],
            vec![],
            &[],
            &[],
            Some(ValueShape::integer(8, 8)),
        );
        narrow_inbound_clobbers(&mut request, |r| *r != MachineRegister::Aarch64X(0));
        assert_eq!(emit_native_entry_bridge(request).unwrap().frame_bytes(), 16);
    }

    #[test]
    fn hidden_result_pointer_remains_caller_state_not_a_direct_output() {
        let mut request = request(
            NativeTarget::macos_arm64(),
            vec![],
            vec![],
            &[],
            &[],
            Some(ValueShape::integer(24, 8)),
        );
        narrow_inbound_clobbers(&mut request, |r| *r != MachineRegister::Aarch64X(8));
        let bridge = emit_native_entry_bridge(request).unwrap();
        assert_eq!(bridge.frame_bytes(), 48);
        for word in [0xf900_17e8_u32, 0xf940_17e8] {
            assert!(
                bridge
                    .bytes()
                    .windows(4)
                    .any(|bytes| bytes == word.to_le_bytes())
            );
        }
    }

    #[test]
    fn microsoft_entry_preserves_sysv_additional_clobbers() {
        let target = NativeTarget::windows_x64();
        let mut request = request(target, vec![], vec![], &[], &[], None);
        request.internal =
            evaluate_call_plan(CallingPolicy::SystemVAMD64, &CallSignature::default()).unwrap();
        let bridge = emit_native_entry_bridge(request).unwrap();
        assert_eq!(bridge.frame_bytes(), 184);
        let before_call = &bridge.bytes()[..bridge.call_field_offset() - 1];
        let after_call = &bridge.bytes()[bridge.call_field_offset() + 4..];
        for (register_bits, offset) in [(0x34, 0), (0x7c, 8)] {
            let save = if offset == 0 {
                vec![0x48, 0x89, register_bits, 0x24]
            } else {
                vec![0x48, 0x89, register_bits, 0x24, offset]
            };
            let mut restore = save.clone();
            restore[1] = 0x8b;
            assert!(before_call.windows(save.len()).any(|bytes| bytes == save));
            assert!(
                after_call
                    .windows(restore.len())
                    .any(|bytes| bytes == restore)
            );
        }
        assert!(
            before_call
                .windows(8)
                .any(|bytes| bytes == [0x0f, 0x11, 0xb4, 0x24, 16, 0, 0, 0])
        );
        assert!(
            after_call
                .windows(9)
                .any(|bytes| bytes == [0x44, 0x0f, 0x10, 0xbc, 0x24, 160, 0, 0, 0])
        );
        validate_native_entry_bridge(&bridge).unwrap();
    }

    #[test]
    fn fully_preserving_inbound_promise_saves_borrowed_scratch() {
        let target = NativeTarget::linux_x64();
        let mut request = request(target, vec![], vec![], &[], &[], None);
        request.inbound.call.ordinary_clobbers = RegisterSet::default();
        request.internal.ordinary_clobbers = RegisterSet::default();
        let bridge = emit_native_entry_bridge(request).unwrap();
        let before_call = &bridge.bytes()[..bridge.call_field_offset() - 1];
        let after_call = &bridge.bytes()[bridge.call_field_offset() + 4..];
        let moves_r11 = |bytes: &[u8], opcode: u8| {
            bytes.windows(4).any(|window| {
                window[0] == 0x4c
                    && window[1] == opcode
                    && window[2] & 0x38 == 0x18
                    && window[2] & 0x07 == 0x04
                    && window[3] == 0x24
            })
        };
        assert!(moves_r11(before_call, 0x89));
        assert!(moves_r11(after_call, 0x8b));
        validate_native_entry_bridge(&bridge).unwrap();
    }

    #[test]
    fn different_conventions_transport_arguments_results_and_shadow_space() {
        let shapes = [
            vec![ValueShape::integer(8, 8); 7],
            vec![ValueShape::float(8); 7],
        ]
        .concat();
        for (internal_policy, inbound_policy) in [
            (CallingPolicy::SystemVAMD64, CallingPolicy::MicrosoftX64),
            (CallingPolicy::MicrosoftX64, CallingPolicy::SystemVAMD64),
        ] {
            let mut request = request(
                NativeTarget::windows_x64(),
                shapes.clone(),
                shapes.clone(),
                &(0..14).collect::<Vec<_>>(),
                &[0; 14],
                Some(ValueShape::float(8)),
            );
            let signature = CallSignature {
                parameters: shapes.clone(),
                result: Some(ValueShape::float(8)),
            };
            request.internal = evaluate_call_plan(internal_policy, &signature).unwrap();
            request.inbound = evaluate_ordinary_boundary_entry_plan(inbound_policy, &signature)
                .unwrap()
                .plan()
                .clone();
            validate_native_entry_bridge(&emit_native_entry_bridge(request).unwrap()).unwrap();
        }
    }

    #[test]
    fn word_permutations_cross_register_stack_and_shadow_areas() {
        for target in [
            NativeTarget::macos_arm64(),
            NativeTarget::linux_arm64(),
            NativeTarget::linux_x64(),
            NativeTarget::macos_x64(),
            NativeTarget::windows_x64(),
        ] {
            let bridge = emit_native_entry_bridge(request(
                target,
                vec![ValueShape::integer(8, 8); 9],
                vec![ValueShape::integer(8, 8); 9],
                &[8, 7, 6, 5, 4, 3, 2, 1, 0],
                &[0; 9],
                None,
            ))
            .unwrap();
            validate_native_entry_bridge(&bridge).unwrap();
            let bias = bridge.call_stack_bias();
            assert_eq!(
                bias,
                if target.architecture == Architecture::X86_64 {
                    8
                } else {
                    0
                }
            );
            assert_eq!((bridge.frame_bytes() + bias) % 16, 0);
            let offset = bridge.call_field_offset();
            match target.architecture {
                Architecture::Aarch64 => assert_eq!(
                    &bridge.bytes()[offset..offset + 4],
                    &isa_aarch64::ORDINARY_DIRECT_CALL_BYTES
                ),
                Architecture::X86_64 => assert_eq!(
                    &bridge.bytes()[offset - 1..offset + 4],
                    &isa_x86_64::ORDINARY_DIRECT_CALL_BYTES
                ),
            }
        }
    }

    #[test]
    fn darwin_record_stack_slots_bridge_to_internal_scalar_geometry() {
        let mut internal = vec![ValueShape::integer(8, 8); 8];
        internal.extend([
            ValueShape::integer(1, 1),
            ValueShape::integer(4, 2),
            ValueShape::integer(1, 1),
        ]);
        let mut inbound = internal.clone();
        inbound[9] = ValueShape::integer_aggregate(4, 2);
        let request = request(
            NativeTarget::macos_arm64(),
            internal,
            inbound,
            &(0..11).collect::<Vec<_>>(),
            &[0; 11],
            None,
        );
        assert!(matches!(
            request.inbound.call.parameters[9].locations.as_slice(),
            [ValueLocation::Stack {
                stack_byte_offset: 8,
                ..
            }]
        ));
        assert!(matches!(
            request.internal.parameters[9].locations.as_slice(),
            [ValueLocation::Stack {
                stack_byte_offset: 2,
                ..
            }]
        ));
        let bridge = emit_native_entry_bridge(request).unwrap();
        validate_native_entry_bridge(&bridge).unwrap();
    }

    #[test]
    fn descriptor_entry_retains_codec_and_parameter_permutation() {
        for target in [
            NativeTarget::macos_arm64(),
            NativeTarget::linux_x64(),
            NativeTarget::windows_x64(),
        ] {
            let bridge = emit_native_entry_bridge(request(
                target,
                vec![ValueShape::borrowed_scalar_reference(); 3],
                vec![ValueShape::integer(8, 8); 3],
                &[1, 0, 2],
                &[8, 258, 516],
                Some(ValueShape::integer(4, 4)),
            ))
            .unwrap();
            assert!(bridge.frame_bytes() >= 3 * 24);
            assert_eq!(
                bridge
                    .request()
                    .parameters
                    .iter()
                    .map(|(_, parameter)| parameter.scalar_codec)
                    .collect::<Vec<_>>(),
                vec![8, 258, 516]
            );
            validate_native_entry_bridge(&bridge).unwrap();
        }
    }

    #[test]
    fn scalar_pointer_without_codec_does_not_become_a_descriptor() {
        let invalid = request(
            NativeTarget::macos_arm64(),
            vec![ValueShape::borrowed_scalar_reference()],
            vec![ValueShape::integer(8, 8)],
            &[0],
            &[0],
            None,
        );
        assert_eq!(
            emit_native_entry_bridge(invalid),
            Err(Error::MissingScalarCodec)
        );
    }

    #[test]
    fn duplicate_parameter_origin_is_not_a_parallel_move() {
        let invalid = request(
            NativeTarget::linux_x64(),
            vec![ValueShape::integer(8, 8); 2],
            vec![ValueShape::integer(8, 8); 2],
            &[0, 0],
            &[0; 2],
            None,
        );
        assert_eq!(
            emit_native_entry_bridge(invalid),
            Err(Error::InvalidParameterJoin)
        );
    }

    #[test]
    fn float_register_permutation_and_result_use_scalar_transfers() {
        for target in [
            NativeTarget::macos_arm64(),
            NativeTarget::linux_x64(),
            NativeTarget::windows_x64(),
        ] {
            let bridge = emit_native_entry_bridge(request(
                target,
                vec![ValueShape::float(4); 2],
                vec![ValueShape::float(4); 2],
                &[1, 0],
                &[0; 2],
                Some(ValueShape::float(4)),
            ))
            .unwrap();
            validate_native_entry_bridge(&bridge).unwrap();
        }
    }

    #[test]
    fn large_frame_uses_target_commit_probes_and_checked_addressing() {
        let mut request = request(
            NativeTarget::macos_arm64(),
            vec![ValueShape::integer(8, 8)],
            vec![ValueShape::integer(8, 8)],
            &[0],
            &[0],
            None,
        );
        request.internal.parameters[0].locations = vec![ValueLocation::Stack {
            stack_byte_offset: 65_536,
            value_byte_offset: 0,
            byte_size: 8,
            alignment: 8,
        }];
        let bridge = emit_native_entry_bridge(request).unwrap();
        assert!(bridge.frame_bytes() > 65_536);
        let touches = bridge
            .bytes()
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|word| **word == 0xf940_03ff_u32.to_le_bytes())
            .count();
        assert_eq!(touches, 5);
    }

    #[test]
    fn occupied_ip_registers_do_not_get_used_as_address_scratch() {
        let mut request = request(
            NativeTarget::macos_arm64(),
            vec![ValueShape::integer(8, 8); 2],
            vec![ValueShape::integer(8, 8); 2],
            &[1, 0],
            &[0; 2],
            None,
        );
        request.inbound.call.parameters[0].locations = vec![ValueLocation::Register {
            register: MachineRegister::Aarch64X(16),
            value_byte_offset: 0,
            byte_size: 8,
        }];
        request.inbound.call.parameters[1].locations = vec![ValueLocation::Register {
            register: MachineRegister::Aarch64X(17),
            value_byte_offset: 0,
            byte_size: 8,
        }];
        validate_native_entry_bridge(&emit_native_entry_bridge(request).unwrap()).unwrap();
    }

    #[test]
    fn aggregate_members_cross_integer_float_banks_in_both_directions() {
        for target in [NativeTarget::macos_arm64(), NativeTarget::linux_x64()] {
            for member_size in [4, 8] {
                let aggregate = ValueShape::homogeneous_float_aggregate(member_size, 2);
                let ordinary = ValueShape::integer(aggregate.byte_size, aggregate.alignment);
                let mut request = request(
                    target,
                    vec![ordinary],
                    vec![aggregate],
                    &[0],
                    &[0],
                    Some(ordinary),
                );
                request.inbound.call.result = evaluate_call_plan(
                    request.inbound.call.policy,
                    &CallSignature {
                        parameters: vec![aggregate],
                        result: Some(aggregate),
                    },
                )
                .unwrap()
                .result;
                let bridge = emit_native_entry_bridge(request).unwrap();
                validate_native_entry_bridge(&bridge).unwrap();
                assert!(bridge.frame_bytes() >= aggregate.byte_size * 2);
            }
        }
    }

    #[test]
    fn owned_indirect_arguments_and_results_keep_separate_frame_storage() {
        for target in [
            NativeTarget::macos_arm64(),
            NativeTarget::linux_arm64(),
            NativeTarget::linux_x64(),
            NativeTarget::macos_x64(),
            NativeTarget::windows_x64(),
        ] {
            let shape = ValueShape::integer(24, 8);
            let bridge = emit_native_entry_bridge(request(
                target,
                vec![shape; 9],
                vec![shape; 9],
                &[8, 7, 6, 5, 4, 3, 2, 1, 0],
                &[0; 9],
                Some(shape),
            ))
            .unwrap();
            validate_native_entry_bridge(&bridge).unwrap();
            assert!(bridge.frame_bytes() >= 10 * shape.byte_size);
        }
    }

    #[test]
    fn direct_float_entry_can_call_indirect_body_and_return_directly() {
        for target in [NativeTarget::macos_arm64(), NativeTarget::linux_arm64()] {
            let aggregate = ValueShape::homogeneous_float_aggregate(8, 4);
            let ordinary = ValueShape::integer(32, 8);
            let mut request = request(
                target,
                vec![ordinary],
                vec![aggregate],
                &[0],
                &[0],
                Some(ordinary),
            );
            request.inbound.call.result = evaluate_call_plan(
                request.inbound.call.policy,
                &CallSignature {
                    parameters: vec![aggregate],
                    result: Some(aggregate),
                },
            )
            .unwrap()
            .result;
            validate_native_entry_bridge(&emit_native_entry_bridge(request).unwrap()).unwrap();
        }
    }

    #[test]
    fn x64_owned_homes_align_inside_fixed_bridge_frames() {
        for target in [NativeTarget::linux_x64(), NativeTarget::windows_x64()] {
            let bridge = emit_native_entry_bridge(request(
                target,
                vec![ValueShape::integer(32, 32), ValueShape::integer(64, 64)],
                vec![ValueShape::integer(32, 32), ValueShape::integer(64, 64)],
                &[0, 1],
                &[0, 0],
                Some(ValueShape::integer(64, 64)),
            ))
            .unwrap();
            assert_eq!(bridge.frame_bytes() % 16, 8);
            assert!(bridge.frame_bytes() > 128);
            assert!(
                bridge
                    .bytes()
                    .windows(4)
                    .any(|word| word == [0xc0, 0xff, 0xff, 0xff])
            );
            validate_native_entry_bridge(&bridge).unwrap();
            let mut changed = bridge.bytes().to_vec();
            let mask = changed
                .windows(4)
                .position(|word| word == [0xc0, 0xff, 0xff, 0xff])
                .unwrap();
            changed[mask] ^= 1;
            assert!(verify_entry_translation(&bridge, &changed).is_err());
        }
    }
}
