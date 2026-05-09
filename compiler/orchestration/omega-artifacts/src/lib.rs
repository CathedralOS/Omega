use std::path::{Path, PathBuf};

use omega_core::allocations::AllocationDelta;
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
use omega_image::{EmittedImageOutput, ImageOutputKind};
use omega_target::{NativeTarget, ObjectFormat};
use omega_typed_program::{Program, machine::Machine, platform::Platform};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseTiming {
    pub phase: String,
    pub microseconds: u128,
    pub allocations: AllocationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmissionPlan {
    pub image_format: ObjectFormat,
    pub entry_symbol: String,
    pub sections: usize,
    pub symbols: usize,
    pub host_bindings: usize,
    pub host_calls: usize,
    pub data_bytes: usize,
    pub selected_instructions: usize,
    pub instruction_operands: usize,
    pub machine_code_bytes: usize,
    pub encoded_machine_bytes: usize,
    pub relocations: usize,
    pub blockers: Arena<EmissionBlocker>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EmissionBlocker {
    pub stage: String,
    pub reason: String,
}

pub fn emission_blocker(stage: &str, reason: &str) -> EmissionBlocker {
    EmissionBlocker {
        stage: stage.to_owned(),
        reason: reason.to_owned(),
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableFinalization {
    pub executable_path: PathBuf,
    pub status: ExecutableFinalizationStatus,
    pub command: Vec<String>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutableFinalizationStatus {
    AlreadyExecutable,
}

pub fn finalize_emitted_image_output(
    target: NativeTarget,
    emitted_output: &EmittedImageOutput,
    output_path: &Path,
) -> Result<ExecutableFinalization, Diagnostic> {
    if emitted_output.kind == ImageOutputKind::DirectExecutable {
        mark_executable_if_needed(output_path)?;
        return Ok(ExecutableFinalization {
            executable_path: output_path.to_path_buf(),
            status: ExecutableFinalizationStatus::AlreadyExecutable,
            command: Vec::new(),
            stdout: "native output is already an executable image".to_owned(),
            stderr: String::new(),
        });
    }

    Err(Diagnostic::error(format!(
        "native output `{}` is {:?} for {:?}; Omega does not invoke external linkers, so this target must emit a direct executable image",
        emitted_output.format, emitted_output.kind, target
    )))
}

#[cfg(unix)]
fn mark_executable_if_needed(path: &Path) -> Result<(), Diagnostic> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .map_err(|error| {
            Diagnostic::error(format!(
                "failed to read executable permissions {}: {error}",
                path.display()
            ))
        })?
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).map_err(|error| {
        Diagnostic::error(format!(
            "failed to mark executable {}: {error}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn mark_executable_if_needed(_path: &Path) -> Result<(), Diagnostic> {
    Ok(())
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
