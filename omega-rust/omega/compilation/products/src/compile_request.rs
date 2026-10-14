//! One invocation owns source, Build staging and policy; targets select realizations.

use self::options::CompileOptions;
use diagnostics::Diagnostic;
use optimization_core::OptimizationRollback;
use package_compilation::build_execution::BuildSnapshotRequest;
use package_compilation::{
    PackageCompilationInputs, PackageCompilationSourceInputs, PackageCompilationTargetInputs,
};
use std::path::PathBuf;
use std::sync::Arc;
use target::TargetProfile;
use terminal_psi_to_abstract_operations::provider_admission::{
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy, current_terminal_authority_policy,
};

mod checked;
mod native;
pub use checked::{CheckedCompileRequest, CheckedExecutionRequest, SettledCheckReuse};
pub mod options;
pub use native::NativeProductRequest;
pub use terminal_installation::PrivateEntryRamInitialization;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestedCompileProduct {
    Check,
    TerminalArtifact,
    NativeArtifact,
}

impl RequestedCompileProduct {
    /// Refuse rollback rules belonging to stages this requested stop never
    /// executes. Request admission applies it before source acquisition -- the
    /// standalone route in `validate_for_execution`, the project route in the
    /// CLI's product dispatch -- so an invalid overlay rejects before frontend
    /// work or auxiliary output, and `CheckedCompilation::prepare_product_report`
    /// applies it again as the product-owned boundary every directly
    /// constructed route crosses
    /// (wiki/spec/build/optimizations.md#release-rollback-and-promotion).
    pub fn validate_optimization_rollback(
        self,
        rollback: &OptimizationRollback,
    ) -> Result<(), Diagnostic> {
        let unavailable = rollback
            .requested_disabled()
            .as_slice()
            .iter()
            .filter(|optimization| match self {
                Self::Check => true,
                Self::TerminalArtifact => !matches!(
                    optimization.execution_phase(),
                    optimization_core::OptimizationExecutionPhase::CheckedTrees
                        | optimization_core::OptimizationExecutionPhase::Psi
                ),
                Self::NativeArtifact => false,
            })
            .map(|optimization| format!("`{}`", optimization.build_case_name()))
            .collect::<Vec<_>>();
        if unavailable.is_empty() {
            Ok(())
        } else {
            Err(Diagnostic::error(format!(
                "optimization rollback {} names stages not executed by {:?}",
                unavailable.join(", "),
                self
            )))
        }
    }

    /// Admit evaluated Build requirements before dispatching product production.
    /// This checks eligibility only; it establishes no artifact or receiving
    /// authority. Only product admission calls it: a production leg never
    /// re-admits the stop its settlement already allowed.
    pub(crate) fn admit_build_requirements(
        self,
        artifact_only: bool,
        optimization_rollback: &OptimizationRollback,
    ) -> Result<(), Vec<Diagnostic>> {
        if artifact_only && self != Self::Check && !optimization_rollback.is_empty() {
            return Err(vec![Diagnostic::error(
                "artifact-only builds execute no product optimization stages to roll back",
            )]);
        }
        Ok(())
    }
}

/// Input shared by every selected target. It cannot disagree between children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedCompileInputs {
    pub root_path: PathBuf,
    pub build_directory: PathBuf,
    pub requested_product: RequestedCompileProduct,
    /// Collect per-stage measurements into every checked record's timing
    /// accumulator; the report carries the recorded ladder.
    pub timings: bool,

    pub package_sources: Option<Arc<PackageCompilationSourceInputs>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RequestedDestination {
    target_name: Option<String>,
}

/// Invocation policy is supplied once, before execution records are expanded.
/// Per-target Build/provider settlement still belongs to the producing stages.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CompilePolicy {
    private_entry_ram: Vec<PrivateEntryRamInitialization>,
    pub(super) build_snapshot: Option<BuildSnapshotRequest>,
    terminal_authority_policy: TerminalAuthorityPolicy,
    /// The receiver-admission axis, supplied only by an explicit
    /// `with_terminal_authority_permission_policy` call. `None` means this
    /// production makes no receiver-admission claim: accepted package
    /// permissions are never projected into receiver rows, and the emitted
    /// artifact cannot satisfy an explicit admission replay.
    pub(super) terminal_authority_permission_policy: Option<TerminalAuthorityPermissionPolicy>,
    package_target_inputs: PackageCompilationTargetInputs,
    pub(super) optimization_rollback: OptimizationRollback,
}

