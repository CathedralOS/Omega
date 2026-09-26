//! ARITHMETIC-POLICY-REALIZATION: `in Trapping` arithmetic and conversions
//! lower to one `TrappingInteger` Terminal operation that is its own `Trap`
//! crash site. These execute the serialized module, so the interpreter's own
//! decode and verification stand between the lowered spelling and the answer:
//! an in-range operation returns the exact mathematical value, an
//! out-of-range one crashes at that operation with cause `Trap`, and neither
//! is ever a wrapped or saturated substitute.
//!
//! The rejection controls mutate the lowered module and check that the
//! verifier, the observation profile, and the trace builder each refuse a
//! relocated site, a changed cause, a changed operand carrier, or a missing
//! same-cause ceiling.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, OperationId};
use terminal_codec::{
    TerminalTraceV1ProfileAcceptanceError, accept_terminal_trace_v1_profile, encode_module,
    encode_proof_section, encode_terminal_trace_v1_profile,
    reconstruct_canonical_terminal_trace_v1_profile,
};
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalCrashSite, TerminalExecutionResult,
    TerminalInterpretError, TerminalScalarValue, interpret_terminal_artifact,
};
use terminal_psi::{
    CrashCause, CrashRouteGuard, OperationKind, TerminalModule, TerminalTraceV1ConstructionError,
    TrappingIntegerOperation, TrappingIntegerPrimitive,
};

fn integer(sign: IntegerSign, bits: u16, value: IntegerValue) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(sign, bits).expect("a fixed integer carrier"),
        value,
    }
}

fn unsigned(bits: u16, value: u128) -> TerminalScalarValue {
    integer(IntegerSign::Unsigned, bits, IntegerValue::Unsigned(value))
}

fn signed(bits: u16, value: i128) -> TerminalScalarValue {
    integer(IntegerSign::Signed, bits, IntegerValue::Signed(value))
}

fn lower(source: &str) -> lowered_psi::LoweredPsi {
    let checked = crate::front_end::checked_program(source);
    checked_trees_to_lowered_psi::lower_machine(&checked, TerminalMachineSelection::Name("value"))
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"))
}

fn run(
    module: &TerminalModule,
    proof: &terminal_psi::ProofBundle,
    arguments: &[TerminalScalarValue],
) -> Result<TerminalExecutionResult, TerminalArtifactInterpretError> {
    let semantics = encode_module(module).expect("canonical semantic bytes");
    let proof = encode_proof_section(module, proof).expect("canonical proof bytes");
    interpret_terminal_artifact(&semantics, &proof, &AdmissionProfile::default(), arguments)
}

fn execute(
    source: &str,
    arguments: &[TerminalScalarValue],
) -> Result<TerminalExecutionResult, TerminalArtifactInterpretError> {
    let lowered = lower(source);
    run(&lowered.semantic_module, &lowered.proof_bundle, arguments)
}

/// The only Trapping operation of the lowered entry machine.
fn trapping_operation(module: &TerminalModule) -> (OperationId, TrappingIntegerOperation) {
    let mut found = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::TrappingInteger {
                operation: trapping,
            } => Some((operation.id, trapping)),
            _ => None,
        });
    let operation = found
        .next()
        .expect("the source lowers a Trapping operation");
    assert!(found.next().is_none(), "exactly one Trapping operation");
    operation
}

fn assert_returns(source: &str, arguments: &[TerminalScalarValue], expected: TerminalScalarValue) {
    match execute(source, arguments) {
        Ok(result) => assert_eq!(
            result,
            TerminalExecutionResult::Scalar(expected),
            "{source} with {arguments:?}"
        ),
        Err(error) => panic!("{source} with {arguments:?}: {error:#?}"),
    }
}

/// The crash is a `Trap` at the Trapping operation itself: no edge, no
/// boundary identity, and no wrapped or saturated result.
fn assert_traps(source: &str, arguments: &[TerminalScalarValue]) {
    let lowered = lower(source);
    let (operation, _) = trapping_operation(&lowered.semantic_module);
    let error = run(&lowered.semantic_module, &lowered.proof_bundle, arguments)
        .expect_err("an out-of-range Trapping operation crashes");
    let TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash)) = error
    else {
        panic!("{source} with {arguments:?}: expected a crash, got {error:#?}");
    };
    assert_eq!(crash.cause, CrashCause::Trap, "{source}");
    assert!(
        matches!(crash.site, TerminalCrashSite::Operation { operation: site, .. } if site == operation),
        "{source}: the crash site is the Trapping operation, got {:?}",
        crash.site
    );
    assert!(crash.site_guard.is_empty());
}

const ADD: &str = "machine value(left: u8 in Trapping, right: u8 in Trapping) -> u8 \
                   { (left + right) as u8 }";

