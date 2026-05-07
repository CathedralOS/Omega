//! Native backend implementation and surface collection.

use omega_abstract_syntax_tree::item::{Item, Machine, Platform};
use omega_core::arena::Arena;

pub mod abi;
pub mod alias_flow;
pub mod architecture;
pub mod control_flow;
pub mod data;
pub mod emission;
pub mod emitter;
pub mod executable_finalization;
pub mod host_calls;
pub mod instructions;
pub mod layout;
pub mod machine_code;
pub mod object;
pub mod plan;
pub mod relocations;
pub mod runtime_dispatch;
pub mod runtime_flow;
pub mod runtime_storage;
pub mod runtime_text;
pub mod state_calls;
pub mod state_schedule;
pub mod state_storage;
pub mod state_values;
pub mod target;
pub mod target_output;

pub use runtime_dispatch::guards as state_guards;
pub use runtime_dispatch::states as state_dispatch;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NativeSurfaceReport {
    pub entry_points: Arena<NativeEntryPoint>,
    pub machines: Arena<NativeMachineSurface>,
    pub platforms: Arena<NativePlatformSurface>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NativeEntryPoint {
    pub machine: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NativeMachineSurface {
    pub name: String,
    pub contained_objects: usize,
    pub owned_data: usize,
    pub states: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NativePlatformSurface {
    pub name: String,
    pub states: usize,
}

pub fn build_native_surface_report(items: &[Item]) -> NativeSurfaceReport {
    let mut report = NativeSurfaceReport::default();

    for item in items {
        match item {
            Item::Capability(_) => {}
            Item::Machine(machine) => collect_machine(&mut report, machine),
            Item::Platform(platform) => collect_platform(&mut report, platform),
            Item::Data(_) | Item::Invariant(_) | Item::Use(_) => {}
            Item::Target(_) | Item::TrustDefinition(_) => {}
        }
    }

    report
}

fn collect_machine(report: &mut NativeSurfaceReport, machine: &Machine) {
    report.machines.insert(NativeMachineSurface {
        name: machine.name.clone(),
        contained_objects: machine.contains.len(),
        owned_data: machine.owned_data.len(),
        states: machine.states.len(),
    });

    if machine.name == "main" && machine.states.iter().any(|state| state.name == "entry") {
        report.entry_points.insert(NativeEntryPoint {
            machine: "main".to_owned(),
            state: "entry".to_owned(),
        });
    }
}

fn collect_platform(report: &mut NativeSurfaceReport, platform: &Platform) {
    report.platforms.insert(NativePlatformSurface {
        name: platform.name.clone(),
        states: platform.states.len(),
    });
}

#[cfg(test)]
mod tests {
    use omega_abstract_syntax_tree::item::{Item, Machine, Platform, State, StateSignature};

    use super::build_native_surface_report;

    #[test]
    fn collects_entry_machine_and_platforms() {
        let report = build_native_surface_report(&[
            Item::Platform(Platform {
                name: "Console".to_owned(),
                states: vec![StateSignature {
                    name: "write_line".to_owned(),
                    parameters: Vec::new(),
                    return_type: None,
                }],
            }),
            Item::Machine(Machine {
                name: "main".to_owned(),
                contains: Vec::new(),
                owned_data: Vec::new(),
                states: vec![State {
                    name: "entry".to_owned(),
                    parameters: Vec::new(),
                    return_type: None,
                    statements: Vec::new(),
                }],
            }),
        ]);

        assert_eq!(report.entry_points.len(), 1);
        assert_eq!(report.platforms.len(), 1);
        assert_eq!(report.machines.len(), 1);
    }
}
