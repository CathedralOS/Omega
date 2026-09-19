use super::{
    activation_crossing_validation_fixture, concrete_task_start_fixture, crossing_error,
    nested_task_call_fixture,
};
use crate::task_plans::carry_crossings::validate_activation_carry_crossing;
use crate::task_plans::runtime_requirements::selected_task_runtime_provider;
use crate::task_plans::specialization_commitments::{
    exact_task_machine_contract, task_specialization_commitment,
};
use crate::task_plans::start_selections::{exact_task_activation_target, task_start_selections};
use crate::task_plans::{
    Arc, CheckedTrees, NativeTarget, TaskActivationPlanSet, elaborate_task_activation_plans,
    settle_task_activation_plans,
};
use language_semantics::{CarryCpu, CarryHostThread};
use task_plans::{ActivationCarryObligations, TaskStartOperation};

#[test]
fn activation_crossings_reject_missing_machine() {
    let (program, _, _, _) = activation_crossing_validation_fixture();
    let mut crossing = program.facts.carry.suspension_crossings[0].clone();
    crossing.machine = symbols::SymbolHandle::invalid();
    let diagnostics = validate_activation_carry_crossing(&program, &crossing)
        .expect_err("missing crossing machine must fail closed");
    assert!(diagnostics[0].message.contains("exact typed machine"));
}

#[test]
fn activation_crossings_reject_cross_machine_state() {
    let (mut program, root, root_state, _) = activation_crossing_validation_fixture();
    program.facts.carry.suspension_crossings[0].state = root_state;
    assert!(crossing_error(&program, root).contains("belong to its exact typed machine"));
}

#[test]
fn activation_crossings_reject_out_of_range_statement() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    program.facts.carry.suspension_crossings[0].statement_index = 1;
    assert!(crossing_error(&program, root).contains("statement must belong"));
}

#[test]
fn activation_crossings_reject_missing_flow_state() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    program.facts.flow.control.states = Default::default();
    assert!(crossing_error(&program, root).contains("one exact checked flow state"));
}

#[test]
fn activation_crossings_reject_ambiguous_flow_state() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    let duplicate = program
        .facts
        .flow
        .control
        .states
        .iter()
        .next()
        .expect("flow state")
        .1
        .clone();
    program.facts.flow.control.states.append(duplicate);
    assert!(crossing_error(&program, root).contains("exactly one checked flow state"));
}

#[test]
fn activation_crossings_reject_invalid_call_span() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    program.facts.flow.control.states.for_each_mut(|_, state| {
        state.calls = arena::HandleSpan::from_parts(arena::Handle::invalid(), 1);
    });
    assert!(crossing_error(&program, root).contains("exact valid call span"));
}

#[test]
fn activation_crossings_reject_missing_call_coordinate() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    program.facts.carry.suspension_crossings[0].call_ordinal = 2;
    assert!(crossing_error(&program, root).contains("one exact checked flow call"));
}

#[test]
fn activation_crossings_reject_ambiguous_call_coordinate() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    let call = program
        .facts
        .flow
        .control
        .calls
        .iter()
        .next()
        .expect("flow call")
        .1
        .clone();
    let calls = program
        .facts
        .flow
        .control
        .calls
        .insert_many([call.clone(), call]);
    program
        .facts
        .flow
        .control
        .states
        .for_each_mut(|_, state| state.calls = calls);
    assert!(crossing_error(&program, root).contains("exactly one checked flow call"));
}

#[test]
fn activation_crossings_reject_target_drift() {
    let (mut program, root, _, child_state) = activation_crossing_validation_fixture();
    program.facts.carry.suspension_crossings[0].target = child_state;
    assert!(crossing_error(&program, root).contains("exact checked call target"));
}

#[test]
fn activation_crossings_reject_missing_typed_target() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    program.facts.carry.suspension_crossings[0].target = symbols::SymbolHandle::invalid();
    program
        .facts
        .flow
        .control
        .calls
        .for_each_mut(|_, call| call.target_symbol = symbols::SymbolHandle::invalid());
    assert!(crossing_error(&program, root).contains("target must name an exact typed state"));
}

#[test]
fn activation_crossings_reject_non_suspending_call() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    program
        .facts
        .flow
        .control
        .calls
        .for_each_mut(|_, call| call.suspension = Default::default());
    assert!(crossing_error(&program, root).contains("may-suspend checked call"));
}

