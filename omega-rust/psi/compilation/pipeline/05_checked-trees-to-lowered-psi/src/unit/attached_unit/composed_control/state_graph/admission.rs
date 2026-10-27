//! Admit a composed graph from its plan: the route limits of its emitter
//! (a closed checked body without attachments), and the derivations the
//! emitter reads back — the live state mask, each position's authored owner,
//! entry aliases, claim transport, exclusive handoffs and the boundaries its
//! calls name. Stage 04 judged every state's parameter custody, entry claims,
//! edge transfers and closed-sum dispatch when it built the plan; lowering
//! lowers that plan and does not re-derive those judgments from source
//! (see the lowering ruling in TASKS.md, "Lowering simplification").
use super::super::super::{CheckedBoundaryMachinePlan, unsupported};
use super::super::{CheckedTrees, LoweringError};
use super::{
    CheckedComposedUnitControlMachinePlan, claims, entry_aliases, exclusive_handoffs, topology,
};
use crate::unit::attached_unit::call_closure::BoundaryIdentity;

pub(in crate::unit::attached_unit) struct AdmittedGraph<'a> {
    /// Entry-state successor-closure mask over the full `plan.states` roster;
    /// dead positions keep their index alignment but emit no block.
    pub(super) live: Vec<bool>,
    /// Each plan position's authored `(owner machine, state)`: a fused roster
    /// mixes owners, so emission resolves both through this row rather than
    /// assuming the dispatch machine's own state list.
    pub(super) authored: Vec<(
        &'a checked_trees::machine::Machine,
        &'a checked_trees::state::State,
    )>,
    pub(super) claim_transport: claims::ClaimTransport,
    /// One map per plan state; a present entry binds that successor
    /// structural parameter to `states[0].structural_parameters[root]`: the
    /// parameters that only ever receive one whole claim-bearing entry
    /// parameter, the entry's whole receiver or a whole entry loan
    /// (`entry_aliases`).
    pub(super) entry_aliases: Vec<std::collections::BTreeMap<u32, u32>>,
    /// Per plan state, the shared parameters declared as the whole exclusive
    /// loan every arrival forwards (`exclusive_handoffs`).
    pub(super) exclusive_handoffs: Vec<std::collections::BTreeSet<u32>>,
    pub(in crate::unit::attached_unit::composed_control) boundaries:
        Vec<(&'a CheckedBoundaryMachinePlan, BoundaryIdentity)>,
}

pub(in crate::unit::attached_unit) fn admit<'a>(
    checked: &'a CheckedTrees,
    plan: &'a CheckedComposedUnitControlMachinePlan,
) -> Result<AdmittedGraph<'a>, LoweringError> {
    super::super::admission::validate_contract(checked, plan)?;
    // The producer assigns scalar qualifications to scalar computations and
    // retains only structural requests here. Reclassifying every committed
    // domain as structural would reject those supported scalar operations.
    if plan.states.is_empty() || !plan.body_qualifications.is_empty() {
        return unsupported("Unit graph has unsupported qualification or provider custody");
    }
    let machine = checked
        .machine(plan.machine)
        .ok_or(LoweringError::Unsupported(
            "Unit graph has no authored machine",
        ))?;
    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !checked.machine_type_parameters(machine).is_empty()
    {
        return unsupported("Unit graph requires a closed checked body");
    }
    // The ordinary callable contract publishes the checked requirements and
    // guarantees. Result custody below independently transports linear claims;
    // a linear carrier does not require a second source-contract admission.
    super::ranking::require_witness_ranks(checked, plan)?;
    super::super::admission::reject_attachment(plan)?;
    // A fused roster admits foreign states: each position resolves its own
    // authored owner, no state may repeat, and the dispatch machine's authored
    // states still all appear in authored order.
    let authored = plan
        .states
        .iter()
        .map(|state| crate::declarations::state_declaration(checked, state.state))
        .collect::<Result<Vec<_>, _>>()?;
    checked
        .machine_states(machine)
        .first()
        .ok_or(LoweringError::Unsupported(
            "Unit graph has no authored entry state",
        ))?;
    if let Some(identity) = plan.result.structural_identity() {
        let mut declarations = checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .filter(|declaration| declaration.identity == identity);
        declarations.next().ok_or(LoweringError::Unsupported(
            "Unit graph result carrier declaration is absent",
        ))?;
        if declarations.next().is_some() {
            return unsupported("Unit graph result carrier declaration is ambiguous");
        }
    }
    // Reachability is pure topology on the checked plan. States outside the
    // entry successor closure never execute: their custody rows cannot
    // disqualify the route and emission prunes them.
    let live = topology::live(plan)?;
    // Dead states are authored and shape-checked above but never execute:
    // their parameter, claim, and call rows have no producing edge, so the
    // emitted graph prunes to the entry state's successor closure.
    // A whole entry parameter proven to arrive unchanged on every edge keeps
    // its invocation place: a claim-bearing owner instead of a block
    // parameter, a loan instead of duplicating its authority.
    let mut entry_aliases = vec![std::collections::BTreeMap::new(); plan.states.len()];
    entry_aliases::alias_forwarded_entry_claims(plan, &live, &mut entry_aliases)?;
    entry_aliases::alias_entry_parameter_loans(plan, &live, &mut entry_aliases)?;
    // Only physical aliases need claim-name rebinding. Ordinary block arguments
    // carry their checked obligations directly; do not replay incoming moves
    // against the checker's permission events to prepare this namespace.
    let claim_transport = claims::bind_invocation_claims(plan, &live, &entry_aliases)?;
    // A shared parameter that only ever receives its source's whole
    // exclusive loan holds that loan; Terminal edges transfer mutable names.
    let exclusive_handoffs =
        exclusive_handoffs::promote_shared_handoffs(checked, plan, &live, &entry_aliases)?;
    let states = plan
        .states
        .iter()
        .zip(&live)
        .filter_map(|(state, live)| live.then_some(state))
        .collect::<Vec<_>>();
    // Every live state's calls take the one call admission; internal Unit
    // targets' signatures come from the closure that emits this graph.
    let boundaries = super::super::admission::admit_calls(checked, &states)?;
    Ok(AdmittedGraph {
        live,
        authored,
        claim_transport,
        entry_aliases,
        exclusive_handoffs,
        boundaries,
    })
}
