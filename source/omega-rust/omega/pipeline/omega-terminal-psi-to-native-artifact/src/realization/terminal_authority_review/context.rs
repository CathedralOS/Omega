//! Exact source, selection, mechanism, and installed-candidate lookup context.

use std::collections::{BTreeMap, BTreeSet};

use omega_abstract_operations::{AbstractFunction, AbstractOperationPlan};
use omega_effects::{
    provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow},
    SelectedProviderPlanFacts, TerminalMechanismIdentity,
};
use psi_core::{BoundaryMachineId, MachineId};

use super::operations::{authority_edge, AuthorityEdge};
use crate::realization::{
    providers::AdmittedTerminalMechanism, TerminalAuthorityPermissionPolicy,
    TerminalAuthorityPolicy,
};

pub(super) struct ReviewContext<'a> {
    functions: BTreeMap<MachineId, &'a AbstractFunction>,
    pub(super) boundaries:
        BTreeMap<BoundaryMachineId, &'a psi_terminal::BoundaryMachineDeclaration>,
    pub(super) selected: &'a SelectedProviderPlanFacts,
    pub(super) physical_policy: &'a TerminalAuthorityPolicy,
    pub(super) permission_policy: &'a TerminalAuthorityPermissionPolicy,
    pub(super) mechanisms: BTreeMap<BoundaryMachineId, TerminalMechanismIdentity>,
    pub(super) installed_candidates: &'a [psi_terminal::ProviderCandidateConformance],
}

impl<'a> ReviewContext<'a> {
    pub(super) fn new(
        plan: &'a AbstractOperationPlan,
        selected: &'a SelectedProviderPlanFacts,
        physical_policy: &'a TerminalAuthorityPolicy,
        permission_policy: &'a TerminalAuthorityPermissionPolicy,
        mechanisms: &[AdmittedTerminalMechanism],
        installed_candidates: &'a [psi_terminal::ProviderCandidateConformance],
    ) -> Result<Self, String> {
        let mut functions = BTreeMap::new();
        for function in &plan.functions {
            if functions.insert(function.machine, function).is_some() {
                return Err(format!(
                    "abstract operation plan repeats machine {:?}",
                    function.machine
                ));
            }
        }
        if !functions.contains_key(&plan.entry) {
            return Err(format!(
                "abstract operation plan entry {:?} has no function",
                plan.entry
            ));
        }
        let mut boundaries = BTreeMap::new();
        let mut boundary_identities = BTreeSet::new();
        for boundary in &plan.boundary_machines {
            if boundary.identity.is_empty()
                || boundaries.insert(boundary.id, boundary).is_some()
                || !boundary_identities.insert(boundary.identity.as_str())
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
            functions,
            boundaries,
            selected,
            physical_policy,
            permission_policy,
            mechanisms: mechanism_map,
            installed_candidates,
        })
    }

    pub(super) fn selected_requirement(
        &self,
        requirement: &str,
    ) -> Result<(&'a ProviderPlan, &'a ProviderPlanRow), String> {
        let matches = self
            .selected
            .plans()
            .iter()
            .flat_map(|plan| {
                plan.rows
                    .iter()
                    .filter(move |row| row.requirement_identity == requirement)
                    .map(move |row| (plan, row))
            })
            .collect::<Vec<_>>();
        let [(plan, row)] = matches.as_slice() else {
            return Err(format!(
                "reachable requirement `{requirement}` resolves to {} selected provider rows",
                matches.len()
            ));
        };
        let methods = plan
            .schema
            .methods
            .iter()
            .filter(|method| method.requirement_identity == requirement)
            .collect::<Vec<_>>();
        if methods.len() != 1 {
            return Err(format!(
                "reachable requirement `{requirement}` resolves to {} exact schema methods",
                methods.len()
            ));
        }
        Ok((plan, row))
    }

    pub(super) fn exact_mechanism(
        &self,
        boundary: BoundaryMachineId,
        binding: &ProviderBinding,
    ) -> Result<TerminalMechanismIdentity, String> {
        let mechanism = self.mechanisms.get(&boundary).copied().ok_or_else(|| {
            format!("reachable terminal boundary {boundary:?} has no admitted physical mechanism")
        })?;
        let role_matches = matches!(
            (binding, mechanism),
            (
                ProviderBinding::CompilerIntrinsic { .. },
                TerminalMechanismIdentity::CompilerIntrinsic(_)
            ) | (
                ProviderBinding::Import { .. },
                TerminalMechanismIdentity::NormalizedForeign(_)
            )
        );
        if !role_matches {
            return Err(format!(
                "reachable terminal boundary {boundary:?} substituted its selected binding role"
            ));
        }
        Ok(mechanism)
    }

    pub(super) fn reachable_boundaries(
        &self,
        entry: MachineId,
    ) -> Result<BTreeSet<BoundaryMachineId>, String> {
        let mut pending = vec![entry];
        let mut visited = BTreeSet::new();
        let mut boundaries = BTreeSet::new();
        while let Some(machine) = pending.pop() {
            if !visited.insert(machine) {
                continue;
            }
            let function = self.functions.get(&machine).ok_or_else(|| {
                format!("reachable internal call names absent machine {machine:?}")
            })?;
            for operation in &function.operations {
                match authority_edge(operation) {
                    AuthorityEdge::None => {}
                    AuthorityEdge::Internal(callee) => pending.push(callee),
                    AuthorityEdge::Boundary(boundary) => {
                        boundaries.insert(boundary);
                    }
                    AuthorityEdge::UnsupportedCheckedPhysical => {
                        return Err(format!(
                            "reachable machine {machine:?} uses a checked physical terminal operation unsupported by the current D45 role sum"
                        ));
                    }
                }
            }
        }
        Ok(boundaries)
    }
}
