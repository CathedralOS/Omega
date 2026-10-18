use arena::Arena;
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use machine_emission::native_entry_bridge::{
    NativeEntryBridgeRequest, NativeEntryParameter, emit_native_entry_bridge,
};

#[test]
fn private_stack_prefix_copies_abi_stack_arguments_and_preserves_return() {
    use iced_x86::{Decoder, DecoderOptions, Mnemonic, Register};
    for parameter_count in [2, 6] {
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8); parameter_count],
            result: Some(ValueShape::integer(8, 8)),
        };
        let plan = evaluate_call_plan(CallingPolicy::MicrosoftX64, &signature).unwrap();
        let mut parameters = Arena::with_capacity(parameter_count);
        for ordinal in 0..parameter_count {
            parameters.insert(NativeEntryParameter {
                inbound_ordinal: ordinal as u32,
                scalar_codec: 0,
            });
        }
        let bridge = emit_native_entry_bridge(NativeEntryBridgeRequest {
            target: target::NativeTarget::uefi_x64(),
            internal: plan.clone(),
            inbound: plan,
            parameters,
        })
        .unwrap();
        let wrapper = super::stack_switch::emit(&bridge).unwrap();
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
