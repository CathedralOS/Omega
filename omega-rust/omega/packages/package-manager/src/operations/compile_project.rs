//! Accepted native production for one already-prepared local project.

use crate::review::SemanticBindingReview;

use super::prepare_project::PreparedProjectTarget;
use super::{PrepareLocalProjectError, PreparedLocalProject};
use crate::admission::{
    AcceptedNativeInput, AcceptedNativeRealizationRequest, AcceptedOrdinaryEvidenceError,
    accept_ordinary_closure_evidence, realize_accepted_native_report,
};
use crate::review::{
    CandidateCompilation, CandidateSourcePreparation, CanonicalPackageReconstructionQuestionLimits,
    CompileResolvedPackageReviewsError, RestrictedBuildCheckpoint,
    ReviewOnlyCapabilityConflictLimits, production_candidate_reusing,
};
use compiler::{CompileReport, OptimizationRollback, TrustAdmission};
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
/// package acceptance is checked against the prepared project's accepted lock
/// target regardless.
pub struct PreparedLocalProjectNativeRequest {
    prepared: PreparedLocalProject,
    build_dir: PathBuf,
    target_profiles: Vec<target::TargetProfile>,

    accepted_trust_admissions: Vec<TrustAdmission>,
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

            accepted_trust_admissions: Vec::new(),
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

    pub fn with_accepted_trust_admissions(mut self, admissions: Vec<TrustAdmission>) -> Self {
        self.accepted_trust_admissions = admissions;
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
    Evidence(AcceptedOrdinaryEvidenceError),
    TrustAdmission(Vec<Diagnostic>),
    Native(Vec<Diagnostic>),
}

impl fmt::Display for CompilePreparedLocalProjectNativeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prepare(error) => write!(formatter, "{error}"),
            Self::Review(error) => {
                write!(formatter, "cannot compile fresh package review: {error}")
            }
            Self::Evidence(error) => {
                write!(
                    formatter,
                    "cannot accept fresh package review evidence: {error}"
                )
            }
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
/// Neither the observation nor a successful report replaces the caller's exact
/// trust-admission settlement before publication or execution.
/// Check accepted project policy and realize one freshly compiled application.
/// `observe` sees the checked compilation before realization; a caller with
/// nothing to observe passes `|_| ()`.
///
/// This operation is the production CLI seam. It neither asks project
/// preparation to infer permissions nor treats decoded policy bytes as
/// evidence: all acceptance starts again from live resolver custody and the
/// exact final checked review pass.
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
/// [`CandidateSourcePreparation`], so a package's binding-independent parse
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

