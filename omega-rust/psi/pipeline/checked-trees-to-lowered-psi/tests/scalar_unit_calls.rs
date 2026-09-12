//! Unit calls retain their authored place among scalar computations and control.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_fuel::{FuelChargeSite, TerminalFuelMeter};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
};
use terminal_psi::OperationKind;

#[path = "scalar_unit_calls/custody.rs"]
mod custody;

const BOOLEAN_BRANCH: &str = r#"
machine replace(destination: &mut bool, replacement: bool) {
    destination = replacement;
}
machine observe(initial: bool, replacement: bool) -> u64 {
    let mut scratch: bool = initial;
    replace(&mut scratch, replacement);
    transition scratch { true -> 1 false -> 0 }
}
"#;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|errors| panic!("{errors:#?}\n{source}"))
}

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

fn execute(source: &str, arguments: &[TerminalScalarValue], expected: TerminalScalarValue) {
    let checked = checked(source);
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "observe")
        .produce_artifact()
        .expect("scalar caller retains its ordinary Unit call closure");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile).unwrap();
    assert_eq!(
        terminal_codec::encode_module(&module).unwrap(),
        artifact.semantic_bytes()
    );
    assert_eq!(
        terminal_codec::encode_proof_bundle(&proof).unwrap(),
        artifact.proof_bytes()
    );
    let calls = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::CallUnit { .. }))
        .map(|operation| operation.id)
        .collect::<Vec<_>>();
    assert!(
        !calls.is_empty(),
        "retain an actual Unit call without a scalar result"
    );
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &profile,
        arguments,
    )
    .unwrap();
    let mut meter = TerminalFuelMeter::with_allowance(0);
    for _ in 0..256 {
        match execution.resume(&mut meter).unwrap() {
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, TerminalExecutionResult::Scalar(expected));
                for call in calls {
                    let usage = meter.usage().at(FuelChargeSite::Operation(call)).unwrap();
                    assert_eq!(usage.executions(), 1, "suspension must not replay a call");
                }
                return;
            }
            TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
            status => panic!("unexpected execution status: {status:?}"),
        }
    }
    panic!("scalar caller did not complete");
}

#[test]
fn boolean_branch_reads_local_after_unit_call() {
    for initial in [false, true] {
        for replacement in [false, true] {
            execute(
                BOOLEAN_BRANCH,
                &[
                    TerminalScalarValue::Boolean(initial),
                    TerminalScalarValue::Boolean(replacement),
                ],
                unsigned(u128::from(replacement)),
            );
        }
    }
}

#[test]
fn write_only_unit_call_ends_before_the_callers_fresh_read() {
    let source = BOOLEAN_BRANCH
        .replace("destination: &mut", "destination: &write")
        .replace("replace(&mut scratch", "replace(&write scratch");
    execute(
        &source,
        &[
            TerminalScalarValue::Boolean(false),
            TerminalScalarValue::Boolean(true),
        ],
        unsigned(1),
    );
}

#[test]
fn unit_calls_compose_with_snapshots_assignments_and_scalar_calls() {
    let source = r#"
machine replace(destination: &mut u64, replacement: u64) { destination = replacement; }
machine identity(value: u64) -> u64 { value }
machine observe(initial: u64, replacement: u64) -> u64 {
    let mut scratch: u64 = initial;
    replace(&mut scratch, replacement);
    let snapshot: u64 = scratch;
    scratch = identity(initial);
    replace(&mut scratch, identity(initial));
    snapshot
}
"#;
    execute(source, &[unsigned(7), unsigned(29)], unsigned(29));
    execute(
        &source.replace("    snapshot\n", "    scratch\n"),
        &[unsigned(7), unsigned(29)],
        unsigned(7),
    );
}

#[test]
fn transitive_scalar_caller_retains_its_helpers_unit_calls() {
    let source = BOOLEAN_BRANCH.replace("machine observe(", "machine branch(")
        + r#"
machine observe(initial: bool, replacement: bool) -> u64 { branch(initial, replacement) }
"#;
    execute(
        &source,
        &[
            TerminalScalarValue::Boolean(false),
            TerminalScalarValue::Boolean(true),
        ],
        unsigned(1),
    );
}

#[test]
fn scalar_only_unit_calls_need_no_invented_local_or_scalar_result() {
    execute(
        r#"
machine consume(value: u64) {}
machine observe(value: u64) -> u64 {
    consume(value);
    let snapshot: u64 = value;
    consume(snapshot);
    snapshot
}

"#,
        &[unsigned(19)],
        unsigned(19),
    );
}

#[test]
fn repeated_shared_actuals_retain_distinct_borrow_occurrences() {
    execute(
        r#"
machine inspect(left: &bool, right: &bool) {}
machine observe(initial: bool) -> u64 {
    let mut scratch: bool = initial;
    inspect(&scratch, &scratch);
    transition scratch { true -> 1 false -> 0 }
}
"#,
        &[TerminalScalarValue::Boolean(true)],
        unsigned(1),
    );
}
