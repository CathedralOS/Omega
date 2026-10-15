//! Product roster rewriting.

use std::collections::HashSet;

use checked_trees::CheckedTrees;
use symbols::SymbolHandle;

use super::dependencies::MachineIndex;

/// Rebuild the machine declaration surface and every machine-keyed `Vec`
/// table so the product's iteration-visible rosters match the selection.
/// Machine-keyed roster arenas with no inbound `Handle`/`HandleSpan`
/// references are rebuilt the same way; arena rows referenced by spans or
/// handles from other retained rows remain as valid dead evidence and are
/// unreachable through the pruned declaration surface.
pub(super) fn apply_pruning(
    checked: CheckedTrees,
    retained: &HashSet<SymbolHandle>,
    pruned: &HashSet<SymbolHandle>,
    index: &MachineIndex,
) -> CheckedTrees {
    let mut typed = checked.typed.into_inner();
    let mut facts = checked.facts;
    validation::execution_projection::retain_machine_roster(&mut typed, retained, pruned);
    super::retain_live_family_members(&mut typed);
    apply_fact_pruning(&mut facts, retained, pruned, index);
    CheckedTrees::with_roots(typed, facts)
}

fn apply_fact_pruning(
    facts: &mut checked_trees::CheckFacts,
    retained: &HashSet<SymbolHandle>,
    pruned: &HashSet<SymbolHandle>,
    index: &MachineIndex,
) {
    facts
        .mutation
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .suspensions
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .blocking
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .synchronous_invocations
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .termination
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .termination
        .build_bound_progress
        .retain(|row| retained.contains(&row.machine));
    // A proof recursive component is one SCC: it survives pruning only when
    // every member survives, which the closed edge relation guarantees for
    // any component containing a retained member.
    facts
        .termination
        .proof_recursive_components
        .retain(|component| {
            component
                .members
                .iter()
                .all(|member| retained.contains(&member.machine))
        });
    facts
        .contract_plans
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .contract_plans
        .realized_envelopes
        .retain(|row| retained.contains(&row.machine));
    facts
        .qualifications
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .qualifications
        .vacuous_uses
        .retain(|row| retained.contains(&row.machine));
    facts
        .qualifications
        .content
        .identity_reshuffles
        .retain(|row| retained.contains(&row.machine_symbol));
    facts
        .qualifications
        .content
        .partition_compositions
        .retain(|row| retained.contains(&row.machine_symbol));
    facts
        .dynamic_conformances
        .selections
        .retain(|row| retained.contains(&row.machine));
    facts
        .carry
        .suspension_crossings
        .retain(|row| retained.contains(&row.machine));
    facts
        .carry
        .activation_wide_carry
        .retain(|row| retained.contains(&row.machine));
    facts
        .placed_view_inputs
        .retain(|row| retained.contains(&row.machine));
    facts
        .provider_closed_requirements
        .retain(|row| retained.contains(&row.provider));
    facts
        .operators
        .symbolic_boundary_applications
        .retain(|row| retained.contains(&row.machine_symbol));
    facts
        .nominal_machine_uses
        .uses
        .retain(|row| match row.site {
            checked_trees::NominalMachineUseSite::Statement(statement) => index
                .statement_machine
                .get(&statement)
                .is_some_and(|owner| retained.contains(owner)),
            checked_trees::NominalMachineUseSite::Expression(_) => true,
        });

    // Contract evidence rows keyed by an owner machine.
    facts
        .proof
        .contract_expression_evidence_calls
        .retain(|row| contract_owner_retained(row.owner, pruned));
    facts
        .proof
        .contract_expression_static_conformance_applications
        .retain(|row| contract_owner_retained(row.owner, pruned));

    // Machine-keyed roster arenas with no inbound handles or spans are
    // rebuilt so iteration sees exactly the retained product. Arenas whose
    // rows are referenced by `Handle`/`HandleSpan` from other retained rows
    // (`borrow.states`, `service_reaches.states`/`calls`,
    // the flow call/statement
    // arenas, and every proof/evidence arena) stay untouched: their dead
    // rows are unreachable through the pruned declaration surface and
    // compacting them would invalidate live inbound references.
    {
        let kept: Vec<checked_trees::FlowStateFact> = facts
            .flow
            .control
            .states
            .iter()
            .map(|(_, state)| state)
            .filter(|state| retained.contains(&state.machine_symbol))
            .cloned()
            .collect();
        let mut states = arena::Arena::default();
        states.insert_many(kept);
        facts.flow.control.states = states.into();
    }
    {
        let kept: Vec<checked_trees::MachineServiceReachRows> = facts
            .service_reaches
            .machines
            .iter()
            .map(|(_, row)| row)
            .filter(|row| retained.contains(&row.machine))
            .cloned()
            .collect();
        let mut machines = arena::Arena::default();
        facts.service_reaches.root_machines = machines.insert_many(kept);
        facts.service_reaches.machines = machines;
    }

    // Whole-product semantic dependencies: drop rows owned by or naming a
    // pruned machine. The table is independently rederivable via
    // `derive_checked_semantic_dependencies`.
    facts.flow.semantic_dependencies.rows.retain(|row| {
        retained.contains(&row.consumer_machine) && !pruned.contains(&row.dependency)
    });

    // Terminal plan families whose machine rosters enumerate the product.
    facts
        .flow
        .terminal_machines
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_debug
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts.flow.guarded_exits.tails.retain(|row| {
        index
            .state_machine
            .get(&row.state)
            .is_none_or(|machine| retained.contains(machine))
    });
    facts
        .flow
        .terminal_unit_effects
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .retain(|row| retained.contains(&row.machine));
    // A static boundary requirement's plan is keyed by its trait signature,
    // not a machine: only plans of a pruned machine leave.
    facts
        .flow
        .terminal_unit_effects
        .boundary_machines
        .retain(|row| !pruned.contains(&row.machine));
    facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .machines
        .retain(|row| retained.contains(&row.machine.machine));
    facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_structural_control_cleanups
        .states
        .retain(|row| retained.contains(&row.machine));
}

/// Whether a contract-fact owner survives pruning: machine owners must be
/// retained; non-machine owners (trait/operator machinery) are always
/// retained.
fn contract_owner_retained(
    owner: checked_trees::ContractProofFactOwner,
    pruned: &HashSet<SymbolHandle>,
) -> bool {
    match owner {
        checked_trees::ContractProofFactOwner::Machine { machine_symbol }
        | checked_trees::ContractProofFactOwner::MachineState { machine_symbol, .. } => {
            !pruned.contains(&machine_symbol)
        }
        _ => true,
    }
}
