//! Native emission, installation, and finalization artifact presentation.

use std::path::{Path, PathBuf};

use omega_image::{EmittedImageOutput, ImageOutputKind};
use omega_target::NativeTarget;
use psi_diagnostics::Diagnostic;

use super::{ArtifactWriter, EmissionPlan};

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

impl ArtifactWriter {
    pub fn write_emission_plan(&self, emission_plan: &EmissionPlan) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Emission Plan\n\n");
        output.push_str(&format!("image format: {:?}\n", emission_plan.image_format));
        output.push_str(&format!("entry symbol: {}\n", emission_plan.entry_symbol));
        output.push_str(&format!("sections: {}\n", emission_plan.sections));
        output.push_str(&format!("symbols: {}\n", emission_plan.symbols));
        output.push_str(&format!("host bindings: {}\n", emission_plan.host_bindings));
        output.push_str(&format!("host calls: {}\n", emission_plan.host_calls));
        output.push_str(&format!("data bytes: {}\n", emission_plan.data_bytes));
        output.push_str(&format!(
            "selected instructions: {}\n",
            emission_plan.selected_instructions
        ));
        output.push_str(&format!(
            "instruction operands: {}\n",
            emission_plan.instruction_operands
        ));
        output.push_str(&format!(
            "machine code bytes: {}\n",
            emission_plan.machine_code_bytes
        ));
        output.push_str(&format!(
            "encoded machine bytes: {}\n",
            emission_plan.encoded_machine_bytes
        ));
        output.push_str(&format!("relocations: {}\n", emission_plan.relocations));
        output.push_str(&format!("blockers: {}\n\n", emission_plan.blockers.len()));

        if emission_plan.blockers.is_empty() {
            output.push_str("status: ready to emit\n");
        } else {
            output.push_str("status: blocked before byte emission\n\n");
            output.push_str("## Blockers\n");

            for (_, blocker) in emission_plan.blockers.iter() {
                output.push_str(&format!("- {}: {}\n", blocker.stage, blocker.reason));
            }
        }

        self.write_text("12_emission.txt", &output)
    }

    pub fn write_emitted_native_output(
        &self,
        emitted_output: &EmittedImageOutput,
    ) -> Result<PathBuf, Diagnostic> {
        let output_path = self.write_bytes(&emitted_output.file_name, &emitted_output.bytes)?;

        let mut output = String::new();
        output.push_str("# Omega Emitted Native Output\n\n");
        output.push_str(&format!("path: {}\n", output_path.display()));
        output.push_str(&format!("format: {}\n", emitted_output.format));
        output.push_str(&format!("kind: {:?}\n", emitted_output.kind));
        output.push_str(&format!("bytes: {}\n", emitted_output.bytes.len()));
        output.push_str(&format!("text bytes: {}\n", emitted_output.text_bytes));
        output.push_str(&format!("data bytes: {}\n", emitted_output.data_bytes));
        output.push_str(&format!("bss bytes: {}\n", emitted_output.bss_bytes));
        output.push_str(&format!("symbols: {}\n", emitted_output.symbols));
        output.push_str(&format!("relocations: {}\n", emitted_output.relocations));
        output.push_str(&format!(
            "final image symbols: {}\n",
            emitted_output.final_image_symbols
        ));
        output.push_str(&format!(
            "final image imports: {}\n",
            emitted_output.final_image_imports
        ));
        output.push_str(&format!(
            "final image relocations: {}\n",
            emitted_output.final_image_relocations
        ));

        self.write_html_report("13_emitted_output.html", "emitted_output", &output)?;

        Ok(output_path)
    }

    pub fn remove_stale_native_output(&self) -> Result<(), Diagnostic> {
        self.remove_files([
            "omega-program",
            "12_emitted_output.txt",
            "12_emitted_output.html",
            "13_emitted_output.txt",
            "13_emitted_output.html",
            "13_finalization.txt",
            "13_finalization.html",
            "14_finalization.txt",
            "14_finalization.html",
        ])
    }

    pub fn write_executable_finalization_report(
        &self,
        finalization: &ExecutableFinalization,
    ) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Executable Finalization\n\n");
        output.push_str(&format!("status: {:?}\n", finalization.status));
        output.push_str(&format!(
            "output: {}\n",
            finalization.executable_path.display()
        ));
        if finalization.command.is_empty() {
            output.push_str("command: none\n");
        } else {
            output.push_str(&format!("command: {}\n", finalization.command.join(" ")));
        }
        if !finalization.stdout.is_empty() {
            output.push_str("\n## stdout\n");
            output.push_str(&finalization.stdout);
            output.push('\n');
        }
        if !finalization.stderr.is_empty() {
            output.push_str("\n## stderr\n");
            output.push_str(&finalization.stderr);
            output.push('\n');
        }
        if finalization.status == ExecutableFinalizationStatus::AlreadyExecutable {
            output.push_str(
                "\nno finalization command was needed; the backend emitted an executable image directly.\n",
            );
        }

        self.write_html_report("14_finalization.html", "finalization", &output)
    }
}