impl Default for CompilePolicy {
    fn default() -> Self {
        Self {
            private_entry_ram: Vec::new(),
            build_snapshot: None,
            terminal_authority_policy: current_terminal_authority_policy(),
            // Ordinary production supplies no receiving permission policy: the
            // receiver-admission axis is claimed only by an explicit setter
            // call, never by defaulting to an empty or fabricated policy.
            terminal_authority_permission_policy: None,
            package_target_inputs: PackageCompilationTargetInputs::default(),
            optimization_rollback: OptimizationRollback::default(),
        }
    }
}

/// One production invocation, including the one-target case. This Rust-host
/// request is not a canonical OCREQ frame and grants no bootstrap authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileRequest {
    shared: SharedCompileInputs,
    policy: CompilePolicy,
    destinations: Vec<RequestedDestination>,
}

impl CompileRequest {
    /// Create one checking configuration. An absent target remains target-neutral
    /// for checking/Terminal production; native production resolves it to the
    /// host's catalogued profile, or rejects with a diagnostic on a host that
    /// owns none.
    pub fn new(options: CompileOptions) -> Self {
        let build_directory = options.build_dir();
        Self {
            shared: SharedCompileInputs {
                root_path: options.root_path,
                build_directory,
                requested_product: RequestedCompileProduct::Check,
                timings: false,

                package_sources: None,
            },
            policy: CompilePolicy::default(),
            destinations: vec![RequestedDestination {
                target_name: options.target_name,
            }],
        }
    }

    /// Compile one source root under one Build activation into selected targets.
    /// Build staging belongs to this invocation, not to a realized target.
    /// Artifact publication chooses its own per-target directories afterward.
    /// Empty, duplicate and targetless multi-target requests reject before effects.
    pub fn for_targets(
        root_path: PathBuf,
        build_directory: PathBuf,
        targets: Vec<Option<TargetProfile>>,
    ) -> Self {
        let mut request = Self::new(CompileOptions {
            root_path,
            target_name: None,
            build_dir: Some(build_directory),
        });
        request.destinations = targets
            .into_iter()
            .map(|target| RequestedDestination {
                target_name: target.map(|target| target.target_name().to_owned()),
            })
            .collect();
        request
    }
    /// Explicitly sponsor fresh private typed entry RAM; default supply is absent.
    pub fn with_private_entry_ram(mut self, inputs: Vec<PrivateEntryRamInitialization>) -> Self {
        self.policy.private_entry_ram = inputs;
        self
    }

    pub fn with_requested_product(mut self, product: RequestedCompileProduct) -> Self {
        self.shared.requested_product = product;
        self
    }

    /// Supply one capture intent to the invocation.
    /// Each target checks its own custody; the admitted invocation executes Build
    /// once. This grants neither output sponsorship nor publication authority.
    pub fn with_build_snapshot(mut self, snapshot: BuildSnapshotRequest) -> Self {
        self.policy.build_snapshot = Some(snapshot);
        self
    }

    /// Supply the package's sources and accepted input policy once. Each
    /// target independently admits the resulting package custody.
    pub fn with_package_inputs(mut self, inputs: PackageCompilationInputs) -> Self {
        let (source, target) = inputs.into_parts();
        self.shared.package_sources = Some(source);
        self.policy.package_target_inputs = target;
        self
    }

    /// Apply a common receiving mechanism-classification policy to every
    /// selected target. Missing demanded rows still reject during authority review.
    pub fn with_terminal_authority_policy(mut self, policy: TerminalAuthorityPolicy) -> Self {
        self.policy.terminal_authority_policy = policy;
        self
    }
    pub fn with_optimization_rollback(mut self, rollback: OptimizationRollback) -> Self {
        self.policy.optimization_rollback = rollback;
        self
    }

    /// Record per-stage timings on every produced report.
    pub fn with_timings(mut self, timings: bool) -> Self {
        self.shared.timings = timings;
        self
    }

