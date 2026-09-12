//! One invocation owns shared input and independently configured target products.

use crate::compiler::{ArtifactEmissionPolicy, CompileOptions, OptimizationRollback};
use diagnostics::Diagnostic;
use package_compilation::{
    PackageCompilationInputs, PackageCompilationSourceInputs, PackageCompilationTargetInputs,
};
use std::path::PathBuf;
use std::sync::Arc;
use target::TargetProfile;

mod outcomes;
mod targets;
pub use outcomes::{CompileOutcomes, CompileTargetOutcome};
pub use targets::ExplicitTargetSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestedCompileProduct {
    Check,
    TerminalArtifact,
    NativeArtifact,
}

/// Input shared by every selected target. It cannot disagree between children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SharedCompileInputs {
    pub(super) root_path: PathBuf,
    pub(super) requested_product: RequestedCompileProduct,
    pub(super) artifact_policy: ArtifactEmissionPolicy,
    package_sources: Option<Arc<PackageCompilationSourceInputs>>,
}

/// Target-local policy, generated inputs and output placement. Source roots and
/// the requested product belong to the invocation, not to this configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetCompileConfiguration {
    target_name: Option<String>,
    build_dir: Option<PathBuf>,
    pub(super) terminal_admission_profile: proof_admission::AdmissionProfile,
    pub(super) terminal_authority_permission_policy:
        native_realization::TerminalAuthorityPermissionPolicy,
    accepted_trust_admissions: Vec<trust_model::TrustAdmission>,
    package_target_inputs: PackageCompilationTargetInputs,
    pub(super) optimization_rollback: OptimizationRollback,
}

impl TargetCompileConfiguration {
    pub fn new(target: TargetProfile) -> Self {
        Self::from_options(Some(target.target_name().to_owned()), None)
    }

    fn from_options(target_name: Option<String>, build_dir: Option<PathBuf>) -> Self {
        Self {
            target_name,
            build_dir,
            terminal_admission_profile: proof_admission::AdmissionProfile::default(),
            terminal_authority_permission_policy:
                native_realization::current_terminal_authority_permission_policy(),
            accepted_trust_admissions: Vec::new(),
            package_target_inputs: PackageCompilationTargetInputs::default(),
            optimization_rollback: OptimizationRollback::default(),
        }
    }

    pub fn with_build_dir(mut self, directory: PathBuf) -> Self {
        self.build_dir = Some(directory);
        self
    }
    pub fn with_admission_profile(mut self, profile: proof_admission::AdmissionProfile) -> Self {
        self.terminal_admission_profile = profile;
        self
    }
    pub fn with_terminal_authority_permission_policy(
        mut self,
        policy: native_realization::TerminalAuthorityPermissionPolicy,
    ) -> Self {
        self.terminal_authority_permission_policy = policy;
        self
    }
    pub fn with_accepted_trust_admissions(
        mut self,
        admissions: Vec<trust_model::TrustAdmission>,
    ) -> Self {
        self.accepted_trust_admissions = admissions;
        self
    }
    pub fn with_package_target_inputs(mut self, inputs: PackageCompilationTargetInputs) -> Self {
        self.package_target_inputs = inputs;
        self
    }
    pub fn with_optimization_rollback(mut self, rollback: OptimizationRollback) -> Self {
        self.optimization_rollback = rollback;
        self
    }
}

/// One production invocation, including the one-target case. This Rust-host
/// request is not a canonical OCREQ frame and grants no bootstrap authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileRequest {
    shared: SharedCompileInputs,
    configurations: Vec<TargetCompileConfiguration>,
}

impl CompileRequest {
    /// Create one checking configuration. An absent target remains target-neutral
    /// for checking/Terminal production; native production resolves it to Host.
    pub fn new(options: CompileOptions) -> Self {
        Self {
            shared: SharedCompileInputs {
                root_path: options.root_path,
                requested_product: RequestedCompileProduct::Check,
                artifact_policy: ArtifactEmissionPolicy::Full,
                package_sources: None,
            },
            configurations: vec![TargetCompileConfiguration::from_options(
                options.target_name,
                options.build_dir,
            )],
        }
    }

    /// Replace the initial configuration with the explicitly supplied targets.
    /// Each configuration owns its policies; this does not inherit discarded
    /// configuration defaults. Empty, duplicate and colliding targets reject.
    pub fn with_target_configurations(
        mut self,
        configurations: Vec<TargetCompileConfiguration>,
    ) -> Self {
        self.configurations = configurations;
        self
    }
    pub fn with_requested_product(mut self, product: RequestedCompileProduct) -> Self {
        self.shared.requested_product = product;
        self
    }
    pub fn with_artifact_policy(mut self, policy: ArtifactEmissionPolicy) -> Self {
        self.shared.artifact_policy = policy;
        self
    }
    pub fn with_package_sources(mut self, sources: Arc<PackageCompilationSourceInputs>) -> Self {
        self.shared.package_sources = Some(sources);
        self
    }

