use crate::BackendReportInput;
use crate::identity::BackendStringStorage;

pub(in crate::identity) fn count_instruction_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, function) in backend_plan.instructions.functions.iter() {
        storage.count_generated_symbol(&function.symbol);
    }
}

pub(in crate::identity) fn count_object_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    storage.count_generated_symbol(&backend_plan.object.entry_symbol);
    for (_, section) in backend_plan.object.sections.iter() {
        storage.count_generated_symbol(&section.name);
    }
    for (_, symbol) in backend_plan.object.symbols.iter() {
        storage.count_generated_symbol(&symbol.name);
        if let Some(section) = &symbol.section {
            storage.count_generated_symbol(section);
        }
    }
}

pub(in crate::identity) fn count_phase_timing_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for timing in backend_plan.phase_timings {
        storage.count_report(&timing.phase);
    }
}
