//! The retired checker no longer projects or settles Build source. A future
//! structural Build-source producer must retain the actual selected source
//! context, declaration authority and pending constant/provider work before
//! passing its frozen owner to AcceptedTerminalSource. An unchecked whole-tree
//! clone or an empty substitute roster cannot supply that custody.

use diagnostics::Diagnostic;
use package_compilation::PackageCompilationInputs;
use psi_compiler::RetainedCheck;

pub fn settle_rejoined_build_slice(
    _input: psi_compiler::BuildSourceInput<'_>,
    _entry: symbols::SymbolHandle,
    _packages: Option<&PackageCompilationInputs>,
    _dependency_activation: bool,
    _execution_profile: Option<target::TargetProfile>,
) -> Result<(), Vec<Diagnostic>> {
    Err(unavailable_settlement())
}

pub(crate) fn prepare_admitted_build(
    _typed: &mut typed_trees::TypedTrees,
    _pre_checks: &mut Vec<build_time_evaluation::PreCheckEvaluation>,
    _entry: symbols::SymbolHandle,
    _packages: Option<&PackageCompilationInputs>,
    _dependency_activation: bool,
    _execution_profile: Option<target::TargetProfile>,
    _timings: &mut artifacts::compile_timings::CompileTimings,
    _priors: &[&RetainedCheck],
    _retain: bool,
    _checked_slices: Option<&dyn psi_compiler::CheckedSliceStore>,
) -> Result<
    (
        std::sync::Arc<psi_compiler::AcceptedTerminalSource>,
        super::provider_execution::BuildProviderPlans,
        Option<RetainedCheck>,
        bool,
    ),
    Vec<Diagnostic>,
> {
    Err(unavailable_settlement())
}

fn unavailable_settlement() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "no product route yet: structural Build source projection, declaration authority and constant/provider settlement custody are unavailable after retiring the old checker",
    )]
}
