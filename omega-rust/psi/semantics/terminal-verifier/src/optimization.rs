//! Independent checks of target-neutral rewrites before Terminal publication.
//!
//! This file owns block-local evidence and machine retention. Each rewrite
//! has its own validator: `dead_scalar_elimination.rs`,
//! `copy_propagation.rs`, `global_value_numbering.rs`,
//! `sparse_conditional_constant_propagation.rs`, `control_flow_cleanup.rs`
//! and `proof_check_elision.rs`.

mod control_flow_cleanup;
mod copy_propagation;
mod dead_scalar_elimination;
mod global_value_numbering;
mod proof_check_elision;
mod sparse_conditional_constant_propagation;

pub use control_flow_cleanup::{ControlFlowCleanupRewriteError, validate_control_flow_cleanup};
pub use copy_propagation::{CopyPropagationRewriteError, validate_copy_propagation};
pub use dead_scalar_elimination::{DeadScalarRewriteError, validate_dead_scalar_elimination};
pub use global_value_numbering::{
    GlobalValueNumberingRewriteError, validate_global_value_numbering,
};
pub use proof_check_elision::{ProofCheckElisionRewriteError, validate_proof_check_elision};
pub use sparse_conditional_constant_propagation::{
    SparseConditionalConstantPropagationRewriteError,
    validate_sparse_conditional_constant_propagation,
};

use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId, ValueId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::TerminalModule;

/// Block-local identities that module-level evidence carriers name by
/// identity rather than through a direct executable use. A control-flow
/// rewrite may not drop a row one of these carriers retains: the referent
/// stays authoritative even where no ordinary operand reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockLocalEvidence {
    /// Blocks named as invariant headers or exact projection owners.
    pub blocks: BTreeSet<BlockId>,
    /// Edges carrying qualification coercions or invariant arrivals.
    pub edges: BTreeSet<EdgeId>,
    /// Operations carrying suspension plans, proof-output joins, dynamic
    /// dispatches, reborrow restorations, closed reach calls, partition
    /// theorem producers, or crash contracts.
    pub operations: BTreeSet<OperationId>,
    /// Values named inside propositions, coercion endpoints, retained
    /// suspension frontiers, float entry range parameters, or float-meaning
    /// projection sources.
    pub values: BTreeSet<ValueId>,
}

