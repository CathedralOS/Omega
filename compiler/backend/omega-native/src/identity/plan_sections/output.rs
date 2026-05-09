use crate::identity::NativeStringStorage;
use crate::plan::NativePlan;

pub(in crate::identity) fn count_instruction_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, function) in native_plan.instructions.functions.iter() {
        storage.count_generated_symbol(&function.symbol);
    }
}

pub(in crate::identity) fn count_object_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    storage.count_generated_symbol(&native_plan.object.entry_symbol);
    for (_, section) in native_plan.object.sections.iter() {
        storage.count_generated_symbol(&section.name);
    }
    for (_, symbol) in native_plan.object.symbols.iter() {
        storage.count_generated_symbol(&symbol.name);
        if let Some(section) = &symbol.section {
            storage.count_generated_symbol(section);
        }
    }
}

pub(in crate::identity) fn count_phase_timing_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for timing in &native_plan.phase_timings {
        storage.count_report(&timing.phase);
    }
}
