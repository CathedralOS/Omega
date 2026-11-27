//! Build checks its toolchain field types once, then uses the admitted fields
//! to issue the initial Build value and read the settings after execution.

mod activation;
mod configuration;
mod initial;
mod optimization;
mod vocabulary;

pub use crate::normalized_settings::{EvaluatedBuildSettings, X86DeploymentClaim};
pub(super) use vocabulary::is_exact_toolchain_build_prelude_data;

use build_model::values::BuildTimeValue;
use diagnostics::Diagnostic;
use source_compiler::representations::source_state::{TypedTrees, data::DataMember};

#[derive(Debug, Clone, Copy)]
pub struct BuildValueAdmission {
    identifier: bool,
    log: bool,
    declarations: bool,
    elf_interpreter: bool,
    tests: bool,
    proof: bool,
    filesystem_roots: bool,
}

impl BuildValueAdmission {
    pub fn admit(typed: &TypedTrees) -> Result<Self, Vec<Diagnostic>> {
        let build = activation::admit_toolchain_build_activation(typed)?;
        let identifier = typed.data_members(build).iter().any(|member| {
            matches!(member, DataMember::Field(field) if field.name.as_str() == "identifier")
        });
        let has = |name| vocabulary::has_exact_toolchain_build_facet(typed, name);
        Ok(Self {
            identifier,
            log: has("BuildLog"),
            declarations: has("BuildDeclarations"),
            elf_interpreter: has("ElfInterpreterSelection"),
            tests: has("BuildTests"),
            proof: has("ProofWorkPolicy"),
            filesystem_roots: has("BuildSource") && has("BuildOutput"),
        })
    }

    pub fn initial_value(&self) -> BuildTimeValue {
        initial::initial_value(self)
    }

    pub fn extract(
        self,
        value: &BuildTimeValue,
    ) -> Result<
        (
            EvaluatedBuildSettings,
            build_model::optimization_requests::OptimizationReportRequest,
        ),
        String,
    > {
        configuration::extract_build_settings(value)
    }
}
