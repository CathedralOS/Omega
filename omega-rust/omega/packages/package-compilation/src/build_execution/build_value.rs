/*
We own the `Build`-value admission, initial value construction, and post-evaluation decoding protocol for `build.omg`.

Before the build interpreter runs, we inspect the checked `Build` declaration in `TypedTrees` to admit the exact toolchain vocabulary (`BuildValueAdmission`). We then construct the initial `BuildTimeValue` (`initial_value`) populated with zeroed counters, default target/subsystem settings, and any admitted root facets (`source`, `output`, `inputs`, `log`, `product`). Once evaluation finishes, we decode the resulting `BuildTimeValue` back into normalized `EvaluatedBuildSettings` and `OptimizationReportRequest`.
*/

/*
Why `BuildValueAdmission` retains private admission state across evaluation:
`BuildTimeValue` intentionally carries no nominal symbol handles after evaluation. If we decoded fields like `optimizations` or `x86_deployment_features` without first checking that the authored `Build` declaration resolved to the exact toolchain prelude symbols, a user-defined lookalike struct could smuggle unverified settings across the interpreter boundary. Retaining `BuildValueAdmission` from pre-evaluation `TypedTrees` guarantees that initial value construction and post-evaluation extraction agree on the exact admitted vocabulary.
*/

/*
Provenance:
- Configuration decoding (`EvaluatedBuildSettings`, `PrivilegedServicesGrants`, `ElfInterpreterSelections`): `src/build_execution/build_value/configuration.rs`.
- Initial `Build` value construction: `src/build_execution/build_value/initial.rs`.
- Optimization vocabulary admission and extraction: `src/build_execution/build_value/optimization.rs`.
- Toolchain `build` machine and facet queries: `src/build_execution/build_value/vocabulary.rs`.
*/

mod configuration;
mod initial;
mod optimization;
mod vocabulary;

pub use configuration::{EvaluatedBuildSettings, PrivilegedServicesGrants};
pub(super) use vocabulary::is_build_machine;

use super::BuildMachineFilesystemScope;
use build_time_evaluation::BuildTimeValue;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

#[derive(Debug, Clone, Copy)]
pub struct BuildValueAdmission {
    optimization: optimization::BuildOptimizationAdmission, // Admitted optimization field vocabulary.
    has_toolchain_build_activation: bool, // Whether the toolchain activation vocabulary is present.
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
