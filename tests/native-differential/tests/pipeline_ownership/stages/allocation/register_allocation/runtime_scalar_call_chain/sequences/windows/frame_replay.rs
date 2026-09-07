//! Genuine selected-program frame plans must retain the exact Microsoft call area.

use super::physical;
use crate::tests::*;

#[test]
fn windows_frame_replay_rejects_missing_or_overlapping_shadow_storage() {
    let selections = OptimizationSelections::new([
        Optimization::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
    ])
    .unwrap();
    let staged = physical(4, &selections)
        .into_post_allocation_machine_for_test()
        .unwrap();
    let frame = staged
        .frame()
        .expect("calling functions require an explicit frame");
    let canonical = frame.layout().plan();
    let index = canonical
        .functions
        .iter()
        .position(|function| function.machine.get() == SCALAR_CALL_UNIT_CALLER)
        .unwrap();
    let caller = &canonical.functions[index];
    assert!(caller.contains_call);
    assert_eq!(caller.outgoing_abi_area.byte_size, 32);
    assert_eq!(caller.outgoing_abi_area.shadow_bytes, 32);
    assert!(
        !caller.callee_save_slots.is_empty(),
        "live call results exercise preservation"
    );
    assert!(
        caller
            .callee_save_slots
            .iter()
            .all(|slot| slot.frame_offset_bytes >= 32)
    );
    let current = staged.allocation().current();
    let replay = |plan| {
        validate_target_frame_layout(
            staged.machine(),
            frame.requirements(),
            frame.storage(),
            current.register_environment(),
            plan,
        )
    };
    replay(canonical.clone()).unwrap();

    for (name, mutation) in [
        (
            "missing",
            (|row: &mut FunctionTargetFrameLayout| {
                row.outgoing_abi_area.byte_size = 0;
                row.outgoing_abi_area.shadow_bytes = 0;
            }) as fn(&mut FunctionTargetFrameLayout),
        ),
        ("undersized", |row| {
            row.outgoing_abi_area.byte_size = 16;
            row.outgoing_abi_area.shadow_bytes = 16;
        }),
        ("wrong shadow claim", |row| {
            row.outgoing_abi_area.shadow_bytes = 0;
        }),
        ("oversized outgoing area", |row| {
            row.outgoing_abi_area.byte_size = 48;
        }),
        ("overlapping save slot", |row| {
            row.callee_save_slots[0].frame_offset_bytes = 24;
        }),
        ("oversized aligned frame", |row| {
            row.frame_size_bytes += 16;
            if let ReturnAddressFrameCustody::CallerActivationStack {
                post_prologue_offset_bytes,
                ..
            } = &mut row.return_address
            {
                *post_prologue_offset_bytes += 16;
            }
        }),
    ] {
        let mut corrupted = canonical.clone();
        mutation(&mut corrupted.functions[index]);
        assert!(
            replay(corrupted).is_err(),
            "{name} must fail independent frame replay"
        );
    }
}
