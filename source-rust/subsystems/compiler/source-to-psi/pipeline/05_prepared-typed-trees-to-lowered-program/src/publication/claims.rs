//! Describe logical claim names and transports after storage lowering. The
//! rows confer no checked status: Psi 06 reconstructs them from its own input.

use semantic_vocabulary::{MachineId, StructuralTypeId};
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use terminal_psi::{
    OperationKind, StructuralFieldType, StructuralTypeShape, TerminalAffineCleanupAction,
    TerminalMachine, TerminalModule, Terminator,
};

fn machine_ordinal_index(module: &TerminalModule) -> Option<Vec<(MachineId, usize)>> {
    let mut index: Vec<(MachineId, usize)> = module
        .machines
        .iter()
        .enumerate()
        .map(|(ordinal, machine)| (machine.id, ordinal))
        .collect();
    index.sort_unstable_by_key(|(id, _)| *id);
    if index.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return None;
    }
    Some(index)
}

fn lookup_ordinal(index: &[(MachineId, usize)], machine: MachineId) -> Option<usize> {
    index
        .binary_search_by_key(&machine, |(id, _)| *id)
        .ok()
        .map(|pos| index[pos].1)
}

fn collect_type_cleanup_machines(
    module: &TerminalModule,
    ordinals: &[(MachineId, usize)],
    type_index: &[(StructuralTypeId, usize)],
    root_type: StructuralTypeId,
    visited_types: &mut [bool],
    targets: &mut Vec<usize>,
) -> bool {
    let mut stack = vec![root_type];
    while let Some(ty) = stack.pop() {
        let Ok(pos) = type_index.binary_search_by_key(&ty, |(id, _)| *id) else {
            return false;
        };
        let slot = type_index[pos].1;
        if visited_types[slot] {
            continue;
        }
        visited_types[slot] = true;
        let declaration = &module.types.structural_types[slot];
        if let Some(selection) = &declaration.nominal_cleanup {
            let Some(target) = lookup_ordinal(ordinals, selection.cleanup_machine) else {
                return false;
            };
            targets.push(target);
        }
        for (_, field) in declaration.shape.fields_with_case() {
            if !field.relevance.is_erased()
                && let StructuralFieldType::Structural(child) = field.field_type
            {
                stack.push(child);
            }
        }
        if let StructuralTypeShape::FixedArray { element, .. } = declaration.shape {
            stack.push(element);
        }
    }
    true
}

