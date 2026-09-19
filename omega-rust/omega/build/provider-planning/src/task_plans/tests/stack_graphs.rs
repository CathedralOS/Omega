//! Whole-call-graph WCSU derivation over synthetic checked trees: cycle
//! rejection, retired-call skipping, and provider-domain or unresolved call
//! targets that enter the frame's unresolved roster instead of silently
//! contributing zero.

use crate::task_plans::CheckedTrees;
use crate::task_plans::stack_graphs::task_call_graph;
use task_plans::{
    CallTargetBinding, UnresolvedCallKind, UnresolvedCallSite, compose_task_stack_demand,
    cover_unresolved_call_sites, project_wcsu_stack_plan,
};

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

#[test]
fn admission_binding_covers_an_unresolved_target_with_the_bound_subtree() {
    let (mut program, alpha_machine, _, beta_machine, beta_state) = mutual_call_fixture();
    // Retire Beta's call so the subtree the provider binds is a leaf.
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
    // Alpha's call names no machine state at graph time — the requirement
    // slot the provider binds at admission.
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
    let demand =
        compose_task_stack_demand(graph.root, graph.frames).expect("the covered subgraph composes");
    assert!(!demand.is_exact(), "the graph-time bound is partial");
    let representation =
        task_plans::StackRepresentationId::from_normalized_identity(9).expect("representation");
    let projection = project_wcsu_stack_plan(&demand, representation);
    let site = projection
        .unresolved_calls()
        .iter()
        .next()
        .expect("one sealed unresolved site")
        .clone();

    // Admission binds the slot to `Beta::run`: the bound callee's validated
    // subtree is exactly the graph derivation produces for it.
    let (beta, beta_entry) = frame_entry(&program, beta_machine);
    let bound = task_call_graph(
        &program,
        NativeTarget::macos_arm64(),
        &[],
        &layouts,
        beta,
        beta_entry,
    )
    .expect("the bound callee subtree derives");
    let covered = cover_unresolved_call_sites(
        &projection,
        &[CallTargetBinding {
            frame: site.frame,
            state: site.state.clone(),
            statement_index: site.statement_index,
            call_ordinal: site.call_ordinal,
            callee: bound.root,
            subtree: bound.frames,
            // Beta's subtree suspends nowhere, so its binding carries no
            // canonical crossing rows.
            crossings: Vec::new(),
        }],
    )
    .expect("the admission binding covers the sealed site");

    // Beta's eight-byte frame charges beneath Alpha's eight-byte live extent,
    // and with the only site covered the projection publishes exact.
    assert!(covered.is_exact());
    assert!(covered.unresolved_calls().is_empty());
    assert_eq!(covered.bytes(), 16);
    assert_eq!(covered.alignment(), 8);
    assert_eq!(covered.stack_plan().bytes, 16);

    // A binding that names no sealed site covers nothing: the partial
    // projection keeps its roster and still cannot lease.
    let unbound = cover_unresolved_call_sites(&projection, &[])
        .expect("an empty binding set re-seals the same partial projection");
    assert!(!unbound.is_exact());
    assert_eq!(
        unbound.unresolved_calls(),
        &std::collections::BTreeSet::from([site])
    );
}

