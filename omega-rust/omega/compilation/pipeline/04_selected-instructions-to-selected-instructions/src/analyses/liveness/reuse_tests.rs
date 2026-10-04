use super::{
    Arc, FuelScheduleIdentity, MachineId, NativeTarget, OptimizationUnitIdentity, fixture,
    selected_instruction_plan_identity,
};
use crate::ValidatedRuntimeSpill;
use crate::analyses::live_ranges::LIVE_RANGE_REUSED_FUNCTIONS as REUSED_FUNCTIONS;
use crate::analyses::liveness::LIVENESS_FUNCTION_COMPUTATIONS as FUNCTION_COMPUTATIONS;
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
