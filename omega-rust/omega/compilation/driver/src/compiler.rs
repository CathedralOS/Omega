//! Check shared source once, then realize independent target products.
//!
//! `compile` assembles source, evaluates Build and completes authored/generated
//! typing and checking once. Each target borrows that judgment, settles its
//! execution, then matching entry closures share Terminal production. Each
//! target realizes its own native product. No realization reopens checking.

use crate::checked::{
    check_product_source, prepare_checked_source, prepare_product_source, run_on_compile_thread,
};
use crate::execution_settlement::SelectedProductTarget;
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
                        &mut psi_compiler::ProductSelections::default(),
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
                    outcome
                        .and_then(CompilationOutput::into_report)
                        .map_err(CompilationProductError::into_diagnostics),
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
) -> Result<ProductRequest, Vec<Diagnostic>> {
    let root_path = shared.root_path.clone();
    let realization =
        SelectedProductTarget::select(source, target.profile(), target.package_inputs(), false)?;

    let product = match shared.requested_product {
        RequestedCompileProduct::Check => CompilationProduct::Check,
        RequestedCompileProduct::TerminalArtifact => {
            CompilationProduct::Terminal(target.optimization_rollback().clone())
        }
        RequestedCompileProduct::NativeArtifact => {
            CompilationProduct::Native(target.into_native_product_request())
        }
    };
    Ok((realization, root_path, product, None))
}

/// The remaining work after source checking. Native authority has no meaning
/// at a check or Terminal stop, so only the native request carries it.
pub enum CompilationProduct {
    Check,
    Terminal(crate::OptimizationRollback),
    Native(NativeProductRequest),
    /// Describe a named checked machine without granting executable authority.
    Inspection(String),
}

impl CompilationProduct {
    pub fn optimization_rollback(&self) -> crate::OptimizationRollback {
        match self {
            Self::Terminal(rollback) => rollback.clone(),
            Self::Native(request) => request.optimization_rollback.clone(),
            Self::Check | Self::Inspection(_) => Default::default(),
        }
    }
}

/// Finished output; checked source and continuation state stay in the compiler.
pub enum CompilationOutput {
    Report(CompileReport),
    Inspection(terminal_codec::CanonicalTerminalArtifact),
}

impl CompilationOutput {
    pub fn into_report(self) -> Result<CompileReport, CompilationProductError> {
        match self {
            Self::Report(report) => Ok(report),
            Self::Inspection(_) => Err(CompilationProductError::Production(vec![
                Diagnostic::error("requested a report from an inspection product"),
            ])),
        }
    }

    pub fn into_inspection(
        self,
    ) -> Result<terminal_codec::CanonicalTerminalArtifact, CompilationProductError> {
        match self {
            Self::Inspection(artifact) => Ok(artifact),
            Self::Report(_) => Err(CompilationProductError::Production(vec![
                Diagnostic::error("requested an inspection from a compilation report"),
            ])),
        }
    }
}

#[derive(Debug)]
pub enum CompilationProductError {
    TrustAdmission(Vec<Diagnostic>),
    Production(Vec<Diagnostic>),
}

impl From<Vec<Diagnostic>> for CompilationProductError {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        Self::Production(diagnostics)
    }
}

impl CompilationProductError {
    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        match self {
            Self::TrustAdmission(diagnostics) | Self::Production(diagnostics) => diagnostics,
        }
    }
}

/// One selected application and the complete output/consent request.
type ProductRequest = (
    SelectedProductTarget,
    PathBuf,
    CompilationProduct,
    Option<package_evidence::permission_acceptance::TerminalPermissionAcceptance>,
);

struct PendingProduct {
    checked: CheckedCompilation,
    requested: CompilationProduct,
    settlement: CheckedProductReport,
    rollback: optimization_core::OptimizationRollbackSettlement,
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
        CompilationProduct::Check | CompilationProduct::Inspection(_) => &default_rollback,
        CompilationProduct::Terminal(rollback) => rollback,
        CompilationProduct::Native(request) => &request.optimization_rollback,
    };
    rollback.settle(&build.config.settings.optimizations)
}

