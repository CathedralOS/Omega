use psi_diagnostics::Diagnostic;
use std::sync::Arc;

/// Deployment-owned executable-TCB inputs for one compiler invocation.
///
/// These values are deliberately separate from source `build.omg` syntax.
/// Opaque executable evidence and platform/profile allowlists are supplied by
/// the build or deployment authority, then matched against the provider plans
/// that this exact compilation selected.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecutableTcbBuildPolicy {
    pub opaque_executable_admissions: Vec<omega_effects::OpaqueExecutableAdmissionCandidate>,
    pub profile: Option<omega_effects::ExecutableTcbProfile>,
}

/// Sealed authorization carried from selected-provider validation to the
/// filesystem installation point. The accepted profile retains the exact
/// manifest it evaluated, so later code cannot substitute a different TCB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ExecutableTcbInstallationAuthorization {
    NoProfileSelected,
    ProfileAccepted(omega_effects::ExecutableTcbProfileAcceptance),
}

impl ExecutableTcbInstallationAuthorization {
    pub(super) fn bind(
        selected: &omega_effects::SelectedProviderPlanFacts,
        profile: Option<&omega_effects::ExecutableTcbProfile>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let Some(profile) = profile else {
            return Ok(Self::NoProfileSelected);
        };
        let manifest = selected.executable_tcb_manifest();
        omega_effects::evaluate_executable_tcb_profile(&manifest, profile)
            .map(Self::ProfileAccepted)
            .map_err(|rejection| {
                vec![Diagnostic::error(format!(
                    "executable TCB profile `{}` rejected artifact installation: {:?}",
                    rejection.profile, rejection.violations
                ))]
            })
    }

    /// Consume the sealed gate immediately before any output path is created.
    pub(super) const fn authorize_installation(&self) {
        match self {
            Self::NoProfileSelected => {}
            Self::ProfileAccepted(acceptance) => {
                let _ = acceptance.manifest();
            }
        }
    }
}

