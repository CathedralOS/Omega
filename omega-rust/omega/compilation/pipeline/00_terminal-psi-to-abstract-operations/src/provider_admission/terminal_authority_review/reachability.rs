//! Root-reachable authority edges of one realized plan.
//!
//! The walk follows the target-realized abstract plan, not checked source:
//! a target family call reaches only the member this target selected, so a
//! boundary another target's member calls is not demanded here. Provider
//! supply and the closure review both read this one walk; a second walk
//! could let the supply check pass a requirement the review then rejects.

use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::AbstractFunction;
use effects::{
    CheckedPhysicalTerminalMechanismIdentity, TerminalAuthorityClosureLeafOrigin,
    TerminalMechanismIdentity,
};
use semantic_vocabulary::{BoundaryMachineId, MachineId};

use super::operations::{AuthorityEdge, authority_edge};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReachableCheckedOperation {
    pub(crate) origin: TerminalAuthorityClosureLeafOrigin,
    pub(crate) service: semantic_vocabulary::ServiceId,
    pub(crate) mechanism: TerminalMechanismIdentity,
}

pub(crate) struct ReachableAuthorityEdges {
    pub(crate) boundaries: BTreeSet<BoundaryMachineId>,
    pub(crate) checked_operations: Vec<ReachableCheckedOperation>,
}

/// The realized plan's functions keyed by machine, with the dynamic dispatch
/// catalog and target profile the walk consults.
pub(crate) struct AuthorityGraph<'a> {
    input: &'a crate::VerifiedPsiOptimizationInput,
    functions: BTreeMap<MachineId, &'a AbstractFunction>,
    dynamic_dispatch: &'a terminal_psi::TerminalDynamicDispatchCatalog,
    target_profile: target::TargetProfile,
}

impl<'a> AuthorityGraph<'a> {
    pub(crate) fn new(
        input: &'a crate::VerifiedPsiOptimizationInput,
        target_profile: target::TargetProfile,
    ) -> Result<Self, String> {
        let plan = input.plan();
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
        Ok(Self {
            input,
            functions,
            dynamic_dispatch: &plan.dynamic_dispatch,
            target_profile,
        })
    }

    pub(crate) fn reachable_authority_edges(
        &self,
        entry: MachineId,
    ) -> Result<ReachableAuthorityEdges, String> {
        let mut pending = vec![entry];
        let mut visited = BTreeSet::new();
        let mut boundaries = BTreeSet::new();
        let mut checked_operations = BTreeMap::new();
        while let Some(machine) = pending.pop() {
            if !visited.insert(machine) {
                continue;
            }
            let function = self.functions.get(&machine).ok_or_else(|| {
                format!("reachable internal call names absent machine {machine:?}")
            })?;
            for operation in &function.operations {
                // Cleanup is executable work on this edge, not a declaration
                // marker. Its hook's mechanisms belong to the same authority
                // closure as ordinary calls from the entry.
                for (_, actions) in operation.cleanup_rosters() {
                    for action in actions {
                        if let terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) =
                            action
                        {
                            pending.push(cleanup.cleanup_machine);
                        }
                    }
                }
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
                        if let Some(terminal_psi::TargetMachineCallable::Machine(member)) =
                            self.input.target_family_member(boundary)?
                        {
                            pending.push(member);
                        } else {
                            boundaries.insert(boundary);
                        }
                    }
                    AuthorityEdge::CheckedPortWrite {
                        service,
                        port,
                        operation,
                    } => {
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
                        let origin = TerminalAuthorityClosureLeafOrigin::CheckedPhysical {
                            machine,
                            operation,
                        };
                        record_checked_operation(
                            &mut checked_operations,
                            ReachableCheckedOperation {
                                origin,
                                service,
                                mechanism,
                            },
                        )?;
                    }
                    AuthorityEdge::CheckedRegister {
                        service,
                        register_operation,
                        operation,
                    } => {
                        if self.target_profile.native_target().architecture
                            != target::Architecture::Aarch64
                        {
                            return Err(format!(
                                "reachable machine {machine:?} uses aarch64 register access on non-aarch64 target profile `{}`",
                                self.target_profile.target_name(),
                            ));
                        }
                        let intrinsic = effects::CompilerIntrinsicExecutionIdentity::checked_register(register_operation)
                            .ok_or_else(|| format!("register access {operation:?} has no admitted compiler catalog identity"))?;
                        let origin = TerminalAuthorityClosureLeafOrigin::CheckedIntrinsic {
                            machine,
                            operation,
                        };
                        let mechanism = intrinsic.into();
                        record_checked_operation(
                            &mut checked_operations,
                            ReachableCheckedOperation {
                                origin,
                                service,
                                mechanism,
                            },
                        )?;
                    }
                }
            }
        }
        Ok(ReachableAuthorityEdges {
            boundaries,
            checked_operations: checked_operations.into_values().collect(),
        })
    }
}

fn record_checked_operation(
    operations: &mut BTreeMap<
        (MachineId, semantic_vocabulary::OperationId),
        ReachableCheckedOperation,
    >,
    instruction: ReachableCheckedOperation,
) -> Result<(), String> {
    let site = instruction
        .origin
        .checked_site()
        .ok_or_else(|| "checked operation has no machine/operation custody".to_owned())?;
    if let Some(previous) = operations.get(&site) {
        if previous != &instruction {
            return Err(format!(
                "checked operation {site:?} claims distinct mechanisms or services"
            ));
        }
    } else {
        operations.insert(site, instruction);
    }
    Ok(())
}