/// Keep ordering and refusal local to each target while sharing portable work.
/// Selection settles first; the target-neutral artifact is produced for the
/// complete roster before any target's installation consumes it.
pub(crate) fn finish_compilations(
    requests: Vec<Result<ProductRequest, CompilationProductError>>,
) -> Vec<Result<CompilationOutput, CompilationProductError>> {
    let mut outcomes: Vec<_> = (0..requests.len()).map(|_| None).collect();
    let mut pending = Vec::new();
    for (request_index, request) in requests.into_iter().enumerate() {
        match request {
            Err(error) => outcomes[request_index] = Some(Err(error)),
            Ok((realization, _, CompilationProduct::Inspection(machine), _)) => {
                outcomes[request_index] = Some(
                    realization
                        .install()
                        .and_then(|checked| crate::inspection::inspect_terminal(checked, &machine))
                        .map(CompilationOutput::Inspection)
                        .map_err(CompilationProductError::Production),
                );
            }
            Ok((realization, root, requested, permissions)) => {
                if matches!(requested, CompilationProduct::Check)
                    || realization.custody.build_admission.artifact_only()
                {
                    // These stops do not need a Terminal program. Complete the
                    // actual requested report, without manufacturing an entry.
                    outcomes[request_index] = Some(
                        realization
                            .install()
                            .map_err(CompilationProductError::Production)
                            .and_then(|checked| {
                                admit_permissions(&checked, permissions.as_ref(), false)?;
                                prepare_product(checked, root, requested)
                            })
                            .and_then(|product| match product.settlement {
                                CheckedProductReport::Complete(report) => {
                                    Ok(CompilationOutput::Report(report))
                                }
                                _ => Err(CompilationProductError::Production(vec![
                                    Diagnostic::error(
                                        "non-executable product requested Terminal production",
                                    ),
                                ])),
                            }),
                    );
                } else {
                    let rollback = settle_product_rollback(&requested, &realization.custody.build);
                    pending.push((
                        request_index,
                        realization,
                        root,
                        requested,
                        rollback,
                        permissions,
                    ));
                }
            }
        }
    }
    let terminals = produce_retained_terminal_artifacts(
        pending
            .iter_mut()
            .map(
                |(_, realization, _, _, rollback, _)| TerminalProductRequest {
                    view: TerminalProductionView::selection(&mut realization.custody),
                    selections: rollback.effective(),
                },
            )
            .collect(),
    );
    for ((request_index, realization, root, requested, rollback, permissions), terminal) in
        pending.into_iter().zip(terminals)
    {
        outcomes[request_index] = Some(
            terminal
                .map_err(CompilationProductError::Production)
                .and_then(|produced| {
                    let checked = realization
                        .install()
                        .map_err(CompilationProductError::Production)?;
                    admit_permissions(&checked, permissions.as_ref(), true)?;
                    let product = prepare_product(checked, root, requested)?;
                    let terminal = finish_terminal_application(
                        &product.checked,
                        produced,
                        rollback.effective(),
                    )
                    .map_err(CompilationProductError::Production)?;
                    finish_product(product, terminal).map(CompilationOutput::Report)
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

fn admit_permissions(
    checked: &CheckedCompilation,
    permissions: Option<&package_evidence::permission_acceptance::TerminalPermissionAcceptance>,
    runtime_product: bool,
) -> Result<(), CompilationProductError> {
    if runtime_product
        && permissions.is_none()
        && checked.sources().package_identity().is_some()
        && !checked
            .execution()
            .selected_provider_plan_facts
            .assumed_bindings()
            .is_empty()
    {
        return Err(CompilationProductError::TrustAdmission(vec![
            Diagnostic::error(
                "assumed binding requires explicit accepted root Product risk rows; run omega update",
            ),
        ]));
    }
    if let Some(permissions) = permissions {
        permissions
            .validate(checked)
            .map_err(CompilationProductError::TrustAdmission)?;
    }
    Ok(())
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
        CompilationProduct::Inspection(_) => {
            return Err(CompilationProductError::Production(vec![
                Diagnostic::error("inspection cannot request executable product settlement"),
            ]));
        }
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
