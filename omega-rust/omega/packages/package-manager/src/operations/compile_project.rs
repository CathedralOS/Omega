//! Native production for one already-prepared local project.
//!
//! An absent lock is a development compilation, not missing acceptance. It
//! uses the same checked package route with no retained bindings or restricted
//! build grants. An existing lock must still match the source, compiler and
//! target; its permission rows are checked before producing the native product.

use super::accepted_compilation::{AcceptedCompilationRequest, compile_accepted_project};

use super::prepare_project::PreparedProjectTarget;
use super::{PrepareLocalProjectError, PreparedLocalProject};
use crate::build_session::PackageSourcePreparation;
use crate::declarations::PackageKey;
use crate::review::CompileResolvedPackageReviewsError;
use compiler::{CompileReport, OptimizationRollback};
use diagnostics::Diagnostic;
use std::fmt;
use std::path::{Path, PathBuf};
use terminal_psi_to_abstract_operations::provider_admission::{
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy, current_terminal_authority_policy,
};

/// Complete policy and output input for one package-aware native production.
///
/// Construction defaults to the toolchain's mechanism-classification policy
/// and no receiving permission policy. An absent receiving policy is not a
/// deny-all or allow-all policy: production then makes no receiver-admission
/// claim, never fabricates receiver rows from accepted package evidence, and
/// the emitted artifact cannot satisfy an explicit admission replay. Callers
/// may replace the mechanism policy or explicitly supply a receiving policy;
/// when a lock exists, package acceptance is checked against its exact target.
pub struct PreparedLocalProjectNativeRequest {
    prepared: PreparedLocalProject,
    build_dir: PathBuf,
    target_profiles: Vec<target::TargetProfile>,

    optimization_rollback: OptimizationRollback,
    build_snapshot: Option<build_evaluation::BuildSnapshotRequest>,
    terminal_authority_policy: TerminalAuthorityPolicy,
    receiving_terminal_authority_permission_policy: Option<TerminalAuthorityPermissionPolicy>,
    timings: bool,
}

impl PreparedLocalProjectNativeRequest {
    pub fn new(
        prepared: PreparedLocalProject,
        build_dir: impl Into<PathBuf>,
        target_profile: target::TargetProfile,
    ) -> Self {
        Self::for_targets(prepared, build_dir, vec![target_profile])
    }

    /// One native production request covering several exact targets. Their
    /// prepared source shares one candidate preparation, so each package's
    /// binding-independent parse frontier is built once for the union; each
    /// target still produces under `build_dir/<target>` with its own accepted
    /// consent.
    pub fn for_targets(
        prepared: PreparedLocalProject,
        build_dir: impl Into<PathBuf>,
        target_profiles: Vec<target::TargetProfile>,
    ) -> Self {
        Self {
            prepared,
            build_dir: build_dir.into(),
            target_profiles,

            optimization_rollback: OptimizationRollback::default(),
            build_snapshot: None,
            terminal_authority_policy: current_terminal_authority_policy(),
            receiving_terminal_authority_permission_policy: None,
            timings: false,
        }
    }

    /// Record per-stage timings on the produced report. Off by default: the
    /// ladder measures nothing until a caller asks for it.
    pub fn with_timings(mut self, timings: bool) -> Self {
        self.timings = timings;
        self
    }

    pub fn with_optimization_rollback(mut self, rollback: OptimizationRollback) -> Self {
        self.optimization_rollback = rollback;
        self
    }

    /// Select the root build's input inventory and output obligations. Dependency
    /// builds retain their independently captured package inventories.
    pub fn with_build_snapshot(mut self, snapshot: build_evaluation::BuildSnapshotRequest) -> Self {
        self.build_snapshot = Some(snapshot);
        self
    }

    /// Supply the mechanism-classification policy. Accepted package evidence
    /// never manufactures these rows. Receiver admission and requested physical
    /// exclusions require complete classification; ordinary production retains
    /// unclassified mechanisms without claiming either guarantee.
    pub fn with_terminal_authority_policy(mut self, policy: TerminalAuthorityPolicy) -> Self {
        self.terminal_authority_policy = policy;
        self
    }

    /// Supply an explicit receiving permission policy. Calling this is the
    /// only way package production claims receiver admission; the policy is
    /// adjudicated exactly as supplied, including an explicit empty policy.
    pub fn with_receiving_terminal_authority_permission_policy(
        mut self,
        policy: TerminalAuthorityPermissionPolicy,
    ) -> Self {
        self.receiving_terminal_authority_permission_policy = Some(policy);
        self
    }
}

