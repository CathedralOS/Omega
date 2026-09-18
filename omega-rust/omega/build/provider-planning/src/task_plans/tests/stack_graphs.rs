//! Whole-call-graph WCSU derivation over synthetic checked trees: cycle
//! rejection, retired-call skipping, and provider-domain or unresolved call
//! targets that enter the frame's unresolved roster instead of silently
//! contributing zero.

use crate::task_plans::CheckedTrees;
use crate::task_plans::stack_graphs::task_call_graph;
use task_plans::{UnresolvedCallKind, UnresolvedCallSite, compose_task_stack_demand};

/// Two checked machines whose entry states call each other. `Alpha::run`
/// calls `Beta::run`; `Beta::run` calls `Alpha::run` unless its call row is
/// retired by selected execution.
fn mutual_call_fixture() -> (
    CheckedTrees,
    symbols::SymbolHandle,
    symbols::SymbolHandle,
    symbols::SymbolHandle,
    symbols::SymbolHandle,
) {
    let alpha_machine = symbols::SymbolHandle::from_arena_index(1);
    let alpha_state = symbols::SymbolHandle::from_arena_index(2);
    let beta_machine = symbols::SymbolHandle::from_arena_index(3);
    let beta_state = symbols::SymbolHandle::from_arena_index(4);
    let mut program = CheckedTrees::default();

    for (machine_symbol, machine_name, state_symbol) in [
        (alpha_machine, "Alpha::run", alpha_state),
        (beta_machine, "Beta::run", beta_state),
    ] {
        let mut machine = checked_trees::machine::Machine {
            symbol: machine_symbol,
            name: checked_trees::name::Identifier::generated(machine_name),
            ..Default::default()
        };
        let mut state = checked_trees::state::State {
            symbol: state_symbol,
            name: checked_trees::name::Identifier::generated("run"),
            ..Default::default()
        };
        program
            .typed
            .statement_table
            .push_statement(&mut state.statement_nodes, Default::default());
        program.typed.push_machine_state(&mut machine, state);
        program.typed.push_machine(machine);
    }

    for (machine_symbol, state_symbol, target) in [
        (alpha_machine, alpha_state, beta_state),
        (beta_machine, beta_state, alpha_state),
    ] {
        let mut calls = arena::HandleSpan::empty();
        program.facts.flow.control.calls.append_to_span(
            &mut calls,
            checked_trees::FlowCallFact {
                statement_index: 0,
                call_ordinal: 0,
                target_symbol: target,
                ..Default::default()
            },
        );
        program
            .facts
            .flow
            .control
            .states
            .append(checked_trees::FlowStateFact {
                machine_symbol,
                state_symbol,
                calls,
                ..Default::default()
            });
    }
    (
        program,
        alpha_machine,
        alpha_state,
        beta_machine,
        beta_state,
    )
}

fn frame_entry(
    program: &CheckedTrees,
    machine_symbol: symbols::SymbolHandle,
) -> (
    &checked_trees::machine::Machine,
    &checked_trees::state::State,
) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("fixture machine");
    let entry = program
        .machine_states(machine)
        .first()
        .expect("fixture entry state");
    (machine, entry)
}

#[test]
fn task_call_graph_rejects_a_non_lowered_call_cycle() {
    let (program, alpha_machine, _, _, _) = mutual_call_fixture();
    let layouts = layout::build_layout_plan(&program, NativeTarget::macos_arm64(), &[])
        .expect("synthetic machines lay out");
    let (machine, entry) = frame_entry(&program, alpha_machine);

    let graph = task_call_graph(
        &program,
        NativeTarget::macos_arm64(),
        &[],
        &layouts,
        machine,
        entry,
    )
    .expect("the reachable graph elaborates before composition rejects the cycle");
    assert_eq!(graph.frames.len(), 2);
    let error = compose_task_stack_demand(graph.root, graph.frames)
        .expect_err("a call cycle has no finite same-stack demand");
    assert!(error.to_string().contains("non-lowered call cycle"));
}

