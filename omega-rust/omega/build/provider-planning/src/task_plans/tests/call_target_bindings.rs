//! Production derivation of `CallTargetBinding` rows: a provider's
//! admission-time callee assignment covers a sealed unresolved site with
//! evidence `task_call_graph` derives for the real bound callee — its
//! validated subtree and canonical suspension crossings — never literals.

use crate::task_plans::CheckedTrees;
use crate::task_plans::stack_graphs::task_call_graph;
use crate::task_plans::tests::stack_graphs::{
    frame_entry, mutual_call_fixture, suspending_subtree_fixture,
};
use crate::task_plans::{CallTargetAssignment, derive_call_target_bindings};
use task_plans::{UnresolvedCallKind, compose_task_stack_demand, project_wcsu_stack_plan};

/// A plan whose projection carries the suspending fixture's one sealed
/// `NonCheckedSupply` site: `Alpha::run` suspends at a call into the
/// boundary-supplied `Slot::park`.
fn suspending_partial_plan() -> (
    CheckedTrees,
    symbols::SymbolHandle,
    task_plans::ValidatedActivationPlan,
    task_plans::UnresolvedCallSite,
    task_plans::CanonicalSuspensionCrossing,
) {
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
    let alpha_crossing = crate::task_plans::carry_crossings::canonical_suspension_crossing(
        &program,
        graph.crossings[0],
    )
    .expect("alpha's crossing canonicalizes");
    let demand =
        compose_task_stack_demand(graph.root, graph.frames).expect("the covered subgraph composes");
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
    (program, beta_machine, plan, site, alpha_crossing)
}

#[test]
fn derived_binding_covers_the_site_and_the_covered_plan_establishes_a_lease() {
    let (program, beta_machine, plan, site, alpha_crossing) = suspending_partial_plan();
    let layouts = layout::build_layout_plan(&program, NativeTarget::macos_arm64(), &[])
        .expect("synthetic machines lay out");
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
    let bound_crossing = crate::task_plans::carry_crossings::canonical_suspension_crossing(
        &program,
        bound.crossings[0],
    )
    .expect("beta's crossing canonicalizes");

    // Admission resolves the boundary-supplied call to the checked-body
    // `Beta::run`: the derivation produces the binding from the real
    // call-graph evidence, not a constructed literal.
    let bindings = derive_call_target_bindings(
        &program,
        NativeTarget::macos_arm64(),
        &[],
        &layouts,
        &plan,
        &[CallTargetAssignment {
            frame: site.frame,
            state: site.state.clone(),
            statement_index: site.statement_index,
            call_ordinal: site.call_ordinal,
            callee_entry: beta_entry.symbol,
        }],
    )
    .expect("the assignment derives one binding");
    assert_eq!(bindings.len(), 1);
    let binding = &bindings[0];
    assert_eq!(binding.frame, site.frame);
    assert_eq!(binding.state, site.state);
    assert_eq!(binding.statement_index, site.statement_index);
    assert_eq!(binding.call_ordinal, site.call_ordinal);
    assert_eq!(binding.callee, bound.root);
    assert_eq!(binding.subtree, bound.frames);
    assert_eq!(binding.crossings, vec![bound_crossing.clone()]);

    let covered = task_plans::TaskRuntimeAdmission::bind_call_targets(&plan, &bindings)
        .expect("the derived binding covers the sealed site");

    let covered_projection = covered.wcsu_stack_projection().expect("covered projection");
    assert!(covered_projection.is_exact());
    assert!(covered_projection.unresolved_calls().is_empty());
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

    // The exact covered plan backs a stack lease against a satisfying
    // provisioned slot.
    task_plans::establish_stack_lease(
        &covered,
        task_plans::StackLeaseBacking {
            provenance: task_plans::TaskStorageProvenance {
                owner: task_plans::TaskStorageOwnerId::from_normalized_identity(11)
                    .expect("storage owner"),
                lease: task_plans::TaskStorageLeaseId::from_normalized_identity(12)
                    .expect("lease era"),
            },
            backing: covered.candidate().stack_plan,
        },
    )
    .expect("the exact covered plan establishes a lease");
}