/// Inventory the block-local rows semantic evidence retains across rewrites.
///
/// Every Terminal identity is module-unique, so the carrier that names a row
/// pins it wherever the row lives. The scan covers only carriers stored in the
/// module itself; the lowering stage separately protects rows its unsealed
/// sidecars name.
pub fn block_local_evidence(module: &TerminalModule) -> BlockLocalEvidence {
    let mut evidence = BlockLocalEvidence::default();
    fn retain_proposition(
        proposition: &semantic_vocabulary::Proposition,
        evidence: &mut BlockLocalEvidence,
    ) {
        proposition.visit_value_ids(|value| {
            evidence.values.insert(value);
        });
    }
    for coercion in &module.scalar_qualifications.coercions {
        evidence.edges.insert(coercion.edge);
        evidence.values.insert(coercion.source);
        evidence.values.insert(coercion.destination);
    }
    for range in &module.scalar_qualifications.float_entry_ranges {
        evidence.values.insert(range.parameter);
    }
    for invariant in &module.scalar_block_invariants {
        evidence.blocks.insert(invariant.header);
        retain_proposition(&invariant.predicate, &mut evidence);
        for arrival in &invariant.arrivals {
            evidence.edges.insert(arrival.edge);
        }
    }
    for contract in &module.operation_crash_contracts {
        evidence.operations.insert(contract.operation);
        // The published roster speaks the row's own formal namespace, so
        // only the continuations — the owning machine's actual values —
        // pin the identities inside their guard predicates.
        for bucket in &contract.crash_continuations {
            for alternative in &bucket.alternatives {
                if let terminal_psi::CrashRouteGuard::Predicate(term) = alternative {
                    retain_proposition(term.proposition(), &mut evidence);
                }
            }
        }
    }
    for site in &module.suspension_call_sites {
        evidence.operations.insert(site.operation);
    }
    for plan in &module.suspension_call_plans {
        evidence.operations.insert(plan.operation);
        for live in &plan.live_values {
            if let terminal_psi::TerminalSuspensionPlace::Scalar(value) = live.place {
                evidence.values.insert(value);
            }
        }
    }
    for call in &module.proof_output_calls {
        if let Some(runtime) = &call.runtime_call {
            evidence.operations.insert(runtime.operation);
        }
    }
    let dispatch = &module.dynamic_dispatch;
    for operation in dispatch
        .arguments
        .iter()
        .map(|row| row.operation)
        .chain(dispatch.direct_dispatches.iter().map(|row| row.operation))
        .chain(dispatch.indirect_dispatches.iter().map(|row| row.operation))
        .chain(dispatch.stored_dispatches.iter().map(|row| row.operation))
        .chain(
            dispatch
                .parameter_dispatches
                .iter()
                .map(|row| row.operation),
        )
    {
        evidence.operations.insert(operation);
    }
    for restoration in &module.reborrow_restored_call_uses {
        evidence.operations.insert(restoration.operation);
    }
    for projection in &module.float_meaning_projections {
        match &projection.source {
            terminal_psi::FloatMeaningSource::DirectMachineParameter(parameter) => {
                evidence.values.insert(parameter.parameter);
            }
            terminal_psi::FloatMeaningSource::DirectMachineResult(result) => {
                evidence.values.insert(result.result);
            }
            terminal_psi::FloatMeaningSource::DirectBlockParameter(parameter) => {
                evidence.blocks.insert(parameter.block);
                evidence.values.insert(parameter.parameter);
            }
            terminal_psi::FloatMeaningSource::DirectOperationResult(result) => {
                evidence.operations.insert(result.producer);
                evidence.values.insert(result.result);
            }
            terminal_psi::FloatMeaningSource::DirectCallResult(result) => {
                evidence.operations.insert(result.producer);
                evidence.values.insert(result.result);
            }
            // A semantic application references only proof-row operands; it
            // retains no runtime value, operation, or block evidence.
            terminal_psi::FloatMeaningSource::TransitionalInput(_)
            | terminal_psi::FloatMeaningSource::DirectStructuralLeaf(_)
            | terminal_psi::FloatMeaningSource::ExactBinary32Literal(_)
            | terminal_psi::FloatMeaningSource::ExactBinary64Literal(_)
            | terminal_psi::FloatMeaningSource::SemanticApplication(_) => {}
        }
    }
    for machine in &module.machines {
        if let Some(application) = &machine.closed_reach_application {
            for call in &application.calls {
                evidence.operations.insert(call.operation);
            }
        }
        for composition in &machine.content_partition_compositions {
            evidence.operations.insert(composition.producer_operation);
        }
        for proposition in &machine.contract.requires {
            retain_proposition(proposition, &mut evidence);
        }
        for clause in &machine.contract.ensures {
            retain_proposition(&clause.proposition, &mut evidence);
        }
        for clause in &machine.contract.outcome_specific_ensures {
            retain_proposition(&clause.proposition, &mut evidence);
        }
        for bucket in &machine.contract.crash_routes {
            for alternative in &bucket.alternatives {
                if let terminal_psi::CrashRouteGuard::Predicate(term) = alternative {
                    retain_proposition(term.proposition(), &mut evidence);
                }
            }
        }
    }
    evidence
}

/// Whether dropping `block` would erase a row module-level evidence still
/// names, or orphan a machine-level structural place declaration rooted at
/// this block's parameters or operation results.
fn evidence_bound_block(block: &terminal_psi::Block, evidence: &BlockLocalEvidence) -> bool {
    if evidence.blocks.contains(&block.id) || !block.structural_parameters.is_empty() {
        return true;
    }
    for operation in &block.operations {
        if operation.static_reach_binding.is_some()
            || evidence.operations.contains(&operation.id)
            || matches!(
                operation.result,
                terminal_psi::OperationResult::Structural(_)
            )
        {
            return true;
        }
        if operation
            .result
            .scalar()
            .is_some_and(|result| evidence.values.contains(&result.id))
        {
            return true;
        }
    }
    if block
        .parameters
        .iter()
        .any(|parameter| evidence.values.contains(&parameter.id))
    {
        return true;
    }
    block
        .terminator
        .edges()
        .any(|edge| evidence.edges.contains(&edge))
}

/// Machines surviving rows keep alive independent of intra-machine
/// reachability: the module entry, every attached or ranked machine, every
/// provider candidate, and every machine a module-level custody or evidence
/// row names are roots, and each retained machine then keeps the machines
/// its call, selected-evidence, cleanup, and closed-reach transitions name.
pub fn retained_machines(module: &TerminalModule) -> BTreeSet<MachineId> {
    retained_machines_with_roots(module, Vec::new())
}

