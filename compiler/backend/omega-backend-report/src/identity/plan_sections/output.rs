use crate::BackendReportInput;
use crate::identity::BackendStringStorage;

pub(in crate::identity) fn count_instruction_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, function) in backend_plan.target_operations.code.functions.iter() {
        storage.count_generated_symbol(&function.symbol);
    }
}

pub(in crate::identity) fn count_object_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, section) in backend_plan.object.layout.sections.iter() {
        storage.count_generated_symbol(&omega_object_file::section_name(
            backend_plan.object.target,
            section.kind,
        ));
    }
    for (_, symbol) in backend_plan.object.layout.symbols.iter() {
        storage.count_generated_symbol(&symbol.name);
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
