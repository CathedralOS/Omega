//! Product roster rewriting and independent post-rewrite graph validation.

use std::collections::HashSet;

use checked_trees::CheckedTrees;
use symbols::SymbolHandle;
use typed_trees::machine::Machine;

use super::CheckedTreeProductRoots;
use super::dependencies::{MachineIndex, collect_machine_edges};

/// Rebuild the machine declaration surface and every machine-keyed `Vec`
/// table so the product's iteration-visible rosters match the selection.
/// Machine-keyed roster arenas with no inbound `Handle`/`HandleSpan`
/// references are rebuilt the same way; arena rows referenced by spans or
/// handles from other retained rows remain as valid dead evidence and are
/// unreachable through the pruned declaration surface.
pub(super) fn apply_pruning(
    checked: &mut CheckedTrees,
    retained: &HashSet<SymbolHandle>,
    pruned: &HashSet<SymbolHandle>,
    index: &MachineIndex,
) {
    let program = &mut checked.typed;

    // Machine declarations: rebuild the arena so `roots.machines` covers
    // exactly the retained declaration surface in source order.
    let retained_machines: Vec<Machine> = program
        .machines()
        .iter()
        .filter(|machine| retained.contains(&machine.symbol))
        .cloned()
        .collect();
    let mut machines = arena::Arena::default();
    let machine_roots = machines.insert_many(retained_machines);
    program.tables.machines = machines;
    program.roots.machines = machine_roots;

    // Typed sidecars keyed by a machine identity.
    program
        .machine_specializations
        .retain(|row| retained.contains(&row.instance));
    program
        .authored_service_reach_rows
        .retain(|row| retained.contains(&row.owner));
    program
        .evidence_forwardings
        .retain(|row| retained.contains(&row.machine_symbol));
    program
        .proof_output_calls
        .retain(|row| retained.contains(&row.machine_symbol));
    program
        .boundary_calling_plans
        .retain(|row| retained.contains(&row.requirement_machine));
    program
        .ranking_expression_custody
        .retain(|row| retained.contains(&row.machine));

    let facts = &mut checked.facts;

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
        .dynamic_conformances
        .storages
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

    // Boundary adapter rows keyed by the selected realization machine.
    facts.boundary_adapter_dispatch.retain(|row| {
        index
            .machine_of(row.realization_state)
            .is_none_or(|machine| retained.contains(&machine))
    });

    // Machine-keyed roster arenas with no inbound handles or spans are
    // rebuilt so iteration sees exactly the retained product. Arenas whose
    // rows are referenced by `Handle`/`HandleSpan` from other retained rows
    // (`borrow.states`, `service_reaches.states`/`calls`,
    // `carry.contained_fields`/`contained_targets`, the flow call/statement
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
        facts.flow.control.states = states;
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
    {
        let kept: Vec<checked_trees::MachineCarryTopologyFact> = facts
            .carry
            .machine_topologies
            .iter()
            .map(|(_, row)| row)
            .filter(|row| retained.contains(&row.machine))
            .cloned()
            .collect();
        let mut topologies = arena::Arena::default();
        topologies.insert_many(kept);
        facts.carry.machine_topologies = topologies;
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
    facts
        .flow
        .terminal_scalar_graphs
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_scalar_graphs
        .guarded_tails
        .retain(|row| {
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
    facts
        .flow
        .terminal_unit_effects
        .boundary_machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_partial_affine_unit_cleanups
        .machines
        .retain(|row| retained.contains(&row.machine.machine));
    facts
        .flow
        .terminal_nominal_affine_unit_cleanups
        .machines
        .retain(|row| retained.contains(&row.machine.machine));
    facts
        .flow
        .terminal_structural_control_cleanups
        .states
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_structural_control_cleanups
        .projected_edges
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_structural_unit_controls
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_structural_scalar_returns
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_structural_scalar_returns
        .selected_operator_machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_structural_scalar_returns
        .trait_operator_machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_boundary_scalar_returns
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_boundary_scalar_returns
        .boundary_machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_structural_returns
        .machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_structural_returns
        .claim_free_affine_machines
        .retain(|row| retained.contains(&row.machine));
    facts
        .flow
        .terminal_structural_call_returns
        .payloadless_guarded_machines
        .retain(|row| retained.contains(&row.machine));

    // Dynamic-dispatch rows keyed by the calling machine.
    let dispatch = &mut facts.flow.terminal_unit_effects.dynamic_dispatch;
    dispatch
        .transfers
        .retain(|row| retained.contains(&row.caller_machine));
    dispatch
        .calls
        .retain(|row| retained.contains(&row.caller_machine()));
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

/// Independent post-pruning validation: re-extract the edge relation on the
/// pruned product and reject any retained machine that still references a
/// pruned declaration. The check reruns the same extraction rather than
/// trusting the pre-pruning analysis.
pub(super) fn validate_pruned_product(
    checked: &CheckedTrees,
    retained: &HashSet<SymbolHandle>,
    pruned: &HashSet<SymbolHandle>,
    roots: &CheckedTreeProductRoots,
) -> Result<(), Vec<String>> {
    let mut failures = Vec::new();

    let index = MachineIndex::build(&checked.typed);
    if checked.typed.machines().len() != retained.len() {
        failures.push(format!(
            "pruned machine table holds {} machines but the selection retains {}",
            checked.typed.machines().len(),
            retained.len()
        ));
    }
    for machine in checked.typed.machines() {
        if !retained.contains(&machine.symbol) {
            failures.push(format!(
                "pruned machine table contains unretained machine symbol #{}",
                machine.symbol.arena_index()
            ));
        }
    }
    for root in roots.machines() {
        if !index.machine_symbols.contains(root) {
            failures.push(format!(
                "product root symbol #{} is absent from the pruned machine table",
                root.arena_index()
            ));
        }
    }

    // The pruned edge relation must be closed: no retained machine may reach
    // a symbol outside the pruned product's machine table.
    let edges = collect_machine_edges(&checked.typed, &checked.facts, &index);
    for (source, targets) in &edges {
        if pruned.contains(source) {
            failures.push(format!(
                "edge relation still names pruned machine #{} as a source",
                source.arena_index()
            ));
        }
        for target in targets {
            if pruned.contains(target) {
                failures.push(format!(
                    "retained machine #{} still references pruned machine #{}",
                    source.arena_index(),
                    target.arena_index()
                ));
            }
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}
