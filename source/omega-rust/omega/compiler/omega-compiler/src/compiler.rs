//! The production compiler coordinator.
//!
//! This file intentionally declares only [`Compiler`]. Request and
//! host-execution infrastructure live beneath it; language and target semantics
//! belong to the stage crates the coordinator invokes.

use psi_diagnostics::Diagnostic;

mod driver;
pub(crate) mod execution;
mod intrinsic_settlements;
mod native_checked;
mod optimization;
mod options;
pub(crate) use omega_compilation_report as report;
mod request;
mod terminal_authority_permissions;
mod terminal_native_realization;
mod terminal_product;

pub use omega_trust_model::{TrustAdmission, TrustAdmissionSettlement};
pub use optimization::{OptimizationRollback, OptimizationRollbackInputError};
pub use options::{ArtifactEmissionPolicy, CompileOptions};
pub use report::{
    CompileOutputKind, CompileReport, ExecutablePublicationDestination,
    ExecutablePublicationReceipt, FinalRealizationEvidenceError, OptimizationRollbackReceipt,
    ProductionArtifactIdentity, ProductionCompilationManifest,
    ProductionCompilationManifestIdentity, ProductionCompilationSubject, RetainedNativeArtifact,
};
pub use request::{
    CompileRequest, ExactTargetCompileOutcome, ExplicitTargetSet, MultiTargetCompileOutcomes,
    MultiTargetCompileRequest, RequestedCompileProduct,
};
pub use terminal_native_realization::{
    SourceEvaluatedImportSettlement,
    realize_retained_terminal_artifact_with_source_evaluated_imports,
    realize_retained_terminal_artifact_with_source_evaluated_imports_and_policy,
    realize_retained_terminal_artifact_with_source_evaluated_imports_and_policy_for_image,
};

/// The reusable production compiler coordinator.
#[derive(Debug, Default, Clone, Copy)]
pub struct Compiler;

impl Compiler {
    pub const fn new() -> Self {
        Self
    }

    pub fn compile(self, request: CompileRequest) -> Result<CompileReport, Vec<Diagnostic>> {
        execution::run_on_compile_thread(move || driver::compile(request))
    }

    /// Compile one caller-supplied canonical target set while retaining every
    /// exact child's ordinary result, including failures.
    pub fn compile_targets(
        self,
        request: MultiTargetCompileRequest,
    ) -> Result<MultiTargetCompileOutcomes, Vec<Diagnostic>> {
        execution::run_on_compile_thread(move || driver::compile_targets(request))
    }
}

/// Execute one typed production compiler request.
pub fn compile(request: CompileRequest) -> Result<CompileReport, Vec<Diagnostic>> {
    Compiler::new().compile(request)
}

/// Execute one explicit multi-target compiler request.
pub fn compile_targets(
    request: MultiTargetCompileRequest,
) -> Result<MultiTargetCompileOutcomes, Vec<Diagnostic>> {
    Compiler::new().compile_targets(request)
}

/// Continue one package-manager-owned checked production to a retained
/// Terminal report without rerunning its build machine or source discovery.
pub fn retained_terminal_report_from_checked_package(
    root_path: std::path::PathBuf,
    checked: crate::CheckedCompilation,
    profile: psi_proof_admission::AdmissionProfile,
) -> Result<CompileReport, Vec<Diagnostic>> {
    execution::run_on_compile_thread(move || {
        driver::retained_terminal_report_from_checked_package(root_path, checked, &profile)
    })
}

/// Reconstruct checked trust obligations and optional compiler observations
/// for an already-checked package production.
///
/// Package orchestration uses this before consuming its retained checked root
/// into Terminal/native production. The accepted set remains explicit and the
/// returned settlement grants no package-review authority.
pub fn report_checked_compilation_observations(
    options: &CompileOptions,
    artifact_policy: ArtifactEmissionPolicy,
    accepted_trust_admissions: &[TrustAdmission],
    checked: &crate::CheckedCompilation,
) -> Result<TrustAdmissionSettlement, Vec<Diagnostic>> {
    crate::pipeline::reporting::report_checked_observations(
        crate::pipeline::reporting::CheckedObservationInput {
            options,
            artifact_policy,
            accepted_trust_admissions,
            checked,
        },
    )
}

#[cfg(test)]
#[path = "compiler/tests.rs"]
mod tests;
