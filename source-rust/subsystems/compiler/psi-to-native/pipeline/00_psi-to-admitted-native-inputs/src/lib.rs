#![forbid(unsafe_code)]
//! Admit target names, product options, rollback requests, and installation
//! assumptions before an individual target starts source or native planning.

use diagnostics::Diagnostic;
use optimization_requests::OptimizationRollback;
pub use provider_admission::provider_admission::{
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy, current_terminal_authority_policy,
};
use std::path::PathBuf;
use target::TargetProfile;
pub use terminal_installation::PrivateEntryRamInitialization;

/// Admit the host's producer decision for the program before its providers are selected.
pub fn admit_published_provider_demand<Publisher>(
    program: std::sync::Arc<terminal_codec::CanonicalTerminalArtifact>,
    producer: Option<&terminal_installation::HostProducerTrust>,
    demand: backend_plan::IssuedProviderDemand<Publisher>,
) -> Result<
    backend_plan::AdmittedProviderDemand<Publisher>,
    terminal_installation::ArtifactTrustError,
> {
    let authorization = terminal_installation::authorize_terminal_artifact(program, producer)?;
    backend_plan::AdmittedProviderDemand::new(demand, authorization)
}

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
}

#[derive(Clone)]
pub struct NativeProductRequest {
    /// Export the serialized derivation record used by separate package review.
    pub retain_native_inputs: bool,
    /// Keep original native stage owners for a requested correspondence audit.
    pub retain_native_stage_inputs: bool,
    pub installation_assumptions: Vec<installation_evidence::InstallationEnvironmentAssumption>,
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
    rows: &[installation_evidence::InstallationEnvironmentAssumption],
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
    rows: Vec<installation_evidence::InstallationEnvironmentAssumption>,
    targets: &[TargetProfile],
) -> Result<Vec<Vec<installation_evidence::InstallationEnvironmentAssumption>>, String> {
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
    PublishedPsi,
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
                Self::TerminalArtifact | Self::PublishedPsi => {
                    optimization.execution_phase()
                        != optimization_core::OptimizationExecutionPhase::Psi
                }
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

    pub fn admit_build_requirements(
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

/// Each returned destination owns exactly the assumptions for its target.
pub struct AdmittedDestination {
    pub profile: Option<TargetProfile>,
    pub installation_assumptions: Vec<installation_evidence::InstallationEnvironmentAssumption>,
}

/// Package acquisition supplies any diagnostics from joining its captured
/// inputs. Report them at their original position in each destination's checks.
pub fn admit_destinations(
    destinations: Vec<Option<String>>,
    product: RequestedCompileProduct,
    rollback: &OptimizationRollback,
    installation_assumptions: Vec<installation_evidence::InstallationEnvironmentAssumption>,
    package_errors: Option<&[Diagnostic]>,
) -> Result<Vec<AdmittedDestination>, Vec<Diagnostic>> {
    if destinations.is_empty() {
        return Err(vec![Diagnostic::error(
            "compilation requires at least one target configuration",
        )]);
    }
    let multiple = destinations.len() > 1;
    if !installation_assumptions.is_empty() && product != RequestedCompileProduct::NativeArtifact {
        return Err(vec![Diagnostic::error(
            "installation assumptions require a native product",
        )]);
    }
    let mut targets = Vec::<AdmittedDestination>::with_capacity(destinations.len());
    let mut diagnostics = Vec::new();
    for mut target_name in destinations {
        if multiple && target_name.is_none() {
            diagnostics.push(Diagnostic::error(
                "multiple configurations must each name an exact target",
            ));
            continue;
        }
        if product == RequestedCompileProduct::NativeArtifact && target_name.is_none() {
            match TargetProfile::host_if_supported() {
                Some(host) => {
                    target_name = Some(host.target_name().to_owned());
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
        let profile = match target_name
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
        if let Err(error) = product.validate_optimization_rollback(rollback) {
            diagnostics.push(error);
            continue;
        }
        if targets.iter().any(|target| target.profile == profile) {
            diagnostics.push(Diagnostic::error(format!(
                "duplicate compilation target {:?}",
                profile
            )));
        }
        if let Some(errors) = package_errors {
            diagnostics.extend_from_slice(errors);
            continue;
        }
        targets.push(AdmittedDestination {
            installation_assumptions: Vec::new(),
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
    Ok(targets)
}

#[cfg(test)]
mod tests {
    use super::admit_published_provider_demand;
    use backend_plan::{IssuedProviderDemand, ProviderDemandInputs};
    use std::sync::Arc;
    use terminal_codec::{
        CanonicalTerminalArtifact, TerminalDebugMap, build_identity_optimization_execution_record,
    };
    use terminal_installation::{ArtifactTrustError, HostProducerTrust};

    #[test]
    fn provider_admission_binds_host_trust_to_the_exact_published_artifact() {
        let module = terminal_psi::TerminalModule::declarations();
        let record = build_identity_optimization_execution_record(&module).unwrap();
        let original = Arc::new(
            CanonicalTerminalArtifact::from_parts(module.clone(), record.clone(), None).unwrap(),
        );
        let issue = || {
            IssuedProviderDemand::issue(
                (),
                ProviderDemandInputs::Unscoped {
                    artifact: Arc::clone(&original),
                    roots: Vec::new(),
                },
            )
        };

        let reopened =
            Arc::new(CanonicalTerminalArtifact::from_bytes(&original.to_bytes()).unwrap());
        assert!(matches!(
            admit_published_provider_demand(Arc::clone(&reopened), None, issue()),
            Err(ArtifactTrustError::ProducerTrustRequired)
        ));
        let host = HostProducerTrust::accept("admission test");
        let (_, admitted) =
            admit_published_provider_demand(Arc::clone(&reopened), Some(&host), issue())
                .unwrap()
                .into_parts();
        assert!(Arc::ptr_eq(admitted.shared_artifact(), &reopened));

        let debug = TerminalDebugMap {
            semantic: original.view().identity(),
            files: Vec::new(),
            sites: Vec::new(),
        };
        let changed =
            Arc::new(CanonicalTerminalArtifact::from_parts(module, record, Some(&debug)).unwrap());
        assert_eq!(changed.view().identity(), original.view().identity());
        let authorized_identity = changed.manifest().identity();
        let demanded_identity = original.manifest().identity();
        assert_ne!(authorized_identity, demanded_identity);
        assert!(matches!(
            admit_published_provider_demand(changed, Some(&host), issue()),
            Err(ArtifactTrustError::ArtifactMismatch { authorized, requested })
                if authorized == authorized_identity && requested == demanded_identity
        ));
    }
}
