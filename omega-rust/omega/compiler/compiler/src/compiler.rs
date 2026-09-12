//! The production compiler coordinator.
//!
//! Request validation, checking, and product selection stay visible here.
//! Target batches reuse this continuation; phase algorithms and publication
//! remain with their named owners.

use diagnostics::Diagnostic;
use request::ValidatedCompileRequest;
use std::path::PathBuf;

mod admission;
pub(crate) mod execution;
mod intrinsic_settlements;
mod native;
mod native_checked;
mod optimization;
mod options;
mod package;
mod targets;
pub(crate) use compilation_report as report;
mod request;
mod terminal_authority_permissions;
mod terminal_native_realization;
mod terminal_product;
pub use terminal_product::validate_lowered_ieee_float_comparison_custody;

pub use admission::{CheckedAdmission, admit_checked_compilation};
pub use optimization::{OptimizationRollback, OptimizationRollbackInputError};
pub use options::{ArtifactEmissionPolicy, CompileOptions};
pub use package::retained_terminal_report_from_checked_package;
pub use report::{
    CompileOutputKind, CompileReport, ExecutablePublicationReceipt, FinalRealizationEvidenceError,
    OptimizationRollbackReceipt, ProductionArtifactIdentity, ProductionCompilationManifest,
    ProductionCompilationManifestIdentity, ProductionCompilationSubject, RetainedNativeArtifact,
};
pub use request::{
    CompileRequest, ExactTargetCompileOutcome, ExplicitTargetSet, MultiTargetCompileOutcomes,
    MultiTargetCompileRequest, RequestedCompileProduct,
};
pub use terminal_native_realization::{
    RetainedNativeRealizationRequest, SourceEvaluatedImportSettlement,
    realize_retained_native_artifact,
};
pub use trust_model::{TrustAdmission, TrustAdmissionSettlement};

/// The reusable production compiler coordinator.
#[derive(Debug, Default, Clone, Copy)]
pub struct Compiler;

impl Compiler {
    pub const fn new() -> Self {
        Self
    }

    pub fn compile(self, request: CompileRequest) -> Result<CompileReport, Vec<Diagnostic>> {
        execution::run_on_compile_thread(move || compile_request(request))
    }

    /// Compile one caller-supplied canonical target set while retaining every
    /// exact child's ordinary result, including failures.
    pub fn compile_targets(
        self,
        request: MultiTargetCompileRequest,
    ) -> Result<MultiTargetCompileOutcomes, Vec<Diagnostic>> {
        execution::run_on_compile_thread(move || targets::compile_targets(request))
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

/// Drive the one production route and stop at the requested product.
///
/// Check, Terminal Psi, and retained native artifacts share one checked-Psi
/// frontend and differ only in how far the result proceeds.
fn compile_request(request: CompileRequest) -> Result<CompileReport, Vec<Diagnostic>> {
    let request = request.validate_for_execution()?;
    compile_validated(request, None)
}

fn compile_validated(
    request: ValidatedCompileRequest,
    prepared: Option<&crate::pipeline::checked_entry::PreparedCheckedSource>,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let (checked, trust_settlement) = check_request(&request, prepared)?;
    let finalize_report =
        |report: CompileReport| report.with_trust_admission_settlement(trust_settlement);
    match request.requested_product() {
        RequestedCompileProduct::Check => checked_report(request, &checked).map(finalize_report),
        RequestedCompileProduct::TerminalArtifact => {
            terminal_report(request, checked).map(finalize_report)
        }
        RequestedCompileProduct::NativeArtifact => {
            native::compile(request, checked).map(finalize_report)
        }
    }
}

fn check_request(
    request: &ValidatedCompileRequest,
    prepared: Option<&crate::pipeline::checked_entry::PreparedCheckedSource>,
) -> Result<
    (
        crate::pipeline::CheckedCompilation,
        TrustAdmissionSettlement,
    ),
    Vec<Diagnostic>,
> {
    let checked = match prepared {
        Some(prepared) => prepared
            .clone()
            .compile_for_terminal(request.options(), request.package_inputs())?,
        None => crate::pipeline::checked_entry::compile_to_checked_for_terminal(
            request.options(),
            request.package_inputs(),
        )?,
    };
    let admission = admit_checked_compilation(&checked, request.accepted_trust_admissions())?;
    admission.write_observations(request.options(), request.artifact_policy())?;
    let trust_settlement = admission.into_settlement();
    Ok((checked, trust_settlement))
}

fn checked_report(
    request: ValidatedCompileRequest,
    checked: &crate::pipeline::CheckedCompilation,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let request = request.into_inner();
    CompileReport::checked(
        request.options.root_path,
        checked.source_file_count(),
        false,
        CompileOutputKind::CheckOnly,
        None,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}

fn terminal_report(
    request: ValidatedCompileRequest,
    checked: crate::pipeline::CheckedCompilation,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let request = request.into_inner();
    let profile = request.terminal_admission_profile;
    retained_terminal_report(
        request.options.root_path,
        checked,
        &profile,
        false,
        &request.optimization_rollback,
    )
}

/// Consume one already-checked package production into its retained Terminal
/// report without reopening source discovery or build evaluation.
///
/// Package review owns construction of the checked value and later admission
/// owns native realization. This handoff only closes the compiler-owned
/// checked-to-Terminal boundary.
fn compile_package_terminal_report(
    root_path: PathBuf,
    checked: crate::pipeline::CheckedCompilation,
    profile: &proof_admission::AdmissionProfile,
) -> Result<CompileReport, Vec<Diagnostic>> {
    retained_terminal_report(
        root_path,
        checked,
        profile,
        true,
        &OptimizationRollback::default(),
    )
}

fn retained_terminal_report(
    root_path: PathBuf,
    checked: crate::pipeline::CheckedCompilation,
    profile: &proof_admission::AdmissionProfile,
    require_package_custody: bool,
    rollback: &OptimizationRollback,
) -> Result<CompileReport, Vec<Diagnostic>> {
    if require_package_custody {
        checked.verify_current_source_consumption()?;
    }
    let production_subject = crate::pipeline::reporting::project_production_subject(&checked)?;
    if require_package_custody && production_subject.is_none() {
        return Err(vec![Diagnostic::error(
            "reviewed package Terminal production requires package-aware checked custody",
        )]);
    }
    let source_file_count = checked.source_file_count();
    let rollback = rollback.settle(checked.optimization_selections());
    let artifact = terminal_product::produce_retained_terminal_artifact(
        &checked,
        profile,
        rollback.effective(),
    )?;
    CompileReport::from_retained_terminal_artifact(
        root_path,
        source_file_count,
        artifact,
        production_subject,
    )
    .and_then(|report| report.with_terminal_optimization_rollback(rollback.into_receipt()))
    .map_err(|message| vec![Diagnostic::error(message)])
}

#[cfg(test)]
#[path = "compiler/tests.rs"]
mod tests;
