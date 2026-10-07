//! Exact source, selection, mechanism, and installed-candidate lookup context.

use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::{AbstractFunction, AbstractOperationPlan};
use effects::{
    CheckedPhysicalTerminalMechanismIdentity, SelectedProviderPlanFacts, TerminalMechanismIdentity,
    provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow},
    terminal_mechanism_identity_bytes,
};
use semantic_vocabulary::{BoundaryMachineId, MachineId};

use super::operations::{AuthorityEdge, authority_edge};
use crate::provider_admission::{
    AdmittedTerminalMechanism, TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy,
};

pub(super) struct ReachableAuthorityEdges {
    pub(super) boundaries: BTreeSet<BoundaryMachineId>,
    pub(super) checked_physical: Vec<TerminalMechanismIdentity>,
}

pub(super) struct ReviewContext<'a> {
    functions: BTreeMap<MachineId, &'a AbstractFunction>,
    dynamic_dispatch: &'a terminal_psi::TerminalDynamicDispatchCatalog,
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
        plan: &'a AbstractOperationPlan,
        selected: &'a SelectedProviderPlanFacts,
        physical_policy: &'a TerminalAuthorityPolicy,
        permission_policy: Option<&'a TerminalAuthorityPermissionPolicy>,
        mechanisms: &[AdmittedTerminalMechanism],
        installed_candidates: &'a [terminal_psi::ProviderCandidateConformance],
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
            dynamic_dispatch: &plan.dynamic_dispatch,
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
        let specialization = self.selected.specialized_requirement_row(requirement);
        // Provider selection resolves each requirement to at most one plan
        // row and each schema's method roster carries the requirement once,
        // so the join takes the identifying row instead of re-counting the
        // matches; the absent case stays a real refusal — a boundary can be
        // declared and reached without a selected provider.
        let (plan, row) = self
            .selected
            .plans()
            .iter()
            .filter(|plan| {
                specialization.is_none_or(|row| {
                    row.provider_plan_report_identity == plan.report_fingerprint()
                })
            })
            .flat_map(|plan| {
                plan.rows
                    .iter()
                    .filter(move |row| {
                        specialization.map_or_else(
                            || row.requirement_identity == requirement,
                            |specialization| {
                                row.requirement_identity
                                    == specialization.selected_requirement_identity
                            },
                        )
                    })
                    .map(move |row| (plan, row))
            })
            .next()
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
            _ => false,
        };
        if !role_matches {
            return Err(format!(
                "reachable terminal boundary {boundary:?} substituted its selected binding role"
            ));
        }
        Ok(mechanism)
    }

    pub(super) fn reachable_authority_edges(
        &self,
        entry: MachineId,
    ) -> Result<ReachableAuthorityEdges, String> {
        let mut pending = vec![entry];
        let mut visited = BTreeSet::new();
        let mut boundaries = BTreeSet::new();
        let mut checked_physical = BTreeMap::new();
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
                    AuthorityEdge::Dynamic { interface, slot } => {
                        // Formation captures no authority. Invocation may reach
                        // any implementation's exact slot, never unrelated rows
                        // or a representative provider's narrower authority.
                        self.dynamic_dispatch.interface(interface)
                            .and_then(|interface| interface.slots.get(slot as usize))
                            .filter(|requirement| requirement.slot == slot)
                            .ok_or_else(|| format!("dynamic authority edge names absent interface {interface} slot {slot}"))?;
                        for table in &self.dynamic_dispatch.tables {
                            if table.interface != interface {
                                continue;
                            }
                            if let Some(machine) = table.rows.get(slot as usize).copied().flatten()
                            {
                                pending.push(machine);
                            }
                        }
                    }
                    AuthorityEdge::Boundary(boundary) => {
                        boundaries.insert(boundary);
                    }
                    AuthorityEdge::CheckedPortWrite { port } => {
                        // Psi's `validate_port_write` already proved the write's
                        // service sits inside the machine's published ceiling;
                        // re-asking it here would replay the same fact.
                        if self.target_profile.native_target().architecture
                            != target::Architecture::X86_64
                        {
                            return Err(format!(
                                "reachable machine {machine:?} uses x86 PortWrite on non-x86 target profile `{}`",
                                self.target_profile.target_name(),
                            ));
                        }
                        let mechanism: TerminalMechanismIdentity =
                            CheckedPhysicalTerminalMechanismIdentity::port_write(
                                self.target_profile,
                                port,
                            )
                            .into();
                        checked_physical
                            .entry(terminal_mechanism_identity_bytes(mechanism))
                            .or_insert(mechanism);
                    }
                }
            }
        }
        Ok(ReachableAuthorityEdges {
            boundaries,
            checked_physical: checked_physical.into_values().collect(),
        })
    }
}
