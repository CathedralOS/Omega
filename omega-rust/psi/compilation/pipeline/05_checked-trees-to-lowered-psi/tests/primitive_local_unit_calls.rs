//! Ordinary Unit calls borrow original primitive locals across every scalar width.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionRequest,
    TerminalProductionTimings,
};

#[path = "primitive_local_unit_calls/call_initializers.rs"]
mod call_initializers;
#[path = "primitive_local_unit_calls/crash_routes.rs"]
mod crash_routes;
#[path = "primitive_local_unit_calls/custody.rs"]
mod custody;
#[path = "primitive_local_unit_calls/reference_custody.rs"]
mod reference_custody;
#[path = "primitive_local_unit_calls/reference_initializer.rs"]
mod reference_initializer;

/// Whether `operation` is an ordinary (non-boundary) call that binds no result.
fn is_ordinary_unit_call(operation: &checked_trees::CheckedUnitEffectOperationPlan) -> bool {
    operation.call().is_some_and(|call| {
        !call.is_boundary() && *call.result() == checked_trees::CheckedUnitCallResult::Unit
    })
}

/// The ordinary Unit call `operation` must be.
fn ordinary_unit_call_mut(
    operation: &mut checked_trees::CheckedUnitEffectOperationPlan,
) -> &mut checked_trees::CheckedUnitCallPlan {
    assert!(is_ordinary_unit_call(operation), "Unit call");
    operation.call_mut().expect("Unit call")
}

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

fn artifact(source: &str) -> terminal_codec::CanonicalTerminalArtifact {
    let artifact = terminal_production::TerminalProductionRequest::new(
        &crate::front_end::checked_program(source),
        TerminalMachineSelection::Name("observe"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap_or_else(|error| panic!("{source}: {error:?}"))
    .into_artifact();
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
fn deferred_primitive_locals_keep_storage_through_calls_and_reads() {
    for (scalar, sign, width) in [
        ("i8", IntegerSign::Signed, 8),
        ("i16", IntegerSign::Signed, 16),
        ("i32", IntegerSign::Signed, 32),
        ("i64", IntegerSign::Signed, 64),
        ("u8", IntegerSign::Unsigned, 8),
        ("u16", IntegerSign::Unsigned, 16),
        ("u32", IntegerSign::Unsigned, 32),
        ("u64", IntegerSign::Unsigned, 64),
    ] {
        let value = |number| {
            if sign == IntegerSign::Signed {
                IntegerValue::Signed(number)
            } else {
                IntegerValue::Unsigned(number as u128)
            }
        };
        let initial = integer(sign, width, value(11));
        let replacement = integer(sign, width, value(42));
        let source = source(scalar).replace(" = initial", "");
        let artifact = artifact(&source);
        execute(&artifact, initial, replacement, &[initial, replacement]);
    }
}

#[test]
fn deferred_primitive_zero_is_replayed_from_its_declaration() {
    use checked_trees::{
        CheckedScalarExpression, CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan,
    };
    let source = source("i32").replace(" = initial", "");
    let mut checked = crate::front_end::checked_program(&source);
    let _ = artifact(&source);
    let initializer = checked
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter_mut()
        .find(|expression| expression.role == CheckedScalarExpressionRole::StorageInitializer)
        .expect("implicit zero initializer");
    let CheckedScalarExpression::IntegerLiteral { literal } = &initializer.expression else {
        panic!("integer zero");
    };
    let one = CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_parts(
            false,
            numerics::literals::IntegerRadix::Decimal,
            "1",
        )
        .unwrap()
        .with_landing(literal.landing().unwrap()),
    };
    initializer.expression = one.clone();
    for plan in &mut checked.facts.flow.terminal_unit_effects.machines {
        for operation in &mut plan.operations {
            if let CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { value, .. } = operation
            {
                *value = checked_trees::CheckedCallScalarArgument::Pure(one.clone());
            }
        }
    }
    let result =
        TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("observe"))
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default(),
            ));
    assert!(
        result.is_err(),
        "coherent cached one cannot replace the declaration's zero"
    );
}

#[test]
fn deferred_scalar_binding_still_requires_mutable_assignment() {
    let diagnostics = crate::front_end::checked_program_result(
        "machine observe(output: &mut i32) { let scratch: i32; scratch = 1; output = scratch; }",
    )
    .expect_err("a bare non-mut local cannot be assigned");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("not mutable")),
        "{diagnostics:#?}"
    );
}

#[test]
fn deferred_scalar_result_locals_have_distinct_zeroed_storage() {
    let source = r#"
        machine compute() -> i32 {
            let mut first: i32;
            let mut second: i32;
            let before: i32 = second;
            first = 42;
            second = first;
            first = before;
            second
        }
        machine observe(output: &mut i32, initial: i32, replacement: i32) {
            output = compute();
        }
    "#;
    let initial = integer(IntegerSign::Signed, 32, IntegerValue::Signed(11));
    let replacement = integer(IntegerSign::Signed, 32, IntegerValue::Signed(42));
    execute(
        &artifact(source),
        initial,
        replacement,
        &[initial, replacement],
    );
}

#[test]
fn deferred_boolean_and_float_locals_keep_their_declared_carriers() {
    use semantic_vocabulary::IeeeFloatValue;
    for (scalar, initial, replacement) in [
        (
            "bool",
            TerminalScalarValue::Boolean(false),
            TerminalScalarValue::Boolean(true),
        ),
        (
            "f32",
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(11.0f32.to_bits())),
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary32(42.0f32.to_bits())),
        ),
        (
            "f64",
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(11.0f64.to_bits())),
            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(42.0f64.to_bits())),
        ),
    ] {
        let artifact = artifact(&source(scalar).replace(" = initial", ""));
        execute(&artifact, initial, replacement, &[initial, replacement]);
    }
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
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[initial, replacement],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 71,
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            primitive_values: &[TerminalStructuralPrimitiveValue {
                argument_index: 0,
                value: initial,
            }],
            ..Default::default()
        },
    )
    .unwrap();
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    let mut observations = Vec::new();
    let mut complete = false;
    for _ in 0..64 {
        let status = execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap();
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
