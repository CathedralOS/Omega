use checked_trees::CheckedUnitEffectOperationPlan;
use checked_trees_to_lowered_psi::lower_machine;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, StructuralPlaceKind};
use terminal_codec::{decode_module, decode_proof_bundle, encode_module, encode_proof_bundle};
use terminal_fuel::{FuelChargeSite, TerminalFuelMeter};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralValue,
};
use terminal_psi::{OperationKind, OperationResult, Terminator};

#[path = "affine_result_calls/uses.rs"]
mod uses;

#[path = "affine_result_calls/chains.rs"]
mod chains;

#[path = "affine_result_calls/chain_custody.rs"]
mod chain_custody;

#[path = "affine_result_calls/nested.rs"]
mod nested;

#[path = "affine_result_calls/argument_order.rs"]
mod argument_order;

#[path = "affine_result_calls/argument_custody.rs"]
mod argument_custody;

const IDENTITY_CALL: &str = "data Value { number: u64; }
    machine forward(value: Value) -> Value { value }
    machine Main::caller(value: Value) { let result: Value = forward(value); }";

fn checked(source: &str) -> checked_trees::CheckedTrees {
    typed_trees_to_checked_trees::lower_typed_trees(typed(source)).expect("check")
}

fn typed(source: &str) -> typed_trees::TypedTrees {
    let source = format!("data Main {{}} machine Main::run() {{}} {source}");
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type")
}

fn assert_call_execution(source: &str, name: &str, scalar_arguments: &[TerminalScalarValue]) {
    let checked = checked(source);
    let lowered = lower_machine(&checked, name).expect("caller lowers");
    let semantic = encode_module(&lowered.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("encode proof");
    let module = decode_module(&semantic).expect("decode semantics");
    let proof_bundle = decode_proof_bundle(&proof).expect("decode proof");
    assert_eq!(module, lowered.semantic_module);
    terminal_verifier::verify_module(&module, &proof_bundle, &AdmissionProfile::default())
        .expect("independent verification after codec roundtrip");
    let [caller, callee] = module.machines.as_slice() else {
        panic!("one caller and one actual ordinary structural producer");
    };
    assert_eq!(module.entry, caller.id);
    let calls = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::CallStructuralWithScalarArguments { .. }
            )
        })
        .collect::<Vec<_>>();
    let [operation] = calls.as_slice() else {
        panic!("the initializer invokes its producer exactly once");
    };
    let OperationKind::CallStructuralWithScalarArguments {
        callee: target,
        structural_arguments,
        ..
    } = &operation.kind
    else {
        unreachable!()
    };
    assert_eq!(*target, callee.id);
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(
        structural_arguments[0].place,
        caller.structural_parameters[0].place
    );
    let OperationResult::Structural(result) = &operation.result else {
        panic!("the call establishes a structural result");
    };
    assert_ne!(result.place, caller.structural_parameters[0].place);
    assert_eq!(
        result.structural_type,
        callee.result.structural().unwrap().structural_type
    );
    assert!(caller.structural_places.iter().any(|place| {
        place.id == result.place
            && matches!(place.kind,
            StructuralPlaceKind::OperationResult { producer, structural_type }
            if producer == operation.id && structural_type == result.structural_type)
    }));
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &caller.blocks.last().unwrap().terminator
    else {
        panic!("caller returns Unit");
    };
    assert_eq!(trivial_affine_discards, &[result.place]);
    let [receipt] = lowered.source_call_occurrences.as_slice() else {
        panic!("one exact source call receipt");
    };
    assert_eq!(receipt.terminal_operation, operation.id);
    assert!(receipt.source_site.is_some());
    assert_eq!(receipt.statement_index, 0);
    assert_eq!(receipt.call_ordinal, 0);
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        scalar_arguments,
        &[TerminalStructuralValue {
            opaque_identity: 0xaff1,
            structural_type: caller.structural_parameters[0].structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        }],
    )
    .expect("verified caller starts");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    let mut completed = false;
    for _ in 0..32 {
        let status = execution.resume(&mut meter).unwrap();
        match status {
            TerminalExecutionStatus::SponsorExhausted(_) => {
                let frontier = execution
                    .live_affine_frontier()
                    .cloned()
                    .collect::<Vec<_>>();
                let units = meter.usage().total_units();
                assert_eq!(execution.resume(&mut meter).unwrap(), status);
                assert_eq!(meter.usage().total_units(), units);
                assert_eq!(
                    execution
                        .live_affine_frontier()
                        .cloned()
                        .collect::<Vec<_>>(),
                    frontier
                );
                meter.replenish(1).unwrap();
            }
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, TerminalExecutionResult::Unit);
                completed = true;
                break;
            }
            TerminalExecutionStatus::Crashed(crash) => panic!("identity caller crashed: {crash:?}"),
        }
    }
    assert!(
        completed,
        "the bounded caller finishes within 32 fuel units"
    );
    assert!(execution.live_affine_frontier().next().is_none());
    assert!(execution.live_claim_frontier().next().is_none());
    assert!(execution.effects().is_empty());
    assert_eq!(
        meter
            .usage()
            .at(FuelChargeSite::Operation(operation.id))
            .unwrap()
            .units(),
        1
    );
}