/// Build the static transitive callee/cleanup closure that
/// `MachineAffine::plan_claims` can read from `module` for each machine.
/// Returns `None` for a machine only when it (or a transitive callee) invokes
/// an unresolved/dynamic callable or names a target outside `module.machines`.
fn static_claim_read_sets(module: &TerminalModule) -> Vec<Option<Vec<MachineId>>> {
    let count = module.machines.len();
    let Some(ordinals) = machine_ordinal_index(module) else {
        return vec![None; count];
    };
    let has_nominal_cleanup = module
        .types
        .structural_types
        .iter()
        .any(|ty| ty.nominal_cleanup.is_some());
    let mut type_index = if has_nominal_cleanup {
        let mut index: Vec<(StructuralTypeId, usize)> = module
            .types
            .structural_types
            .iter()
            .enumerate()
            .map(|(slot, ty)| (ty.id, slot))
            .collect();
        index.sort_unstable_by_key(|(id, _)| *id);
        index
    } else {
        Vec::new()
    };
    let type_index_valid = !type_index.windows(2).any(|pair| pair[0].0 == pair[1].0);
    if !type_index_valid {
        type_index.clear();
    }
    let mut visited_types = vec![false; module.types.structural_types.len()];
    let mut direct: Vec<Option<Vec<usize>>> = Vec::with_capacity(count);
    for machine in &module.machines {
        let mut callees = Vec::new();
        let mut complete = true;
        'blocks: for block in &machine.blocks {
            for operation in &block.operations {
                if let Some(callee) = operation.kind.direct_callee() {
                    let Some(target) = lookup_ordinal(&ordinals, callee) else {
                        complete = false;
                        break 'blocks;
                    };
                    callees.push(target);
                } else if let OperationKind::CallDynamic {
                    interface, slot, ..
                } = &operation.kind
                {
                    let Some(slot_index) = usize::try_from(*slot).ok() else {
                        complete = false;
                        break 'blocks;
                    };
                    if module
                        .execution
                        .dynamic_dispatch
                        .interface(*interface)
                        .and_then(|iface| iface.slots.get(slot_index))
                        .is_none()
                    {
                        complete = false;
                        break 'blocks;
                    }
                    for table in module
                        .execution
                        .dynamic_dispatch
                        .tables
                        .iter()
                        .filter(|table| table.interface == *interface)
                    {
                        let Some(Some(target_id)) = table.rows.get(slot_index) else {
                            complete = false;
                            break 'blocks;
                        };
                        let Some(target) = lookup_ordinal(&ordinals, *target_id) else {
                            complete = false;
                            break 'blocks;
                        };
                        callees.push(target);
                    }
                } else if matches!(operation.kind, OperationKind::CallMachine { .. }) {
                    complete = false;
                    break 'blocks;
                }
            }
            if let Terminator::InvokeNoNormal { callee, .. } = &block.terminator {
                let Some(target) = lookup_ordinal(&ordinals, *callee) else {
                    complete = false;
                    break 'blocks;
                };
                callees.push(target);
            }
            for (_, actions) in block.terminator.cleanup_rosters() {
                for action in actions {
                    if let TerminalAffineCleanupAction::InvokeOwned(cleanup) = action {
                        let Some(target) = lookup_ordinal(&ordinals, cleanup.cleanup_machine)
                        else {
                            complete = false;
                            break 'blocks;
                        };
                        callees.push(target);
                    }
                }
            }
        }
        if complete && has_nominal_cleanup {
            if !type_index_valid {
                complete = false;
            } else {
                visited_types.fill(false);
                for place in &machine.structural_places {
                    let Some(root_type) = machine.structural_place_type(place.id) else {
                        complete = false;
                        break;
                    };
                    if !collect_type_cleanup_machines(
                        module,
                        &ordinals,
                        &type_index,
                        root_type,
                        &mut visited_types,
                        &mut callees,
                    ) {
                        complete = false;
                        break;
                    }
                }
            }
        }
        if complete {
            callees.sort_unstable();
            callees.dedup();
            direct.push(Some(callees));
        } else {
            direct.push(None);
        }
    }

    let mut closures = Vec::with_capacity(count);
    let mut visited = vec![false; count];
    let mut stack = Vec::new();
    for root in 0..count {
        if direct[root].is_none() {
            closures.push(None);
            continue;
        }
        visited.fill(false);
        visited[root] = true;
        stack.clear();
        stack.push(root);
        let mut reachable = Vec::new();
        let mut complete = true;
        while let Some(node) = stack.pop() {
            let Some(edges) = &direct[node] else {
                complete = false;
                break;
            };
            for &next in edges {
                if !visited[next] {
                    visited[next] = true;
                    stack.push(next);
                    reachable.push(module.machines[next].id);
                }
            }
        }
        if complete {
            reachable.sort_unstable();
            closures.push(Some(reachable));
        } else {
            closures.push(None);
        }
    }
    closures
}

