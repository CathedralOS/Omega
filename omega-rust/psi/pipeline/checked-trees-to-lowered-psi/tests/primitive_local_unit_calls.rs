//! Ordinary Unit calls borrow original primitive locals across every scalar width.

use semantic_vocabulary::{IeeeFloatValue, IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use terminal_psi::{OperationKind, StructuralAccess};

#[path = "primitive_local_unit_calls/crash_routes.rs"]
mod crash_routes;
#[path = "primitive_local_unit_calls/custody.rs"]
mod custody;
#[path = "primitive_local_unit_calls/reference_custody.rs"]
mod reference_custody;
#[path = "primitive_local_unit_calls/reference_initializer.rs"]
mod reference_initializer;

fn source(scalar: &str) -> String {
    format!(
        "machine replace(destination: &mut {scalar}, value: {scalar}) {{
            destination = value;
        }}
        machine observe(output: &mut {scalar}, initial: {scalar}, replacement: {scalar}) {{
            let mut scratch: {scalar} = initial;
            replace(&mut scratch, replacement);
            output = scratch;
        }}"
    )
}

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check")
}

fn artifact(source: &str) -> terminal_codec::CanonicalTerminalArtifact {
    let artifact = terminal_production::TerminalProductionRequest::new(&checked(source), "observe")
        .produce_artifact()
        .unwrap_or_else(|error| panic!("{source}: {error:?}"));
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).expect("reload module");
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).expect("reload proof");
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independently verify reloaded artifact");
    artifact
}

fn integer(sign: IntegerSign, width: u16, value: IntegerValue) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(sign, width).unwrap(),
        value,
    }
}

#[test]
fn all_integer_widths_preserve_local_unit_calls_and_current_reads() {
    for width in [8, 16, 32, 64] {
        for sign in [IntegerSign::Unsigned, IntegerSign::Signed] {
            let (name, initial, replacement) = match sign {
                IntegerSign::Unsigned => (
                    format!("u{width}"),
                    IntegerValue::Unsigned(1),
                    IntegerValue::Unsigned((1u128 << width) - 1),
                ),
                IntegerSign::Signed => (
                    format!("i{width}"),
                    IntegerValue::Signed(1),
                    IntegerValue::Signed(-(1i128 << (width - 1))),
                ),
            };
            let initial = integer(sign, width, initial);
            let replacement = integer(sign, width, replacement);
            let artifact = artifact(&source(&name));
            assert_local_call(&artifact);
            execute(&artifact, initial, replacement, &[initial, replacement]);
        }
    }
}

#[test]
fn boolean_local_unit_call_preserves_current_read() {
    let artifact = artifact(&source("bool"));
    assert_local_call(&artifact);
    let initial = TerminalScalarValue::Boolean(false);
    let replacement = TerminalScalarValue::Boolean(true);
    execute(&artifact, initial, replacement, &[initial, replacement]);
}

#[test]
fn ieee_local_unit_calls_preserve_exact_payload_bits() {
    for (name, initial, replacement) in [
        (
            "f32",
            IeeeFloatValue::Binary32(0x8000_0000),
            IeeeFloatValue::Binary32(0x7fc0_0042),
        ),
        (
            "f64",
            IeeeFloatValue::Binary64(0x8000_0000_0000_0000),
            IeeeFloatValue::Binary64(0x7ff8_0000_0000_0042),
        ),
    ] {
        let artifact = artifact(&source(name));
        assert_local_call(&artifact);
        let initial = TerminalScalarValue::IeeeFloat(initial);
        let replacement = TerminalScalarValue::IeeeFloat(replacement);
        execute(&artifact, initial, replacement, &[initial, replacement]);
    }
}

