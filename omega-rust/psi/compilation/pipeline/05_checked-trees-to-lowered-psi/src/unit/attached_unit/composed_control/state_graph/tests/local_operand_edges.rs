//! A call supplied as an internal state argument whose operand is one of the
//! source state's own `let` locals. Each edge's call reads (a `[copy]` local)
//! or takes (an affine local) that local after the edge's selection; the
//! affine local dies on the sibling edge instead. The source-free artifact
//! must run only the selected call, exactly once, and the independent edge
//! validation must reject a plan whose operand names another local's custody
//! or whose ledger no longer records the edge's move.
use super::{
    CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlTerminatorPlan,
    CheckedUnitEffectOperationPlan, admission,
};
use checked_trees::{
    CheckedStructuralAccess, CheckedStructuralControlSuccessorPlan,
    CheckedStructuralControlTransferSourcePlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralResultBindingPlan,
};
use language_semantics::{Multiplicity, PermissionEventKind, PermissionEventSource};
use semantic_vocabulary::IntegerValue;
use terminal_interpreter::{
    AcceptTerminalEffects, TerminalEffect, TerminalExecutionResult, TerminalScalarValue,
    TerminalStructuralInputs, interpret_terminal_artifact_measured,
};

// `select` picks the edge. Each helper reports its own argument through the
// `Sink` boundary before it returns, so the artifact's effects list exactly
// the calls that ran. `spare` and `other` are second locals of each operand's
// type: both edges dispose of `spare`, and `other` is copyable and unread.
const SOURCE: &str = r#"
    boundary trait Sink { machine finish(value: u32); }
    data Pair [copy] { a: u32 in Wrapping; b: u32 in Wrapping; }
    data Owned { value: u32 in Wrapping; }
    machine shift(pair: Pair, by: u32 in Wrapping) -> Pair reaches Sink {
        Sink::finish(by as u32);
        Pair { a: pair.a + by, b: pair.b + by }
    }
    machine settle(owned: Owned) -> Pair reaches Sink {
        Sink::finish(owned.value as u32);
        Pair { a: owned.value, b: owned.value + 2 }
    }
    data Root {}
    machine Root::enter(select: bool) -> u32 in Wrapping reaches Sink {
        let seed: Pair = Pair { a: 1, b: 2 };
        let owned: Owned = Owned { value: 6 };
        let spare: Owned = Owned { value: 40 };
        let other: Pair = Pair { a: 30, b: 40 };
        transition select {
            true -> take(settle(owned))
            false -> take(shift(seed, 10))
        }
        state take(pair: Pair) -> u32 in Wrapping { pair.a + pair.b }
    }
"#;

fn plan(checked: &checked_trees::CheckedTrees) -> CheckedComposedUnitControlMachinePlan {
    checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| checked.symbols.display_path(plan.machine, "::") == "Root::enter")
        .expect("local operand edges keep the composed graph")
        .clone()
}

/// The entry's local results in establishment order: `seed`, `owned`,
/// `spare`, `other`.
fn local_results(
    plan: &CheckedComposedUnitControlMachinePlan,
) -> [CheckedUnitStructuralResultBindingPlan; 4] {
    plan.states[0]
        .operations
        .iter()
        .filter_map(CheckedUnitEffectOperationPlan::structural_result_binding)
        .cloned()
        .collect::<Vec<_>>()
        .try_into()
        .expect("four locals")
}

fn edges(
    plan: &mut CheckedComposedUnitControlMachinePlan,
) -> (
    &mut CheckedStructuralControlSuccessorPlan,
    &mut CheckedStructuralControlSuccessorPlan,
) {
    let CheckedComposedUnitControlTerminatorPlan::Conditional {
        when_true,
        when_false,
        ..
    } = &mut plan.states[0].terminator
    else {
        panic!("the entry selects between two call-bearing edges");
    };
    (when_true, when_false)
}

/// Rewrite the one structural argument of an edge's operand call that names
/// a source-state local.
fn mutate_local_operand(
    edge: &mut CheckedStructuralControlSuccessorPlan,
    mutate: impl FnOnce(&mut checked_trees::CheckedUnitStructuralArgumentPlan),
) {
    let [transfer] = edge.transfers.as_mut_slice() else {
        panic!("one composite argument per edge");
    };
    let CheckedStructuralControlTransferSourcePlan::EstablishedValue { establishment } =
        &mut transfer.source
    else {
        panic!("the call result is established on its own edge");
    };
    let CheckedUnitEffectOperationPlan::EstablishStructuralValue { calls, .. } =
        establishment.as_mut()
    else {
        panic!("an ordinary structural establishment");
    };
    let [call] = calls.as_mut_slice() else {
        panic!("the edge's one call");
    };
    let mut operation = call.operation().clone();
    let argument = operation
        .call_mut()
        .expect("an ordinary call")
        .structural_arguments
        .iter_mut()
        .find(|argument| {
            argument
                .source_structural_result_binding_ordinal()
                .is_some()
        })
        .expect("the operand names a state local");
    mutate(argument);
    *call = checked_trees::CheckedStructuralValueCall::new(call.value, operation)
        .expect("still an ordinary structural call");
}

