//! Compile shared source into independently checked target products.
//!
//! `compile` assembles source, evaluates Build and completes authored/generated
//! typing once. Each target then settles its execution and produces its product.
//! Checking and Terminal production still need to move into the shared prefix;
//! provider metadata currently changes their typed inputs.

use crate::checked::{
    check_completed_source, prepare_checked_source, prepare_product_source, run_on_compile_thread,
};
use crate::native::produce_native_product;
use crate::terminal::produce_terminal_product;
use crate::{
    CheckedCompilation, CompileOutcomes, CompileReport, CompileRequest, CompileTargetOutcome,
};
use compilation_products::compile_request::{SharedCompileInputs, ValidatedTargetCompilation};
use compilation_products::{
    CheckedProductReport, CompileFailure, NativeProductRequest, PreparedProductSource,
    RequestedCompileProduct, SettledCheckReuse,
};
use diagnostics::Diagnostic;
use std::path::PathBuf;

pub(crate) mod package;

/// Compile every requested target, retaining failures alongside successful products.
/// Invalid requests reject before source acquisition. Shared preparation failures
/// are reported for every target. Publication remains a separate operation.
pub fn compile(request: CompileRequest) -> Result<CompileOutcomes, Vec<Diagnostic>> {
    run_on_compile_thread(move || {
        let (shared, targets) = request.validate_for_execution()?.into_parts();
        // Admit each target's generated-package custody without executing Build.
        // A rejected realization neither changes the common input nor prevents
        // another admitted realization from using it.
        let admissions: Vec<_> = targets
            .iter()
            .map(|target| {
                target
                    .checked_execution(&shared.build_directory)
                    .admit_package_inputs()
            })
            .collect();
        let source = targets
            .iter()
            .zip(&admissions)
            .find(|(_, admission)| admission.is_ok())
            .map(|(target, _)| {
                let parsed = prepare_checked_source(
                    &shared.root_path,
                    shared.package_sources.clone(),
                    shared.timings,
                )?;
                prepare_product_source(
                    &shared.root_path,
                    parsed,
                    target.checked_execution(&shared.build_directory),
                )
                .map_err(CompileFailure::into_diagnostics)
            });
        let target_count = targets.len();
        let mut outcomes = Vec::with_capacity(target_count);
        // No resumable frontend or Build reaches this loop. Target settlement
        // still needs private typed metadata; the final target consumes the
        // original storage rather than cloning another copy.
        let admitted_count = admissions
            .iter()
            .filter(|admission| admission.is_ok())
            .count();
        let mut sources = std::iter::repeat_n(source, admitted_count);
        for (target, admission) in targets.into_iter().zip(admissions) {
            let profile = target.profile();
            let outcome = admission.and_then(|()| {
                sources
                    .next()
                    .flatten()
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "admitted target has no shared source preparation",
                        )]
                    })?
                    .and_then(|source| compile_target(&shared, target, source))
            });
            outcomes.push(CompileTargetOutcome::new(profile, outcome));
        }
        Ok(CompileOutcomes::new(outcomes))
    })
}

/// One target settles the complete shared source, then produces its product.
fn compile_target(
    shared: &SharedCompileInputs,
    target: ValidatedTargetCompilation,
    source: PreparedProductSource,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let root_path = shared.root_path.clone();
    let checked = check_completed_source(
        source,
        target.profile(),
        target.package_inputs(),
        target.optimization_rollback(),
        false,
        SettledCheckReuse::default(),
    )
    .result
    .map_err(CompileFailure::into_diagnostics)?;

    let product = match shared.requested_product {
        RequestedCompileProduct::Check => CompilationProduct::Check,
        RequestedCompileProduct::TerminalArtifact => {
            CompilationProduct::Terminal(target.optimization_rollback().clone())
        }
        RequestedCompileProduct::NativeArtifact => {
            CompilationProduct::Native(target.into_native_product_request())
        }
    };
    finish_compilation(checked, root_path, product)
        .map_err(CompilationProductError::into_diagnostics)
}

/// The remaining work after source checking. Native authority has no meaning
/// at a check or Terminal stop, so only the native request carries it.
pub enum CompilationProduct {
    Check,
    Terminal(crate::OptimizationRollback),
    Native(NativeProductRequest),
}

#[derive(Debug)]
pub enum CompilationProductError {
    TrustAdmission(Vec<Diagnostic>),
    Production(Vec<Diagnostic>),
}

impl CompilationProductError {
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        match self {
            Self::TrustAdmission(diagnostics) | Self::Production(diagnostics) => diagnostics,
        }
    }
}

/// Continue the one compilation pipeline after package policy has accepted its
/// checked inputs. Both standalone and package compilation use this operation;
/// callers neither choose stage entrances nor assemble product reports.
/// This consumes the checkpoint without repeating checking or Build effects.
pub fn finish_compilation(
    checked: CheckedCompilation,
    root_path: PathBuf,
    requested: CompilationProduct,
) -> Result<CompileReport, CompilationProductError> {
    checked
        .admit_trust()
        .map_err(CompilationProductError::TrustAdmission)?;
    let default_rollback = crate::OptimizationRollback::default();
    let (kind, rollback) = match &requested {
        CompilationProduct::Check => (RequestedCompileProduct::Check, &default_rollback),
        CompilationProduct::Terminal(rollback) => {
            (RequestedCompileProduct::TerminalArtifact, rollback)
        }
        CompilationProduct::Native(request) => (
            RequestedCompileProduct::NativeArtifact,
            &request.optimization_rollback,
        ),
    };
    let settlement = checked
        .prepare_product_report(root_path, kind, rollback)
        .map_err(CompilationProductError::Production)?;
    let result = match (settlement, requested) {
        (CheckedProductReport::Complete(report), _) => Ok(report),
        // Psi 05-07: lower, optimize and publish the Terminal artifact.
        (CheckedProductReport::Terminal(settlement), CompilationProduct::Terminal(rollback)) => {
            produce_terminal_product(checked, &rollback).and_then(|produced| {
                settlement
                    .with_timings(produced.timings)
                    .finish_terminal(produced.artifact, produced.rollback_receipt)
            })
        }
        // The same retained Terminal boundary, then Omega 00-09 and image emission.
        (CheckedProductReport::Native(settlement), CompilationProduct::Native(request)) => {
            produce_native_product(request, checked).and_then(|produced| {
                settlement
                    .with_timings(produced.timings)
                    .finish_native(produced.artifact, produced.rollback_receipt)
                    .and_then(|report| {
                        report
                            .with_runtime_provisions(produced.runtime_provisions)
                            .map_err(|message| vec![Diagnostic::error(message)])
                    })
            })
        }
        _ => Err(vec![Diagnostic::error(
            "product settlement disagrees with the requested compilation stop",
        )]),
    };
    result.map_err(CompilationProductError::Production)
}
