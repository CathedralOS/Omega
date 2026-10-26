//! The Build-value protocol: admit exact vocabulary, construct its initial
//! value, then decode evaluated settings. Interpreter execution and
//! independently checked provider/root/trust receipts remain separate.

mod configuration;
mod initial;
mod optimization;
mod vocabulary;

pub use configuration::EvaluatedBuildSettings;
pub(super) use vocabulary::is_build_machine;

use super::BuildMachineFilesystemScope;
use build_time_evaluation::BuildTimeValue;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

/// Exact vocabulary admission retained across one build evaluation.
/// Private fields prevent mixing legacy inputs with canonical output decoding.
#[derive(Debug, Clone, Copy)]
pub struct BuildValueAdmission {
    optimization: optimization::BuildOptimizationAdmission,
    has_toolchain_build_activation: bool,
}

impl BuildValueAdmission {
    pub fn admit(typed: &TypedTrees) -> Result<Self, Vec<Diagnostic>> {
        let optimization = optimization::BuildOptimizationAdmission::admit(typed)?;
        let has_toolchain_build_activation =
            build_time_evaluation::admit_toolchain_build_activation(typed)?;
        Ok(Self {
            optimization,
            has_toolchain_build_activation,
        })
    }

    pub fn initial_value(
        &self,
        typed: &TypedTrees,
        machine: &typed_trees::machine::Machine,
        filesystem_scope: &BuildMachineFilesystemScope,
    ) -> BuildTimeValue {
        initial::initial_value(self, typed, machine, filesystem_scope)
    }

    pub fn extract(
        self,
        value: &BuildTimeValue,
    ) -> Result<
        (
            EvaluatedBuildSettings,
            optimization_core::OptimizationReportRequest,
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
