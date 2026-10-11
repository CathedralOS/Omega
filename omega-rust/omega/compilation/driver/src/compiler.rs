//! Check shared source once, then realize independent target products.
//!
//! `compile` assembles source, evaluates Build and completes authored/generated
//! typing and checking once. Each target borrows that judgment, settles its
//! execution, then matching entry closures share Terminal production. Each
//! target realizes its own native product. No realization reopens checking.

use crate::checked::{
    check_product_source, prepare_checked_source, prepare_product_source, run_on_compile_thread,
};
use crate::execution_settlement::{
    SelectedProviderCustody, install_selected_target, select_product_providers,
};
use crate::native::produce_native_product;
use crate::terminal::{
    TerminalProductRequest, TerminalProductionView, finish_terminal_application,
    produce_retained_terminal_artifacts,
};
use crate::{
    CheckedCompilation, CompileOutcomes, CompileReport, CompileRequest, CompileTargetOutcome,
};
use compilation_products::compile_request::{SharedCompileInputs, ValidatedTargetCompilation};
use compilation_products::{
    CheckedProductReport, CheckedProductSource, CompileFailure, NativeProductRequest,
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
                .and_then(|source| {
                    check_product_source(
                        source,
                        target.package_inputs(),
                        target.optimization_rollback(),
                        SettledCheckReuse::default(),
                    )
                })
            });
        let target_count = targets.len();
        let mut profiles = Vec::with_capacity(target_count);
        let mut products = Vec::with_capacity(target_count);
        // All targets borrow the same checked arenas. Neither frontend work,
        // Build, nor a mutable typed copy reaches this loop.
        for (target, admission) in targets.into_iter().zip(admissions) {
            let profile = target.profile();
            let outcome = admission.and_then(|()| {
                source
                    .as_ref()
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "admitted target has no shared source preparation",
                        )]
                    })?
                    .as_ref()
                    .map_err(Clone::clone)
                    .and_then(|source| prepare_target(&shared, target, source))
            });
            profiles.push(profile);
            products.push(outcome.map_err(CompilationProductError::Production));
        }
        // Settle every consumer before producing shared Psi. Native realization
        // begins only after those shared products have been published.
        let outcomes = profiles
            .into_iter()
            .zip(finish_compilations(products))
            .map(|(profile, outcome)| {
                CompileTargetOutcome::new(
                    profile,
                    outcome.map_err(CompilationProductError::into_diagnostics),
                )
            })
            .collect();
        Ok(CompileOutcomes::new(outcomes))
    })
}

/// Select one target's providers against shared checked source without
/// starting production or installation. The custody record is what Terminal
/// production reads before the selected execution settles beside the artifact.
fn prepare_target(
    shared: &SharedCompileInputs,
    target: ValidatedTargetCompilation,
    source: &CheckedProductSource,
) -> Result<(PendingRealization, PathBuf, CompilationProduct), Vec<Diagnostic>> {
    let root_path = shared.root_path.clone();
    let custody = select_product_providers(source, target.profile(), target.package_inputs())?;
    let realization = PendingRealization::Selection {
        custody,
        retained_checks: source.retained_checks.clone(),
        package_inputs: target.package_inputs().cloned(),
        permit_unsettled_fused_service_fields: false,
    };

    let product = match shared.requested_product {
        RequestedCompileProduct::Check => CompilationProduct::Check,
        RequestedCompileProduct::TerminalArtifact => {
            CompilationProduct::Terminal(target.optimization_rollback().clone())
        }
        RequestedCompileProduct::NativeArtifact => {
            CompilationProduct::Native(target.into_native_product_request())
        }
    };
    Ok((realization, root_path, product))
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
    finish_compilations(vec![Ok((
        PendingRealization::Installed(checked),
        root_path,
        requested,
    ))])
        .into_iter()
        .next()
        .ok_or_else(|| {
            CompilationProductError::Production(vec![Diagnostic::error(
                "compilation lost its product outcome",
            )])
        })?
}

struct PendingProduct {
    checked: CheckedCompilation,
    requested: CompilationProduct,
    settlement: CheckedProductReport,
    rollback: optimization_core::OptimizationRollbackSettlement,
}

/// How a target's checked compilation arrives at realization: a fresh
/// selection custody the invocation installs after Terminal production, or an
/// already-installed product the package route sealed itself.
enum PendingRealization {
    Selection {
        custody: SelectedProviderCustody,
        retained_checks: compilation_products::RetainedCompilationChecks,
        package_inputs: Option<package_compilation::PackageCompilationInputs>,
        permit_unsettled_fused_service_fields: bool,
    },
    Installed(CheckedCompilation),
}

impl PendingRealization {
    fn view(&mut self) -> TerminalProductionView<'_> {
        match self {
            Self::Selection { custody, .. } => TerminalProductionView::Selection(custody),
            Self::Installed(checked) => TerminalProductionView::Installed(checked),
        }
    }

    fn build(&self) -> &compilation_products::ComputedBuildConfig {
        match self {
            Self::Selection { custody, .. } => &*custody.build,
            Self::Installed(checked) => checked.build(),
        }
    }

    fn install(self) -> Result<CheckedCompilation, Vec<Diagnostic>> {
        match self {
            Self::Installed(checked) => Ok(checked),
            Self::Selection {
                custody,
                retained_checks,
                package_inputs,
                permit_unsettled_fused_service_fields,
            } => install_selected_target(
                custody,
                retained_checks,
                package_inputs.as_ref(),
                permit_unsettled_fused_service_fields,
            ),
        }
    }
}

