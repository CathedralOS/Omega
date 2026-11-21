//! Build checks its toolchain field types once, then uses the admitted fields
//! to issue the initial Build value and read the settings after execution.

mod activation;
mod configuration;
mod initial;
mod optimization;
mod vocabulary;

pub use normalized_build_settings::{EvaluatedBuildSettings, X86DeploymentClaim};
pub(super) use vocabulary::is_exact_toolchain_build_prelude_data;

use super::BuildMachineFilesystemScope;
use build_time_values::BuildTimeValue;
use diagnostics::Diagnostic;
use source_compiler::representations::source_state::{TypedTrees, data::DataMember};

#[derive(Debug, Clone, Copy)]
pub struct BuildValueAdmission {
    optimization: optimization::BuildOptimizationAdmission, // Admitted optimization field vocabulary.
    has_toolchain_build_activation: bool,
    identifier: bool,
    log: bool,
    product: bool,
    elf_interpreter: bool,
    tests: bool,
    filesystem_roots: bool,
    inputs: bool,
}

impl BuildValueAdmission {
    pub fn admit(typed: &TypedTrees) -> Result<Self, Vec<Diagnostic>> {
        let optimization = optimization::BuildOptimizationAdmission::admit(typed)?;
        let build = activation::admit_toolchain_build_activation(typed)?;
        let identifier = build.is_some_and(|build| {
            typed.data_members(build).iter().any(|member| {
                matches!(member, DataMember::Field(field)
                if field.name.as_str() == "identifier")
            })
        });
        let has = |name| vocabulary::has_exact_toolchain_build_facet(typed, name);
        Ok(Self {
            optimization,
            has_toolchain_build_activation: build.is_some(),
            identifier,
            log: has("BuildLog"),
            product: has("BuildProduct"),
            elf_interpreter: has("ElfInterpreterSelection"),
            tests: has("BuildTests"),
            filesystem_roots: has("BuildSource") && has("BuildOutput"),
            inputs: has("BuildInputs"),
        })
    }

    pub fn initial_value(&self, filesystem_scope: &BuildMachineFilesystemScope) -> BuildTimeValue {
        initial::initial_value(self, filesystem_scope)
    }

    pub fn extract(
        self,
        value: &BuildTimeValue,
    ) -> Result<
        (
            EvaluatedBuildSettings,
            optimization_requests::OptimizationReportRequest,
        ),
        String,
    > {
        configuration::extract_build_settings(
            value,
            self.optimization,
            self.has_toolchain_build_activation,
        )
    }
}
