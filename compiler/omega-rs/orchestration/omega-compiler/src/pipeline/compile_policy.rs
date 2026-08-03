use psi_diagnostics::Diagnostic;

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

#[cfg(test)]
mod tests {
    use super::*;
    use omega_effects::{ExecutionScope, IncompleteScopePolicy};

    fn profile(name: &str) -> omega_effects::ExecutableTcbProfile {
        omega_effects::ExecutableTcbProfile {
            name: name.into(),
            scope: ExecutionScope::CallerAddressSpace,
            allow_static_current_artifact_checked_bodies: true,
            exact_allowances: Vec::new(),
            incomplete_scope: IncompleteScopePolicy::Reject,
        }
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
