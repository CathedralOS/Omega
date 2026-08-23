use omega_backend_report_types::BackendSurfaceReport;

pub(super) fn write_source_native_surface(
    output: &mut String,
    backend_surface: &BackendSurfaceReport,
) {
    output.push_str("## Source Native Surface\n");
    output.push_str(&format!(
        "entry candidates: {}\n",
        backend_surface.entry_points.len()
    ));
    for (_, entry_point) in backend_surface.entry_points.iter() {
        output.push_str(&format!(
            "- entry {}.{}\n",
            entry_point.machine, entry_point.state
        ));
    }

    output.push_str(&format!("machines: {}\n", backend_surface.machines.len()));
    for (_, machine) in backend_surface.machines.iter() {
        output.push_str(&format!(
            "- machine {}: contains {}, owned data {}, states {}\n",
            machine.name, machine.contained_machines, machine.owned_data, machine.states
        ));
    }
    output.push('\n');
}