/// The settled optimization rollback production consumes before installation
/// completes. Identical derivation to `prepare_product`'s post-install settle —
/// the build rows it reads are shared custody either way.
fn settle_product_rollback(
    requested: &CompilationProduct,
    build: &compilation_products::ComputedBuildConfig,
) -> optimization_core::OptimizationRollbackSettlement {
    let default_rollback = crate::OptimizationRollback::default();
    let rollback = match requested {
        CompilationProduct::Check => &default_rollback,
        CompilationProduct::Terminal(rollback) => rollback,
        CompilationProduct::Native(request) => &request.optimization_rollback,
    };
    rollback.settle(&build.config.settings.optimizations)
}

/// Keep ordering and refusal local to each target while sharing portable work.
/// Selection settles first; the target-neutral artifact is produced for the
/// complete roster before any target's installation consumes it.
fn finish_compilations(
    requests: Vec<
        Result<(PendingRealization, PathBuf, CompilationProduct), CompilationProductError>,
    >,
) -> Vec<Result<CompileReport, CompilationProductError>> {
    let mut outcomes: Vec<_> = (0..requests.len()).map(|_| None).collect();
    let mut pending = Vec::new();
    for (request_index, request) in requests.into_iter().enumerate() {
        match request {
            Err(error) => outcomes[request_index] = Some(Err(error)),
            Ok((realization, root, requested)) => {
                if matches!(requested, CompilationProduct::Check) {
                    // Check products produce nothing: install now, keep the
                    // report, and never join the production roster.
                    let prepared = realization
                        .install()
                        .map_err(CompilationProductError::Production)
                        .and_then(|checked| prepare_product(checked, root.clone(), requested));
                    match prepared {
                        Ok(product) => {
                            if let CheckedProductReport::Complete(report) = product.settlement {
                                outcomes[request_index] = Some(Ok(report));
                            } else {
                                let rollback = product.rollback;
                                pending.push((
                                    request_index,
                                    PendingRealization::Installed(product.checked),
                                    root,
                                    product.requested,
                                    rollback,
                                ));
                            }
                        }
                        Err(error) => outcomes[request_index] = Some(Err(error)),
                    }
                } else {
                    let rollback = settle_product_rollback(&requested, realization.build());
                    pending.push((request_index, realization, root, requested, rollback));
                }
            }
        }
    }
    let terminals = produce_retained_terminal_artifacts(
        pending
            .iter_mut()
            .map(|(_, realization, _, _, rollback)| TerminalProductRequest {
                view: realization.view(),
                selections: rollback.effective(),
            })
            .collect(),
    );
    for ((request_index, realization, root, requested, rollback), terminal) in
        pending.into_iter().zip(terminals)
    {
        outcomes[request_index] = Some(
            terminal
                .map_err(CompilationProductError::Production)
                .and_then(|produced| {
                    let checked =
                        realization.install().map_err(CompilationProductError::Production)?;
                    let product = prepare_product(checked, root, requested)?;
                    let terminal = finish_terminal_application(
                        &product.checked,
                        produced,
                        rollback.effective(),
                    )
                    .map_err(CompilationProductError::Production)?;
                    finish_product(product, terminal)
                }),
        );
    }
    outcomes
        .into_iter()
        .map(|outcome| {
            outcome.unwrap_or_else(|| {
                Err(CompilationProductError::Production(vec![
                    Diagnostic::error("compilation lost its target outcome"),
                ]))
            })
        })
        .collect()
}

fn prepare_product(
    checked: CheckedCompilation,
    root_path: PathBuf,
    requested: CompilationProduct,
) -> Result<PendingProduct, CompilationProductError> {
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
    let rollback = rollback.settle(&checked.build().config.settings.optimizations);
    Ok(PendingProduct {
        checked,
        requested,
        settlement,
        rollback,
    })
}

fn finish_product(
    product: PendingProduct,
    terminal: compilation_products::RetainedTerminalArtifact,
) -> Result<CompileReport, CompilationProductError> {
    let PendingProduct {
        checked,
        requested,
        settlement,
        rollback,
    } = product;
    let timings = checked.timings().phases().to_vec();
    let result = match (settlement, requested) {
        (CheckedProductReport::Terminal(settlement), CompilationProduct::Terminal(_)) => settlement
            .with_timings(timings)
            .finish_terminal(terminal, rollback.into_receipt()),
        // Omega 00-09 and image emission consume the same Terminal boundary.
        (CheckedProductReport::Native(settlement), CompilationProduct::Native(request)) => {
            produce_native_product(request, &checked, terminal, rollback.effective()).and_then(
                |produced| {
                    settlement
                        .with_timings(timings)
                        .finish_native(produced.artifact, rollback.into_receipt())
                        .and_then(|report| {
                            report
                                .with_runtime_provisions(produced.runtime_provisions)
                                .map_err(|message| vec![Diagnostic::error(message)])
                        })
                },
            )
        }
        _ => Err(vec![Diagnostic::error(
            "product settlement disagrees with the requested compilation stop",
        )]),
    };
    result.map_err(CompilationProductError::Production)
}
