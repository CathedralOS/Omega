//! Suspension call-plan retention across the admitted call flavors.
//! In-module callees join a crossing by owning-machine identity whether the
//! recorded occurrence spells the callee machine or its resolved entry state;
//! Unit and structural-result operations retain the crossing beside scalar
//! calls. Boundary and dynamic callees still fail closed — their declaration
//! symbols are not machine members, so `canonical_suspension_crossing_id`
//! cannot resolve them — and `CallUnit` arguments are not an admitted scalar
//! CallArgument namespace.
use super::{checked_scalar_suspension_fixture, checked_source};
use crate::TerminalMachineSelection;
use crate::lower_machine;
use crate::lowering_error::LoweringError;
use checked_trees::{
    SuspensionCrossingCarryFact, SuspensionCrossingLiveValueFact, SuspensionCrossingStorage,
    SuspensionCrossingValueOrigin,
};
use language_semantics::CarryPolicy;
use symbols::SymbolHandle;
use terminal_psi::TerminalSuspensionCallTarget;

const FREE_UNIT_CALL: &str = r#"
    machine sink(flag: u64) {}
    machine run(flag: u64) { sink(flag); }
"#;

const STRUCTURAL_SCALAR_CALL: &str = r#"
    machine bump(value: &mut u64, delta: u64) -> u64 {
        let before: u64 = value;
        value = delta;
        before
    }
    machine run(acc: &mut u64, base: u64) -> u64 {
        let before: u64 = bump(acc, base);
        before
    }
"#;

const BOUNDARY_SCALAR_CALL: &str = r#"
    boundary trait Host { machine mark(value: u64) reaches Host; }
    machine run(v: u64) reaches Host { Host::mark(v); }
"#;

fn machine_and_state(
    checked: &checked_trees::CheckedTrees,
    machine_name: &str,
) -> (SymbolHandle, SymbolHandle) {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .expect("authored machine");
    let state = checked
        .machine_states(machine)
        .first()
        .expect("machine entry state");
    (machine.symbol, state.symbol)
}

/// The checked call coordinate's `target_symbol` — the resolved callee entry
/// state — is what `check_suspension_carry` records on a real crossing.
fn call_target_symbol(
    checked: &checked_trees::CheckedTrees,
    statement_index: usize,
    call_ordinal: usize,
) -> SymbolHandle {
    checked
        .facts
        .flow
        .control
        .calls
        .iter()
        .find(|(_, call)| {
            call.statement_index == statement_index && call.call_ordinal == call_ordinal
        })
        .map(|(_, call)| call.target_symbol)
        .expect("checked call fact")
}

fn state_parameter(
    checked: &checked_trees::CheckedTrees,
    state_symbol: SymbolHandle,
    parameter_name: &str,
) -> (SymbolHandle, typed_trees::types::TypeReferenceHandle) {
    let state = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .find(|state| state.symbol == state_symbol)
        .expect("state");
    let parameter = checked
        .state_parameters(state)
        .iter()
        .find(|parameter| checked.typed.symbols.name(parameter.symbol) == parameter_name)
        .expect("state parameter");
    (parameter.symbol, parameter.type_reference)
}

fn push_crossing(
    checked: &mut checked_trees::CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
    target: SymbolHandle,
    live_values: Vec<SuspensionCrossingLiveValueFact>,
) {
    checked
        .facts
        .carry
        .suspension_crossings
        .push(SuspensionCrossingCarryFact {
            machine,
            state,
            statement_index,
            call_ordinal,
            target,
            receiver: None,
            effective: CarryPolicy::PERMISSIVE,
            live_values,
        });
}

fn expect_single_verified_plan(
    lowered: &lowered_psi::LoweredPsi,
) -> &terminal_psi::TerminalSuspensionCallPlan {
    let [site] = lowered.semantic_module.suspension_call_sites.as_slice() else {
        panic!("one exact suspension call site")
    };
    let [plan] = lowered.semantic_module.suspension_call_plans.as_slice() else {
        panic!("one exact suspension call plan")
    };
    assert_eq!(site.operation, plan.operation);
    assert_eq!(site.crossing, plan.crossing);
    assert_eq!(
        site.frontier_commitment,
        terminal_psi::suspension_frontier_commitment(plan)
    );
    let operation = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| machine.blocks.iter())
        .flat_map(|block| block.operations.iter())
        .find(|operation| operation.id == plan.operation)
        .expect("planned operation");
    assert_eq!(operation.suspension_crossing, Some(plan.crossing));
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("exact suspension plan verifies");
    let bytes =
        terminal_codec::encode_module(&lowered.semantic_module).expect("suspension plan encodes");
    assert_eq!(
        terminal_codec::decode_module(&bytes).expect("suspension plan decodes"),
        lowered.semantic_module
    );
    plan
}

/// The checked crossing's `target` is the callee's resolved entry state while
/// scalar-result emissions record the callee machine; both name the same
/// callable and must join by owning-machine identity.
#[test]
fn scalar_call_suspension_plan_rejoins_callee_entry_state_target() {
    let mut checked = checked_scalar_suspension_fixture();
    let (root, state) = machine_and_state(&checked, "root");
    let target = call_target_symbol(&checked, 1, 0);
    push_crossing(&mut checked, root, state, 1, 0, target, Vec::new());
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("root"))
        .expect("entry-state target joins");
    let plan = expect_single_verified_plan(&lowered);
    assert!(matches!(
        plan.target,
        TerminalSuspensionCallTarget::Machine(_)
    ));
}

