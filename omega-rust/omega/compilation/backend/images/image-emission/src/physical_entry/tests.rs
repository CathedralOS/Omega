use super::relocated_bridge;
use arena::Arena;
use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use machine_emission::native_entry_bridge::{
    NativeEntryBridgeRequest, NativeEntryParameter, emit_native_entry_bridge,
};

#[test]
fn relocated_physical_call_preserves_transport_and_status() {
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8), ValueShape::integer(8, 8)],
        result: Some(ValueShape::integer(8, 8)),
    };
    let plan = evaluate_call_plan(CallingPolicy::MicrosoftX64, &signature).unwrap();
    let mut parameters = Arena::with_capacity(2);
    parameters.insert(NativeEntryParameter {
        inbound_ordinal: 1,
        scalar_codec: 0,
    });
    parameters.insert(NativeEntryParameter {
        inbound_ordinal: 0,
        scalar_codec: 0,
    });
    let bridge = emit_native_entry_bridge(NativeEntryBridgeRequest {
        target: target::NativeTarget::uefi_x64(),
        internal: plan.clone(),
        inbound: plan,
        parameters,
    })
    .unwrap();
    let field = bridge.call_field_offset();
    for (offset, destination) in [(0x100, 0x20), (3, 0x200)] {
        let bytes = relocated_bridge(&bridge, offset, destination).unwrap();
        assert_eq!(&bytes[..field], &bridge.bytes()[..field]);
        assert_eq!(&bytes[field + 4..], &bridge.bytes()[field + 4..]);
        let distance = i32::from_le_bytes(bytes[field..field + 4].try_into().unwrap());
        assert_eq!(
            (offset + field + 4) as i64 + i64::from(distance),
            destination as i64
        );
    }
    assert!(relocated_bridge(&bridge, usize::MAX, 0).is_err());
    assert!(relocated_bridge(&bridge, 0, usize::MAX).is_err());
}