/// Run the source-free artifact, after both codec roundtrips and independent
/// verification, returning the result and every value `Sink` observed.
fn run(checked: &checked_trees::CheckedTrees, select: bool) -> (u128, Vec<u128>) {
    let lowered = crate::lower_machine(
        checked,
        crate::TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("local operand edges lower through verified Terminal Psi");
    let semantic_bytes = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof_bytes =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    terminal_verifier::verify_module(
        &terminal_codec::decode_module(&semantic_bytes).unwrap(),
        &terminal_codec::decode_proof_bundle(&proof_bytes).unwrap(),
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the source-free artifact verifies");
    let execution = interpret_terminal_artifact_measured(
        &semantic_bytes,
        &proof_bytes,
        &proof_admission::AdmissionProfile::default(),
        &[TerminalScalarValue::Boolean(select)],
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .expect("the source-free artifact executes");
    let TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
        value: IntegerValue::Unsigned(value),
        ..
    }) = execution.value()
    else {
        panic!("the machine completes with its unsigned scalar result")
    };
    let reported = execution
        .effects()
        .iter()
        .map(|effect| {
            let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                panic!("only the sink reports");
            };
            let [
                TerminalScalarValue::Integer {
                    value: IntegerValue::Unsigned(reported),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("one reported value");
            };
            *reported
        })
        .collect();
    (value, reported)
}

#[test]
fn local_operand_edge_calls_run_once_after_selection() {
    let checked = crate::front_end::checked_program(SOURCE);
    let mut plan = plan(&checked);
    admission::admit(&checked, &plan).expect("source-exact local operand edges");
    let [seed, owned, spare, _] = local_results(&plan);
    assert_eq!(seed.multiplicity, Multiplicity::Unrestricted);
    assert_eq!(owned.multiplicity, Multiplicity::Affine);
    assert!(
        plan.states[0]
            .operations
            .iter()
            .all(|operation| operation.call().is_none()),
        "no edge call is hoisted ahead of the selection"
    );
    let (when_true, when_false) = edges(&mut plan);
    // The `settle` edge moves `owned` into its call; the sibling edge leaves
    // it for disposal. `seed` is only ever copied, and `spare` dies on both.
    assert_eq!(when_true.result_moves(&owned), 1);
    assert_eq!(when_false.result_moves(&owned), 0);
    assert_eq!(when_false.operand_result_moves(seed.binding_ordinal), 1);
    assert_eq!(when_false.result_moves(&seed), 0);
    assert_eq!(when_true.result_moves(&spare), 0);
    assert_eq!(when_false.result_moves(&spare), 0);
    // settle(owned { 6 }) = 6 + 8 and reports 6; shift(seed, 10) = 11 + 12
    // and reports 10. The unselected edge's call reports nothing.
    assert_eq!(run(&checked, true), (14, vec![6]));
    assert_eq!(run(&checked, false), (23, vec![10]));
}

#[test]
fn local_operand_edge_calls_reject_drifted_local_custody() {
    let checked = crate::front_end::checked_program(SOURCE);
    let original = plan(&checked);
    let [_, owned, spare, other] = local_results(&original);
    for drift in [
        "same-typed local",
        "same-typed copy",
        "copied local",
        "borrowed move",
    ] {
        let mut changed = original.clone();
        let (when_true, when_false) = edges(&mut changed);
        match drift {
            // `spare` has the operand's type, but the authored call takes
            // `owned`: the edge would move one local and dispose of neither.
            "same-typed local" => mutate_local_operand(when_true, |argument| {
                argument.source = CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: spare.binding_ordinal,
                };
            }),
            // `other` is a copy of the operand's type, but the authored call
            // reads `seed`: the call would deliver a different value.
            "same-typed copy" => mutate_local_operand(when_false, |argument| {
                argument.source = CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: other.binding_ordinal,
                };
            }),
            // The copy edge cannot read the affine local in place of `seed`.
            "copied local" => mutate_local_operand(when_false, |argument| {
                argument.source = CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                    binding_ordinal: owned.binding_ordinal,
                };
            }),
            // A loan of `owned` moves nothing, so the edge would leak it.
            _ => mutate_local_operand(when_true, |argument| {
                argument.access = CheckedStructuralAccess::SharedBorrow;
            }),
        }
        assert!(
            admission::admit(&checked, &changed).is_err(),
            "{drift} drift must not pass edge validation"
        );
    }
    // The ledger is the authority for the move: without its Transfer row at
    // the operand call, the edge's move has no owner.
    let mut ledger = checked.clone();
    let row = ledger
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find_map(|(handle, event)| {
            (event.kind == PermissionEventKind::Transfer
                && matches!(event.source, PermissionEventSource::Call { call_ordinal, .. }
                    if call_ordinal != 0))
            .then_some(handle)
        })
        .expect("the operand call's transfer row");
    ledger.facts.flow.ownership.permissions.get_mut(row).kind = PermissionEventKind::Consume;
    assert!(
        admission::admit(&ledger, &original).is_err(),
        "a ledger without the operand move must not pass edge validation"
    );
}
