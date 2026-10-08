//! Compile shared source into independently checked target products.
//!
//! `compile` prepares the source set once, then runs each requested target
//! through the stage chain in `compile_target`, isolating one target's
//! failure from the others. Every step there is one call; the folders beside
//! this file own what each call does.

use crate::checked::{
    PreparedCheckedSource, check_prepared_source, prepare_checked_source, run_on_compile_thread,
};
use crate::native::produce_native_product;
use crate::terminal::produce_terminal_product;
use crate::{
    CheckedCompilation, CompileOutcomes, CompileReport, CompileRequest, CompileTargetOutcome,
};
use compilation_products::compile_request::{SharedCompileInputs, ValidatedTargetCompilation};
use compilation_products::{
    CheckedProductReport, CompileFailure, NativeProductRequest, PreparedBuilds,
    RequestedCompileProduct,
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
        // Psi 00-01: read, tokenize and parse the root's source set once.
        let source = prepare_checked_source(
            &shared.root_path,
            shared.package_sources.clone(),
            shared.timings,
        );
        let target_count = targets.len();
        let mut builds = PreparedBuilds::for_targets(target_count);
        let mut outcomes = Vec::with_capacity(target_count);
        // Checkpoint clones share immutable parsing. repeat_n moves the final
        // copy, so the last (and the only) target consumes the original arenas.
        let sources = std::iter::repeat_n(source, target_count);
        for (target, source) in targets.into_iter().zip(sources) {
            let profile = target.profile();
            let outcome =
                source.and_then(|source| compile_target(&shared, target, source, &mut builds));
            outcomes.push(CompileTargetOutcome::new(profile, outcome));
        }
        Ok(CompileOutcomes::new(outcomes))
    })
}

/// One target through the stage chain: assemble its sources and evaluate the
/// build (once for every target with equal build inputs), check, admit, then
/// produce the requested product.
fn compile_target(
    shared: &SharedCompileInputs,
    target: ValidatedTargetCompilation,
    source: PreparedCheckedSource,
    builds: &mut PreparedBuilds,
) -> Result<CompileReport, Vec<Diagnostic>> {
    let root_path = target.options().root_path.clone();
    let build_dir = target.options().build_dir();
    let child = target.checked_execution(&build_dir);
    let checked = check_prepared_source(&root_path, source, child, Some(builds), None)
        .map_err(CompileFailure::into_diagnostics)?
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