#[test]
fn a_local_affine_call_result_has_a_real_producer_and_cleanup() {
    assert_call_execution(IDENTITY_CALL, "Main::caller", &[]);
}

#[test]
fn ordinary_structural_initializers_retain_nested_and_generic_owned_types() {
    for (declarations, identity) in [
        (
            "data Inner { number: u64; } data Outer { inner: Inner; count: u32; }",
            "Outer",
        ),
        ("data Entry { number: u64; }", "[Entry; 3]"),
        (
            "data Entry { number: u64; } data Buffer<T> { entries: [T; 3]; }",
            "Buffer<Entry>",
        ),
        (
            "data Entry { number: u64; } data Maybe<T> { case None; case Some(value: T); }",
            "Maybe<Entry>",
        ),
    ] {
        let source = format!(
            "{declarations}
            machine forward(value: {identity}) -> {identity} {{ value }}
            machine Main::caller(value: {identity}) {{ let result: {identity} = forward(value); }}"
        );
        assert_call_execution(&source, "Main::caller", &[]);
    }
}

#[test]
fn free_callers_and_static_attached_producers_share_the_same_call_path() {
    assert_call_execution(
        "data Value { number: u64; }
        machine Main::forward(value: Value) -> Value { value }
        machine caller(value: Value) { let result: Value = Main::forward(value); }",
        "caller",
        &[],
    );
}

#[test]
fn structural_initializers_preserve_mixed_authored_parameter_positions() {
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    assert_call_execution(
        "data Value { number: u64; }
        machine forward(before: u64, value: Value, after: u64) -> Value { value }
        machine Main::caller(number: u64, value: Value) {
            let result: Value = forward(number, value, 9);
        }",
        "Main::caller",
        &[TerminalScalarValue::Integer {
            scalar_type: unsigned,
            value: IntegerValue::Unsigned(7),
        }],
    );
}

fn structural_call_mut(
    checked: &mut checked_trees::CheckedTrees,
) -> &mut CheckedUnitEffectOperationPlan {
    checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.operations)
        .find(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::StructuralCall { .. }
            )
        })
        .expect("ordinary checked structural call plan")
}

#[test]
fn ordinary_result_calls_reject_checked_custody_drift() {
    for corruption in 0..4 {
        let mut checked = checked(IDENTITY_CALL);
        let CheckedUnitEffectOperationPlan::StructuralCall {
            target_contract_report_fingerprint,
            result,
            discard_result_on_return,
            ..
        } = structural_call_mut(&mut checked)
        else {
            unreachable!()
        };
        match corruption {
            0 => *target_contract_report_fingerprint ^= 1,
            1 => result.type_identity.push_str("-missing"),
            2 => result.binding_ordinal += 1,
            3 => *discard_result_on_return = false,
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&checked, "Main::caller").is_err(),
            "corruption {corruption}"
        );
    }
}

