//! Source/backend audit-surface construction.

use psi_checked_trees::{CheckedTrees, machine::Machine};

use super::{BackendEntryPoint, BackendMachineSurface, BackendSurfaceReport};

/// Build the source/backend audit surface around the exact Build-selected
/// entry. With no selected entry, the report must not invent one from a source
/// name.
pub fn build_backend_surface_report(
    program: &CheckedTrees,
    selected_entry_machine: Option<&str>,
) -> BackendSurfaceReport {
    let mut report = BackendSurfaceReport::default();

    for machine in program.machines() {
        collect_machine(&mut report, program, machine);
    }

    let selected = selected_entry_machine.and_then(|name| entry_point_for_machine(program, name));
    if let Some(entry) = selected {
        report.entry_points.insert(entry);
    }

    report
}

fn collect_machine(report: &mut BackendSurfaceReport, program: &CheckedTrees, machine: &Machine) {
    report.machines.insert(BackendMachineSurface {
        name: machine.name.to_string(),
        contained_machines: program
            .facts
            .carry
            .contained_fields_for_machine(machine.symbol)
            .len(),
        owned_data: program.machine_owned_data(machine).len(),
        states: program.machine_states(machine).len(),
    });
}

fn entry_point_for_machine(program: &CheckedTrees, name: &str) -> Option<BackendEntryPoint> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)?;
    let state = program.machine_states(machine).first()?;
    Some(BackendEntryPoint {
        machine: machine.name.as_str().to_owned(),
        state: state.name.as_str().to_owned(),
    })
}