    /// Supply shared sources and the same target inputs to every current
    /// configuration. Distinct target bundles belong on their configurations.
    pub fn with_package_inputs(mut self, inputs: PackageCompilationInputs) -> Self {
        let (source, target) = inputs.into_parts();
        self.shared.package_sources = Some(source);
        if let Some((last, earlier)) = self.configurations.split_last_mut() {
            for configuration in earlier {
                configuration.package_target_inputs = target.clone();
            }
            last.package_target_inputs = target;
        }
        self
    }

    /// Apply a common admission choice to every current configuration.
    pub fn with_admission_profile(mut self, profile: proof_admission::AdmissionProfile) -> Self {
        for configuration in &mut self.configurations {
            configuration.terminal_admission_profile = profile.clone();
        }
        self
    }
    pub fn with_terminal_authority_permission_policy(
        mut self,
        policy: native_realization::TerminalAuthorityPermissionPolicy,
    ) -> Self {
        for configuration in &mut self.configurations {
            configuration.terminal_authority_permission_policy = policy.clone();
        }
        self
    }
    pub fn with_accepted_trust_admissions(
        mut self,
        admissions: Vec<trust_model::TrustAdmission>,
    ) -> Self {
        for configuration in &mut self.configurations {
            configuration.accepted_trust_admissions = admissions.clone();
        }
        self
    }
    pub fn with_optimization_rollback(mut self, rollback: OptimizationRollback) -> Self {
        for configuration in &mut self.configurations {
            configuration.optimization_rollback = rollback.clone();
        }
        self
    }

    pub(super) fn validate_for_execution(self) -> Result<ValidatedCompileRequest, Vec<Diagnostic>> {
        if self.configurations.is_empty() {
            return Err(vec![Diagnostic::error(
                "compilation requires at least one target configuration",
            )]);
        }
        let multiple = self.configurations.len() > 1;
        let shared = Arc::new(self.shared);
        let mut targets =
            Vec::<ValidatedTargetCompilation>::with_capacity(self.configurations.len());
        let mut diagnostics = Vec::new();
        for mut configuration in self.configurations {
            if multiple && configuration.target_name.is_none() {
                diagnostics.push(Diagnostic::error(
                    "multiple configurations must each name an exact target",
                ));
                continue;
            }
            if shared.requested_product == RequestedCompileProduct::NativeArtifact
                && configuration.target_name.is_none()
            {
                configuration.target_name = Some(TargetProfile::host().target_name().to_owned());
            }
            let profile = match configuration
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
            let unavailable = configuration
                .optimization_rollback
                .requested_disabled()
                .as_slice()
                .iter()
                .filter(|optimization| match shared.requested_product {
                    RequestedCompileProduct::Check => true,
                    RequestedCompileProduct::TerminalArtifact => {
                        optimization.execution_phase()
                            != optimization_core::OptimizationExecutionPhase::Psi
                    }
                    RequestedCompileProduct::NativeArtifact => false,
                })
                .map(|optimization| format!("`{}`", optimization.build_case_name()))
                .collect::<Vec<_>>();
            if !unavailable.is_empty() {
                diagnostics.push(Diagnostic::error(format!(
                    "optimization rollback {} names stages not executed by {:?}",
                    unavailable.join(", "),
                    shared.requested_product
                )));
                continue;
            }
            let options = CompileOptions {
                root_path: shared.root_path.clone(),
                target_name: profile.map(|profile| profile.target_name().to_owned()),
                build_dir: configuration.build_dir.clone(),
            };
            if targets.iter().any(|target| target.profile == profile) {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate compilation target {:?}",
                    profile
                )));
            }
            if targets
                .iter()
                .any(|target| target.options.build_dir() == options.build_dir())
            {
                diagnostics.push(Diagnostic::error(format!("target configurations name the same build directory `{}`; each target requires separate staging", options.build_dir().display())));
            }
            let target_inputs = std::mem::take(&mut configuration.package_target_inputs);
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
                shared: shared.clone(),
                options,
                configuration,
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
pub(super) struct ValidatedCompileRequest {
    pub(super) shared: Arc<SharedCompileInputs>,
    pub(super) targets: Vec<ValidatedTargetCompilation>,
}

/// One admitted execution of the shared invocation, not another public request.
#[derive(Debug)]
pub(super) struct ValidatedTargetCompilation {
    shared: Arc<SharedCompileInputs>,
    pub(super) options: CompileOptions,
    pub(super) configuration: TargetCompileConfiguration,
    package_inputs: Option<PackageCompilationInputs>,
    pub(super) profile: Option<TargetProfile>,
}

impl ValidatedTargetCompilation {
    pub(super) fn options(&self) -> &CompileOptions {
        &self.options
    }
    pub(super) fn artifact_policy(&self) -> ArtifactEmissionPolicy {
        self.shared.artifact_policy
    }
    pub(super) fn accepted_trust_admissions(&self) -> &[trust_model::TrustAdmission] {
        &self.configuration.accepted_trust_admissions
    }
    pub(super) fn package_inputs(&self) -> Option<&PackageCompilationInputs> {
        self.package_inputs.as_ref()
    }
}

#[cfg(test)]
mod tests;
