use arena::Arena;
use calling_conventions::{
    CallSignature, CallingPolicy, ValueShape, evaluate_ordinary_boundary_entry_plan,
};
use machine_emission::native_entry_bridge::{
    NativeEntryBridgeRequest, NativeEntryParameter, emit_native_entry_bridge,
};

#[test]
fn retained_suffix_includes_alignment_after_nonempty_program_bss() {
    for existing in [0, 1, 15, 16, 17, 255, 4097] {
        let partitions = super::PhysicalEntryPartitions::derive(existing, 16, 131_073, 131_072)
            .expect("bounded partition geometry");
        assert_eq!(partitions.retained_start_offset, existing);
        assert!(partitions.retained_start_offset <= partitions.saved_state_offset);
        assert!(partitions.saved_state_offset + 24 <= partitions.stack_offset);
        assert_eq!(partitions.stack_byte_count, 131_088);
        assert_eq!(partitions.end_offset, partitions.stack_offset + 131_088);
        for aligned in [
            partitions.saved_state_offset,
            partitions.stack_offset,
            partitions.end_offset,
        ] {
            assert!(aligned.is_multiple_of(16));
        }
    }
    assert!(super::PhysicalEntryPartitions::derive(u64::MAX, 16, 128, 131_072).is_err());
    assert!(super::PhysicalEntryPartitions::derive(17, 3, 128, 131_072).is_err());
    assert!(super::PhysicalEntryPartitions::derive(17, 16, u64::MAX, 131_072).is_err());
}

#[test]
fn private_stack_realizes_target_floor_and_larger_checked_demand() {
    let target = target::TargetSemantics::close_guaranteed_entry_stack::<target::UefiX86_64>(
        target::TargetProfile::UefiX64,
    )
    .unwrap();
    let floor = target.guaranteed_available_bytes();
    for (demand, expected) in [
        (0, floor),
        (4096, floor),
        (floor, floor),
        (floor + 1, floor + 16),
    ] {
        let partitions = super::PhysicalEntryPartitions::derive(17, 16, demand, floor).unwrap();
        assert_eq!(partitions.stack_byte_count, expected);
        assert_eq!(partitions.end_offset - partitions.stack_offset, expected);
        assert!(partitions.saved_state_offset + 24 <= partitions.stack_offset);
    }
    assert!(super::PhysicalEntryPartitions::derive(17, 16, 0, u64::MAX).is_err());
}

#[test]
fn private_stack_prefix_copies_abi_stack_arguments_and_preserves_return() {
    use iced_x86::{Decoder, DecoderOptions, Mnemonic, Register};
    for parameter_count in [2, 6] {
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8); parameter_count],
            result: Some(ValueShape::integer(8, 8)),
        };
        let plan =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::MicrosoftX64, &signature).unwrap();
        let mut parameters = Arena::with_capacity(parameter_count);
        for ordinal in 0..parameter_count {
            parameters.insert(NativeEntryParameter {
                inbound_ordinal: ordinal as u32,
                scalar_codec: 0,
            });
        }
        let bridge = emit_native_entry_bridge(NativeEntryBridgeRequest {
            target: target::NativeTarget::uefi_x64(),
            internal: plan.plan().call.clone(),
            inbound: plan.plan().clone(),
            body_footprint: None,
            parameters,
        })
        .unwrap();
        let wrapper = super::stack_switch::emit(&bridge).unwrap();
        assert_eq!(wrapper.incoming.bytes_below_entry_rsp, 0);
        assert_eq!(wrapper.incoming.caller_alignment, 16);
        assert_eq!(wrapper.incoming.caller_rsp_bias, 8);
        assert_eq!(
            wrapper.incoming.readable_bytes_from_entry_rsp,
            if parameter_count == 6 { 56 } else { 8 },
        );
        let instructions = Decoder::new(64, &wrapper.bytes, DecoderOptions::NONE)
            .into_iter()
            .take_while(|instruction| instruction.mnemonic() != Mnemonic::Ret)
            .collect::<Vec<_>>();
        assert!(
            instructions
                .iter()
                .all(|instruction| !instruction.is_invalid())
        );
        let call = instructions
            .iter()
            .position(|instruction| instruction.mnemonic() == Mnemonic::Call)
            .unwrap();
        assert_eq!(instructions[call - 2].op0_register(), Register::RAX);
        assert_eq!(instructions[call - 1].op0_register(), Register::R10);
        assert_eq!(instructions[call + 1].mnemonic(), Mnemonic::Mov);
        assert_eq!(instructions[call + 1].op0_register(), Register::RSP);
        assert_eq!(instructions.len(), call + 2);
        let bridge_offset = instructions[call].near_branch_target() as usize;
        assert_eq!(&wrapper.bytes[bridge_offset..], bridge.bytes());
        let copies = instructions
            .iter()
            .filter(|instruction| instruction.memory_base() == Register::R10)
            .map(|instruction| instruction.memory_displacement64())
            .collect::<Vec<_>>();
        assert_eq!(
            copies,
            if parameter_count == 6 {
                vec![40, 48]
            } else {
                vec![]
            }
        );
        assert!(
            copies
                .iter()
                .all(|offset| offset + 8 <= wrapper.incoming.readable_bytes_from_entry_rsp)
        );
        let switch = instructions
            .iter()
            .position(|instruction| {
                instruction.mnemonic() == Mnemonic::Lea
                    && instruction.op0_register() == Register::RSP
            })
            .unwrap();
        assert!(instructions[..switch].iter().all(|instruction| !matches!(
            instruction.mnemonic(),
            Mnemonic::Push | Mnemonic::Call
        ) && instruction.memory_base()
            != Register::RSP));
        assert_eq!(
            wrapper.outgoing_frame_bytes,
            if parameter_count == 6 { 48 } else { 32 }
        );
        assert_eq!(
            wrapper.stack_overhead_bytes(),
            u64::from(wrapper.outgoing_frame_bytes) + 8
        );
    }
}