/// A receiver-free Unit call retains its crossing: the plan joins the exact
/// Terminal `CallUnit` operation and names its machine target.
#[test]
fn unit_call_suspension_plan_rejoins_receiver_free_unit_target() {
    let mut checked = checked_source(FREE_UNIT_CALL);
    let (run, state) = machine_and_state(&checked, "run");
    let target = call_target_symbol(&checked, 0, 0);
    push_crossing(&mut checked, run, state, 0, 0, target, Vec::new());
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("run"))
        .expect("Unit call crossing retains");
    let plan = expect_single_verified_plan(&lowered);
    let operation = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| machine.blocks.iter())
        .flat_map(|block| block.operations.iter())
        .find(|operation| operation.id == plan.operation)
        .expect("planned operation");
    let terminal_psi::OperationKind::CallUnit { callee, .. } = &operation.kind else {
        panic!("the suspension plan binds the Unit call operation")
    };
    assert_eq!(plan.target, TerminalSuspensionCallTarget::Machine(*callee));
}

/// A structural-result call with retained scalar arguments admits a scalar
/// CallArgument live value — the verifier's `scalar_call_arguments` roster —
/// beside its machine target.
#[test]
fn structural_scalar_call_suspension_plan_rejoins_argument_frontier() {
    let mut checked = checked_source(STRUCTURAL_SCALAR_CALL);
    let (run, state) = machine_and_state(&checked, "run");
    let (_, base_type) = state_parameter(&checked, state, "base");
    let target = call_target_symbol(&checked, 0, 0);
    push_crossing(
        &mut checked,
        run,
        state,
        0,
        0,
        target,
        vec![SuspensionCrossingLiveValueFact {
            type_reference: base_type,
            storage: SuspensionCrossingStorage::CallArgument,
            origin: SuspensionCrossingValueOrigin::CallArgument { position: 0 },
            claims: Vec::new(),
            effective: CarryPolicy::PERMISSIVE,
        }],
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("run"))
        .expect("scalar call-argument frontier joins");
    let plan = expect_single_verified_plan(&lowered);
    let [live] = plan.live_values.as_slice() else {
        panic!("one exact suspension live value")
    };
    assert_eq!(
        live.storage,
        terminal_psi::TerminalSuspensionStorage::CallArgument
    );
    assert!(matches!(
        live.place,
        terminal_psi::TerminalSuspensionPlace::Scalar(_)
    ));
    assert_eq!(live.claim_count, 0);
    assert!(live.claims.is_empty());
}

/// `CallUnit` carries scalar arguments but the verifier does not admit them as
/// a CallArgument namespace, so the producer must refuse rather than emit a
/// plan that cannot verify.
#[test]
fn unit_call_scalar_argument_frontier_fails_closed() {
    let mut checked = checked_source(FREE_UNIT_CALL);
    let (run, state) = machine_and_state(&checked, "run");
    let (_, flag_type) = state_parameter(&checked, state, "flag");
    let target = call_target_symbol(&checked, 0, 0);
    push_crossing(
        &mut checked,
        run,
        state,
        0,
        0,
        target,
        vec![SuspensionCrossingLiveValueFact {
            type_reference: flag_type,
            storage: SuspensionCrossingStorage::CallArgument,
            origin: SuspensionCrossingValueOrigin::CallArgument { position: 0 },
            claims: Vec::new(),
            effective: CarryPolicy::PERMISSIVE,
        }],
    );
    assert!(
        matches!(
            lower_machine(&checked, TerminalMachineSelection::Name("run")),
            Err(LoweringError::Unsupported(reason))
                if reason.contains("call argument position is unavailable")
        ),
        "a CallUnit argument frontier fails closed"
    );
}

/// Unit-call emissions do not record the caller's scalar environment, so a
/// Parameter live value still lacks an exact frontier mapping.
#[test]
fn unit_call_scalar_environment_frontier_fails_closed() {
    let mut checked = checked_source(FREE_UNIT_CALL);
    let (run, state) = machine_and_state(&checked, "run");
    let (flag_symbol, flag_type) = state_parameter(&checked, state, "flag");
    let target = call_target_symbol(&checked, 0, 0);
    push_crossing(
        &mut checked,
        run,
        state,
        0,
        0,
        target,
        vec![SuspensionCrossingLiveValueFact {
            type_reference: flag_type,
            storage: SuspensionCrossingStorage::Parameter,
            origin: SuspensionCrossingValueOrigin::Parameter {
                symbol: flag_symbol,
                position: 0,
            },
            claims: Vec::new(),
            effective: CarryPolicy::PERMISSIVE,
        }],
    );
    assert!(
        matches!(
            lower_machine(&checked, TerminalMachineSelection::Name("run")),
            Err(LoweringError::Unsupported(reason))
                if reason.contains("scalar environment position is unavailable")
        ),
        "a Unit call parameter frontier fails closed"
    );
}

/// Boundary machines are not members of `program.machines()`, so a crossing
/// aimed at one cannot resolve a canonical identity — the boundary frontier
/// stays fenced until checked-trees' `symbol_identity` covers those
/// declaration symbols.
#[test]
fn boundary_call_suspension_frontier_fails_closed_on_target_identity() {
    let mut checked = checked_source(BOUNDARY_SCALAR_CALL);
    let (run, state) = machine_and_state(&checked, "run");
    let target = call_target_symbol(&checked, 0, 0);
    push_crossing(&mut checked, run, state, 0, 0, target, Vec::new());
    assert!(
        matches!(
            lower_machine(&checked, TerminalMachineSelection::Name("run")),
            Err(LoweringError::Unsupported(reason))
                if reason.contains("cannot resolve its source symbols")
        ),
        "a boundary call crossing fails closed at identity resolution"
    );
}