#[derive(Debug)]
pub enum CompilePreparedLocalProjectNativeError {
    /// One target's own prepared section failed while sibling targets carried
    /// on; reported per target in the multi-target route.
    Prepare(PrepareLocalProjectError),
    Review(CompileResolvedPackageReviewsError),
    MissingAcceptance(PackageKey),
    Acceptance {
        package: PackageKey,
        error: crate::lock::PackageLockError,
    },
    TrustAdmission(Vec<Diagnostic>),
    Native(Vec<Diagnostic>),
}

impl fmt::Display for CompilePreparedLocalProjectNativeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prepare(error) => write!(formatter, "{error}"),
            Self::Review(error) => {
                write!(formatter, "cannot compile accepted package inputs: {error}")
            }
            Self::MissingAcceptance(package) => {
                write!(
                    formatter,
                    "package `{}` has no accepted compilation inputs; run omega update before native compilation",
                    package.name().as_str(),
                )
            }
            Self::Acceptance { package, error } => write!(
                formatter,
                "accepted package `{}` has inconsistent compilation permissions: {error}; run omega update before native compilation",
                package.name().as_str(),
            ),
            Self::TrustAdmission(diagnostics) => {
                write!(formatter, "cannot admit package trust: {diagnostics:?}")
            }
            Self::Native(diagnostics) => {
                write!(
                    formatter,
                    "cannot realize accepted package production: {diagnostics:?}"
                )
            }
        }
    }
}

impl std::error::Error for CompilePreparedLocalProjectNativeError {}

/// Observe the exact checked program that native production consumes, without
/// repeating package acquisition or sponsored build execution. The observer must
/// only compute a private result: native realization can still reject afterward.
/// A caller with nothing to observe passes `|_| ()`.
///
/// This operation is the production CLI seam. It does not ask project
/// preparation to infer permissions. Source and compiler identities must match
/// the accepted lock before Build execution; native realization consumes fresh
/// checked semantics, not reconstructed package-review findings.
pub fn compile_prepared_local_project_for_native<Observation>(
    request: PreparedLocalProjectNativeRequest,
    observe: impl FnOnce(&compiler::CheckedCompilation) -> Observation,
) -> Result<(CompileReport, Observation), CompilePreparedLocalProjectNativeError> {
    if request.target_profiles.len() != 1 {
        return Err(CompilePreparedLocalProjectNativeError::Native(vec![
            Diagnostic::error(
                "compile_prepared_local_project_for_native needs a single-target request",
            ),
        ]));
    }
    let mut observe = Some(observe);
    compile_prepared_local_project_for_native_targets(request, move |checked| {
        observe
            .take()
            .expect("one native observation per requested target")(checked)
    })
    .pop()
    .expect("a native request produces one outcome per requested target")
    .result
}

/// One target's native production result inside a shared multi-target
/// request. A failing target never stops or hides a sibling.
pub struct CompiledProjectTargetOutcome<Observation> {
    pub target: target::TargetProfile,
    pub result: Result<(CompileReport, Observation), CompilePreparedLocalProjectNativeError>,
}

/// Native production for every requested target under one prepared source
/// union. All targets share the resolved closure and one
/// source custody, so a package's binding-independent parse
/// frontier — and any typed frontier whose inputs two targets carry
/// identically — assembles exactly once for the request. Each target still
/// produces under `build_dir/<target>`, arms its own accepted checkpoint, and
/// settles its own evidence, admission, and realization.
pub fn compile_prepared_local_project_for_native_targets<Observation>(
    request: PreparedLocalProjectNativeRequest,
    mut observe: impl FnMut(&compiler::CheckedCompilation) -> Observation,
) -> Vec<CompiledProjectTargetOutcome<Observation>> {
    let PreparedLocalProjectNativeRequest {
        prepared,
        build_dir,
        target_profiles,
        optimization_rollback,
        build_snapshot,
        terminal_authority_policy,
        receiving_terminal_authority_permission_policy,
        timings,
    } = request;
    let (entry_path, source_closure, prepared_targets) = prepared.into_target_review_parts();
    let mut preparation = PackageSourcePreparation::for_closure(&source_closure);
    prepared_targets
        .into_iter()
        .filter(|target| target_profiles.contains(&target.profile))
        .map(|prepared_target| {
            let profile = prepared_target.profile;
            let result = compile_one_prepared_target(
                prepared_target,
                &source_closure,
                &entry_path,
                &build_dir,
                build_snapshot.as_ref(),
                &optimization_rollback,
                terminal_authority_policy.clone(),
                receiving_terminal_authority_permission_policy.clone(),
                timings,
                &mut preparation,
                &mut observe,
            );
            CompiledProjectTargetOutcome {
                target: profile,
                result,
            }
        })
        .collect()
}