/// One claim list in a machine's blocks that `ClaimsPlan::install`
/// overwrites. Installing never changes the blocks' operations or
/// terminators, so the walk visits the same slots, in the same order, before
/// and after an install.
enum ClaimSlot<'m> {
    Results(&'m mut Vec<terminal_psi::StructuralResultClaimBinding>),
    Transfers(&'m mut Vec<terminal_psi::ClaimTransfer>),
    Receipts(&'m mut Vec<terminal_psi::CompletionReceipt>),
}

/// A claim list moved out of its slot before an install.
enum PriorClaims {
    Results(Vec<terminal_psi::StructuralResultClaimBinding>),
    Transfers(Vec<terminal_psi::ClaimTransfer>),
    Receipts(Vec<terminal_psi::CompletionReceipt>),
}

impl ClaimSlot<'_> {
    fn is_empty(&self) -> bool {
        match self {
            Self::Results(claims) => claims.is_empty(),
            Self::Transfers(claims) => claims.is_empty(),
            Self::Receipts(claims) => claims.is_empty(),
        }
    }

    fn take(self) -> PriorClaims {
        match self {
            Self::Results(claims) => PriorClaims::Results(std::mem::take(claims)),
            Self::Transfers(claims) => PriorClaims::Transfers(std::mem::take(claims)),
            Self::Receipts(claims) => PriorClaims::Receipts(std::mem::take(claims)),
        }
    }

    fn holds(&self, prior: &PriorClaims) -> bool {
        match (self, prior) {
            (Self::Results(claims), PriorClaims::Results(prior)) => **claims == *prior,
            (Self::Transfers(claims), PriorClaims::Transfers(prior)) => **claims == *prior,
            (Self::Receipts(claims), PriorClaims::Receipts(prior)) => **claims == *prior,
            _ => false,
        }
    }
}

/// Visit every claim list of `machine`'s blocks that `ClaimsPlan::install`
/// overwrites, in block order.
fn for_each_claim_slot(machine: &mut TerminalMachine, mut visit: impl FnMut(ClaimSlot<'_>)) {
    for block in &mut machine.blocks {
        for operation in &mut block.operations {
            if let terminal_psi::OperationResult::Structural(result) = &mut operation.result {
                visit(ClaimSlot::Results(&mut result.claims));
            }
            match &mut operation.kind {
                OperationKind::CallUnit {
                    claim_transfers, ..
                }
                | OperationKind::CallStructuralScalar {
                    claim_transfers, ..
                }
                | OperationKind::CallStructural {
                    claim_transfers, ..
                }
                | OperationKind::CallStructuralWithScalarArguments {
                    claim_transfers, ..
                }
                | OperationKind::CallMachine {
                    claim_transfers, ..
                }
                | OperationKind::CallDynamic {
                    claim_transfers, ..
                } => visit(ClaimSlot::Transfers(claim_transfers)),
                OperationKind::BoundaryCall {
                    completion_receipts,
                    ..
                } => visit(ClaimSlot::Receipts(completion_receipts)),
                _ => {}
            }
        }
        for (_, actions) in block.terminator.cleanup_rosters_mut() {
            for action in actions {
                if let TerminalAffineCleanupAction::InvokeOwned(cleanup) = action {
                    visit(ClaimSlot::Transfers(&mut cleanup.claim_transfers));
                }
            }
        }
        match &mut block.terminator {
            Terminator::InvokeNoNormal {
                claim_transfers, ..
            } => visit(ClaimSlot::Transfers(claim_transfers)),
            Terminator::ReturnStructural {
                returned_claims, ..
            } => visit(ClaimSlot::Results(returned_claims)),
            _ => {}
        }
    }
}

/// Install `plan` and report whether it changed `machine`, which tells
/// `plan_then_apply` to plan again any machine whose walk read this one.
/// The old non-empty claim lists are moved out rather than copied: the
/// install overwrites every slot, and an unchanged machine is the common
/// case, so the comparison costs no clone of any claim list.
fn install_claims_plan(
    machine: &mut TerminalMachine,
    plan: terminal_semantics::plain_affine::ClaimsPlan,
) -> Result<bool, terminal_semantics::plain_affine::Error> {
    let header_same = machine.contract.entry_claims == plan.entry_claims
        && machine.block_entry_claims == plan.block_entry_claims;
    if !header_same {
        plan.install(machine)?;
        return Ok(true);
    }
    let prior_loan_completions = std::mem::take(&mut machine.loan_completions);
    let mut prior: Vec<(usize, PriorClaims)> = Vec::new();
    let mut slot_index = 0;
    for_each_claim_slot(machine, |slot| {
        if !slot.is_empty() {
            prior.push((slot_index, slot.take()));
        }
        slot_index += 1;
    });
    plan.install(machine)?;
    let mut changed = machine.loan_completions != prior_loan_completions;
    let mut prior = prior.into_iter().peekable();
    let mut slot_index = 0;
    for_each_claim_slot(machine, |slot| {
        let unchanged = match prior.next_if(|(index, _)| *index == slot_index) {
            Some((_, old)) => slot.holds(&old),
            None => slot.is_empty(),
        };
        changed |= !unchanged;
        slot_index += 1;
    });
    Ok(changed)
}

fn preparation_error(
    machine: MachineId,
    error: terminal_semantics::plain_affine::Error,
) -> Refusal {
    let refusal = crate::lower::machine_order::WalkRefusal::new(&error);
    Refusal {
        reason: refusal.reason,
        site: terminal_check_tables::RefusalSite::Machine(machine, refusal.concerns),
        source_span: None,
        detail: format!("preparing logical claim transport: {}", refusal.detail),
        obligation: None,
    }
}

pub(crate) fn publish(
    diagnostic: MachineId,
    module: &mut TerminalModule,
    parallel: bool,
) -> Checked<()> {
    // Complete every public header before deriving call rows. The body and a
    // selected requirement can expose different contracts; neither borrows the
    // other's private promises. Boundary custody is separately authenticated.
    for (machine, contract) in module
        .execution
        .declaration_contracts
        .iter_mut()
        .map(|contract| (diagnostic, contract))
        .chain(
            module
                .machines
                .iter_mut()
                .map(|machine| (machine.id, &mut machine.contract)),
        )
    {
        terminal_semantics::plain_affine::publish_contract_claims(
            contract,
            &module.types.structural_types,
        )
        .map_err(|error| preparation_error(machine, error))?;
        terminal_semantics::content_transformations::entry_claims::publish_declared_entry_contents(
            &module.types.structural_types,
            &module.types.structural_domains,
            &module.identities,
            contract,
        )
        .map_err(|detail| Refusal {
            reason: RefusalReason::ForeignIdentity,
            site: terminal_check_tables::RefusalSite::Machine(machine, Construct::Machine),
            source_span: None,
            detail: detail.into(),
            obligation: None,
        })?;
    }
    // Each machine's claim rows come from its own ownership walk over the
    // module; the walks run on the invocation's workers (`plan_then_apply`).
    // A walk reads only this machine and its transitive callees/cleanups,
    // falling back to `None` (all machines) only when an unresolved/dynamic
    // callable or missing target is encountered.
    let read_sets = static_claim_read_sets(module);
    let machine_ordinals = machine_ordinal_index(module);
    struct PlannedClaims<'a> {
        reads: Option<&'a [MachineId]>,
        plan: Result<
            terminal_semantics::plain_affine::ClaimsPlan,
            terminal_semantics::plain_affine::Error,
        >,
    }
    crate::lower::machine_order::plan_then_apply_with(
        module,
        parallel,
        |walks, machine| {
            let reads = machine_ordinals
                .as_deref()
                .and_then(|ordinals| lookup_ordinal(ordinals, machine.id))
                .and_then(|slot| read_sets.get(slot))
                .and_then(Option::as_deref);
            let plan = walks
                .affine(machine.id)
                .and_then(|walks| walks.plan_claims());
            PlannedClaims { reads, plan }
        },
        |planned| planned.reads,
        |machine, planned| {
            planned
                .plan
                .and_then(|plan| install_claims_plan(machine, plan))
                .map_err(|error| preparation_error(machine.id, error))
        },
    )?;
    if !terminal_semantics::content_transformations::boundary_calls::has_declared_conservation(
        module,
    ) {
        return Ok(());
    }
    for ordinal in 0..module.machines.len() {
        let caller = &module.machines[ordinal];
        let mut rows = Vec::new();
        for operation in caller.operations() {
            // An affine value may carry a declared mathematical law without owning
            // any resource claim. That case still lowers for declaration checking.
            // Stage 06 independently requires evidence at each actual invocation.
            if !matches!(&operation.kind, terminal_psi::OperationKind::BoundaryCall { completion_receipts, .. } if !completion_receipts.is_empty())
                || !operation
                    .result
                    .as_structural()
                    .is_some_and(|result| !result.claims.is_empty())
            {
                continue;
            }
            rows.extend(
                terminal_semantics::content_transformations::boundary_calls::compositions(
                    module, caller, operation,
                )
                .map_err(|detail| Refusal {
                    reason: RefusalReason::Unsupported,
                    site: terminal_check_tables::RefusalSite::Machine(
                        caller.id,
                        Construct::Operation(operation.id),
                    ),
                    source_span: None,
                    detail: detail.into(),
                    obligation: None,
                })?,
            );
        }
        rows.sort_unstable();
        module.machines[ordinal]
            .content_partition_compositions
            .extend(rows);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena::NumericId;
    use semantic_vocabulary::{BlockId, CallableContractId, EdgeId, OperationId, ValueId};
    use terminal_psi::{
        Block, BoundaryMachineResult, MachineContract, Operation, OperationResult,
        TerminalDynamicInterface, TerminalDynamicSlot, TerminalDynamicTable, TerminalIdentityIndex,
        TerminalMachine,
    };

    fn id<T: NumericId>(raw: u64) -> T {
        T::new(raw).unwrap()
    }

    fn unit_machine(machine_id: u64, ops: Vec<Operation>) -> TerminalMachine {
        let mut block = Block::plain(
            id::<BlockId>(1),
            Vec::new(),
            Terminator::ReturnUnit {
                edge: id::<EdgeId>(1),
                cleanup_actions: Vec::new(),
            },
        );
        block.operations = ops;
        TerminalMachine {
            call_formations: Vec::new(),
            assembly_target_requirement: Default::default(),
            id: id::<MachineId>(machine_id),
            attachment: None,
            ranked_scc: None,
            structural_places: Vec::new(),
            block_entry_claims: Vec::new(),
            loan_completions: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: id::<BlockId>(1),
            blocks: vec![block],
            contract: MachineContract::empty(id(machine_id)),
        }
    }

    #[test]
    fn static_claim_read_sets_tracks_transitive_callees_and_dynamic_dispatch() {
        let mut module = TerminalModule::declarations();
        module
            .execution
            .dynamic_dispatch
            .interfaces
            .push(TerminalDynamicInterface {
                id: 0,
                identity: TerminalIdentityIndex(1),
                slots: vec![
                    TerminalDynamicSlot {
                        slot: 0,
                        declaring_trait_identity: TerminalIdentityIndex(1),
                        requirement_identity: TerminalIdentityIndex(1),
                        family_tuple: Vec::new(),
                        scalar_parameters: Vec::new(),
                        structural_parameters: Vec::new(),
                        entry_claim_paths: Vec::new(),
                        result: BoundaryMachineResult::Unit,
                        result_claim_paths: Vec::new(),
                        result_reference_regions: Vec::new(),
                    },
                    TerminalDynamicSlot {
                        slot: 1,
                        declaring_trait_identity: TerminalIdentityIndex(1),
                        requirement_identity: TerminalIdentityIndex(2),
                        family_tuple: Vec::new(),
                        scalar_parameters: Vec::new(),
                        structural_parameters: Vec::new(),
                        entry_claim_paths: Vec::new(),
                        result: BoundaryMachineResult::Unit,
                        result_claim_paths: Vec::new(),
                        result_reference_regions: Vec::new(),
                    },
                ],
            });
        module
            .execution
            .dynamic_dispatch
            .tables
            .push(TerminalDynamicTable {
                id: 0,
                interface: 0,
                implementation_identity: TerminalIdentityIndex(3),
                rows: vec![Some(id(2)), None],
            });
        module
            .execution
            .dynamic_dispatch
            .tables
            .push(TerminalDynamicTable {
                id: 1,
                interface: 0,
                implementation_identity: TerminalIdentityIndex(4),
                rows: vec![Some(id(3)), Some(id(3))],
            });

        let call_m2 = Operation::unsuspended(
            id::<OperationId>(1),
            OperationResult::Unit,
            OperationKind::Call {
                callee: id(2),
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        );
        let call_m3 = Operation::unsuspended(
            id::<OperationId>(1),
            OperationResult::Unit,
            OperationKind::Call {
                callee: id(3),
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        );
        let call_dyn_resolved = Operation::unsuspended(
            id::<OperationId>(1),
            OperationResult::Unit,
            OperationKind::CallDynamic {
                selector: id::<ValueId>(1),
                interface: 0,
                slot: 0,
                arguments: Vec::new(),
                family_actuals: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        );
        let call_dyn_unresolved = Operation::unsuspended(
            id::<OperationId>(1),
            OperationResult::Unit,
            OperationKind::CallDynamic {
                selector: id::<ValueId>(1),
                interface: 0,
                slot: 1,
                arguments: Vec::new(),
                family_actuals: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        );
        let call_machine_val = Operation::unsuspended(
            id::<OperationId>(1),
            OperationResult::Unit,
            OperationKind::CallMachine {
                callee: id::<ValueId>(1),
                contract: id::<CallableContractId>(1),
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        );

        module.machines = vec![
            unit_machine(1, vec![call_m2]),
            unit_machine(2, vec![call_m3]),
            unit_machine(3, Vec::new()),
            unit_machine(4, vec![call_dyn_resolved]),
            unit_machine(5, vec![call_dyn_unresolved]),
            unit_machine(6, vec![call_machine_val]),
        ];

        let sets = static_claim_read_sets(&module);
        assert_eq!(sets[0], Some(vec![id(2), id(3)]));
        assert_eq!(sets[1], Some(vec![id(3)]));
        assert_eq!(sets[2], Some(Vec::new()));
        assert_eq!(sets[3], Some(vec![id(2), id(3)]));
        assert_eq!(sets[4], None);
        assert_eq!(sets[5], None);
    }

    #[test]
    fn install_claims_plan_reports_exact_mutation_on_claim_free_and_claim_bearing_machines() {
        let mut plain = unit_machine(1, Vec::new());
        let plan = terminal_semantics::plain_affine::ClaimsPlan::default();
        assert!(!install_claims_plan(&mut plain, plan).unwrap());

        let mut mutating = unit_machine(2, Vec::new());
        let mut non_empty = terminal_semantics::plain_affine::ClaimsPlan::default();
        non_empty.entry_claims.push(terminal_psi::EntryClaim {
            claim: id(1),
            input: id(1),
            path: Vec::new(),
        });
        assert!(install_claims_plan(&mut mutating, non_empty.clone()).unwrap());
        assert!(!install_claims_plan(&mut mutating, non_empty).unwrap());
    }

    /// A call's claim transfers sit in the machine's blocks, not its header:
    /// installing them changes the machine, installing them again does not,
    /// and installing a plan without them changes it back.
    #[test]
    fn install_claims_plan_compares_block_claims_it_overwrites() {
        let call = Operation::unsuspended(
            id::<OperationId>(1),
            OperationResult::Unit,
            OperationKind::CallUnit {
                callee: id(2),
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        );
        let mut machine = unit_machine(1, vec![call]);
        let mut transferring = terminal_semantics::plain_affine::ClaimsPlan::default();
        transferring.call_transfers.insert(
            semantic_vocabulary::InvocationSite::Operation(id(1)),
            vec![terminal_psi::ClaimTransfer {
                claim: id(1),
                argument_index: 0,
                path: Vec::new(),
            }],
        );
        let empty = terminal_semantics::plain_affine::ClaimsPlan::default();
        assert!(!install_claims_plan(&mut machine, empty.clone()).unwrap());
        assert!(install_claims_plan(&mut machine, transferring.clone()).unwrap());
        assert!(!install_claims_plan(&mut machine, transferring).unwrap());
        assert!(install_claims_plan(&mut machine, empty).unwrap());
        let OperationKind::CallUnit {
            claim_transfers, ..
        } = &machine.blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        assert!(claim_transfers.is_empty());
    }
}
