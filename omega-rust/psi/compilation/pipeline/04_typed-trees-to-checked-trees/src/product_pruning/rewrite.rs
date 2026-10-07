//! Product roster rewriting.

use std::collections::HashSet;

use checked_trees::CheckedTrees;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;

use super::dependencies::MachineIndex;

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
    apply_typed_pruning(&mut checked.typed, retained, pruned);
    apply_fact_pruning(checked, retained, pruned, index);
}

/// Rebuild machine roots and machine-keyed sidecars. Backing declarations stay
/// available to exact authored-selection and visibility evidence; only roots
/// determine which implementations belong to this executable program.
pub(crate) fn apply_typed_pruning(
    program: &mut TypedTrees,
    retained: &HashSet<SymbolHandle>,
    pruned: &HashSet<SymbolHandle>,
) {
    // Append the retained roster without invalidating existing declaration
    // handles or erasing payloads still named by authored-selection evidence.
    let retained_machines: Vec<Machine> = program
        .machines()
        .iter()
        .filter(|machine| retained.contains(&machine.symbol))
        .cloned()
        .collect();
    program.roots.machines = program.tables.machines.insert_many(retained_machines);

    // Typed sidecars keyed by a machine identity.
    program
        .machine_specializations
        .retain(|row| retained.contains(&row.instance));
    // Trait requirement signatures own reach rows too; only rows owned by a
    // pruned machine leave.
    program
        .authored_service_reach_rows
        .retain(|row| !pruned.contains(&row.owner));
    program
        .evidence_forwardings
        .retain(|row| retained.contains(&row.machine_symbol));
    program
        .proof_output_calls
        .retain(|row| retained.contains(&row.machine_symbol));
    // Calling plans also belong to trait requirement signatures, which never
    // enter the machine roster. Keep their evaluated commitments; only a
    // plan whose requirement was actually pruned loses its identity.
    program
        .boundary_calling_plans
        .retain(|row| !pruned.contains(&row.requirement_machine));
    program
        .ranking_expression_custody
        .retain(|row| retained.contains(&row.machine));
}

/// Project sidecars whose exact declaration owner left the root roster.
/// Unknown/orphan owners are deliberately retained for ordinary validation:
/// absence is not authorization to discard malformed evidence.
pub(crate) fn apply_declaration_sidecar_pruning(
    program: &mut TypedTrees,
    removed: &HashSet<SymbolHandle>,
) {
    let symbols = &program.symbols;
    let owner_removed = |mut symbol: SymbolHandle| {
        while symbol.is_valid() {
            if removed.contains(&symbol) {
                return true;
            }
            symbol = symbols.get(symbol).parent;
        }
        false
    };
    program
        .plan_laid_layouts
        .retain(|row| !owner_removed(row.data_symbol));
    program
        .placed_view_plans
        .retain(|row| !owner_removed(row.data_symbol));
    program
        .wire_schema_plans
        .retain(|row| !owner_removed(row.schema));
    program
        .authored_service_reach_rows
        .retain(|row| !owner_removed(row.owner));
    program
        .boundary_calling_plans
        .retain(|row| !owner_removed(row.requirement_machine));
    program
        .fused_service_erasures
        .retain(|row| !owner_removed(row.requirement));
    // This roster has no inbound handles; source roles join by declaration
    // symbols. Wire placement arenas and other referenced backing rows stay.
    let completions = program
        .boundary_completion_declarations
        .iter()
        .filter(|(_, row)| !owner_removed(row.owner))
        .map(|(_, row)| row.clone())
        .collect::<Vec<_>>();
    let mut declarations = arena::Arena::default();
    declarations.insert_many(completions);
    program.boundary_completion_declarations = declarations;
}

fn apply_fact_pruning(
    checked: &mut CheckedTrees,
    retained: &HashSet<SymbolHandle>,
    pruned: &HashSet<SymbolHandle>,
    index: &MachineIndex,
) {
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
        .retain(|row| retained.contains(&row.machine.machine));
    facts
        .flow
        .terminal_structural_control_cleanups
        .states
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
        .retain(|row| !pruned.contains(&row.machine));
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