#[test]
fn derived_binding_covers_an_unresolved_target_site() {
    let (mut program, alpha_machine, _, _, beta_state) = mutual_call_fixture();
    // Retire Beta's call so the subtree the provider binds is a leaf, and
    // leave Alpha's call naming no machine state at all — the requirement
    // slot the provider resolves at admission.
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
    assert_eq!(site.kind, UnresolvedCallKind::UnresolvedTarget);
    // Alpha's calls never suspend, so the plan publishes no crossings.
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
            may_suspend: false,
            may_block: false,
            canonical_suspension_crossings: Vec::new(),
            carry_obligations: task_plans::ActivationCarryObligations::none(),
            cancellation_required: true,
        },
        projection,
    )
    .expect("the partial plan validates");

    let bindings = derive_call_target_bindings(
        &program,
        NativeTarget::macos_arm64(),
        &[],
        &layouts,
        &plan,
        &[CallTargetAssignment {
            frame: site.frame,
            state: site.state.clone(),
            statement_index: site.statement_index,
            call_ordinal: site.call_ordinal,
            callee_entry: beta_state,
        }],
    )
    .expect("the requirement-slot assignment derives a binding");
    assert_eq!(bindings.len(), 1);
    assert!(bindings[0].crossings.is_empty());

    // Beta's eight-byte frame charges beneath Alpha's eight-byte live
    // extent, and with the only site covered the plan publishes exact.
    let covered = task_plans::TaskRuntimeAdmission::bind_call_targets(&plan, &bindings)
        .expect("the derived binding covers the sealed site");
    let covered_projection = covered.wcsu_stack_projection().expect("covered projection");
    assert!(covered_projection.is_exact());
    assert_eq!(covered.candidate().stack_plan.bytes, 16);
    task_plans::establish_stack_lease(
        &covered,
        task_plans::StackLeaseBacking {
            provenance: task_plans::TaskStorageProvenance {
                owner: task_plans::TaskStorageOwnerId::from_normalized_identity(11)
                    .expect("storage owner"),
                lease: task_plans::TaskStorageLeaseId::from_normalized_identity(12)
                    .expect("lease era"),
            },
            backing: covered.candidate().stack_plan,
        },
    )
    .expect("the exact covered plan establishes a lease");
}

#[test]
fn an_assignment_to_a_non_checked_body_callee_rejects() {
    let (program, _, plan, site, _) = suspending_partial_plan();
    let layouts = layout::build_layout_plan(&program, NativeTarget::macos_arm64(), &[])
        .expect("synthetic machines lay out");
    // The call's nominal target is the boundary-supplied `Slot::park`
    // state; assigning it as the callee asserts checked evidence the
    // boundary machine cannot produce.
    let slot_state = program
        .facts
        .flow
        .control
        .calls
        .iter()
        .map(|(_, call)| call.target_symbol)
        .find(|target| {
            program
                .machines()
                .iter()
                .filter(|machine| {
                    program
                        .machine_states(machine)
                        .iter()
                        .any(|state| state.symbol == *target)
                })
                .any(|machine| {
                    machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                })
        })
        .expect("the unresolved call's nominal boundary target");

    let diagnostics = derive_call_target_bindings(
        &program,
        NativeTarget::macos_arm64(),
        &[],
        &layouts,
        &plan,
        &[CallTargetAssignment {
            frame: site.frame,
            state: site.state.clone(),
            statement_index: site.statement_index,
            call_ordinal: site.call_ordinal,
            callee_entry: slot_state,
        }],
    )
    .expect_err("a non-checked-body callee cannot supply validated subtree evidence");
    assert!(
        diagnostics
            .first()
            .expect("one diagnostic")
            .message
            .contains("checked body"),
        "unexpected diagnostic: {}",
        diagnostics.first().expect("one diagnostic").message
    );

    // The rejected claim mints no binding: the site stays uncovered and the
    // plan keeps refusing a lease.
    let covered = task_plans::TaskRuntimeAdmission::bind_call_targets(&plan, &[])
        .expect("an empty binding set re-seals the same partial plan");
    let projection = covered.wcsu_stack_projection().expect("still partial");
    assert!(!projection.is_exact());
    assert_eq!(
        projection.unresolved_calls().iter().collect::<Vec<_>>(),
        vec![&site],
    );
    task_plans::establish_stack_lease(
        &covered,
        task_plans::StackLeaseBacking {
            provenance: task_plans::TaskStorageProvenance {
                owner: task_plans::TaskStorageOwnerId::from_normalized_identity(11)
                    .expect("storage owner"),
                lease: task_plans::TaskStorageLeaseId::from_normalized_identity(12)
                    .expect("lease era"),
            },
            backing: covered.candidate().stack_plan,
        },
    )
    .expect_err("a site with no honest binding keeps the lease closed");
}

