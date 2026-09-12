//! The production compiler coordinator.
//!
//! Request validation, checking, and product selection stay visible here.
//! Target batches reuse this continuation; phase algorithms and publication
//! remain with their named owners.

use diagnostics::Diagnostic;
use request::ValidatedTargetCompilation;
use std::path::PathBuf;

mod admission;
pub(crate) mod execution;
mod intrinsic_settlements;
mod native;
mod native_checked;
mod optimization;
mod options;
mod package;
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
    CompileOutcomes, CompileRequest, CompileTargetOutcome, ExplicitTargetSet,
    RequestedCompileProduct, TargetCompileConfiguration,
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

    pub fn compile(self, request: CompileRequest) -> Result<CompileOutcomes, Vec<Diagnostic>> {
        execution::run_on_compile_thread(move || compile_request(request))
    }
}

/// Execute one typed production compiler request.
pub fn compile(request: CompileRequest) -> Result<CompileOutcomes, Vec<Diagnostic>> {
    Compiler::new().compile(request)
}

/// Drive the one production route and stop at the requested product.
///
/// Check, Terminal Psi, and retained native artifacts share one checked-Psi
/// frontend and differ only in how far the result proceeds.
fn compile_request(request: CompileRequest) -> Result<CompileOutcomes, Vec<Diagnostic>> {
    let mut request = request.validate_for_execution()?;
    let prepared = crate::pipeline::checked_entry::PreparedCheckedSource::prepare(
        &request.shared.root_path,
        request
            .targets
            .first()
            .and_then(ValidatedTargetCompilation::package_inputs),
    );
    let prepared = match prepared {
        Ok(prepared) => prepared,
        Err(diagnostics) => {
            return Ok(CompileOutcomes::new(
                request
                    .targets
                    .into_iter()
                    .map(|target| {
                        CompileTargetOutcome::new(target.profile, Err(diagnostics.clone()))
                    })
                    .collect(),
            ));
        }
    };
    let finish: fn(
        ValidatedTargetCompilation,
        crate::pipeline::CheckedCompilation,
    ) -> Result<CompileReport, Vec<Diagnostic>> = match request.shared.requested_product {
        RequestedCompileProduct::Check => checked_report,
        RequestedCompileProduct::TerminalArtifact => terminal_report,
        RequestedCompileProduct::NativeArtifact => {
            return native::compile_targets(request.targets, prepared);
        }
    };
    let last = request
        .targets
        .pop()
        .ok_or_else(|| vec![Diagnostic::error("validated compilation lost every target")])?;
    let compile_target = |target: ValidatedTargetCompilation, prepared| {
        let profile = target.profile;
        let result = check_request(&target, prepared).and_then(|(checked, trust_settlement)| {
            finish(target, checked)
                .map(|report| report.with_trust_admission_settlement(trust_settlement))
        });
        CompileTargetOutcome::new(profile, result)
    };
    let mut outcomes = request
        .targets
        .into_iter()
        .map(|target| compile_target(target, prepared.clone()))
        .collect::<Vec<_>>();
    // The last (including only) child owns the frontier, allowing its underlying
    // arenas to move rather than copying because of a retained coordinator owner.
    outcomes.push(compile_target(last, prepared));
    Ok(CompileOutcomes::new(outcomes))
}

fn check_request(
    request: &ValidatedTargetCompilation,
    prepared: crate::pipeline::checked_entry::PreparedCheckedSource,
) -> Result<
    (
        crate::pipeline::CheckedCompilation,
        TrustAdmissionSettlement,
    ),
    Vec<Diagnostic>,
> {
    let checked = prepared.compile_for_terminal(request.options(), request.package_inputs())?;
    let admission = admit_checked_compilation(&checked, request.accepted_trust_admissions())?;
    admission.write_observations(request.options(), request.artifact_policy())?;
    let trust_settlement = admission.into_settlement();
    Ok((checked, trust_settlement))
}

fn checked_report(
    request: ValidatedTargetCompilation,
    checked: crate::pipeline::CheckedCompilation,
) -> Result<CompileReport, Vec<Diagnostic>> {
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
    request: ValidatedTargetCompilation,
    checked: crate::pipeline::CheckedCompilation,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let profile = request.configuration.terminal_admission_profile;
    retained_terminal_report(
        request.options.root_path,
        checked,
        &profile,
        false,
        &request.configuration.optimization_rollback,
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