#[test]
fn trapping_add_returns_the_exact_sum_or_traps_at_the_operation() {
    assert_returns(ADD, &[unsigned(8, 100), unsigned(8, 50)], unsigned(8, 150));
    assert_returns(ADD, &[unsigned(8, 0), unsigned(8, 255)], unsigned(8, 255));
    // 200 + 100 = 300: Wrapping would answer 44 and Saturating 255.
    assert_traps(ADD, &[unsigned(8, 200), unsigned(8, 100)]);
    assert_traps(ADD, &[unsigned(8, 1), unsigned(8, 255)]);
}

#[test]
fn trapping_subtract_and_multiply_trap_outside_their_carrier() {
    let subtract = "machine value(left: u8 in Trapping, right: u8 in Trapping) -> u8 \
                    { (left - right) as u8 }";
    assert_returns(subtract, &[unsigned(8, 5), unsigned(8, 3)], unsigned(8, 2));
    assert_traps(subtract, &[unsigned(8, 3), unsigned(8, 5)]);

    let multiply = "machine value(left: i8 in Trapping, right: i8 in Trapping) -> i8 \
                    { (left * right) as i8 }";
    assert_returns(multiply, &[signed(8, -8), signed(8, 16)], signed(8, -128));
    assert_traps(multiply, &[signed(8, 16), signed(8, 8)]);
    assert_traps(multiply, &[signed(8, -16), signed(8, -8)]);
}

#[test]
fn trapping_division_and_remainder_trap_on_zero_and_signed_overflow() {
    let divide = "machine value(left: i32 in Trapping, right: i32 in Trapping) -> i32 \
                  { (left / right) as i32 }";
    assert_returns(divide, &[signed(32, 140), signed(32, 2)], signed(32, 70));
    assert_returns(divide, &[signed(32, -7), signed(32, 2)], signed(32, -3));
    assert_traps(divide, &[signed(32, 7), signed(32, 0)]);
    assert_traps(divide, &[signed(32, i128::from(i32::MIN)), signed(32, -1)]);

    let remainder = "machine value(left: i32 in Trapping, right: i32 in Trapping) -> i32 \
                     { (left % right) as i32 }";
    assert_returns(remainder, &[signed(32, -7), signed(32, 2)], signed(32, -1));
    assert_traps(remainder, &[signed(32, 7), signed(32, 0)]);
    // The common quotient of `MIN % -1` is unrepresentable, so it traps
    // rather than answering Wrapping's and Saturating's zero.
    assert_traps(
        remainder,
        &[signed(32, i128::from(i32::MIN)), signed(32, -1)],
    );
}

#[test]
fn trapping_shifts_trap_on_counts_and_shifted_out_value() {
    let left = "machine value(input: u8 in Trapping, count: u32) -> u8 \
                { (input << count) as u8 }";
    assert_returns(left, &[unsigned(8, 1), unsigned(32, 7)], unsigned(8, 128));
    // 200 << 1 is 400, not the wrapped 144.
    assert_traps(left, &[unsigned(8, 200), unsigned(32, 1)]);
    // An out-of-range count traps even when the value is zero.
    assert_traps(left, &[unsigned(8, 0), unsigned(32, 8)]);

    let right = "machine value(input: i32 in Trapping, count: u32) -> i32 \
                 { (input >> count) as i32 }";
    assert_returns(right, &[signed(32, -8), unsigned(32, 1)], signed(32, -4));
    assert_traps(right, &[signed(32, 1), unsigned(32, 32)]);
}

#[test]
fn trapping_conversion_returns_the_same_value_or_traps() {
    let narrow = "machine value(input: u16) -> u8 { (input as u8 in Trapping) as u8 }";
    assert_returns(narrow, &[unsigned(16, 200)], unsigned(8, 200));
    assert_traps(narrow, &[unsigned(16, 300)]);

    let cross = "machine value(input: i16) -> u8 { (input as u8 in Trapping) as u8 }";
    assert_returns(cross, &[signed(16, 255)], unsigned(8, 255));
    assert_traps(cross, &[signed(16, -1)]);
}

