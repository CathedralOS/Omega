//! Exact source, selection, mechanism, and installed-candidate lookup context.

use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::{
    AbstractDynamicDescriptorArgument, AbstractDynamicDescriptorSource, AbstractFunction,
    AbstractOperationPlan, AbstractParameterDynamicDispatch,
};
use effects::{
    CheckedPhysicalTerminalMechanismIdentity, SelectedProviderPlanFacts, TerminalMechanismIdentity,
    provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow},
    terminal_mechanism_identity_bytes,
};
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_psi::{ClosedConformanceApplication, ClosedConformanceApplicationCommitment};

use super::operations::{AuthorityEdge, authority_edge};
use crate::realization::{
    TerminalAuthorityPermissionPolicy, TerminalAuthorityPolicy,
    providers::AdmittedTerminalMechanism,
};

type DynamicBindings = BTreeMap<u32, ClosedConformanceApplication>;

pub(super) struct ReachableAuthorityEdges {
    pub(super) boundaries: BTreeSet<BoundaryMachineId>,
    pub(super) checked_physical: Vec<TerminalMechanismIdentity>,
}

fn binding_identity(
    machine: MachineId,
    bindings: &DynamicBindings,
) -> (
    MachineId,
    Vec<(u32, ClosedConformanceApplicationCommitment)>,
) {
    (
        machine,
        bindings
            .iter()
            .map(|(ordinal, application)| (*ordinal, application.commitment))
            .collect(),
    )
}

pub(super) struct ReviewContext<'a> {
    functions: BTreeMap<MachineId, &'a AbstractFunction>,
    pub(super) boundaries:
        BTreeMap<BoundaryMachineId, &'a terminal_psi::BoundaryMachineDeclaration>,
    pub(super) selected: &'a SelectedProviderPlanFacts,
    pub(super) physical_policy: &'a TerminalAuthorityPolicy,
    pub(super) permission_policy: &'a TerminalAuthorityPermissionPolicy,
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
        permission_policy: &'a TerminalAuthorityPermissionPolicy,
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
            boundaries,
            selected,
            physical_policy,
            permission_policy,
            mechanisms: mechanism_map,
            installed_candidates,
            target_profile,
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
        if plan.target != self.target_profile.target_name() {
            return Err(format!(
                "reachable requirement `{requirement}` selected target `{}` instead of exact profile `{}`",
                plan.target,
                self.target_profile.target_name(),
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
        let mut pending = vec![(entry, DynamicBindings::new())];
        let mut visited = BTreeSet::new();
        let mut boundaries = BTreeSet::new();
        let mut checked_physical = BTreeMap::new();
        while let Some((machine, bindings)) = pending.pop() {
            if !visited.insert(binding_identity(machine, &bindings)) {
                continue;
            }
            let function = self.functions.get(&machine).ok_or_else(|| {
                format!("reachable internal call names absent machine {machine:?}")
            })?;
            for operation in &function.operations {
                match authority_edge(operation) {
                    AuthorityEdge::None => {}
                    AuthorityEdge::Internal(callee) => {
                        pending.push((callee, DynamicBindings::new()));
                    }
                    AuthorityEdge::InternalWithDynamicArguments { callee, arguments } => {
                        pending.push((
                            callee,
                            self.bind_dynamic_arguments(machine, callee, &bindings, arguments)?,
                        ));
                    }
                    AuthorityEdge::DynamicParameterDispatch(dispatch) => {
                        pending.push((
                            self.parameter_dispatch_realization(machine, &bindings, dispatch)?,
                            DynamicBindings::new(),
                        ));
                    }
                    AuthorityEdge::Boundary(boundary) => {
                        boundaries.insert(boundary);
                    }
                    AuthorityEdge::CheckedPortWrite { service, port } => {
                        if !function.published_service_ceiling.contains(&service) {
                            return Err(format!(
                                "reachable machine {machine:?} uses PortWrite outside its verified service ceiling"
                            ));
                        }
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

    fn bind_dynamic_arguments(
        &self,
        caller: MachineId,
        callee: MachineId,
        caller_bindings: &DynamicBindings,
        arguments: &[AbstractDynamicDescriptorArgument],
    ) -> Result<DynamicBindings, String> {
        let mut callee_bindings = DynamicBindings::new();
        for argument in arguments {
            if argument.argument.owner != caller || argument.target.owner != callee {
                return Err(format!(
                    "reachable dynamic argument from {caller:?} does not bind callee {callee:?}"
                ));
            }
            let application = match &argument.source {
                AbstractDynamicDescriptorSource::Selection { application, .. }
                | AbstractDynamicDescriptorSource::Rebound { application, .. } => {
                    application.clone()
                }
                AbstractDynamicDescriptorSource::Parameter(source) => caller_bindings
                    .get(&source.ordinal)
                    .cloned()
                    .ok_or_else(|| {
                        format!(
                            "reachable machine {caller:?} forwards unbound dynamic parameter {}",
                            source.ordinal
                        )
                    })?,
            };
            if callee_bindings
                .insert(argument.target.ordinal, application)
                .is_some()
            {
                return Err(format!(
                    "reachable call from {caller:?} repeats dynamic parameter {} for {callee:?}",
                    argument.target.ordinal
                ));
            }
        }
        Ok(callee_bindings)
    }

    fn parameter_dispatch_realization(
        &self,
        machine: MachineId,
        bindings: &DynamicBindings,
        dispatch: &AbstractParameterDynamicDispatch,
    ) -> Result<MachineId, String> {
        let parameter = &dispatch.parameter;
        if parameter.owner != machine
            || dispatch.dispatch.owner != machine
            || dispatch.dispatch.parameter_ordinal != parameter.ordinal
        {
            return Err(format!(
                "reachable machine {machine:?} has inconsistent dynamic-parameter dispatch custody"
            ));
        }
        let application = bindings.get(&parameter.ordinal).ok_or_else(|| {
            format!(
                "reachable machine {machine:?} dispatches unbound dynamic parameter {}",
                parameter.ordinal
            )
        })?;
        if application.trait_identity != parameter.trait_identity {
            return Err(format!(
                "reachable machine {machine:?} substituted the dynamic parameter trait application"
            ));
        }
        let requirements = parameter
            .requirements
            .iter()
            .filter(|requirement| requirement.slot == dispatch.dispatch.requirement_slot)
            .collect::<Vec<_>>();
        let [requirement] = requirements.as_slice() else {
            return Err(format!(
                "reachable machine {machine:?} dynamic parameter slot does not resolve exactly once"
            ));
        };
        let rows = application
            .rows
            .iter()
            .filter(|row| {
                row.declaring_trait_identity == requirement.declaring_trait_identity
                    && row.public_requirement_identity == requirement.public_requirement_identity
            })
            .collect::<Vec<_>>();
        let [row] = rows.as_slice() else {
            return Err(format!(
                "reachable machine {machine:?} dynamic parameter requirement does not resolve exactly once"
            ));
        };
        let callable_identity = row.realization_callable_identity.as_deref().ok_or_else(|| {
            format!(
                "reachable machine {machine:?} dynamic parameter requirement has no realization callable"
            )
        })?;
        let callables = application
            .realization_callables
            .iter()
            .filter(|callable| {
                callable.source_callable_identity == callable_identity
                    && callable.result == requirement.result
            })
            .collect::<Vec<_>>();
        let [callable] = callables.as_slice() else {
            return Err(format!(
                "reachable machine {machine:?} dynamic parameter realization does not resolve exactly once"
            ));
        };
        Ok(callable.machine)
    }
}