#[test]
fn a_derived_binding_carries_the_bound_subtrees_own_unresolved_sites() {
    let (mut program, beta_machine, plan, site, _) = suspending_partial_plan();
    // `Beta::run` still calls `Gamma::run`, but Gamma is now
    // boundary-supplied: the bound subtree's own call cannot resolve to a
    // checked body, so binding Alpha's site to Beta must not launder that
    // nested site into exactness.
    let gamma = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Gamma::run")
        .expect("gamma machine")
        .symbol;
    program
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.symbol == gamma)
        .expect("gamma machine")
        .supply_mode = language_semantics::MachineSupplyMode::Boundary;
    let layouts = layout::build_layout_plan(&program, NativeTarget::macos_arm64(), &[])
        .expect("synthetic machines lay out");
    let (_, beta_entry) = frame_entry(&program, beta_machine);

    let bindings = derive_call_target_bindings(
        &program,
        NativeTarget::macos_arm64(),
        &[],
        &layouts,
        &plan,
        &[CallTargetAssignment {
            frame: site.frame,
            state: site.state.clone(),
            statement_index: site.statement_index,
            call_ordinal: site.call_ordinal,
            callee_entry: beta_entry.symbol,
        }],
    )
    .expect("a checked-body callee with its own unresolved site still derives");
    assert_eq!(bindings.len(), 1);
    let nested: Vec<_> = bindings[0]
        .subtree
        .iter()
        .flat_map(|frame| frame.summary().unresolved_calls.iter())
        .collect();
    assert_eq!(nested.len(), 1, "Beta's frame carries its own sealed site");
    assert_eq!(nested[0].kind, UnresolvedCallKind::NonCheckedSupply);

    // Covering moves Alpha's site but Beta's nested site stays unresolved:
    // the covered projection stays partial and the lease stays closed.
    let covered = task_plans::TaskRuntimeAdmission::bind_call_targets(&plan, &bindings)
        .expect("the binding covers Alpha's site");
    let projection = covered.wcsu_stack_projection().expect("covered projection");
    assert!(!projection.is_exact());
    assert_eq!(
        projection.unresolved_calls().iter().collect::<Vec<_>>(),
        nested,
    );
    task_plans::establish_stack_lease(
        &covered,
        task_plans::StackLeaseBacking {
            provenance: task_plans::TaskStorageProvenance {
                owner: task_plans::TaskStorageOwnerId::from_normalized_identity(11)
                    .expect("storage owner"),
                lease: task_plans::TaskStorageLeaseId::from_normalized_identity(12)
                    .expect("lease era"),
            },
            backing: covered.candidate().stack_plan,
        },
    )
    .expect_err("a nested unresolved site keeps the lease closed");
}

#[test]
fn an_assignment_addressing_no_sealed_site_changes_nothing() {
    let (program, beta_machine, plan, site, _) = suspending_partial_plan();
    let layouts = layout::build_layout_plan(&program, NativeTarget::macos_arm64(), &[])
        .expect("synthetic machines lay out");
    let (_, beta_entry) = frame_entry(&program, beta_machine);

    // A coordinate the roster never sealed — the row may address another
    // plan's site — mints no binding.
    let bindings = derive_call_target_bindings(
        &program,
        NativeTarget::macos_arm64(),
        &[],
        &layouts,
        &plan,
        &[CallTargetAssignment {
            frame: site.frame,
            state: site.state.clone(),
            statement_index: 9,
            call_ordinal: site.call_ordinal,
            callee_entry: beta_entry.symbol,
        }],
    )
    .expect("an unaddressed assignment derives no binding");
    assert!(bindings.is_empty());

    let covered = task_plans::TaskRuntimeAdmission::bind_call_targets(&plan, &bindings)
        .expect("no bindings re-seal the same partial plan");
    assert_eq!(
        covered.normalized_identity(),
        plan.normalized_identity(),
        "nothing addressed means nothing changed"
    );
}

use target::NativeTarget;