/// `Alpha::run` suspends at a call into a boundary-supplied machine state —
/// the unresolved site provider admission later binds — while `Beta::run`
/// makes a may-suspend checked call to `Gamma::run`. Both suspending calls
/// retain their canonical crossing facts.
fn suspending_subtree_fixture() -> (CheckedTrees, symbols::SymbolHandle, symbols::SymbolHandle) {
    let alpha_machine = symbols::SymbolHandle::from_arena_index(1);
    let alpha_state = symbols::SymbolHandle::from_arena_index(2);
    let beta_machine = symbols::SymbolHandle::from_arena_index(3);
    let beta_state = symbols::SymbolHandle::from_arena_index(4);
    let gamma_machine = symbols::SymbolHandle::from_arena_index(5);
    let gamma_state = symbols::SymbolHandle::from_arena_index(6);
    let slot_machine = symbols::SymbolHandle::from_arena_index(7);
    let slot_state = symbols::SymbolHandle::from_arena_index(8);
    let mut program = CheckedTrees::default();

    for (machine_symbol, machine_name, state_symbol, supply_mode) in [
        (
            alpha_machine,
            "Alpha::run",
            alpha_state,
            language_semantics::MachineSupplyMode::CheckedBody,
        ),
        (
            beta_machine,
            "Beta::run",
            beta_state,
            language_semantics::MachineSupplyMode::CheckedBody,
        ),
        (
            gamma_machine,
            "Gamma::run",
            gamma_state,
            language_semantics::MachineSupplyMode::CheckedBody,
        ),
        (
            slot_machine,
            "Slot::park",
            slot_state,
            language_semantics::MachineSupplyMode::Boundary,
        ),
    ] {
        let mut machine = checked_trees::machine::Machine {
            symbol: machine_symbol,
            name: checked_trees::name::Identifier::generated(machine_name),
            supply_mode,
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
        (alpha_machine, alpha_state, slot_state),
        (beta_machine, beta_state, gamma_state),
    ] {
        let mut calls = arena::HandleSpan::empty();
        program.facts.flow.control.calls.append_to_span(
            &mut calls,
            checked_trees::FlowCallFact {
                statement_index: 0,
                call_ordinal: 0,
                target_symbol: target,
                suspension: language_semantics::SuspensionSummary {
                    direct_may_suspend: true,
                    transitive_may_suspend: false,
                },
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
        program
            .facts
            .carry
            .suspension_crossings
            .push(checked_trees::SuspensionCrossingCarryFact {
                machine: machine_symbol,
                state: state_symbol,
                statement_index: 0,
                call_ordinal: 0,
                target,
                receiver: None,
                effective: language_semantics::CarryPolicy::PERMISSIVE,
                live_values: Vec::new(),
            });
    }
    (program, alpha_machine, beta_machine)
}

#[test]
fn admission_binding_joins_the_bound_subtrees_canonical_crossings() {
    let (program, alpha_machine, beta_machine) = suspending_subtree_fixture();
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
    .expect("the unresolved boundary site still elaborates");
    assert_eq!(graph.frames.len(), 1);
    // Alpha's own suspending call already owns one canonical crossing row.
    assert_eq!(graph.crossings.len(), 1);
    let alpha_crossing = crate::task_plans::carry_crossings::canonical_suspension_crossing(
        &program,
        graph.crossings[0],
    )
    .expect("alpha's crossing canonicalizes");
    let demand =
        compose_task_stack_demand(graph.root, graph.frames).expect("the covered subgraph composes");
    assert!(!demand.is_exact(), "the graph-time bound is partial");
    let projection = project_wcsu_stack_plan(
        &demand,
        task_plans::StackRepresentationId::from_normalized_identity(9).expect("representation"),
    );
    let site = projection
        .unresolved_calls()
        .iter()
        .next()
        .expect("one sealed unresolved site")
        .clone();
    assert_eq!(site.kind, UnresolvedCallKind::NonCheckedSupply);

    // The plan validates over the graph-time roster — alpha's own crossing.
    let plan = task_plans::validate_wcsu_activation_plan(
        task_plans::ActivationPlanCandidate {
            machine_contract: task_plans::MachineContractId::from_normalized_identity(1)
                .expect("contract"),
            entry: task_plans::MachineEntryId::from_normalized_identity(2).expect("entry"),
            argument_layout: task_plans::TaskArgumentLayout::new(
                task_plans::ValueLayoutId::from_normalized_identity(3).expect("layout"),
                &[(8, 8), (4, 4)],
            )
            .expect("canonical argument layout"),
            terminal_outcome_layout: task_plans::ValueLayoutId::from_normalized_identity(4)
                .expect("outcome layout"),
            calling_plan: task_plans::CallingPlanId::from_normalized_identity(5).expect("calling"),
            stack_plan: projection.stack_plan(),
            may_suspend: true,
            may_block: false,
            canonical_suspension_crossings: vec![alpha_crossing.clone()],
            carry_obligations: task_plans::ActivationCarryObligations::none(),
            cancellation_required: true,
        },
        projection,
    )
    .expect("the graph-time roster validates over the partial projection");

    // Derive the bound subtree — `Beta::run` plus its checked `Gamma::run`
    // callee — and the canonical crossings its own may-suspend call owns.
    let (beta, beta_entry) = frame_entry(&program, beta_machine);
    let bound = task_call_graph(
        &program,
        NativeTarget::macos_arm64(),
        &[],
        &layouts,
        beta,
        beta_entry,
    )
    .expect("the bound callee subtree derives");
    assert_eq!(bound.frames.len(), 2);
    assert_eq!(bound.crossings.len(), 1);
    let bound_crossing = crate::task_plans::carry_crossings::canonical_suspension_crossing(
        &program,
        bound.crossings[0],
    )
    .expect("beta's crossing canonicalizes");

    // A binding presenting a retained crossing identity with different
    // content is a coordinate conflict and fails closed.
    let mut conflicting = alpha_crossing.clone();
    conflicting.preserve_host_thread = true;
    let diagnostic = task_plans::TaskRuntimeAdmission::bind_call_targets(
        &plan,
        &[CallTargetBinding {
            frame: site.frame,
            state: site.state.clone(),
            statement_index: site.statement_index,
            call_ordinal: site.call_ordinal,
            callee: bound.root,
            subtree: bound.frames.clone(),
            crossings: vec![conflicting],
        }],
    )
    .expect_err("a conflicting crossing coordinate fails closed");
    assert!(
        diagnostic.0.contains("conflicts"),
        "unexpected diagnostic: {}",
        diagnostic.0
    );

    let covered = task_plans::TaskRuntimeAdmission::bind_call_targets(
        &plan,
        &[CallTargetBinding {
            frame: site.frame,
            state: site.state.clone(),
            statement_index: site.statement_index,
            call_ordinal: site.call_ordinal,
            callee: bound.root,
            subtree: bound.frames,
            crossings: vec![bound_crossing.clone()],
        }],
    )
    .expect("the admission binding covers the site and joins its crossing");

    assert!(
        covered
            .wcsu_stack_projection()
            .expect("covered projection")
            .is_exact()
    );
    let mut expected = vec![alpha_crossing.identity, bound_crossing.identity];
    expected.sort();
    assert_eq!(
        covered
            .candidate()
            .canonical_suspension_crossings
            .iter()
            .map(|crossing| crossing.identity)
            .collect::<Vec<_>>(),
        expected,
        "the bound subtree's canonical crossing joins the plan's roster in canonical order"
    );
    assert_ne!(
        covered.normalized_identity(),
        plan.normalized_identity(),
        "joining the bound subtree's crossing re-seals the activation plan"
    );
}

use target::NativeTarget;
