//! Native backend implementation and surface collection.

use omega_core::arena::Arena;
use omega_typed_program::{Program, machine::Machine, platform::Platform};

pub mod abi;
pub mod alias_flow;
pub mod architecture;
pub mod control_flow;
pub mod data;
pub mod emission;
pub mod emitter;
pub mod executable_finalization;
pub mod host_calls;
pub mod identity;
pub mod instructions;
pub mod machine_code;
pub mod object;
pub(crate) mod place_keys;
pub mod plan;
pub mod relocations;
pub mod runtime_dispatch;
pub mod runtime_flow;
pub mod runtime_storage;
pub mod runtime_text;
pub mod state_analysis;
pub mod state_calls;
pub mod state_schedule;
pub mod state_storage;
pub mod state_values;
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

pub fn build_native_surface_report(program: &Program) -> NativeSurfaceReport {
    let mut report = NativeSurfaceReport::default();

    for machine in &program.machines {
        collect_machine(&mut report, machine);
    }

    for platform in &program.platforms {
        collect_platform(&mut report, platform);
    }

    report
}

fn collect_machine(report: &mut NativeSurfaceReport, machine: &Machine) {
    report.machines.insert(NativeMachineSurface {
        name: machine.name.to_string(),
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
        name: platform.name.to_string(),
        states: platform.states.len(),
    });
}

#[cfg(test)]
mod tests {
    use omega_core::symbols::SymbolHandle;
    use omega_typed_program::Program;
    use omega_typed_program::machine::Machine;
    use omega_typed_program::name::ProgramName;
    use omega_typed_program::platform::Platform;
    use omega_typed_program::signature::StateSignature;
    use omega_typed_program::state::State;

    use super::build_native_surface_report;

    #[test]
    fn collects_entry_machine_and_platforms() {
        let report = build_native_surface_report(&Program {
            platforms: vec![Platform {
                symbol: SymbolHandle::default(),
                name: ProgramName::generated("Console"),
                states: vec![StateSignature {
                    symbol: SymbolHandle::default(),
                    name: ProgramName::generated("write_line"),
                    parameters: Vec::new(),
                    return_type: None,
                }],
            }],
            machines: vec![Machine {
                symbol: SymbolHandle::default(),
                name: ProgramName::generated("main"),
                contains: Vec::new(),
                owned_data: Vec::new(),
                states: vec![State {
                    symbol: SymbolHandle::default(),
                    name: ProgramName::generated("entry"),
                    parameters: Vec::new(),
                    return_type: None,
                    statements: Vec::new(),
                }],
            }],
            ..Program::default()
        });

        assert_eq!(report.entry_points.len(), 1);
        assert_eq!(report.platforms.len(), 1);
        assert_eq!(report.machines.len(), 1);
    }
}
