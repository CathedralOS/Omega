use std::fs;
use std::path::{Path, PathBuf};

use omega_core::allocations::AllocationDelta;
use omega_core::arena::Arena;
use omega_core::diagnostics::Diagnostic;
use omega_image::{EmittedImageOutput, ImageOutputKind};
use omega_target::{NativeTarget, ObjectFormat};
use omega_typed_program::{Program, machine::Machine, platform::Platform};

pub struct ArtifactWriter {
    root: PathBuf,
}

impl ArtifactWriter {
    pub fn new(build_dir: &Path) -> Result<Self, Diagnostic> {
        let root = build_dir.to_path_buf();
        fs::create_dir_all(&root).map_err(|error| {
            Diagnostic::error(format!(
                "failed to create artifact directory {}: {error}",
                root.display()
            ))
        })?;

        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn write_text(&self, file_name: &str, contents: &str) -> Result<(), Diagnostic> {
        let path = self.root.join(file_name);
        fs::write(&path, contents).map_err(|error| {
            Diagnostic::error(format!(
                "failed to write artifact {}: {error}",
                path.display()
            ))
        })
    }

    pub fn write_bytes(&self, file_name: &str, bytes: &[u8]) -> Result<PathBuf, Diagnostic> {
        let path = self.root.join(file_name);
        fs::write(&path, bytes).map_err(|error| {
            Diagnostic::error(format!(
                "failed to write artifact {}: {error}",
                path.display()
            ))
        })?;

        Ok(path)
    }

    pub fn remove_files<'a>(
        &self,
        file_names: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), Diagnostic> {
        for file_name in file_names {
            let path = self.root.join(file_name);
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(Diagnostic::error(format!(
                        "failed to remove stale artifact {}: {error}",
                        path.display()
                    )));
                }
            }
        }

        Ok(())
    }

    pub fn write_sources(&self, source_artifact: &SourceLoadArtifact) -> Result<(), Diagnostic> {
        let mut output = String::new();

        let total_bytes = source_artifact
            .files
            .iter()
            .map(|file| file.byte_count)
            .sum::<usize>();
        let total_lines = source_artifact
            .files
            .iter()
            .map(|file| file.line_count)
            .sum::<usize>();
        let total_non_empty_lines = source_artifact
            .files
            .iter()
            .map(|file| file.non_empty_line_count)
            .sum::<usize>();

        output.push_str("# Omega Source Load\n\n");
        output.push_str("## Totals\n");
        output.push_str(&format!("files: {}\n", source_artifact.files.len()));
        output.push_str(&format!("items: {}\n", source_artifact.item_count));
        output.push_str(&format!("bytes: {}\n", format_bytes(total_bytes as u64)));
        output.push_str(&format!("lines: {}\n", total_lines));
        output.push_str(&format!("non-empty lines: {}\n\n", total_non_empty_lines));

        output.push_str("## Files\n");
        output.push_str(&format!(
            "{:<4} {:>8} {:>7} {:>7} {:>7} {:>11} {}\n",
            "id", "bytes", "lines", "code", "items", "item range", "path"
        ));
        output.push_str(&format!(
            "{:<4} {:>8} {:>7} {:>7} {:>7} {:>11} {}\n",
            "--", "-----", "-----", "----", "-----", "----------", "----"
        ));

        for file in &source_artifact.files {
            write_source_file_artifact(&mut output, file);
        }

        self.write_text("01_sources.txt", &output)
    }

    pub fn write_ast(&self, ast_artifact: &AstArtifact) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega AST\n\n");
        output.push_str(&format!("files: {}\n", ast_artifact.file_count));
        output.push_str(&format!("items: {}\n\n", ast_artifact.item_count));

        output.push_str("## Identity Storage\n");
        output.push_str(&format!(
            "owned identifier strings: {}\n",
            ast_artifact.identity.owned_identifier_strings
        ));
        output.push_str(&format!(
            "identifiers: {}\n",
            ast_artifact.identity.identifiers
        ));
        output.push_str(&format!(
            "source identifiers: {}\n",
            ast_artifact.identity.source_identifiers
        ));
        output.push_str(&format!(
            "generated identifiers: {}\n",
            ast_artifact.identity.generated_identifiers
        ));
        output.push_str(&format!(
            "path members: {}\n",
            ast_artifact.identity.path_members
        ));
        output.push_str(&format!(
            "string literals: {}\n",
            ast_artifact.identity.string_literals
        ));
        output.push_str(&format!(
            "float literals: {}\n",
            ast_artifact.identity.float_literals
        ));
        output.push_str(&format!(
            "source float literals: {}\n",
            ast_artifact.identity.source_float_literals
        ));
        output.push_str(&format!(
            "generated float literals: {}\n\n",
            ast_artifact.identity.generated_float_literals
        ));

        for file in &ast_artifact.files {
            write_ast_file_artifact(&mut output, file);
        }

        self.write_text("02_ast.txt", &output)
    }
}