/// Complete the compiler-owned provider and executable-TCB transaction before
/// publishing the selected-provider sidecar to later pipeline stages.
pub(super) fn settle_compiler_executable_tcb_installation(
    checked: &mut super::stages::CheckedProgramSurface,
    provider_candidates: &[omega_effects::provider_plan::ProviderPlan],
    selected: omega_effects::SelectedProviderPlanFacts,
    root_grants: &[String],
    policy: &ExecutableTcbBuildPolicy,
) -> Result<ExecutableTcbInstallationAuthorization, Vec<Diagnostic>> {
    let selected = super::provider_plans::bind_selected_provider_plan_facts(
        Arc::get_mut(&mut checked.program)
            .expect("checked program must be uniquely owned before backend fan-out"),
        provider_candidates,
        selected,
        root_grants,
    )?
    .with_opaque_executable_admissions(policy.opaque_executable_admissions.iter().cloned())
    .map_err(|reason| {
        vec![Diagnostic::error(format!(
            "executable TCB admission rejected: {reason}"
        ))]
    })?;
    let authorization =
        ExecutableTcbInstallationAuthorization::bind(&selected, policy.profile.as_ref())?;

    checked.selected_provider_plans = Arc::new(selected);
    Ok(authorization)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_effects::{
        ExecutionScope, IncompleteScopePolicy, OpaqueExecutableAdmissionCandidate,
        OpaqueInProcessBinding,
    };

    fn profile(name: &str) -> omega_effects::ExecutableTcbProfile {
        omega_effects::ExecutableTcbProfile {
            name: name.into(),
            scope: ExecutionScope::CallerAddressSpace,
            allow_static_current_artifact_checked_bodies: true,
            exact_allowances: Vec::new(),
            incomplete_scope: IncompleteScopePolicy::Reject,
        }
    }

    fn checked_surface() -> super::super::stages::CheckedProgramSurface {
        super::super::stages::CheckedProgramSurface {
            program: Arc::new(psi_checked_trees::CheckedTrees::default()),
            selected_provider_plans: Arc::new(omega_effects::SelectedProviderPlanFacts::default()),
            component_progress: None,
            task_activations: Arc::new(omega_task_plans::TaskActivationPlanSet::default()),
            callback_placements: Arc::from([]),
        }
    }

    fn wrong_scope_admission() -> OpaqueExecutableAdmissionCandidate {
        OpaqueExecutableAdmissionCandidate {
            provider_plan_identity: 7,
            method: "read".into(),
            requirement_identity: "Storage::read#exact".into(),
            binding: OpaqueInProcessBinding::StringBackedImportBootstrap {
                library: "platform-storage".into(),
                symbol: "read".into(),
            },
            executable_identity: "platform-baseline:storage-v1".into(),
            implementation_evidence_identity: "receipt:storage-v1".into(),
            execution_scope: ExecutionScope::IsolatedProvider(7),
            containment: Vec::new(),
            executable_closure_evidence_identity: None,
        }
    }

    #[test]
    fn default_policy_commits_the_bound_selected_sidecar_after_authorization() {
        let mut checked = checked_surface();
        let original_sidecar = Arc::clone(&checked.selected_provider_plans);

        let authorization = settle_compiler_executable_tcb_installation(
            &mut checked,
            &[],
            omega_effects::SelectedProviderPlanFacts::default(),
            &[],
            &ExecutableTcbBuildPolicy::default(),
        )
        .expect("default policy should authorize an empty selected closure");

        assert!(matches!(
            authorization,
            ExecutableTcbInstallationAuthorization::NoProfileSelected
        ));
        assert!(!Arc::ptr_eq(
            &original_sidecar,
            &checked.selected_provider_plans
        ));
        assert_eq!(
            checked.selected_provider_plans.as_ref(),
            &omega_effects::SelectedProviderPlanFacts::default()
        );
    }

    #[test]
    fn valid_profile_commits_the_exact_manifest_accepted_by_authorization() {
        let mut checked = checked_surface();
        let original_sidecar = Arc::clone(&checked.selected_provider_plans);
        let policy = ExecutableTcbBuildPolicy {
            opaque_executable_admissions: Vec::new(),
            profile: Some(profile("empty-artifact")),
        };

        let authorization = settle_compiler_executable_tcb_installation(
            &mut checked,
            &[],
            omega_effects::SelectedProviderPlanFacts::default(),
            &[],
            &policy,
        )
        .expect("complete empty selected closure should satisfy profile");

        let ExecutableTcbInstallationAuthorization::ProfileAccepted(acceptance) = authorization
        else {
            panic!("selected profile must produce a sealed acceptance");
        };
        assert!(!Arc::ptr_eq(
            &original_sidecar,
            &checked.selected_provider_plans
        ));
        assert_eq!(
            acceptance.manifest(),
            &checked.selected_provider_plans.executable_tcb_manifest()
        );
    }

    #[test]
    fn invalid_opaque_admission_leaves_the_selected_sidecar_uncommitted() {
        let mut checked = checked_surface();
        let original_sidecar = Arc::clone(&checked.selected_provider_plans);
        let policy = ExecutableTcbBuildPolicy {
            opaque_executable_admissions: vec![wrong_scope_admission()],
            profile: None,
        };

        let diagnostics = settle_compiler_executable_tcb_installation(
            &mut checked,
            &[],
            omega_effects::SelectedProviderPlanFacts::default(),
            &[],
            &policy,
        )
        .expect_err("scope-substituted opaque admission must reject");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            "executable TCB admission rejected: opaque executable admission scope IsolatedProvider(7) does not match selected closure scope CallerAddressSpace"
        );
        assert!(Arc::ptr_eq(
            &original_sidecar,
            &checked.selected_provider_plans
        ));
    }

    #[test]
    fn invalid_profile_leaves_the_selected_sidecar_uncommitted() {
        let mut checked = checked_surface();
        let original_sidecar = Arc::clone(&checked.selected_provider_plans);
        let policy = ExecutableTcbBuildPolicy {
            opaque_executable_admissions: Vec::new(),
            profile: Some(profile("")),
        };

        let diagnostics = settle_compiler_executable_tcb_installation(
            &mut checked,
            &[],
            omega_effects::SelectedProviderPlanFacts::default(),
            &[],
            &policy,
        )
        .expect_err("invalid profile must reject before sidecar commitment");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            "executable TCB profile `` rejected artifact installation: [EmptyProfileIdentity]"
        );
        assert!(Arc::ptr_eq(
            &original_sidecar,
            &checked.selected_provider_plans
        ));
    }

    #[test]
    fn exact_profile_acceptance_is_carried_to_installation() {
        let selected = omega_effects::SelectedProviderPlanFacts::default();
        let authorization = ExecutableTcbInstallationAuthorization::bind(
            &selected,
            Some(&profile("empty-artifact")),
        )
        .expect("complete empty selected closure should satisfy profile");

        let ExecutableTcbInstallationAuthorization::ProfileAccepted(acceptance) = authorization
        else {
            panic!("selected profile must produce a sealed acceptance");
        };
        assert_eq!(acceptance.profile().name, "empty-artifact");
        assert_eq!(acceptance.manifest(), &selected.executable_tcb_manifest());
    }

    #[test]
    fn rejected_profile_cannot_mint_installation_authorization() {
        let selected = omega_effects::SelectedProviderPlanFacts::default();
        let diagnostics =
            ExecutableTcbInstallationAuthorization::bind(&selected, Some(&profile("")))
                .expect_err("invalid profile must reject before installation");

        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("rejected artifact installation")
        );
    }
}