/// Produce one prepared target inside the shared request. A locked target's
/// restricted-request checkpoint arms the pass; an unlocked target supplies
/// no such grants. In either case, a restricted request without a grant
/// rejects before its build effect executes or generated sources reach
/// realization.
#[allow(clippy::too_many_arguments)]
fn compile_one_prepared_target<Observation>(
    prepared_target: PreparedProjectTarget,
    source_closure: &crate::resolution::graph::ResolvedPackageSourceClosure,
    entry_path: &Path,
    build_dir: &Path,
    build_snapshot: Option<&build_evaluation::BuildSnapshotRequest>,
    optimization_rollback: &OptimizationRollback,
    terminal_authority_policy: TerminalAuthorityPolicy,
    receiving_terminal_authority_permission_policy: Option<TerminalAuthorityPermissionPolicy>,
    timings: bool,
    preparation: &mut PackageSourcePreparation,
    observe: &mut impl FnMut(&compiler::CheckedCompilation) -> Observation,
) -> Result<(CompileReport, Observation), CompilePreparedLocalProjectNativeError> {
    let target_profile = prepared_target.profile;
    if source_closure.root_role() != crate::declarations::BuildDeclarationKind::Application {
        return Err(CompilePreparedLocalProjectNativeError::Review(
            CompileResolvedPackageReviewsError::InvalidProductionRootRole {
                package: source_closure.graph().root().clone(),
                role: source_closure.root_role(),
            },
        ));
    }
    let accepted_target = prepared_target
        .accepted
        .map_err(CompilePreparedLocalProjectNativeError::Prepare)?;
    let target_closure = source_closure.for_exact_target(target_profile);
    let target_build_dir = build_dir.join(target_profile.target_name());
    let checked = compile_accepted_project(
        AcceptedCompilationRequest {
            closure: &target_closure,
            accepted: accepted_target.as_ref(),
            entry: entry_path,
            build_root: &target_build_dir,
            snapshot: build_snapshot,
            verify: false,
            timings,
        },
        preparation,
    )
    .map_err(CompilePreparedLocalProjectNativeError::Review)?;
    // Bind the permissions supplied by retained compilation inputs to this
    // root occurrence's consent before native production can consume them.
    // This projects only supplied permission meaning, not package review or
    // provider demand. A sibling package or Build occurrence cannot authorize
    // this root's Product execution.
    // With no lock there are no supplied permissions to compare. Do not turn
    // a missing or invalid occurrence inside an existing lock into that case.
    if let Some(accepted_target) = &accepted_target {
        let root = source_closure.graph().root();
        let acceptance = accepted_target
            .occurrences()
            .iter()
            .find(|occurrence| {
                occurrence.acceptance().package() == root.identity()
                    && occurrence.context().purpose().is_product()
                    && occurrence.context().target() == target_profile
            })
            .ok_or_else(|| {
                CompilePreparedLocalProjectNativeError::MissingAcceptance(root.clone())
            })?;
        let permissions = package_evidence::project_checked_terminal_permission_policy(
            &checked,
            target_profile,
            root.identity(),
        )
        .map_err(CompilePreparedLocalProjectNativeError::Native)?;
        acceptance
            .acceptance()
            .validate_terminal_permissions(&permissions)
            .map_err(|error| CompilePreparedLocalProjectNativeError::Acceptance {
                package: root.clone(),
                error,
            })?;
    }
    checked
        .admit_trust()
        .map_err(CompilePreparedLocalProjectNativeError::TrustAdmission)?;
    let observation = observe(&checked);
    let product = checked
        .prepare_product_report(
            entry_path.to_path_buf(),
            compiler::RequestedCompileProduct::NativeArtifact,
            optimization_rollback,
        )
        .map_err(CompilePreparedLocalProjectNativeError::Native)?;
    let report = match product {
        compilation_products::CheckedProductReport::Complete(report) => Ok(report),
        compilation_products::CheckedProductReport::Native(settlement) => {
            compiler::native::produce_native_product(
                compilation_products::NativeProductRequest {
                    terminal_authority_policy,
                    terminal_authority_permission_policy:
                        receiving_terminal_authority_permission_policy,
                    optimization_rollback: optimization_rollback.clone(),
                },
                checked,
            )
            .and_then(|produced| {
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
        compilation_products::CheckedProductReport::Terminal(_) => Err(vec![Diagnostic::error(
            "native preparation returned an unexpected Terminal-only request",
        )]),
    }
    .map_err(CompilePreparedLocalProjectNativeError::Native)?;
    Ok((report, observation))
}
