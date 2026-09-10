//! The runtime-spill consumer needs fresh ranges for every cumulative rewrite,
//! but one range analysis must not replay its immutable prerequisite twice.

use super::*;
use crate::LIVENESS_REPLAYS;
use crate::{
    LiveRangeError, LivenessError, analyze_live_ranges, analyze_liveness, validate_live_ranges,
};

#[test]
fn range_analysis_and_public_validation_each_replay_liveness_once() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let selected = fixture(target);
        let liveness = analyze_liveness(&selected).unwrap();
        LIVENESS_REPLAYS.set(0);
        let ranges = analyze_live_ranges(&selected, &liveness).unwrap();
        assert_eq!(LIVENESS_REPLAYS.get(), 1, "producer prerequisite");
        LIVENESS_REPLAYS.set(0);
        let replayed = validate_live_ranges(&selected, &liveness, ranges.plan().clone()).unwrap();
        assert_eq!(LIVENESS_REPLAYS.get(), 1, "fresh public prerequisite");
        assert_eq!(ranges, replayed);
    }
}

#[test]
fn invalid_liveness_precedes_range_computation_and_public_range_errors() {
    let selected = fixture(NativeTarget::linux_x64());
    let liveness = analyze_liveness(&selected).unwrap();
    let ranges = analyze_live_ranges(&selected, &liveness).unwrap();
    for mutation in 0..3 {
        let mut changed = liveness.clone();
        let expected = match mutation {
            0 => {
                changed.receipt.instruction_count += 1;
                LiveRangeError::LivenessReceiptMismatch
            }
            1 => {
                Arc::make_mut(&mut changed.plan).functions.clear();
                LiveRangeError::LivenessRevalidation(LivenessError::RootMismatch)
            }
            2 => {
                Arc::make_mut(&mut changed.plan).functions[0].blocks[0].instructions[0]
                    .virtual_live_in
                    .clear();
                // Recomputing the hash cannot turn altered facts into evidence.
                changed.receipt.identity = selected_instructions::liveness_identity(changed.plan());
                let rejected =
                    crate::validate_liveness(&selected, changed.plan().clone()).unwrap_err();
                LiveRangeError::LivenessRevalidation(rejected)
            }
            _ => unreachable!(),
        };
        assert_eq!(
            analyze_live_ranges(&selected, &changed),
            Err(expected.clone())
        );
        let mut corrupt_ranges = ranges.plan().clone();
        corrupt_ranges.functions.clear();
        assert_eq!(
            validate_live_ranges(&selected, &changed, corrupt_ranges),
            Err(expected)
        );
    }
}

#[test]
fn every_changed_selected_input_requires_fresh_liveness_custody() {
    let selected = fixture(NativeTarget::linux_x64());
    let liveness = analyze_liveness(&selected).unwrap();
    let ranges = analyze_live_ranges(&selected, &liveness).unwrap();
    for mutation in 0..3 {
        let mut changed = selected.clone();
        match mutation {
            0 => Arc::make_mut(&mut changed.transformed).target = NativeTarget::linux_arm64(),
            1 => changed.receipt.optimization_unit = OptimizationUnitIdentity::from_bytes([3; 32]),
            2 => {
                Arc::make_mut(&mut changed.transformed).functions[0].blocks[0].instructions[0]
                    .implicit_uses
                    .push(register_model::RegisterUnitId(999));
            }
            _ => unreachable!(),
        }
        // Keep the old selected identity deliberately: exact replay must reject
        // changed facts even if the caller has not refreshed the claimed root.
        let expected = crate::validate_liveness(&changed, liveness.plan().clone()).unwrap_err();
        assert_eq!(
            analyze_live_ranges(&changed, &liveness),
            Err(LiveRangeError::LivenessRevalidation(expected.clone()))
        );
        assert_eq!(
            validate_live_ranges(&changed, &liveness, ranges.plan().clone()),
            Err(LiveRangeError::LivenessRevalidation(expected))
        );
    }
}

#[test]
fn checked_liveness_does_not_admit_altered_live_ranges() {
    let selected = fixture(NativeTarget::linux_x64());
    let liveness = analyze_liveness(&selected).unwrap();
    let ranges = analyze_live_ranges(&selected, &liveness).unwrap();
    for mutation in 0..3 {
        let mut changed = ranges.plan().clone();
        match mutation {
            0 => changed.functions.clear(),
            1 => changed.functions[0].virtual_registers[0].fragments.clear(),
            2 => changed.functions[0].block_domains.clear(),
            _ => unreachable!(),
        }
        assert!(validate_live_ranges(&selected, &liveness, changed).is_err());
    }
}