#[test]
fn a_transitive_unit_caller_retains_the_structural_producer_in_one_catalog() {
    let checked = checked(
        "data Value { number: u64; }
        data Unused { number: u32; }
        machine forward(value: Value) -> Value { value }
        machine unused(value: Unused) -> Unused { value }
        machine Main::receive(value: Value) { let result: Value = forward(value); }
        machine Main::caller(value: Value) { Main::receive(value); }",
    );
    let lowered = lower_machine(&checked, "Main::caller").expect("transitive Unit closure");
    let module = &lowered.semantic_module;
    assert_eq!(module.machines.len(), 3);
    assert_eq!(lowered.source_call_occurrences.len(), 2);
    assert!(
        !module
            .structural_types
            .iter()
            .any(|declaration| declaration.identity.contains("Unused"))
    );
    terminal_verifier::verify_module(module, &lowered.proof_bundle, &AdmissionProfile::default())
        .expect("shared type and machine identities verify");
    let calls = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    assert_eq!(
        calls
            .iter()
            .filter(|operation| matches!(operation.kind, OperationKind::CallUnit { .. }))
            .count(),
        1
    );
    assert_eq!(
        calls
            .iter()
            .filter(|operation| matches!(
                operation.kind,
                OperationKind::CallStructuralWithScalarArguments { .. }
            ))
            .count(),
        1
    );
}

#[test]
fn a_result_is_disposed_before_an_unused_entry_parameter() {
    let checked = checked(
        "data Value { number: u64; }
        machine forward(value: Value) -> Value { value }
        machine caller(value: Value, unused: Value) { let result: Value = forward(value); }",
    );
    let lowered = lower_machine(&checked, "caller").expect("caller with an unused owned input");
    let caller = &lowered.semantic_module.machines[0];
    let operation = &caller.blocks[0].operations[0];
    let OperationResult::Structural(result) = &operation.result else {
        panic!("structural result")
    };
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &caller.blocks[0].terminator
    else {
        panic!("Unit return")
    };
    assert_eq!(
        trivial_affine_discards,
        &[result.place, caller.structural_parameters[1].place]
    );
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("reverse declaration order cleanup verifies");
}

#[test]
fn root_and_transitive_calls_share_one_structural_producer() {
    let checked = checked(
        "data Value { number: u64; }
        machine forward(value: Value) -> Value { value }
        machine Main::receive(value: Value) { let result: Value = forward(value); }
        machine caller(first: Value, second: Value) {
            let result: Value = forward(first);
            Main::receive(second);
        }",
    );
    let lowered = lower_machine(&checked, "caller").expect("shared ordinary result producer");
    assert_eq!(lowered.semantic_module.machines.len(), 3);
    let targets = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::CallStructuralWithScalarArguments { callee, .. } => Some(callee),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 2);
    assert_eq!(targets[0], targets[1]);
    assert_eq!(lowered.source_call_occurrences.len(), 3);
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("shared producer retains exact type and machine identities");
}

#[test]
fn a_structural_initializer_evaluates_its_scalar_expression() {
    assert_call_execution(
        "data Value { number: u64; }
        machine forward(number: u64, value: Value) -> Value { value }
        machine Main::caller(value: Value) { let result: Value = forward(3 + 4, value); }",
        "Main::caller",
        &[],
    );
}

#[test]
fn constructed_result_calls_require_an_executable_structural_producer() {
    let checked = checked(
        "data Packet { flag: bool; }
         machine identity(input: bool) -> bool { input }
         machine packet(input: bool) -> Packet { Packet { flag: input } }
         machine value(input: bool) {
             let saved: Packet = packet(identity(input));
         }",
    );
    assert!(
        lower_machine(&checked, "value").is_err(),
        "source-family eligibility cannot manufacture a structural result producer"
    );
}
