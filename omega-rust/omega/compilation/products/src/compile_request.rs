/*
Compilation requests, target-local checking inputs, and native product policies.

One invocation owns shared source inputs, Build staging, and policy while
targets select realizations. `CompileRequest::validate_for_execution` validates
target destinations, optimization rollback eligibility, package input custody,
and installation-assumption target partitioning before any frontend or Build
work runs.
*/

/*
`CheckedCompileRequest` and `CheckedExecutionRequest` admit target-local
checking and Build execution inputs without carrying native publication
authority. `NativeProductRequest` carries the two independent authority axes
for native realization: `terminal_authority_policy` (the receiving authority's
explicit mechanism-classification table) and
`terminal_authority_permission_policy` (the optional receiver-admission claim,
which defaults to `None` so ordinary production never fabricates a receiving
permission policy).
*/

use diagnostics::Diagnostic;
use optimization_core::OptimizationRollback;
use package_compilation::build_execution::BuildSnapshotRequest;
use package_compilation::{
    BuildExecutionInputs, PackageCompilationInputs, PackageCompilationSourceInputs,
    PackageCompilationTargetInputs,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use target::TargetProfile;
pub use terminal_installation::PrivateEntryRamInitialization;
pub use terminal_psi_to_abstract_operations::provider_admission::{
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy, current_terminal_authority_policy,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileOptions {
    pub root_path: PathBuf,
    pub build_dir: Option<PathBuf>,
    pub target_name: Option<String>,
}

impl CompileOptions {
    pub fn build_dir(&self) -> PathBuf {
        self.build_dir.clone().unwrap_or_else(|| {
            self.root_path
                .parent()
                .map(|parent| parent.join("build"))
                .unwrap_or_else(|| PathBuf::from("build"))
        })
    }

    pub fn retain_build_dir(&mut self) -> PathBuf {
        let build_dir = self.build_dir();
        self.build_dir = Some(build_dir.clone());
        build_dir
    }
}

pub struct CheckedCompileRequest {
    pub root_path: PathBuf,
    pub target_name: Option<String>, // None preserves targetless semantic checking
    pub package_inputs: Option<PackageCompilationInputs>,
    pub source_capture: Option<package_compilation::CapturedLocalSource>, // mutually exclusive with a supplied package graph
    pub build: BuildExecutionInputs,
    pub optimization_rollback: OptimizationRollback,
    pub collect_timings: bool,
}

impl CheckedCompileRequest {
    pub fn new(root_path: &Path, target_name: Option<&str>) -> Self {
        Self {
            root_path: root_path.to_owned(),
            target_name: target_name.map(str::to_owned),
            package_inputs: None,
            source_capture: None,
            build: BuildExecutionInputs::default(),
            optimization_rollback: OptimizationRollback::default(),
            collect_timings: false,
        }
    }
}

pub struct CheckedExecutionRequest<'request> {
    pub selected_target_profile: Option<TargetProfile>,
    pub package_inputs: Option<&'request PackageCompilationInputs>,
    pub build: BuildExecutionInputs,
    pub optimization_rollback: OptimizationRollback,
}

impl CheckedExecutionRequest<'_> {
    pub fn admit_package_inputs(&self) -> Result<(), Vec<Diagnostic>> {
        let Some(inputs) = self.package_inputs else {
            return Ok(());
        };
        inputs.validate_execution_context(self.selected_target_profile, self.build.profile)
    }
}

