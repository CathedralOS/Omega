//! Provider realization settles the target, calling plans and installation reach of an accepted program.
#![forbid(unsafe_code)]

// Canonical Terminal/native plan data and Build-row receipt normalization.
// Source inspection and nominations belong to the source coordinator; missing
// provider authority never becomes a source fallback or a fabricated proof.

pub mod calling_policy_plans;
pub use direct_boundary_bindings::{compiler_primitive_schemas, hosted_requirement_schema};
pub mod native_bindings;
pub(crate) use ::wire::digest_encoder;
pub mod program_entry;
mod provider_planning;
pub mod target_supply;

pub use provider_planning::provider_rows::{
    DirectBoundaryInput, DirectBoundaryMechanism, direct_compiler_primitive_leaves,
};
pub use provider_planning::{
    CompilerIntrinsicExecutionIdentity, CompilerNumericType, CompilerPrimitiveFloatBinaryOperation,
    CompilerPrimitiveIntegerComparisonOperation,
};
pub use semantic_vocabulary::source_selections::CompositionMode;

pub use terminal_semantics::logical_providers::satisfies_plan_name;

mod execution;
mod provider_execution_inputs;
pub use host_provider_inputs::ProviderExecutionOrigins;
pub use provider_execution_inputs::plan_provider_execution_inputs;
mod folded_entry;
mod package_review;
pub use package_review::{
    PackageTargetReview, ReviewedNativeBinding, ReviewedNativeBindings, ReviewedNumericBinding,
    ReviewedProviderRow,
};
mod selected_execution;
pub mod native_execution;
pub use execution::{
    NativeProgramPlanningError, plan_target_execution,
    plan_unscoped_execution,
};
pub use folded_entry::FoldedTargetProgram;
pub use provider_correspondence::trust;
pub use selected_execution::{SelectedExecution, SelectedProviderResult};
pub use target_supply::SelectedProviderRealization;

#[cfg(feature = "test-support")]
pub mod test_support {
    use super::{SelectedExecution, target_supply::RealizedTargetSupply};
    pub fn select_realization(
        module: &terminal_psi::TerminalModule,
        profile: Option<target::TargetProfile>,
        realization: RealizedTargetSupply,
    ) -> Result<SelectedExecution, Vec<diagnostics::Diagnostic>> {
        let program = terminal_codec::terminal_psi_identity(module)
            .map_err(|error| vec![diagnostics::Diagnostic::error(error.to_string())])?;
        realization
            .require_program(program)
            .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])?;
        SelectedExecution::from_realization(module, profile, realization)
    }
}

mod provider_selection;
pub use provider_selection::{PublishedSelection, select_published_providers};
