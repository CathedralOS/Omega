/*
Lookup context for terminal-authority closure review.

`ReviewContext` indexes boundary declarations, admitted terminal mechanisms,
selected provider plan rows, specialization rows, and component callable
authority for a single `TargetProfile`. When a reachable requirement is missing
from the selected provider rows on the target, `missing_requirement` formats a
bounded refusal diagnostic listing up to 12 selected rows with matching arity
without ever admitting equal arity as a replacement.
*/

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
    pub(super) input: &'a crate::AdmissionSource<'a>,
    pub(super) graph: AuthorityGraph<'a>,
    pub(super) boundaries:
        BTreeMap<BoundaryMachineId, &'a terminal_psi::BoundaryMachineDeclaration>,
    pub(super) selected: &'a SelectedProviderPlanFacts,
    pub(super) physical_policy: &'a TerminalAuthorityPolicy,
    pub(super) permission_policy: Option<&'a TerminalAuthorityPermissionPolicy>, // `None` records classification and exercised authority without claiming receiver admission.
    pub(super) mechanisms: BTreeMap<BoundaryMachineId, TerminalMechanismIdentity>,
    pub(super) installed_candidates: &'a [terminal_psi::ProviderCandidateConformance],
    target_profile: target::TargetProfile,
    components: &'a [crate::provider_admission::NativeProviderSettlement<'a>],
    pub(super) installed_interrupts: &'a crate::AdmittedInstalledInterruptCohort,
}

impl<'a> ReviewContext<'a> {
    pub(super) fn new(
        target_profile: target::TargetProfile,
        input: &'a crate::AdmissionSource<'a>,
        selected: &'a SelectedProviderPlanFacts,
        physical_policy: &'a TerminalAuthorityPolicy,
        permission_policy: Option<&'a TerminalAuthorityPermissionPolicy>,
        mechanisms: &[AdmittedTerminalMechanism],
        installed_candidates: &'a [terminal_psi::ProviderCandidateConformance],
        components: &'a [crate::provider_admission::NativeProviderSettlement<'a>],
        installed_interrupts: &'a crate::AdmittedInstalledInterruptCohort,
    ) -> Result<Self, String> {
        let plan = input.plan();
        let graph = AuthorityGraph::new(input, target_profile)?
            .with_installed_interrupts(installed_interrupts)?;
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
            components,
            installed_interrupts,
        })
    }

    pub(super) fn component_authority(
        &self,
        boundary: BoundaryMachineId,
    ) -> Result<Option<&'a crate::provider_admission::AdmittedNativeCallableAuthority>, String>
    {
        let mut matches =
            self.components
                .iter()
                .filter_map(|settlement| match settlement.realization {
                    crate::provider_admission::NativeBoundaryRealization::Component {
                        call,
                        authority,
                        ..
                    } if call.selection().receiving_boundary() == boundary => Some(authority),
                    _ => None,
                });
        let result = matches.next();
        if matches.next().is_some() {
            return Err("component authority has competing separately emitted executions for one receiving boundary".into());
        }
        Ok(result)
    }

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
        let (plan, row) = self
            .selected
            .selected_requirement_row(requirement)
            .ok_or_else(|| self.missing_requirement(requirement))?;
        if plan.target != self.target_profile.target_name() {
            return Err(format!(
                "reachable requirement `{requirement}` selected target `{}` instead of exact profile `{}`",
                plan.target,
                self.target_profile.target_name(),
            ));
        }
        Ok((plan, row, specialization))
    }

    fn missing_requirement(&self, requirement: &str) -> String {
        let target = self.target_profile.target_name();
        let arity = self
            .boundaries
            .values()
            .find(|boundary| {
                self.input.plan().identities.text(boundary.identity) == Some(requirement)
            })
            .map(|boundary| boundary.parameter_order.len());
        let mut message = format!(
            "reachable requirement `{requirement}` resolves to 0 selected provider rows on target `{target}`"
        );
        let mut shown = 0;
        let mut offered = 0;
        for plan in self
            .selected
            .plans()
            .iter()
            .filter(|plan| plan.target == target)
        {
            for row in &plan.rows {
                let identity = plan.identities.text(row.requirement_identity);
                if !plan.schema.declaration.methods.iter().any(|method| {
                    Some(method.parameter_count) == arity
                        && plan.schema.identity_text(method.requirement_identity) == identity
                }) {
                    continue;
                }
                offered += 1;
                let detail = format!(
                    "\n  selected schema `{}` offers `{identity}`",
                    plan.schema
                        .identity_text(plan.schema.declaration.trait_name)
                );
                if shown < 12 && message.len() + detail.len() <= 16384 {
                    message.push_str(&detail);
                    shown += 1;
                }
            }
        }
        message.push_str(&format!(
            "\n  showing {shown} of {offered} selected rows with parameter count {arity:?}"
        ));
        message
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