/// `retained_machines` closed over extra roots only a producer can name.
/// A row the verifier's inventory cannot see — the lowering stage's
/// unsealed checked-source and selected-IEEE sidecars — can keep a machine
/// authored without making it a retention root, and the machines that kept
/// machine's own surviving transitions name must stay as well.
pub fn retained_machines_with_roots(
    module: &TerminalModule,
    additional_roots: impl IntoIterator<Item = MachineId>,
) -> BTreeSet<MachineId> {
    let mut retained = machine_retention_roots(module);
    retained.extend(additional_roots);
    let by_id: BTreeMap<MachineId, &terminal_psi::TerminalMachine> = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect();
    let mut frontier: Vec<MachineId> = retained.iter().copied().collect();
    while let Some(machine) = frontier.pop() {
        let Some(body) = by_id.get(&machine) else {
            // A row may name a machine outside the module's own machine
            // table; that root still forbids removal but closes over nothing.
            continue;
        };
        for target in machine_transitions(body) {
            if retained.insert(target) {
                frontier.push(target);
            }
        }
    }
    retained
}

/// Every machine identity a module-level row names is a retention root:
/// coercions, float entry ranges, invariants, operation crash contracts,
/// suspensions, proof outputs, conformance applications, dynamic-dispatch
/// custody, reborrow publications, placed views, float projections, evidence
/// lanes, providers, and attached machines each keep their named machine
/// regardless of call reachability.
fn machine_retention_roots(module: &TerminalModule) -> BTreeSet<MachineId> {
    let mut roots = BTreeSet::new();
    roots.insert(module.entry);
    for coercion in &module.scalar_qualifications.coercions {
        roots.insert(coercion.machine);
    }
    for range in &module.scalar_qualifications.float_entry_ranges {
        roots.insert(range.machine);
    }
    for invariant in &module.scalar_block_invariants {
        roots.insert(invariant.machine);
    }
    for contract in &module.operation_crash_contracts {
        roots.insert(contract.machine);
    }
    for site in &module.suspension_call_sites {
        if let terminal_psi::TerminalSuspensionCallTarget::Machine(machine) = site.target {
            roots.insert(machine);
        }
    }
    for plan in &module.suspension_call_plans {
        if let terminal_psi::TerminalSuspensionCallTarget::Machine(machine) = plan.target {
            roots.insert(machine);
        }
    }
    for call in &module.proof_output_calls {
        roots.insert(call.caller);
        if let Some(runtime) = &call.runtime_call {
            roots.insert(runtime.callee);
        }
        if let Some(dispatch) = &call.static_requirement_dispatch {
            roots.insert(dispatch.realization);
        }
    }
    for application in &module.closed_conformance_applications {
        roots.insert(application.owner);
        for callable in &application.realization_callables {
            roots.insert(callable.machine);
        }
    }
    let dispatch = &module.dynamic_dispatch;
    for owner in dispatch
        .parameters
        .iter()
        .map(|row| row.owner)
        .chain(dispatch.arguments.iter().map(|row| row.owner))
        .chain(dispatch.selections.iter().map(|row| row.owner))
        .chain(dispatch.rebound_descriptors.iter().map(|row| row.owner))
        .chain(dispatch.stored_descriptors.iter().map(|row| row.owner))
        .chain(dispatch.direct_dispatches.iter().map(|row| row.owner))
        .chain(dispatch.indirect_dispatches.iter().map(|row| row.owner))
        .chain(dispatch.stored_dispatches.iter().map(|row| row.owner))
        .chain(dispatch.parameter_dispatches.iter().map(|row| row.owner))
    {
        roots.insert(owner);
    }
    for realization in dispatch
        .direct_dispatches
        .iter()
        .map(|row| row.realization)
        .chain(
            dispatch
                .indirect_dispatches
                .iter()
                .map(|row| row.realization),
        )
        .chain(dispatch.stored_dispatches.iter().map(|row| row.realization))
    {
        roots.insert(realization);
    }
    for handoff in &module.reborrow_root_handoffs {
        roots.insert(handoff.machine);
    }
    for use_row in &module.reborrow_restored_call_uses {
        roots.insert(use_row.machine);
        roots.insert(use_row.call_target_machine);
    }
    for input in &module.placed_view_inputs {
        roots.insert(input.machine);
    }
    for lane in &module.evidence_contract_lanes {
        roots.insert(lane.machine);
    }
    for candidate in &module.provider_candidates {
        roots.insert(candidate.candidate);
    }
    for projection in &module.float_meaning_projections {
        match &projection.source {
            terminal_psi::FloatMeaningSource::DirectMachineParameter(row) => {
                roots.insert(row.owner);
            }
            terminal_psi::FloatMeaningSource::DirectMachineResult(row) => {
                roots.insert(row.owner);
            }
            terminal_psi::FloatMeaningSource::DirectBlockParameter(row) => {
                roots.insert(row.owner);
            }
            terminal_psi::FloatMeaningSource::DirectOperationResult(row) => {
                roots.insert(row.owner);
            }
            terminal_psi::FloatMeaningSource::DirectCallResult(row) => {
                roots.insert(row.owner);
            }
            terminal_psi::FloatMeaningSource::DirectStructuralLeaf(row) => {
                roots.insert(row.owner);
            }
            terminal_psi::FloatMeaningSource::TransitionalInput(_)
            | terminal_psi::FloatMeaningSource::ExactBinary32Literal(_)
            | terminal_psi::FloatMeaningSource::ExactBinary64Literal(_)
            | terminal_psi::FloatMeaningSource::SemanticApplication(_) => {}
        }
    }
    for machine in &module.machines {
        // Attached machines are reachable through nominal-type custody rather
        // than call edges, and ranked machines carry execution-position
        // evidence that must survive: both are unconditional roots.
        if machine.attachment.is_some() || machine.ranked_scc.is_some() {
            roots.insert(machine.id);
        }
    }
    roots
}