fn write_source_file_artifact(output: &mut String, file: &SourceFileArtifact) {
    let item_range = if file.item_count == 0 {
        String::from("-")
    } else {
        format!("{}..{}", file.first_item, file.first_item + file.item_count)
    };

    output.push_str(&format!(
        "{:<4} {:>8} {:>7} {:>7} {:>7} {:>11} {}\n",
        file.id,
        format_bytes(file.byte_count as u64),
        file.line_count,
        file.non_empty_line_count,
        file.item_count,
        item_range,
        file.path.display()
    ));
}

fn write_ast_file_artifact(output: &mut String, file: &AstFileArtifact) {
    output.push_str(&format!("## {}\n", file.path.display()));

    if !file.item_range_valid {
        output.push_str("invalid item range\n\n");
        return;
    }

    if file.item_summaries.is_empty() {
        output.push_str("items: none\n\n");
        return;
    }

    for (index, summary) in file.item_summaries.iter().enumerate() {
        output.push_str(&format!(
            "- item {}: {}\n",
            file.first_item + index,
            summary
        ));
    }

    output.push('\n');
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;

    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.2} GiB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.2} MiB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.2} KiB", bytes / KIB)
    } else {
        format!("{} B", bytes as u64)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseTiming {
    pub phase: String,
    pub microseconds: u128,
    pub allocations: AllocationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceLoadArtifact {
    pub item_count: usize,
    pub files: Vec<SourceFileArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceFileArtifact {
    pub id: usize,
    pub path: PathBuf,
    pub first_item: usize,
    pub item_count: usize,
    pub byte_count: usize,
    pub line_count: usize,
    pub non_empty_line_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AstArtifact {
    pub file_count: usize,
    pub item_count: usize,
    pub identity: AstIdentityArtifact,
    pub files: Vec<AstFileArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AstIdentityArtifact {
    pub owned_identifier_strings: usize,
    pub identifiers: usize,
    pub source_identifiers: usize,
    pub generated_identifiers: usize,
    pub path_members: usize,
    pub string_literals: usize,
    pub float_literals: usize,
    pub source_float_literals: usize,
    pub generated_float_literals: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AstFileArtifact {
    pub path: PathBuf,
    pub first_item: usize,
    pub item_summaries: Vec<String>,
    pub item_range_valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrustReport {
    pub targets: Arena<TrustTarget>,
    pub trust_roots: Arena<TrustRoot>,
    pub trusted_contracts: Arena<TrustedContract>,
    pub unresolved_trusts: Arena<UnresolvedTrustReference>,
    pub unchecked_policies: Arena<UncheckedTrustPolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrustTarget {
    pub name: String,
    pub host_provider: String,
    pub host_settings: usize,
    pub checked_trusts: usize,
    pub unchecked_trusts: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrustRoot {
    pub name: String,
    pub token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TrustedContract {
    pub capability: String,
    pub state: String,
    pub trust_level: String,
    pub resolved: bool,
    pub requires_count: usize,
    pub ensures_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnresolvedTrustReference {
    pub capability: String,
    pub state: String,
    pub trust_level: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UncheckedTrustPolicy {
    pub target: String,
    pub name: String,
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