fn assert_local_call(artifact: &terminal_codec::CanonicalTerminalArtifact) {
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert_eq!(module.machines.len(), 2);
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert_eq!(caller.parameters.len(), 2);
    assert_eq!(
        caller.structural_parameters.len(),
        1,
        "scratch is not a parameter"
    );
    let operations = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let [establish, call, read, store] = operations.as_slice() else {
        panic!("exact establishment/call/read/store sequence: {operations:?}");
    };
    assert!(
        matches!(establish.kind, OperationKind::EstablishPrimitiveLocal { value }
        if value == caller.parameters[0].id)
    );
    let local = establish.result.structural().unwrap().place;
    assert_ne!(local, caller.structural_parameters[0].place);
    let OperationKind::CallUnit {
        callee,
        arguments,
        structural_arguments,
        ..
    } = &call.kind
    else {
        panic!("ordinary Unit call");
    };
    assert_eq!(call.result, terminal_psi::OperationResult::Unit);
    assert_eq!(arguments, &[caller.parameters[1].id]);
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].place, local);
    assert!(structural_arguments[0].path.is_empty());
    assert_eq!(
        structural_arguments[0].access,
        StructuralAccess::MutableBorrow
    );
    assert!(matches!(read.kind, OperationKind::PrimitiveScalarRead { source } if source == local));
    assert!(
        matches!(store.kind, OperationKind::WriteOnlyPrimitiveStore { destination, value }
        if destination == caller.structural_parameters[0].place
            && Some(value) == read.result.scalar().map(|result| result.id))
    );
    let callee = module
        .machines
        .iter()
        .find(|machine| machine.id == *callee)
        .unwrap();
    assert!(callee.blocks.iter().flat_map(|block| &block.operations).any(|operation|
        matches!(operation.kind, OperationKind::WriteOnlyPrimitiveStore { destination, value }
            if destination == callee.structural_parameters[0].place && value == callee.parameters[0].id)));
}

#[test]
fn local_store_rhs_and_snapshots_keep_order_across_unit_call() {
    let source = source("u64")
        .replace("replace(&mut scratch, replacement);", "let snapshot: u64 = scratch; scratch = scratch ^ replacement; output = scratch; replace(&mut scratch, replacement);")
        .replace("output = scratch;\n        }", "output = scratch; output = snapshot;\n        }");
    let artifact = artifact(&source);
    let initial = integer(IntegerSign::Unsigned, 64, IntegerValue::Unsigned(9));
    let replacement = integer(IntegerSign::Unsigned, 64, IntegerValue::Unsigned(37));
    let computed = integer(IntegerSign::Unsigned, 64, IntegerValue::Unsigned(44));
    execute(
        &artifact,
        initial,
        replacement,
        &[initial, computed, replacement, initial],
    );
}

fn execute(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    initial: TerminalScalarValue,
    replacement: TerminalScalarValue,
    expected: &[TerminalScalarValue],
) {
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let parameter = &caller.structural_parameters[0];
    let mut execution =
        TerminalExecution::start_artifact_with_structural_arguments_and_primitive_values(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[initial, replacement],
            &[TerminalStructuralValue {
                opaque_identity: 71,
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            &[TerminalStructuralPrimitiveValue {
                argument_index: 0,
                value: initial,
            }],
        )
        .unwrap();
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    let mut observations = Vec::new();
    let mut complete = false;
    for _ in 0..64 {
        let status = execution.resume(&mut meter).unwrap();
        observations.push(execution.structural_primitive_values()[0].value);
        match status {
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, TerminalExecutionResult::Unit);
                complete = true;
                break;
            }
            TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
            other => panic!("unexpected status {other:?}"),
        }
    }
    assert!(complete);
    observations.dedup();
    assert_eq!(observations, expected);
    for operation in module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
    {
        let usage = meter
            .usage()
            .at(terminal_fuel::FuelChargeSite::Operation(operation.id))
            .unwrap();
        assert_eq!(
            usage.executions(),
            1,
            "operation {:?} must not replay",
            operation.id
        );
        assert_eq!(usage.units(), 1);
    }
}