#[test]
fn the_operation_crash_site_row_carries_the_exact_primitive_denotation() {
    let lowered = lower(ADD);
    let module = &lowered.semantic_module;
    let (operation, trapping) = trapping_operation(module);
    assert!(matches!(trapping, TrappingIntegerOperation::Add { .. }));
    let profile =
        reconstruct_canonical_terminal_trace_v1_profile(module).expect("verifier-derived profile");
    let [row] = profile.operation_crash_sites.as_slice() else {
        panic!(
            "one operation crash site: {:?}",
            profile.operation_crash_sites
        );
    };
    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
    assert_eq!(row.operation, operation);
    assert_eq!(row.cause, CrashCause::Trap);
    assert_eq!(row.primitive, TrappingIntegerPrimitive::Add);
    assert_eq!((row.result_type, row.operand_type), (u8_type, u8_type));
    // No edge row is fabricated for the operation-level trap.
    assert!(profile.crash_sites.is_empty());

    let bytes = encode_terminal_trace_v1_profile(&profile).expect("canonical profile bytes");
    assert_eq!(
        accept_terminal_trace_v1_profile(module, &bytes),
        Ok(profile.clone())
    );

    // The interpreter's crash coordinate closes a trace at exactly this row.
    let machine = row.machine;
    let block = row.block;
    profile
        .begin_trace()
        .finish_operation_crash(machine, block, operation, CrashCause::Trap)
        .expect("the declared operation crash site");
    assert_eq!(
        profile
            .begin_trace()
            .finish_operation_crash(machine, block, operation, CrashCause::Abort),
        Err(TerminalTraceV1ConstructionError::CrashCauseMismatch {
            declared: CrashCause::Trap,
            actual: CrashCause::Abort,
        })
    );
    let elsewhere = OperationId::new(operation.get() + 1000).unwrap();
    assert_eq!(
        profile
            .begin_trace()
            .finish_operation_crash(machine, block, elsewhere, CrashCause::Trap),
        Err(
            TerminalTraceV1ConstructionError::UnknownOperationCrashSite {
                machine,
                block,
                operation: elsewhere,
            }
        )
    );

    // A relocated or re-caused row is not the module's observer.
    let mut relocated = profile.clone();
    relocated.operation_crash_sites[0].operation = elsewhere;
    let relocated = encode_terminal_trace_v1_profile(&relocated).expect("well-formed bytes");
    assert_eq!(
        accept_terminal_trace_v1_profile(module, &relocated),
        Err(TerminalTraceV1ProfileAcceptanceError::ProfileMismatch)
    );
    let mut missing = profile.clone();
    missing.operation_crash_sites.clear();
    let missing = encode_terminal_trace_v1_profile(&missing).expect("well-formed bytes");
    assert_eq!(
        accept_terminal_trace_v1_profile(module, &missing),
        Err(TerminalTraceV1ProfileAcceptanceError::ProfileMismatch)
    );
    let mut aborting = profile;
    aborting.operation_crash_sites[0].cause = CrashCause::Abort;
    assert!(encode_terminal_trace_v1_profile(&aborting).is_err());
}

#[test]
fn a_trapping_site_without_an_unconditional_trap_ceiling_rejects() {
    let lowered = lower(ADD);
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .position(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    assert!(
        lowered.semantic_module.machines[machine]
            .contract
            .crash_routes
            .iter()
            .any(|bucket| bucket.cause == CrashCause::Trap
                && bucket.alternatives == [CrashRouteGuard::Truth]),
        "the inferred body publishes its Trapping site's unconditional Trap route"
    );
    assert!(
        run(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &[unsigned(8, 1), unsigned(8, 2)]
        )
        .is_ok()
    );

    // Missing evidence: without the Trap ceiling the site is uncovered.
    let mut uncovered = lowered.semantic_module.clone();
    uncovered.machines[machine]
        .contract
        .crash_routes
        .retain(|bucket| bucket.cause != CrashCause::Trap);
    assert!(matches!(
        terminal_verifier::validate_module(&uncovered),
        Err(terminal_verifier::ModuleError::TrappingIntegerCrashUncovered { .. })
    ));

    // Abort does not cover a Trap site.
    let mut aborting = lowered.semantic_module.clone();
    for bucket in &mut aborting.machines[machine].contract.crash_routes {
        bucket.cause = CrashCause::Abort;
    }
    assert!(matches!(
        terminal_verifier::validate_module(&aborting),
        Err(terminal_verifier::ModuleError::TrappingIntegerCrashUncovered { .. })
    ));
}

#[test]
fn a_changed_trapping_operand_carrier_rejects() {
    // Point the add's right operand at the u8 result of a different value of
    // the wrong carrier: the verifier reconstructs the denotation from the
    // module's own value types, never from the operation's claim.
    let lowered = lower(
        "machine value(left: u8 in Trapping, right: u8 in Trapping, other: u16) -> u8 \
         { (left + right) as u8 }",
    );
    let mut module = lowered.semantic_module.clone();
    let entry = module.entry;
    let machine = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == entry)
        .unwrap();
    let other = machine.parameters[2].id;
    for operation in machine
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
    {
        if let OperationKind::TrappingInteger {
            operation: TrappingIntegerOperation::Add { right, .. },
        } = &mut operation.kind
        {
            *right = other;
        }
    }
    assert!(matches!(
        terminal_verifier::validate_module(&module),
        Err(terminal_verifier::ModuleError::TrappingIntegerOperandTypeMismatch { .. })
    ));
}
