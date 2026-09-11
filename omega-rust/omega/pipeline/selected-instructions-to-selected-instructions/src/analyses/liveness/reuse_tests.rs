use super::*;
use crate::LIVE_RANGE_REUSED_FUNCTIONS as REUSED_FUNCTIONS;
use crate::LIVENESS_FUNCTION_COMPUTATIONS as FUNCTION_COMPUTATIONS;
use crate::{
    analyze_live_ranges, analyze_live_ranges_reusing, analyze_liveness, analyze_liveness_reusing,
};

fn two_functions(target: NativeTarget) -> ValidatedRuntimeSpill {
    let mut selected = fixture(target);
    let mut second = selected.transformed.functions[0].clone();
    second.machine = MachineId::new(2).unwrap();
    Arc::make_mut(&mut selected.transformed)
        .functions
        .push(second);
    refresh_identity(&mut selected);
    selected
}

fn refresh_identity(selected: &mut ValidatedRuntimeSpill) {
    let identity = selected_instruction_plan_identity(&selected.transformed);
    selected.receipt.source_selected = identity;
    selected.receipt.transformed_selected = identity;
    selected.receipt.fuel_schedule = selected.transformed.fuel_schedule;
}

#[test]
fn function_reuse_matches_full_analysis_and_computes_only_changed_body() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let previous = two_functions(target);
        let old_live = analyze_liveness(&previous).unwrap();
        let old_ranges = analyze_live_ranges(&previous, &old_live).unwrap();
        for mutation in 0..3 {
            let changed_body = mutation == 1;
            let mut selected = previous.clone();
            if changed_body {
                Arc::make_mut(&mut selected.transformed).functions[0].blocks[0].instructions[0]
                    .implicit_uses
                    .push(register_model::RegisterUnitId(999));
            }
            if mutation == 2 {
                let detached = selected.transformed.functions.iter().cloned().collect();
                Arc::make_mut(&mut selected.transformed).functions = detached;
                assert!(
                    !selected
                        .transformed
                        .functions
                        .shares_function_storage(&previous.transformed.functions, 0)
                );
            }
            refresh_identity(&mut selected);
            FUNCTION_COMPUTATIONS.set(0);
            let live = analyze_liveness_reusing(&previous, &old_live, &selected).unwrap();
            assert_eq!(FUNCTION_COMPUTATIONS.get(), usize::from(changed_body));
            assert_eq!(live, analyze_liveness(&selected).unwrap());
            REUSED_FUNCTIONS.set(0);
            let ranges =
                analyze_live_ranges_reusing(&previous, &old_live, &old_ranges, &selected, &live)
                    .unwrap();
            assert_eq!(REUSED_FUNCTIONS.get(), 2 - usize::from(changed_body));
            assert_eq!(ranges, analyze_live_ranges(&selected, &live).unwrap());
        }
    }
}

#[test]
fn function_reuse_invalidates_target_unit_fuel_and_function_order() {
    let previous = two_functions(NativeTarget::linux_x64());
    let old_live = analyze_liveness(&previous).unwrap();
    let old_ranges = analyze_live_ranges(&previous, &old_live).unwrap();
    for mutation in 0..4 {
        let mut selected = previous.clone();
        match mutation {
            0 => Arc::make_mut(&mut selected.transformed).target = NativeTarget::linux_arm64(),
            1 => selected.receipt.optimization_unit = OptimizationUnitIdentity::from_bytes([3; 32]),
            2 => {
                Arc::make_mut(&mut selected.transformed).fuel_schedule =
                    FuelScheduleIdentity::new(2).unwrap()
            }
            3 => {
                let functions = &mut Arc::make_mut(&mut selected.transformed).functions;
                let first = functions[0].clone();
                functions[0] = functions[1].clone();
                functions[1] = first;
            }
            _ => unreachable!(),
        }
        refresh_identity(&mut selected);
        FUNCTION_COMPUTATIONS.set(0);
        let live = analyze_liveness_reusing(&previous, &old_live, &selected).unwrap();
        assert_eq!(FUNCTION_COMPUTATIONS.get(), 2);
        assert_eq!(live, analyze_liveness(&selected).unwrap());
        REUSED_FUNCTIONS.set(0);
        let ranges =
            analyze_live_ranges_reusing(&previous, &old_live, &old_ranges, &selected, &live)
                .unwrap();
        assert_eq!(REUSED_FUNCTIONS.get(), 0);
        assert_eq!(ranges, analyze_live_ranges(&selected, &live).unwrap());
    }
}

#[test]
fn reused_corrupt_facts_are_rejected_by_independent_replay() {
    let selected = two_functions(NativeTarget::linux_x64());
    let live = analyze_liveness(&selected).unwrap();
    let ranges = analyze_live_ranges(&selected, &live).unwrap();
    let mut corrupt_live = live.clone();
    Arc::make_mut(&mut corrupt_live.plan).functions[1].blocks[0].instructions[0]
        .virtual_live_in
        .clear();
    corrupt_live.receipt.identity = selected_instructions::liveness_identity(corrupt_live.plan());
    assert!(analyze_liveness_reusing(&selected, &corrupt_live, &selected).is_err());
    let mut corrupt_ranges = ranges.clone();
    Arc::make_mut(&mut corrupt_ranges.plan).functions[1]
        .block_domains
        .clear();
    corrupt_ranges.receipt.identity =
        selected_instructions::live_range_identity(corrupt_ranges.plan());
    assert!(
        analyze_live_ranges_reusing(&selected, &live, &corrupt_ranges, &selected, &live).is_err()
    );
}
