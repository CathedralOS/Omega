//! Exact source, selection, mechanism, and installed-candidate lookup context.

use std::collections::{BTreeMap, BTreeSet};

use effects::{
    SelectedProviderPlanFacts, TerminalMechanismIdentity,
    provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow},
};
use semantic_vocabulary::BoundaryMachineId;

use super::reachability::AuthorityGraph;
use crate::provider_admission::{
    AdmittedTerminalMechanism, TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy,
};

pub(super) struct ReviewContext<'a> {
    pub(super) input: &'a crate::VerifiedPsiOptimizationInput,
    pub(super) graph: AuthorityGraph<'a>,
    pub(super) boundaries:
        BTreeMap<BoundaryMachineId, &'a terminal_psi::BoundaryMachineDeclaration>,
    pub(super) selected: &'a SelectedProviderPlanFacts,
    pub(super) physical_policy: &'a TerminalAuthorityPolicy,
    /// The explicitly supplied receiving permission policy, or `None` when the
    /// review makes no receiver-admission claim. `None` is not deny-all or
    /// allow-all: leaves then record classification and exercised authority
    /// with no adjudicated permission.
    pub(super) permission_policy: Option<&'a TerminalAuthorityPermissionPolicy>,
    pub(super) mechanisms: BTreeMap<BoundaryMachineId, TerminalMechanismIdentity>,
    pub(super) installed_candidates: &'a [terminal_psi::ProviderCandidateConformance],
    target_profile: target::TargetProfile,
}

impl<'a> ReviewContext<'a> {
    pub(super) fn new(
        target_profile: target::TargetProfile,
        input: &'a crate::VerifiedPsiOptimizationInput,
        selected: &'a SelectedProviderPlanFacts,
        physical_policy: &'a TerminalAuthorityPolicy,
        permission_policy: Option<&'a TerminalAuthorityPermissionPolicy>,
        mechanisms: &[AdmittedTerminalMechanism],
        installed_candidates: &'a [terminal_psi::ProviderCandidateConformance],
    ) -> Result<Self, String> {
        let plan = input.plan();
        let graph = AuthorityGraph::new(input, target_profile)?;
        let mut boundaries = BTreeMap::new();
        let mut boundary_identities = BTreeSet::new();
        for boundary in &plan.boundary_machines {
            if boundary.identity == terminal_psi::TerminalIdentityIndex::ABSENT
                || boundaries.insert(boundary.id, boundary).is_some()
                || !boundary_identities.insert(boundary.identity)
            {
                return Err(
                    "abstract operation plan has duplicate or empty boundary identity".into(),
                );
            }
        }
        let mut mechanism_map = BTreeMap::new();
        for admitted in mechanisms {
            if mechanism_map
                .insert(admitted.boundary, admitted.mechanism)
                .is_some()
            {
                return Err(format!(
                    "native settlement repeats terminal mechanism for boundary {:?}",
                    admitted.boundary
                ));
            }
        }
        Ok(Self {
            input,
            graph,
            boundaries,
            selected,
            physical_policy,
            permission_policy,
            mechanisms: mechanism_map,
            installed_candidates,
            target_profile,
        })
    }

    /// The selected plan row serving one reachable requirement, plus the
    /// specialization roster row when the requirement is a closed instance of
    /// a generic template. Terminal declarations name the closed identity;
    /// the selected row retains the template identity its plan derived before
    /// specialization settled.
    pub(super) fn selected_requirement(
        &self,
        requirement: &str,
    ) -> Result<
        (
            &'a ProviderPlan,
            &'a ProviderPlanRow,
            Option<&'a effects::SelectedRequirementSpecialization>,
        ),
        String,
    > {
        let requirement = self.selected.assumed_supplier_identity(requirement)?;
        let specialization = self.selected.specialized_requirement_row(requirement);
        // The absent case stays a real refusal here: provider supply rejects
        // an unselected requirement the entry reaches before settlement, but a
        // selected provider's own nested boundary calls first meet selection
        // in this closure expansion.
        let (plan, row) = self
            .selected
            .selected_requirement_row(requirement)
            .ok_or_else(|| {
                format!(
                    "reachable requirement `{requirement}` resolves to 0 selected provider rows"
                )
            })?;
        if plan.target != self.target_profile.target_name() {
            return Err(format!(
                "reachable requirement `{requirement}` selected target `{}` instead of exact profile `{}`",
                plan.target,
                self.target_profile.target_name(),
            ));
        }
        Ok((plan, row, specialization))
    }

    pub(super) fn exact_mechanism(
        &self,
        boundary: BoundaryMachineId,
        binding: &ProviderBinding,
    ) -> Result<TerminalMechanismIdentity, String> {
        let mechanism = self.mechanisms.get(&boundary).copied().ok_or_else(|| {
            format!("reachable terminal boundary {boundary:?} has no admitted physical mechanism")
        })?;
        let role_matches = match (binding, mechanism) {
            (
                ProviderBinding::CompilerIntrinsic { .. },
                TerminalMechanismIdentity::CompilerIntrinsic(_),
            )
            | (ProviderBinding::Import { .. }, TerminalMechanismIdentity::NormalizedForeign(_)) => {
                true
            }
            (ProviderBinding::Syscall { number }, TerminalMechanismIdentity::Syscall(syscall)) => {
                u32::try_from(*number).ok() == Some(syscall.number())
                    && syscall.target() == self.target_profile
            }
            (
                ProviderBinding::VtableField { .. },
                TerminalMechanismIdentity::Table {
                    target,
                    dispatch_only: false,
                    ..
                },
            )
            | (
                ProviderBinding::TableFunction { .. },
                TerminalMechanismIdentity::Table {
                    target,
                    dispatch_only: true,
                    ..
                },
            ) => target == self.target_profile,
            _ => false,
        };
        if !role_matches {
            return Err(format!(
                "reachable terminal boundary {boundary:?} substituted its selected binding role"
            ));
        }
        Ok(mechanism)
    }
}