    pub fn validate_for_execution(self) -> Result<ValidatedCompileRequest, Vec<Diagnostic>> {
        if self.destinations.is_empty() {
            return Err(vec![Diagnostic::error(
                "compilation requires at least one target configuration",
            )]);
        }
        let multiple = self.destinations.len() > 1;
        let shared = Arc::new(self.shared);
        let policy = Arc::new(self.policy);
        let mut targets = Vec::<ValidatedTargetCompilation>::with_capacity(self.destinations.len());
        let mut diagnostics = Vec::new();
        for mut destination in self.destinations {
            if multiple && destination.target_name.is_none() {
                diagnostics.push(Diagnostic::error(
                    "multiple configurations must each name an exact target",
                ));
                continue;
            }
            if shared.requested_product == RequestedCompileProduct::NativeArtifact
                && destination.target_name.is_none()
            {
                match TargetProfile::host_if_supported() {
                    Some(host) => {
                        destination.target_name = Some(host.target_name().to_owned());
                    }
                    None => {
                        let catalogued = TargetProfile::ALL
                            .into_iter()
                            .filter(|profile| profile.native_realization().is_some())
                            .map(|profile| profile.target_name())
                            .collect::<Vec<_>>()
                            .join(", ");
                        diagnostics.push(Diagnostic::error(format!(
                            "native production needs an exact target profile: none was \
                             named and this host has no catalogued Omega deployment \
                             profile (name one of {catalogued})"
                        )));
                        continue;
                    }
                }
            }
            let profile = match destination
                .target_name
                .as_deref()
                .map(|name| TargetProfile::from_omega_target_name(Some(name)))
                .transpose()
            {
                Ok(profile) => profile,
                Err(error) => {
                    diagnostics.push(error);
                    continue;
                }
            };
            if let Err(error) = shared
                .requested_product
                .validate_optimization_rollback(&policy.optimization_rollback)
            {
                diagnostics.push(error);
                continue;
            }
            if targets.iter().any(|target| target.profile == profile) {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate compilation target {:?}",
                    profile
                )));
            }
            let target_inputs = policy.package_target_inputs.clone();
            let package_inputs = match &shared.package_sources {
                Some(source) => {
                    match PackageCompilationInputs::from_parts(source.clone(), target_inputs) {
                        Ok(inputs) => Some(inputs),
                        Err(errors) => {
                            diagnostics.extend(
                                errors
                                    .into_iter()
                                    .map(|error| Diagnostic::error(error.to_string())),
                            );
                            continue;
                        }
                    }
                }
                None if target_inputs == PackageCompilationTargetInputs::default() => None,
                None => {
                    diagnostics.push(Diagnostic::error(
                        "target package inputs require shared package sources",
                    ));
                    continue;
                }
            };
            targets.push(ValidatedTargetCompilation {
                policy: policy.clone(),
                package_inputs,
                profile,
            });
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        targets.sort_by_key(|target| {
            target.profile.and_then(|profile| {
                TargetProfile::ALL
                    .iter()
                    .position(|entry| *entry == profile)
            })
        });
        Ok(ValidatedCompileRequest { shared, targets })
    }
}

#[derive(Debug)]
pub struct ValidatedCompileRequest {
    shared: Arc<SharedCompileInputs>,
    targets: Vec<ValidatedTargetCompilation>,
}

impl ValidatedCompileRequest {
    /// Consume validated configuration without transferring compilation or publication authority.
    pub fn into_parts(self) -> (Arc<SharedCompileInputs>, Vec<ValidatedTargetCompilation>) {
        (self.shared, self.targets)
    }
}

/// One admitted execution of the shared invocation, not another public request.
#[derive(Debug)]
pub struct ValidatedTargetCompilation {
    policy: Arc<CompilePolicy>,
    package_inputs: Option<PackageCompilationInputs>,
    profile: Option<TargetProfile>,
}

impl ValidatedTargetCompilation {
    pub const fn profile(&self) -> Option<TargetProfile> {
        self.profile
    }
    pub fn build_snapshot(&self) -> Option<&BuildSnapshotRequest> {
        self.policy.build_snapshot.as_ref()
    }
    pub fn optimization_rollback(&self) -> &OptimizationRollback {
        &self.policy.optimization_rollback
    }
    pub fn package_inputs(&self) -> Option<&PackageCompilationInputs> {
        self.package_inputs.as_ref()
    }

    /// Release this target's native product request; the shared invocation
    /// keeps only what native production needs.
    pub fn into_native_product_request(self) -> NativeProductRequest {
        NativeProductRequest {
            private_entry_ram: self.policy.private_entry_ram.clone(),
            terminal_authority_policy: self.policy.terminal_authority_policy.clone(),
            terminal_authority_permission_policy: self
                .policy
                .terminal_authority_permission_policy
                .clone(),
            optimization_rollback: self.policy.optimization_rollback.clone(),
        }
    }
}
