//! Native production for one already-prepared local project.
//!
//! An absent lock is a development compilation, not missing acceptance. It
//! uses the same checked package route with no retained bindings or restricted
//! build grants. An existing lock must still match the source, compiler and
//! target; its permission rows are checked before producing the native product.

use super::accepted_compilation::{AcceptedCompilationRequest, compile_accepted_projects};

use super::{PrepareLocalProjectError, PreparedLocalProject};
use crate::build_session::PackageBuildSession;
use crate::review::CompileResolvedPackageReviewsError;
use compiler::{CompileReport, OptimizationRollback};
use diagnostics::Diagnostic;
use std::fmt;
use std::path::PathBuf;
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
    private_entry_ram: Vec<compilation_products::PrivateEntryRamInitialization>,
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
            private_entry_ram: Vec::new(),
        }
    }

    /// Record per-stage timings on the produced report. Off by default: the
    /// ladder measures nothing until a caller asks for it.
    pub fn with_private_entry_ram(
        mut self,
        inputs: Vec<compilation_products::PrivateEntryRamInitialization>,
    ) -> Self {
        self.private_entry_ram = inputs;
        self
    }

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
}

#[derive(Debug)]
pub enum CompilePreparedLocalProjectNativeError {
    /// One target's own prepared section failed while sibling targets carried
    /// on; reported per target in the multi-target route.
    Prepare(PrepareLocalProjectError),
    Review(CompileResolvedPackageReviewsError),
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
            Self::TrustAdmission(diagnostics) => {
                write!(formatter, "cannot admit package trust:")?;
                crate::review::render_diagnostics(formatter, diagnostics)
            }
            Self::Native(diagnostics) => {
                write!(formatter, "cannot realize accepted package production:")?;
                crate::review::render_diagnostics(formatter, diagnostics)
            }
        }
    }
}

impl std::error::Error for CompilePreparedLocalProjectNativeError {}

/// This operation is the production CLI seam. It does not ask project
/// preparation to infer permissions. Source and compiler identities must match
/// the accepted lock before Build execution; native realization consumes fresh
/// checked semantics, not reconstructed package-review findings.
pub fn compile_prepared_local_project_for_native(
    request: PreparedLocalProjectNativeRequest,
) -> Result<CompileReport, CompilePreparedLocalProjectNativeError> {
    if request.target_profiles.len() != 1 {
        return Err(CompilePreparedLocalProjectNativeError::Native(vec![
            Diagnostic::error(
                "compile_prepared_local_project_for_native needs a single-target request",
            ),
        ]));
    }
    compile_prepared_local_project_for_native_targets(request)
        .pop()
        .expect("a native request produces one outcome per requested target")
        .result
}

/// One target's native production result inside a shared multi-target
/// request. A failing target never stops or hides a sibling.
pub struct CompiledProjectTargetOutcome {
    pub target: target::TargetProfile,
    pub result: Result<CompileReport, CompilePreparedLocalProjectNativeError>,
}

/// Native production for every requested target under one prepared source
/// union. All targets share the resolved closure and one
/// source custody, so a package's binding-independent parse
/// frontier — and any typed frontier whose inputs two targets carry
/// identically — assembles exactly once for the request. Each target still
/// produces under `build_dir/<target>`, arms its own accepted checkpoint, and
/// settles its own evidence, admission, and realization.
pub fn compile_prepared_local_project_for_native_targets(
    request: PreparedLocalProjectNativeRequest,
) -> Vec<CompiledProjectTargetOutcome> {
    let PreparedLocalProjectNativeRequest {
        prepared,
        build_dir,
        target_profiles,
        optimization_rollback,
        build_snapshot,
        terminal_authority_policy,
        receiving_terminal_authority_permission_policy,
        timings,
        private_entry_ram,
    } = request;
    let (entry_path, source_closure, prepared_targets) = prepared.into_target_review_parts();
    let targets: Vec<_> = prepared_targets
        .into_iter()
        .filter(|target| target_profiles.contains(&target.profile))
        .collect();
    if source_closure.root_role() != crate::declarations::BuildDeclarationKind::Application {
        return targets
            .into_iter()
            .map(|target| CompiledProjectTargetOutcome {
                target: target.profile,
                result: Err(CompilePreparedLocalProjectNativeError::Review(
                    CompileResolvedPackageReviewsError::InvalidProductionRootRole {
                        package: source_closure.graph().root().clone(),
                        role: source_closure.root_role(),
                    },
                )),
            })
            .collect();
    }
    // The whole batch borrows one invocation: sponsors, staging and the pure
    // Build store persist across targets rather than replenishing per compile.
    let mut session = match PackageBuildSession::create(&build_dir) {
        Ok(session) => session,
        Err(error) => {
            let message = error.to_string();
            return targets
                .iter()
                .map(|target| CompiledProjectTargetOutcome {
                    target: target.profile,
                    result: Err(CompilePreparedLocalProjectNativeError::Review(
                        CompileResolvedPackageReviewsError::Compilation {
                            package: source_closure.graph().root().clone(),
                            diagnostics: vec![Diagnostic::error(message.clone())],
                        },
                    )),
                })
                .collect();
        }
    };
    let requests = targets
        .iter()
        .filter_map(|target| {
            let accepted = target.accepted.as_ref().ok()?;
            Some(AcceptedCompilationRequest {
                closure: source_closure.for_exact_target(target.profile),
                accepted: accepted.as_ref(),
                entry: &entry_path,
                snapshot: build_snapshot.as_ref(),
                verify: false,
                timings,
                product: compiler::CompilationProduct::Native(
                    compilation_products::NativeProductRequest {
                        private_entry_ram: private_entry_ram.clone(),
                        terminal_authority_policy: terminal_authority_policy.clone(),
                        terminal_authority_permission_policy:
                            receiving_terminal_authority_permission_policy.clone(),
                        optimization_rollback: optimization_rollback.clone(),
                    },
                ),
            })
        })
        .collect();
    let mut products = compile_accepted_projects(requests, &mut session).into_iter();
    let outcomes = targets
        .into_iter()
        .map(|target| {
            let result = target
                .accepted
                .map_err(CompilePreparedLocalProjectNativeError::Prepare)
                .and_then(|_| {
                    products
                        .next()
                        .ok_or_else(|| {
                            CompilePreparedLocalProjectNativeError::Native(vec![Diagnostic::error(
                                "compiler lost a package target outcome",
                            )])
                        })?
                        .map_err(CompilePreparedLocalProjectNativeError::Review)
                })
                .and_then(|product| product.into_report().map_err(product_failure));
            CompiledProjectTargetOutcome {
                target: target.profile,
                result,
            }
        })
        .collect();
    // Staging cleanup is disposal work, not a target outcome; the shared
    // Drop would otherwise swallow it silently.
    let _ = session.close();

    outcomes
}

fn product_failure(
    error: compiler::CompilationProductError,
) -> CompilePreparedLocalProjectNativeError {
    match error {
        compiler::CompilationProductError::TrustAdmission(diagnostics) => {
            CompilePreparedLocalProjectNativeError::TrustAdmission(diagnostics)
        }
        compiler::CompilationProductError::Production(diagnostics) => {
            CompilePreparedLocalProjectNativeError::Native(diagnostics)
        }
    }
}