/// Machines a retained machine keeps alive through its surviving contents:
/// every direct call callee, each selected-evidence use target, each nominal
/// cleanup machine, and each callee the closed reach application selects.
/// Blocks already removed by the rewrite no longer carry their callees.
fn machine_transitions(machine: &terminal_psi::TerminalMachine) -> Vec<MachineId> {
    let mut targets = Vec::new();
    for block in &machine.blocks {
        for operation in &block.operations {
            match &operation.kind {
                terminal_psi::OperationKind::Call { callee, .. }
                | terminal_psi::OperationKind::CallUnit { callee, .. }
                | terminal_psi::OperationKind::CallStructuralScalar { callee, .. }
                | terminal_psi::OperationKind::CallStructuralWithScalarArguments {
                    callee, ..
                } => targets.push(*callee),
                terminal_psi::OperationKind::CallStructural {
                    callee,
                    selected_evidence,
                    ..
                } => {
                    targets.push(*callee);
                    for evidence in selected_evidence {
                        for evidence_use in &evidence.uses {
                            targets.push(evidence_use.target);
                        }
                    }
                }
                _ => {}
            }
        }
        match &block.terminator {
            terminal_psi::Terminator::Return {
                cleanup_actions, ..
            } => {
                for action in cleanup_actions {
                    if let terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup) =
                        action
                    {
                        targets.push(cleanup.cleanup_machine);
                    }
                }
            }
            terminal_psi::Terminator::ReturnUnitNominalAffine { cleanups, .. } => {
                for cleanup in cleanups {
                    targets.push(cleanup.cleanup_machine);
                }
            }
            _ => {}
        }
    }
    if let Some(application) = &machine.closed_reach_application {
        for parameter in &application.telescope {
            if let terminal_psi::ClosedReachParameter::Machine(binding) = parameter
                && let Some(callee) = binding.callee
            {
                targets.push(callee);
            }
        }
        for call in &application.calls {
            if let Some(application) = &call.application {
                targets.push(application.callee);
            }
        }
    }
    targets
}

/// Whether dropping `machine` would erase a block, edge, operation, or value
/// surviving evidence still names, including the machine's own scalar
/// parameters and declared scalar result. Machine-local declarations such as
/// structural places and reach bindings die with the machine and do not pin
/// it; only identities a surviving row can still name matter here.
pub fn machine_evidence_bound(
    machine: &terminal_psi::TerminalMachine,
    evidence: &BlockLocalEvidence,
) -> bool {
    if machine
        .parameters
        .iter()
        .any(|parameter| evidence.values.contains(&parameter.id))
        || machine
            .result
            .scalar_ref()
            .is_some_and(|result| evidence.values.contains(&result.id))
    {
        return true;
    }
    for block in &machine.blocks {
        if evidence.blocks.contains(&block.id)
            || block
                .parameters
                .iter()
                .any(|parameter| evidence.values.contains(&parameter.id))
            || block
                .terminator
                .edges()
                .any(|edge| evidence.edges.contains(&edge))
        {
            return true;
        }
        for operation in &block.operations {
            if evidence.operations.contains(&operation.id)
                || operation
                    .result
                    .scalar_ref()
                    .is_some_and(|result| evidence.values.contains(&result.id))
            {
                return true;
            }
        }
    }
    false
}
