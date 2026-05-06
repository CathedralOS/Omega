use std::fs;
use std::path::{Path, PathBuf};

use crate::diagnostics::Diagnostic;
use crate::driver::compile::{LoadedFile, LoadedProgram, PhaseTiming};
use crate::ir::Program;
use crate::native::plan::NativePlan;
use crate::proof::obligations::ProofPlan;
use crate::semantic::effects::EffectPlan;

pub(crate) struct ArtifactWriter {
    root: PathBuf,
}

impl ArtifactWriter {
    pub(crate) fn new(build_dir: &Path) -> Result<Self, Diagnostic> {
        let root = build_dir.to_path_buf();
        fs::create_dir_all(&root).map_err(|error| {
            Diagnostic::error(format!(
                "failed to create artifact directory {}: {error}",
                root.display()
            ))
        })?;

        Ok(Self { root })
    }

    pub(crate) fn write_sources(&self, loaded_program: &LoadedProgram) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Source Load\n\n");
        output.push_str(&format!("files: {}\n", loaded_program.files.len()));
        output.push_str(&format!("items: {}\n\n", loaded_program.items.len()));

        for file in &loaded_program.files {
            write_loaded_file(&mut output, file);
        }

        self.write("01_sources.txt", &output)
    }

    pub(crate) fn write_ast(&self, loaded_program: &LoadedProgram) -> Result<(), Diagnostic> {
        self.write("02_ast.txt", &format!("{:#?}\n", loaded_program.items))
    }

    pub(crate) fn write_ir(&self, program: &Program) -> Result<(), Diagnostic> {
        self.write("05_driver_ir.txt", &format!("{program:#?}\n"))
    }

    pub(crate) fn write_effects(&self, effect_plan: &EffectPlan) -> Result<(), Diagnostic> {
        self.write("04_types.txt", &format!("{effect_plan:#?}\n"))
    }

    pub(crate) fn write_validation(&self, program: &Program) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Validation\n\n");
        output.push_str("status: ok\n");
        output.push_str(&format!(
            "data definitions: {}\n",
            program.data_definitions.len()
        ));
        output.push_str(&format!("platforms: {}\n", program.platforms.len()));
        output.push_str(&format!("machines: {}\n", program.machines.len()));

        self.write("06_validation.txt", &output)
    }

    pub(crate) fn write_proof_plan(&self, proof_plan: &ProofPlan) -> Result<(), Diagnostic> {
        self.write("08_proof.txt", &format!("{proof_plan:#?}\n"))
    }

    pub(crate) fn write_native_plan(&self, native_plan: &NativePlan) -> Result<(), Diagnostic> {
        self.write("09_native_plan.txt", &format!("{native_plan:#?}\n"))
    }

    pub(crate) fn write_timings(&self, timings: &[PhaseTiming]) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Phase Timings\n\n");

        for timing in timings {
            output.push_str(&format!("{}: {} us\n", timing.phase, timing.microseconds));
        }

        self.write("00_timings.txt", &output)
    }

    pub(crate) fn write_placeholder(&self, file_name: &str, title: &str) -> Result<(), Diagnostic> {
        self.write(
            file_name,
            &format!("# {title}\n\nstatus: not implemented yet\n"),
        )
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    fn write(&self, file_name: &str, contents: &str) -> Result<(), Diagnostic> {
        let path = self.root.join(file_name);
        fs::write(&path, contents).map_err(|error| {
            Diagnostic::error(format!(
                "failed to write artifact {}: {error}",
                path.display()
            ))
        })
    }
}

fn write_loaded_file(output: &mut String, file: &LoadedFile) {
    output.push_str(&format!("## {}\n", file.path.display()));
    output.push_str(&format!("first item: {}\n", file.first_item));
    output.push_str(&format!("item count: {}\n\n", file.item_count));
}