#[test]
fn activation_crossings_reject_duplicate_coordinate() {
    let (mut program, root, _, _) = activation_crossing_validation_fixture();
    let duplicate = program.facts.carry.suspension_crossings[0].clone();
    program.facts.carry.suspension_crossings.push(duplicate);
    assert!(crossing_error(&program, root).contains("one row per exact call coordinate"));
}

#[test]
fn nested_checked_call_composes_whole_graph_wcsu_and_roster() {
    let (checked, selected, _) = nested_task_call_fixture();

    let task_activations =
        elaborate_task_activation_plans(&checked, &selected, NativeTarget::macos_arm64(), &[])
            .expect("a task target calling a suspending checked callee should elaborate");
    let activation = task_activations
        .as_slice()
        .iter()
        .find(|activation| activation.operation == TaskStartOperation::Start)
        .expect("start activation plan");
    let plan = activation.plan.candidate();

    // Both the `Worker::run` frame and the nested `Helper::work` frame must
    // contribute to the composed demand: 24 live bytes of caller frontier plus
    // the callee's 16-byte frame placed on the same fixed stack.
    assert_eq!(plan.stack_plan.bytes, 40);
    assert_eq!(plan.stack_plan.alignment, 8);
    let projection = activation
        .plan
        .wcsu_stack_projection()
        .expect("the nested activation stack must carry sealed WCSU evidence");
    assert_eq!(projection.frame_validations().len(), 2);
    assert_eq!(projection.stack_plan(), plan.stack_plan);

    // The `Sleeper::park` boundary calls in `Worker::run` and `Helper::work`
    // resolve to non-checked supply: they place no checked frame on this
    // stack, so the sealed bound is partial and the roster names each exact
    // call coordinate provider admission must still cover.
    assert!(!projection.is_exact());
    assert_eq!(
        projection
            .unresolved_calls()
            .iter()
            .map(|site| site.kind)
            .collect::<Vec<_>>(),
        vec![
            task_plans::UnresolvedCallKind::NonCheckedSupply,
            task_plans::UnresolvedCallKind::NonCheckedSupply
        ]
    );

    // The canonical roster covers both crossings in the root frame and the
    // parking call inside the nested checked frame.
    assert!(plan.may_suspend);
    assert_eq!(plan.canonical_suspension_crossings.len(), 3);
}

#[test]
fn suspending_call_without_canonical_crossing_rejects_elaboration() {
    let (mut checked, selected, _) = concrete_task_start_fixture();
    // Erase the checked crossing row that covers the parked `park` call: the
    // may-suspend call remains in the flow facts, so derivation must reject
    // rather than publish a roster that lost the demand.
    let worker = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Worker::run")
        .expect("worker machine")
        .symbol;
    checked
        .facts
        .carry
        .suspension_crossings
        .retain(|crossing| crossing.machine != worker);
    let diagnostics =
        elaborate_task_activation_plans(&checked, &selected, NativeTarget::macos_arm64(), &[])
            .expect_err("a possibly-suspending call without a crossing must fail closed");
    assert!(
        diagnostics[0]
            .message
            .contains("no canonical suspension crossing"),
        "unexpected diagnostic: {}",
        diagnostics[0].message
    );
}