        accepted_trust_admissions,
        optimization_rollback,
        build_snapshot,
        terminal_authority_policy,
        receiving_terminal_authority_permission_policy,
        timings,
    } = request;
    let (_entry_path, source_closure, prepared_targets) = prepared.into_target_review_parts();
    let mut preparation =
        CandidateSourcePreparation::for_closure(&source_closure).collecting_timings(timings);
    prepared_targets
        .into_iter()
        .filter(|target| target_profiles.contains(&target.profile))
        .map(|prepared_target| {
            let profile = prepared_target.profile;
            let result = compile_one_prepared_target(
                prepared_target,
                &source_closure,
                &build_dir,
                build_snapshot.as_ref(),
                &accepted_trust_admissions,
                &optimization_rollback,
                terminal_authority_policy.clone(),
                receiving_terminal_authority_permission_policy.clone(),
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

/// Produce one prepared target inside the shared request. Native production
/// is an executor of the accepted policy: the retained target's
/// restricted-request checkpoint arms the pass, so a projected request the
/// accepted rows do not grant rejects before its own build effect executes —
/// before generated sources or the checked root can reach realization.
#[allow(clippy::too_many_arguments)]
fn compile_one_prepared_target<Observation>(
    prepared_target: PreparedProjectTarget,
    source_closure: &crate::resolution::graph::ResolvedPackageSourceClosure,
    build_dir: &Path,
    build_snapshot: Option<&build_evaluation::BuildSnapshotRequest>,
    accepted_trust_admissions: &[TrustAdmission],
    optimization_rollback: &OptimizationRollback,
    terminal_authority_policy: TerminalAuthorityPolicy,
    receiving_terminal_authority_permission_policy: Option<TerminalAuthorityPermissionPolicy>,
    preparation: &mut CandidateSourcePreparation,
    observe: &mut impl FnMut(&compiler::CheckedCompilation) -> Observation,
) -> Result<(CompileReport, Observation), CompilePreparedLocalProjectNativeError> {
    let target_profile = prepared_target.profile;
    let accepted_target = prepared_target
        .accepted
        .map_err(CompilePreparedLocalProjectNativeError::Prepare)?;
    let target_closure = source_closure.for_exact_target(target_profile);
    let target_build_dir = build_dir.join(target_profile.target_name());
    let checkpoint = accepted_target
        .as_ref()
        .map(RestrictedBuildCheckpoint::derive);
    let candidate = production_candidate_reusing(
        CandidateCompilation {
            target_closure: &target_closure,
            build_root: &target_build_dir,
            root_build_snapshot: build_snapshot,
            restricted_build_checkpoint: checkpoint.as_ref(),
        },
        SemanticBindingReview::Discover,
        preparation,
    )
    .map_err(CompilePreparedLocalProjectNativeError::Review)?;
    // Snapshot the ladder before realization as the fallback for report paths
    // that produce no timing rows of their own.
    let stage_timings = candidate.checked_root().timings().phases().to_vec();
    let evidence = accept_ordinary_closure_evidence(
        &target_closure,
        candidate.reviews(),
        CanonicalPackageReconstructionQuestionLimits::default(),
        ReviewOnlyCapabilityConflictLimits::default(),
        accepted_target.as_ref(),
    )
    .map_err(CompilePreparedLocalProjectNativeError::Evidence)?;
    let trust_settlement = candidate
        .checked_root()
        .admit_trust(accepted_trust_admissions)
        .map_err(CompilePreparedLocalProjectNativeError::TrustAdmission)?;
    let observation = observe(candidate.checked_root());
    realize_accepted_native_report(
        AcceptedNativeInput::Reviewed {
            candidate: Box::new(candidate),
            optimization_rollback,
        },
        AcceptedNativeRealizationRequest {
            evidence: &evidence,
            profile: &proof_admission::AdmissionProfile::default(),
            terminal_authority_policy,
            receiving_terminal_authority_permission_policy,
            imports: &[],
        },
    )
    .map(|report| {
        let timings = if report.timings().is_empty() {
            stage_timings
        } else {
            report.timings().to_vec()
        };
        (
            report
                .with_trust_admission_settlement(trust_settlement)
                .with_timings(timings),
            observation,
        )
    })
    .map_err(CompilePreparedLocalProjectNativeError::Native)
}

#[cfg(test)]
mod tests {
    use super::PathBuf;
    mod accepted_lock;
    mod elf_loader;
    mod receiving_admission;
    use crate::review::ReviewOnlyRootPolicyDisposition;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TREE: AtomicU64 = AtomicU64::new(0);

    struct TemporaryProject {
        source: PathBuf,
        workspace: PathBuf,
    }

    impl TemporaryProject {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "omega-cli-package-native-{}-{}",
                std::process::id(),
                NEXT_TREE.fetch_add(1, Ordering::Relaxed),
            ));
            std::fs::create_dir(&path).expect("create temporary package project");
            std::fs::write(
                path.join("build.omg"),
                r#"
machine build(builder: &mut Build) {
    Build::application(builder, "accepted_claim_app");
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
}
"#,
            )
            .expect("write package build declaration");
            std::fs::write(
                path.join("main.omg"),
                r#"pub proposition accepted(value: u64);

boundary machine trusted_value() -> u64
ensures accepted(result);

data Main { }

machine Main::main(&mut self) { }
"#,
            )
            .expect("write package application");
            let workspace = std::env::temp_dir().join(format!(
                "omega-cli-package-workspace-{}-{}",
                std::process::id(),
                NEXT_TREE.fetch_add(1, Ordering::Relaxed),
            ));
            std::fs::create_dir(&workspace).expect("create temporary build workspace");
            Self {
                source: path,
                workspace,
            }
        }

        fn entry(&self) -> PathBuf {
            self.source.join("main.omg")
        }

        /// An application that reaches the Console exit leaf through a
        /// minimal local provider package: the demanded closure is the
        /// package-owned `Console::exit_process` compiler-intrinsic
        /// termination mechanism, so receiver admission is exercised by the
        /// receiving permission policy alone without the standard library.
        /// The dependency sits inside the workspace directory so `Drop`
        /// cleans it with the rest of the fixture.
        fn console_project(target: target::TargetProfile) -> Self {
            let path = std::env::temp_dir().join(format!(
                "omega-cli-package-native-console-{}-{}",
                std::process::id(),
                NEXT_TREE.fetch_add(1, Ordering::Relaxed),
            ));
            std::fs::create_dir(&path).expect("create temporary package project");
            let workspace = std::env::temp_dir().join(format!(
                "omega-cli-package-workspace-{}-{}",
                std::process::id(),
                NEXT_TREE.fetch_add(1, Ordering::Relaxed),
            ));
            std::fs::create_dir(&workspace).expect("create temporary build workspace");
            let dependency = workspace.join("console-provider");
            std::fs::create_dir(&dependency).expect("create temporary provider package");
            std::fs::write(
                dependency.join("build.omg"),
                r#"machine build(builder: &mut Build) {
    Build::package(builder, "console_provider");
}
"#,
            )
            .expect("write provider package build declaration");
            std::fs::write(
                dependency.join("console.omg"),
                r#"pub boundary trait Console {
    machine exit_process(return_code: i32)
    reaches
        Console;
    machine write_byte(byte: i32)
    reaches
        Console;
}

pub data ConsoleNativeProvider { }

linux_x86_64 boundary machine ConsoleNativeProvider::exit_process(return_code: i32)
    satisfies Console::exit_process;
linux_x86_64 boundary machine ConsoleNativeProvider::write_byte(byte: i32)
    satisfies Console::write_byte;

linux_x86_64 machine ConsoleNativeProvider::provider_defaults(defaults: &mut ConsoleNativeProvider) {
    defaults.select_provider<Console, ConsoleNativeProvider>();
}
"#.replace("linux_x86_64", target.target_name()),
            )
            .expect("write provider package source");
            let dependency = dependency.to_string_lossy().replace('\\', "/");
            let target_name = target.target_name();
            std::fs::write(
                path.join("build.omg"),
                format!(
                    r#"
machine build(builder: &mut Build) {{
    Build::application(builder, "receiving_admission_app");
    Build::depend(builder, Source::Path {{ location: "{dependency}" }});
    builder.select_provider<console_provider::Console, console_provider::ConsoleNativeProvider>();
    builder.roots.bind({target_name}::ProgramEntry, Main::main);
}}
"#,
                ),
            )
            .expect("write package build declaration");
            std::fs::write(
                path.join("main.omg"),
                r#"use omega::language::core::binding;
use console_provider::console;

data Main { console: Binding<Console>; }

machine Main::main(&mut self)
reaches Console
{
    self.console.exit_process(70);
}
"#,
            )
            .expect("write package application");
            Self {
                source: path,
                workspace,
            }
        }
    }

    impl Drop for TemporaryProject {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.source);
            let _ = std::fs::remove_dir_all(&self.workspace);
        }
    }
}