#[derive(Clone)]
pub struct NativeProductRequest {
    pub retain_native_inputs: bool,
    pub installation_assumptions: Vec<crate::InstallationEnvironmentAssumption>,
    pub image: NativeImageDemand,
    pub private_entry_ram: Vec<PrivateEntryRamInitialization>,
    pub terminal_authority_policy: TerminalAuthorityPolicy,
    pub terminal_authority_permission_policy: Option<TerminalAuthorityPermissionPolicy>,
    pub optimization_rollback: OptimizationRollback,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NativeImageDemand {
    #[default]
    Emit,
    RuntimeAdmissionOnly,
}

pub fn validate_installation_assumption_targets(
    rows: &[crate::InstallationEnvironmentAssumption],
    targets: &[TargetProfile],
) -> Result<(), String> {
    for row in rows {
        row.validate_description()?;
        if !targets.contains(&row.profile()) {
            return Err(format!(
                "installation assumption names unrequested target {}",
                row.profile().target_name()
            ));
        }
    }
    Ok(())
}

pub fn partition_installation_assumptions(
    rows: Vec<crate::InstallationEnvironmentAssumption>,
    targets: &[TargetProfile],
) -> Result<Vec<Vec<crate::InstallationEnvironmentAssumption>>, String> {
    validate_installation_assumption_targets(&rows, targets)?;
    let mut groups = vec![Vec::new(); targets.len()];
    for row in rows {
        let position = targets
            .iter()
            .position(|target| *target == row.profile())
            .ok_or("installation assumption lost its validated target")?;
        groups[position].push(row);
    }
    Ok(groups)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestedCompileProduct {
    Check,
    TerminalArtifact,
    NativeArtifact,
}

impl RequestedCompileProduct {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedCompileInputs {
    pub root_path: PathBuf,
    pub build_directory: PathBuf,
    pub requested_product: RequestedCompileProduct,
    pub timings: bool,
    pub package_sources: Option<Arc<PackageCompilationSourceInputs>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RequestedDestination {
    target_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompilePolicy {
    native_image: NativeImageDemand,
    private_entry_ram: Vec<PrivateEntryRamInitialization>,
    installation_assumptions: Vec<crate::InstallationEnvironmentAssumption>,
    pub(super) build_snapshot: Option<BuildSnapshotRequest>,
    terminal_authority_policy: TerminalAuthorityPolicy,
    pub(super) terminal_authority_permission_policy: Option<TerminalAuthorityPermissionPolicy>,
    package_target_inputs: PackageCompilationTargetInputs,
    pub(super) optimization_rollback: OptimizationRollback,
}

impl Default for CompilePolicy {
    fn default() -> Self {
        Self {
            native_image: NativeImageDemand::Emit,
            private_entry_ram: Vec::new(),
            installation_assumptions: Vec::new(),
            build_snapshot: None,
            terminal_authority_policy: current_terminal_authority_policy(),
            terminal_authority_permission_policy: None,
            package_target_inputs: PackageCompilationTargetInputs::default(),
            optimization_rollback: OptimizationRollback::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileRequest {
    shared: SharedCompileInputs,
    policy: CompilePolicy,
    destinations: Vec<RequestedDestination>,
}

impl CompileRequest {
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

    pub fn with_native_image_demand(mut self, demand: NativeImageDemand) -> Self {
        self.policy.native_image = demand;
        self
    }

    pub fn with_installation_assumptions(
        mut self,
        inputs: Vec<crate::InstallationEnvironmentAssumption>,
    ) -> Self {
        self.policy.installation_assumptions = inputs;
        self
    }

    pub fn with_bootstrap_assumptions(
        mut self,
        inputs: Vec<crate::BootstrapEnvironmentAssumption>,
    ) -> Self {
        self.policy.installation_assumptions.extend(
            inputs
                .into_iter()
                .map(crate::InstallationEnvironmentAssumption::Bootstrap),
        );
        self
    }

    pub fn with_interrupt_delivery_assumptions(
        mut self,
        inputs: Vec<crate::X86InterruptDeliveryAssumption>,
    ) -> Self {
        self.policy.installation_assumptions.extend(
            inputs
                .into_iter()
                .map(crate::InstallationEnvironmentAssumption::InterruptDelivery),
        );
        self
    }

    pub fn with_private_entry_ram(mut self, inputs: Vec<PrivateEntryRamInitialization>) -> Self {
        self.policy.private_entry_ram = inputs;
        self
    }

    pub fn with_requested_product(mut self, product: RequestedCompileProduct) -> Self {
        self.shared.requested_product = product;
        self
    }

    pub fn with_build_snapshot(mut self, snapshot: BuildSnapshotRequest) -> Self {
        self.policy.build_snapshot = Some(snapshot);
        self
    }

    pub fn with_package_inputs(mut self, inputs: PackageCompilationInputs) -> Self {
        let (source, target) = inputs.into_parts();
        self.shared.package_sources = Some(source);
        self.policy.package_target_inputs = target;
        self
    }

    pub fn with_terminal_authority_policy(mut self, policy: TerminalAuthorityPolicy) -> Self {
        self.policy.terminal_authority_policy = policy;
        self
    }

    pub fn with_optimization_rollback(mut self, rollback: OptimizationRollback) -> Self {
        self.policy.optimization_rollback = rollback;
        self
    }

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
        let mut policy = self.policy;
        let installation_assumptions = std::mem::take(&mut policy.installation_assumptions);
        if !installation_assumptions.is_empty()
            && shared.requested_product != RequestedCompileProduct::NativeArtifact
        {
            return Err(vec![Diagnostic::error(
                "installation assumptions require a native product",
            )]);
        }
        let policy = Arc::new(policy);
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
                installation_assumptions: Vec::new(),
                package_inputs,
                profile,
            });
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        let profiles = targets
            .iter()
            .filter_map(|target| target.profile)
            .collect::<Vec<_>>();
        let groups = partition_installation_assumptions(installation_assumptions, &profiles)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let mut groups = groups.into_iter();
        for target in &mut targets {
            if target.profile.is_some() {
                target.installation_assumptions = groups.next().ok_or_else(|| {
                    vec![Diagnostic::error(
                        "installation assumption partition lost a target",
                    )]
                })?;
            }
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
    pub fn into_parts(self) -> (Arc<SharedCompileInputs>, Vec<ValidatedTargetCompilation>) {
        (self.shared, self.targets)
    }
}

#[derive(Debug)]
pub struct ValidatedTargetCompilation {
    policy: Arc<CompilePolicy>,
    installation_assumptions: Vec<crate::InstallationEnvironmentAssumption>,
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

    pub fn checked_execution<'request>(
        &'request self,
        build_directory: &Path,
    ) -> CheckedExecutionRequest<'request> {
        let package_inputs = self.package_inputs();
        CheckedExecutionRequest {
            selected_target_profile: self.profile(),
            package_inputs,
            build: BuildExecutionInputs {
                directory: Some(build_directory.to_path_buf()),
                snapshot: self.build_snapshot().cloned(),
                ..BuildExecutionInputs::default()
            }
            .resolve_compilation_defaults(package_inputs),
            optimization_rollback: self.optimization_rollback().clone(),
        }
    }

    pub fn into_native_product_request(self) -> NativeProductRequest {
        NativeProductRequest {
            retain_native_inputs: false,
            image: self.policy.native_image,
            private_entry_ram: self.policy.private_entry_ram.clone(),
            installation_assumptions: self.installation_assumptions,
            terminal_authority_policy: self.policy.terminal_authority_policy.clone(),
            terminal_authority_permission_policy: self
                .policy
                .terminal_authority_permission_policy
                .clone(),
            optimization_rollback: self.policy.optimization_rollback.clone(),
        }
    }
}