#[test]
fn compiler_task_activation_settlement_borrows_shared_program_and_commits_two_rows() {
    let (checked, selected, _) = concrete_task_start_fixture();
    let original_sidecar = Arc::new(TaskActivationPlanSet::default());
    let mut retained = Arc::clone(&original_sidecar);

    settle_task_activation_plans(
        &mut retained,
        &checked,
        &selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect("shared checked custody should produce the complete activation sidecar");

    assert!(!Arc::ptr_eq(&original_sidecar, &retained));
    assert_eq!(retained.as_slice().len(), 2);
    assert!(
        retained
            .as_slice()
            .iter()
            .any(|activation| { activation.operation == TaskStartOperation::Start })
    );
    assert!(
        retained
            .as_slice()
            .iter()
            .any(|activation| { activation.operation == TaskStartOperation::TryStart })
    );
}

#[test]
fn compact_equal_specialization_substitution_changes_authoritative_commitment() {
    let (mut checked, selected, _) = concrete_task_start_fixture();
    let baseline_start = task_start_selections(&checked)
        .expect("baseline task selections")
        .into_iter()
        .find(|selection| selection.operation == TaskStartOperation::Start)
        .expect("baseline start selection");
    let target_specialization_index = checked.typed.machine_specializations.len();
    checked
        .typed
        .machine_specializations
        .push(typed_trees::typed_trees::MachineSpecialization {
            template: baseline_start.target_machine,
            instance: baseline_start.target_machine,
            type_argument_identities: vec!["exact::Baseline".to_owned()],
            report_fingerprint: 0x4455,
            ..Default::default()
        });
    let baseline_runtime = selected_task_runtime_provider(&checked, &selected, &baseline_start)
        .expect("baseline selected task runtime");
    let (baseline_machine, baseline_entry) = exact_task_activation_target(
        &checked,
        baseline_start.target_machine,
        baseline_start.target_entry,
    )
    .expect("baseline exact task target");
    let baseline_contract = exact_task_machine_contract(
        &checked,
        baseline_machine.symbol,
        baseline_machine.name.as_str(),
    )
    .expect("baseline exact task contract");
    let baseline_commitment = task_specialization_commitment(
        &checked,
        &baseline_start,
        baseline_machine,
        baseline_entry,
        baseline_contract,
        baseline_runtime.requirement_identity.as_str(),
    )
    .expect("baseline task specialization commitment");

    let mut substituted = checked.clone();
    substituted.typed.machine_specializations[target_specialization_index]
        .type_argument_identities[0] = "exact::Substituted".to_owned();
    assert_eq!(
        substituted.machine_specializations[target_specialization_index].report_fingerprint,
        checked.machine_specializations[target_specialization_index].report_fingerprint,
    );
    let (changed_machine, changed_entry) = exact_task_activation_target(
        &substituted,
        baseline_start.target_machine,
        baseline_start.target_entry,
    )
    .expect("changed exact task target");
    let changed_contract = exact_task_machine_contract(
        &substituted,
        changed_machine.symbol,
        changed_machine.name.as_str(),
    )
    .expect("changed exact task contract");
    let changed_commitment = task_specialization_commitment(
        &substituted,
        &baseline_start,
        changed_machine,
        changed_entry,
        changed_contract,
        baseline_runtime.requirement_identity.as_str(),
    )
    .expect("changed task specialization commitment");

    assert_ne!(baseline_commitment, changed_commitment);
}

#[test]
fn compiler_task_activation_rejection_preserves_prior_sidecar_identity() {
    let (mut checked, selected, _) = concrete_task_start_fixture();
    let retained =
        elaborate_task_activation_plans(&checked, &selected, NativeTarget::macos_arm64(), &[])
            .expect("fixture should produce a retained activation sidecar");
    let target = retained
        .as_slice()
        .iter()
        .find(|activation| activation.operation == TaskStartOperation::Start)
        .expect("fixture start activation")
        .target_machine;
    checked
        .facts
        .suspensions
        .machines
        .retain(|fact| fact.machine != target);
    let original_sidecar = Arc::new(retained);
    let mut retained = Arc::clone(&original_sidecar);

    let diagnostics = settle_task_activation_plans(
        &mut retained,
        &checked,
        &selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect_err("missing exact suspension evidence must reject settlement");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "task activation target `Worker::run` has no checked suspension plan"
    );
    assert!(Arc::ptr_eq(&original_sidecar, &retained));
}

#[test]
fn compiler_task_activation_settlement_commits_canonical_empty_sidecar() {
    let original_sidecar = Arc::new(TaskActivationPlanSet::default());
    let checked = CheckedTrees::default();
    let selected = effects::SelectedProviderPlanFacts::default();
    let mut retained = Arc::clone(&original_sidecar);

    settle_task_activation_plans(
        &mut retained,
        &checked,
        &selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect("an empty checked program should produce an empty activation sidecar");

    assert!(!Arc::ptr_eq(&original_sidecar, &retained));
    assert_eq!(retained.as_ref(), &TaskActivationPlanSet::default());
}

#[test]
fn concrete_task_start_specialization_elaborates_a_validated_plan() {
    let (checked, selected, provider_plans) = concrete_task_start_fixture();

    let mut foreign_leaf_plan = provider_plans[0].clone();
    foreign_leaf_plan.schema.trait_name = "other::TaskRuntime".to_owned();
    let foreign_leaf_selected = effects::SelectedProviderPlanFacts::from_selection(
        &[foreign_leaf_plan.clone()],
        &[foreign_leaf_plan.name.clone()],
    )
    .expect("same-leaf foreign schema remains a structurally complete plan");
    let diagnostics = elaborate_task_activation_plans(
        &checked,
        &foreign_leaf_selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect_err("same-leaf foreign schema must not satisfy exact TaskRuntime owner");
    assert!(
        diagnostics[0]
            .message
            .contains("no retained selected provider plan")
    );

    let mut wrong_method_owner_plan = provider_plans[0].clone();
    for method in &mut wrong_method_owner_plan.schema.methods {
        method.requirement_owner = "other::TaskRuntime".to_owned();
    }
    let wrong_method_owner_selected = effects::SelectedProviderPlanFacts::from_selection(
        &[wrong_method_owner_plan.clone()],
        &[wrong_method_owner_plan.name.clone()],
    )
    .expect("method-owner drift remains structurally complete");
    let diagnostics = elaborate_task_activation_plans(
        &checked,
        &wrong_method_owner_selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect_err("method owner drift must not satisfy exact TaskRuntime requirement");
    assert!(
        diagnostics[0]
            .message
            .contains("no retained selected provider plan")
    );

    let task_activations =
        elaborate_task_activation_plans(&checked, &selected, NativeTarget::macos_arm64(), &[])
            .expect("elaborate activation plan");
    let activations = task_activations.as_slice();
    assert_eq!(activations.len(), 2);
    let activation = activations
        .iter()
        .find(|activation| activation.operation == TaskStartOperation::Start)
        .expect("start activation plan");
    let target_machine_symbol = activation.target_machine;
    assert!(
        activations
            .iter()
            .any(|activation| activation.operation == TaskStartOperation::TryStart)
    );
    let target = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == activation.target_machine)
        .expect("target machine");
    assert_eq!(target.name.as_str(), "Worker::run");
    let plan = activation.plan.candidate();
    // The whole-call-graph bound retains the resume word (8), the activation
    // argument `job` (4), the local `value` (4) and the staged `park`
    // call-argument copy (4) aligned to the pointer grid.
    assert_eq!(plan.stack_plan.bytes, 24);
    assert_eq!(plan.stack_plan.alignment, 8);
    let projection = activation
        .plan
        .wcsu_stack_projection()
        .expect("the activation stack must carry sealed WCSU evidence");
    assert_eq!(projection.stack_plan(), plan.stack_plan);
    assert_eq!(projection.frame_validations().len(), 1);
    assert!(
        projection
            .admitted_contribution_report_identities()
            .is_empty()
    );
    assert!(plan.may_suspend);
    assert!(!plan.may_block);
    assert_eq!(plan.canonical_suspension_crossings.len(), 1);
    assert_eq!(plan.carry_obligations, ActivationCarryObligations::none());
    assert!(plan.cancellation_required);
    assert_ne!(plan.machine_contract.normalized_identity(), 0);
    assert_ne!(plan.argument_layout.identity.normalized_identity(), 0);
    // `Worker::run` takes one `Job { value: i32 }` argument: the marshalling
    // layout packs it as one four-byte field at offset 0 in a four-byte
    // image — the exact byte extent a moved start bundle marshals under.
    assert_eq!(plan.argument_layout.fields.len(), 1);
    assert_eq!(plan.argument_layout.fields[0].offset, 0);
    assert_eq!(plan.argument_layout.fields[0].bytes, 4);
    assert_eq!(plan.argument_layout.fields[0].alignment, 4);
    assert_eq!(plan.argument_layout.bytes, 4);
    assert_eq!(plan.argument_layout.alignment, 4);
    assert_ne!(plan.terminal_outcome_layout.normalized_identity(), 0);
    assert_ne!(
        activation.plan.normalized_identity().normalized_identity(),
        0
    );
    assert_eq!(
        activation.selected_runtime.provider_plan_name,
        "LocalTaskRuntime::satisfies::TaskRuntime"
    );
    assert_eq!(
        activation.selected_runtime.runtime.normalized_identity(),
        selected
            .plans()
            .first()
            .expect("selected runtime plan")
            .report_fingerprint()
    );
    assert!(
        activation
            .selected_runtime
            .requirement_identity
            .contains("TaskRuntime")
    );

    let mut missing_suspension = checked.clone();
    missing_suspension
        .facts
        .suspensions
        .machines
        .retain(|fact| fact.machine != activation.target_machine);
    let diagnostics = elaborate_task_activation_plans(
        &missing_suspension,
        &selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect_err("missing exact target suspension facts must fail closed");
    assert_eq!(
        diagnostics[0].message,
        "task activation target `Worker::run` has no checked suspension plan"
    );

    let mut missing_blocking = checked.clone();
    missing_blocking
        .facts
        .blocking
        .machines
        .retain(|fact| fact.machine != activation.target_machine);
    let diagnostics = elaborate_task_activation_plans(
        &missing_blocking,
        &selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect_err("missing exact target blocking facts must fail closed");
    assert_eq!(
        diagnostics[0].message,
        "task activation target `Worker::run` has no checked blocking plan"
    );

    let unrelated_machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol != target_machine_symbol)
        .expect("unrelated checked machine")
        .symbol;

    let mut missing_carry = checked.clone();
    missing_carry
        .facts
        .carry
        .activation_wide_carry
        .retain(|fact| fact.machine != target_machine_symbol);
    let diagnostics = elaborate_task_activation_plans(
        &missing_carry,
        &selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect_err("missing exact target carry envelope must fail closed");
    assert_eq!(
        diagnostics[0].message,
        "task activation target `Worker::run` has no exact activation-wide CPU/thread carry envelope"
    );

    let mut duplicate_carry = checked.clone();
    let duplicate = duplicate_carry
        .facts
        .carry
        .activation_wide_carry
        .iter()
        .find(|fact| fact.machine == target_machine_symbol)
        .expect("target carry envelope")
        .clone();
    duplicate_carry
        .facts
        .carry
        .activation_wide_carry
        .push(duplicate);
    let diagnostics = elaborate_task_activation_plans(
        &duplicate_carry,
        &selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect_err("duplicate exact target carry envelope must fail closed");
    assert_eq!(
        diagnostics[0].message,
        "task activation target `Worker::run` has duplicate exact activation-wide CPU/thread carry envelopes"
    );

    let mut incomplete_carry = checked.clone();
    incomplete_carry
        .facts
        .carry
        .activation_wide_carry
        .iter_mut()
        .find(|fact| fact.machine == target_machine_symbol)
        .expect("target carry envelope")
        .analysis_complete = false;
    let diagnostics = elaborate_task_activation_plans(
        &incomplete_carry,
        &selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect_err("incomplete exact target carry envelope must fail closed");
    assert_eq!(
        diagnostics[0].message,
        "task activation target `Worker::run` has incomplete activation-wide CPU/thread carry analysis"
    );

    let mut authoritative_carry = checked.clone();
    let target_carry = authoritative_carry
        .facts
        .carry
        .activation_wide_carry
        .iter_mut()
        .find(|fact| fact.machine == target_machine_symbol)
        .expect("target carry envelope");
    target_carry.effective.cpu = CarryCpu::Origin;
    target_carry.effective.host_thread = CarryHostThread::Origin;
    let authoritative = elaborate_task_activation_plans(
        &authoritative_carry,
        &selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect("exact checked target envelope remains the sole carry authority");
    let authoritative_plan = authoritative
        .as_slice()
        .iter()
        .find(|activation| activation.operation == TaskStartOperation::Start)
        .expect("authoritative start activation")
        .plan
        .candidate();
    assert_eq!(
        authoritative_plan.carry_obligations,
        ActivationCarryObligations {
            preserve_cpu: true,
            preserve_host_thread: true,
        }
    );
    assert_eq!(authoritative_plan.stack_plan, plan.stack_plan);
    assert_eq!(
        authoritative_plan.canonical_suspension_crossings,
        plan.canonical_suspension_crossings,
    );

    let mut unrelated_carry = checked.clone();
    let mut unrelated = unrelated_carry
        .facts
        .carry
        .activation_wide_carry
        .iter()
        .find(|fact| fact.machine == target_machine_symbol)
        .expect("target carry envelope")
        .clone();
    unrelated.machine = unrelated_machine;
    unrelated_carry
        .facts
        .carry
        .activation_wide_carry
        .push(unrelated);
    assert_eq!(
        elaborate_task_activation_plans(
            &unrelated_carry,
            &selected,
            NativeTarget::macos_arm64(),
            &[],
        )
        .expect("unrelated carry envelope must not perturb exact target selection"),
        task_activations,
    );

    let mut unrelated_only = checked.clone();
    unrelated_only
        .facts
        .carry
        .activation_wide_carry
        .iter_mut()
        .find(|fact| fact.machine == target_machine_symbol)
        .expect("target carry envelope")
        .machine = unrelated_machine;
    let diagnostics = elaborate_task_activation_plans(
        &unrelated_only,
        &selected,
        NativeTarget::macos_arm64(),
        &[],
    )
    .expect_err("unrelated carry envelope must not satisfy exact target selection");
    assert_eq!(
        diagnostics[0].message,
        "task activation target `Worker::run` has no exact activation-wide CPU/thread carry envelope"
    );
}
