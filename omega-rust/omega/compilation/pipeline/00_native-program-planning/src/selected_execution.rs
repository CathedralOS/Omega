// Selected execution owns one realized provider closure. Native and Psi
// consumers borrow its canonical plan roster and exact row execution data;
// choosing a provider never reconstructs declarations from source later.

use diagnostics::Diagnostic;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedExecution {
    target_profile: Option<target::TargetProfile>,
    realization: Arc<crate::target_supply::RealizedTargetSupply>,
}

impl SelectedExecution {
    /// Retains the provider realization already formed for this exact target.
    pub(super) fn from_realization(
        target_profile: Option<target::TargetProfile>,
        realization: crate::target_supply::RealizedTargetSupply,
    ) -> Result<Self, Vec<Diagnostic>> {
        if realization.target() != target_profile.map(|profile| profile.target_name()) {
            return Err(vec![Diagnostic::error(
                "selected execution and realized provider supply name different targets",
            )]);
        }
        Ok(Self {
            target_profile,
            realization: Arc::new(realization),
        })
    }

    pub fn realization(&self) -> &crate::target_supply::RealizedTargetSupply {
        &self.realization
    }

    /// The deployment profile the request named. Profiles that share a
    /// native ABI (such as windows_x64 and uefi_x64) stay distinct here.
    pub fn target_profile(&self) -> Option<target::TargetProfile> {
        self.target_profile
    }

    pub fn native_target(&self) -> Option<target::NativeTarget> {
        self.target_profile
            .map(target::TargetProfile::native_target)
    }

    pub fn admit_root_grants(
        &self,
        declarations: &declaration_contracts::trust_declarations::TrustDeclarations,
        grants: &[declaration_contracts::selection::AuthoredRootGrant],
        policy: crate::trust::RootTrustPolicy,
        module: Option<&terminal_psi::TerminalModule>,
    ) -> Result<Vec<crate::trust::TrustAdmission>, Vec<Diagnostic>> {
        let plans = self.plans();
        let native = self.native_grant_evidence(grants, module)?;
        crate::trust::reconstruct_trust_obligations(
            declarations, grants, &plans.plans().iter().collect::<Vec<_>>(),
            plans, &native, policy,
        )
    }

    fn native_grant_evidence(
        &self,
        grants: &[declaration_contracts::selection::AuthoredRootGrant],
        module: Option<&terminal_psi::TerminalModule>,
    ) -> Result<Vec<Result<backend_plan::NativeRequirementTrust, String>>, Vec<Diagnostic>> {
        use declaration_contracts::selection::RootGrantSubject;
        let requests = grants
            .iter()
            .filter_map(|grant| {
                let RootGrantSubject::BoundaryRequirement {
                    declaration,
                    callable,
                } = &grant.subject
                else {
                    return None;
                };
                Some((grant, (declaration.as_str(), callable.as_str())))
            })
            .collect::<Vec<_>>();
        if requests.is_empty() {
            return Ok(Vec::new());
        }
        let realization = self.realization();
        let mut leaves = realization
            .direct_boundary_leaves()
            .iter()
            .collect::<Vec<_>>();
        fn key(leaf: &backend_plan::DirectBoundaryLeaf) -> (&str, &str) {
            (&leaf.declaration_identity, &leaf.callable_identity)
        }
        leaves.sort_unstable_by(|left, right| key(left).cmp(&key(right)));
        let module = module.ok_or_else(|| vec![Diagnostic::error("native grant has no accepted program")])?;
        Ok(requests.into_iter().map(|(grant, selected)| {
            let first = leaves.partition_point(|leaf| key(leaf) < selected);
            let last = leaves.partition_point(|leaf| key(leaf) <= selected);
            let [leaf] = &leaves[first..last] else {
                return Err(format!(
                    "root grant `{}` has no unique selected native requirement with its exact declaration and callable application",
                    grant.selector
                ));
            };
            leaf.prepare_native_trust(grant, realization.target(), module,
                realization.evaluated_native_bindings(), realization.evaluated_binding_identities())
        }).collect())
    }

    pub fn plans(&self) -> &effects::SelectedProviderPlanFacts {
        self.realization.plans()
    }

    pub fn shared_plans(&self) -> &Arc<effects::SelectedProviderPlanFacts> {
        self.realization.shared_plans()
    }

    pub fn with_folded_program(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
    ) -> Result<Self, String> {
        Ok(Self {
            target_profile: self.target_profile,
            realization: Arc::new(self.realization.with_folded_program(artifact)?),
        })
    }

    /// Rejoin the actual selected declarations to this canonical artifact.
    pub fn provider_proposal(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
        optimization_selection: optimization_core::OptimizationSelectionIdentity,
    ) -> Option<backend_plan::TerminalTargetProviderProposal> {
        let target = self.target_profile?.target_name();
        let selection = self
            .realization
            .require_program(artifact.identity())
            .and_then(|()| {
                backend_plan::TerminalTargetProviderSelection::from_plans(
                    self.realization.provider_declarations().to_vec(),
                    artifact,
                )
            })
            .map(|mut selection| {
                selection.assumed_bindings = self.plans().assumed_bindings().to_vec();
                selection
            })
            .map_err(|reason| vec![format!("provider proposal for `{target}`: {reason}")]);
        Some(backend_plan::TerminalTargetProviderProposal {
            target: target.to_owned(),
            terminal_psi: artifact.identity(),
            optimization_selection,
            application_roots: None,
            application_required: true,
            selection,
        })
    }
}
