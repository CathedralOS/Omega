#![forbid(unsafe_code)]

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
use optimization_requests::OptimizationRollback;
use package_compilation::build_execution::BuildSnapshotRequest;
use package_compilation::{
    BuildExecutionInputs, PackageCompilationInputs, PackageCompilationSourceInputs,
    PackageCompilationTargetInputs,
};
pub use provider_admission::provider_admission::{
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy, current_terminal_authority_policy,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use target::TargetProfile;
pub use terminal_installation::PrivateEntryRamInitialization;

pub use crate::request_admission::{
    CompileOptions, NativeImageDemand, NativeProductRequest, RequestedCompileProduct,
    partition_installation_assumptions, validate_installation_assumption_targets,
};

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
    retain_native_stage_inputs: bool,
    native_image: NativeImageDemand,
    private_entry_ram: Vec<PrivateEntryRamInitialization>,
    installation_assumptions: Vec<installation_evidence::InstallationEnvironmentAssumption>,
    build_snapshot: Option<BuildSnapshotRequest>,
    terminal_authority_policy: TerminalAuthorityPolicy,
    terminal_authority_permission_policy: Option<TerminalAuthorityPermissionPolicy>,
    package_target_inputs: PackageCompilationTargetInputs,
    optimization_rollback: OptimizationRollback,
}

impl Default for CompilePolicy {
    fn default() -> Self {
        Self {
            retain_native_stage_inputs: false,
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

    /// Retain original native-stage inputs for independent correspondence audits.
    pub fn with_retained_native_stage_inputs(mut self, retain: bool) -> Self {
        self.policy.retain_native_stage_inputs = retain;
        self
    }

    pub fn with_installation_assumptions(
        mut self,
        inputs: Vec<installation_evidence::InstallationEnvironmentAssumption>,
    ) -> Self {
        self.policy.installation_assumptions = inputs;
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

    pub fn with_optimization_rollback(mut self, rollback: OptimizationRollback) -> Self {
        self.policy.optimization_rollback = rollback;
        self
    }

    pub fn with_timings(mut self, timings: bool) -> Self {
        self.shared.timings = timings;
        self
    }

    pub fn validate_for_execution(self) -> Result<ValidatedCompileRequest, Vec<Diagnostic>> {
        let shared = Arc::new(self.shared);
        let mut policy = self.policy;
        let target_inputs = std::mem::take(&mut policy.package_target_inputs);
        let packages = match &shared.package_sources {
            Some(source) => PackageCompilationInputs::from_parts(Arc::clone(source), target_inputs)
                .map(Some)
                .map_err(|errors| {
                    errors
                        .into_iter()
                        .map(|error| Diagnostic::error(error.to_string()))
                        .collect::<Vec<_>>()
                }),
            None if target_inputs == PackageCompilationTargetInputs::default() => Ok(None),
            None => Err(vec![Diagnostic::error(
                "target package inputs require shared package sources",
            )]),
        };
        let admitted = crate::request_admission::admit_destinations(
            self.destinations
                .into_iter()
                .map(|row| row.target_name)
                .collect(),
            shared.requested_product,
            &policy.optimization_rollback,
            std::mem::take(&mut policy.installation_assumptions),
            packages.as_ref().err().map(Vec::as_slice),
        )?;
        let package_inputs = packages?;
        let policy = Arc::new(policy);
        let targets = admitted
            .into_iter()
            .map(|destination| ValidatedTargetCompilation {
                profile: destination.profile,
                installation_assumptions: destination.installation_assumptions,
                package_inputs: package_inputs.clone(),
                policy: Arc::clone(&policy),
            })
            .collect();
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
    installation_assumptions: Vec<installation_evidence::InstallationEnvironmentAssumption>,
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
            retain_native_stage_inputs: self.policy.retain_native_stage_inputs,
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

#[cfg(test)]
mod native_retention_tests {
    use super::{CompileRequest, RequestedCompileProduct, TargetProfile};

    #[test]
    fn original_native_inputs_are_retained_only_on_request() {
        for requested in [false, true] {
            let request = CompileRequest::for_targets(
                "unchanged.omg".into(),
                "build".into(),
                vec![Some(TargetProfile::WindowsX64)],
            )
            .with_requested_product(RequestedCompileProduct::NativeArtifact);
            let request = if requested {
                request.with_retained_native_stage_inputs(true)
            } else {
                request
            };
            let (_, targets) = request.validate_for_execution().unwrap().into_parts();
            let product = targets
                .into_iter()
                .next()
                .unwrap()
                .into_native_product_request();
            assert_eq!(product.retain_native_stage_inputs, requested);
            assert!(
                !product.retain_native_inputs,
                "a stage audit does not export a separate derivation record"
            );
        }
    }
}