#[test]
fn task_call_graph_skips_calls_retired_by_selected_execution() {
    let (mut program, alpha_machine, _, _, beta_state) = mutual_call_fixture();
    program
        .facts
        .flow
        .control
        .retired_calls
        .push(checked_trees::RetiredFlowCall {
            state_symbol: beta_state,
            statement_index: 0,
            call_ordinal: 0,
        });
    let layouts = layout::build_layout_plan(&program, NativeTarget::macos_arm64(), &[])
        .expect("synthetic machines lay out");
    let (machine, entry) = frame_entry(&program, alpha_machine);

    let graph = task_call_graph(
        &program,
        NativeTarget::macos_arm64(),
        &[],
        &layouts,
        machine,
        entry,
    )
    .expect("retired call rows place no checked frame");
    assert_eq!(graph.frames.len(), 2);
    // With the `Beta::run -> Alpha::run` edge retired, the chain is acyclic:
    // the callee's frame extends the caller's live extent on the same stack.
    let demand =
        compose_task_stack_demand(graph.root, graph.frames).expect("acyclic chain composes");
    assert_eq!(demand.bytes(), 16);
    assert_eq!(demand.alignment(), 8);
}

#[test]
fn task_call_graph_records_unresolved_targets_in_a_partial_bound() {
    let (mut program, alpha_machine, _, _, beta_state) = mutual_call_fixture();
    // A call whose target names no checked machine state -- a requirement
    // slot, machine parameter or dynamic coordinate -- contributes no checked
    // edge, so `Alpha::run` remains the only frame in its own graph. But the
    // call still enters the unresolved roster: the covered subgraph's demand
    // publishes as partial, never as an exact whole-call-graph WCSU.
    program.facts.flow.control.calls.for_each_mut(|_, call| {
        if call.target_symbol == beta_state {
            call.target_symbol = symbols::SymbolHandle::from_arena_index(99);
        }
    });
    let layouts = layout::build_layout_plan(&program, NativeTarget::macos_arm64(), &[])
        .expect("synthetic machines lay out");
    let (machine, entry) = frame_entry(&program, alpha_machine);

    let graph = task_call_graph(
        &program,
        NativeTarget::macos_arm64(),
        &[],
        &layouts,
        machine,
        entry,
    )
    .expect("unresolved targets need no checked frame to elaborate");
    assert_eq!(graph.frames.len(), 1);
    let site = UnresolvedCallSite {
        frame: graph.root,
        state: "run".into(),
        statement_index: 0,
        call_ordinal: 0,
        kind: UnresolvedCallKind::UnresolvedTarget,
    };
    assert_eq!(
        graph.frames[0].summary().unresolved_calls,
        vec![site.clone()]
    );
    let demand =
        compose_task_stack_demand(graph.root, graph.frames).expect("a lone frame composes");
    assert_eq!(demand.bytes(), 8);
    assert!(!demand.is_exact());
    assert_eq!(
        demand.unresolved_calls(),
        &std::collections::BTreeSet::from([site])
    );
}

#[test]
fn task_call_graph_records_non_checked_supply_targets_in_a_partial_bound() {
    let (mut program, alpha_machine, _, beta_machine, _) = mutual_call_fixture();
    // A call whose target resolves to a machine state supplied by non-checked
    // means -- requirement, boundary, admission-claim or external realization
    // -- places no checked frame on this stack, but the live call still
    // enters the unresolved roster so the bound publishes as partial.
    let beta = program
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.symbol == beta_machine)
        .expect("fixture machine");
    beta.supply_mode = language_semantics::MachineSupplyMode::Boundary;
    let layouts = layout::build_layout_plan(&program, NativeTarget::macos_arm64(), &[])
        .expect("synthetic machines lay out");
    let (machine, entry) = frame_entry(&program, alpha_machine);

    let graph = task_call_graph(
        &program,
        NativeTarget::macos_arm64(),
        &[],
        &layouts,
        machine,
        entry,
    )
    .expect("a boundary callee needs no checked frame to elaborate");
    assert_eq!(graph.frames.len(), 1);
    let site = UnresolvedCallSite {
        frame: graph.root,
        state: "run".into(),
        statement_index: 0,
        call_ordinal: 0,
        kind: UnresolvedCallKind::NonCheckedSupply,
    };
    assert_eq!(
        graph.frames[0].summary().unresolved_calls,
        vec![site.clone()]
    );
    let demand =
        compose_task_stack_demand(graph.root, graph.frames).expect("a lone frame composes");
    assert_eq!(demand.bytes(), 8);
    assert!(!demand.is_exact());
    assert_eq!(
        demand.unresolved_calls(),
        &std::collections::BTreeSet::from([site])
    );
}

use target::NativeTarget;
