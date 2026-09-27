//! Source-to-serialized-Terminal-Psi execution, without a native program entry.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_codec::{encode_module, encode_proof_section};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};

fn execute(source: &str, arguments: &[TerminalScalarValue]) -> TerminalExecutionResult {
    let checked = crate::front_end::checked_program(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("value"),
    )
    .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    let semantics = encode_module(&lowered.semantic_module).expect("canonical semantic bytes");
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("canonical proof bytes");
    // The interpreter decodes and verifies the serialized sections independently;
    // no checked scalar tree or producer-owned module is an execution input.
    interpret_terminal_artifact(&semantics, &proof, &AdmissionProfile::default(), arguments)
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"))
}

fn signed(value: i64) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, 32).expect("i32"),
        value: IntegerValue::Signed(i128::from(value)),
    }
}

fn unsigned(value: u64) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"),
        value: IntegerValue::Unsigned(u128::from(value)),
    }
}

#[test]
fn explicitly_typed_wrapping_division_executes_truncating_integer_arithmetic() {
    for (body, target, literal, expected) in [
        ("(7i32 as i32 in Wrapping) / 2 * 2", "i32", 6, signed(6)),
        (
            "(4097u32 as u32 in Wrapping) / 4096 * 4096",
            "u32",
            4096,
            unsigned(4096),
        ),
    ] {
        let source = format!(
            "machine value() -> {target} in Wrapping\nrequires {literal}{target} == {literal}{target}\nensures {literal}{target} == {literal}{target}\n{{ {body} }}"
        );
        assert_eq!(
            execute(&source, &[]),
            TerminalExecutionResult::Scalar(expected)
        );
    }
}

/// A declared nominal operator of the same spelling is not this occurrence's
/// selected declaration. `core/nat.omg` declares
/// `operator - Nat::subtract(left: Nat, right: Nat) -> Nat`, so every program
/// importing the bundled library used to lose builtin meaning for wholly
/// anonymous integer arithmetic: the destination's value fact was never
/// produced and the enclosing Unit body reached no plan. The operand lookup
/// that answered the question treats unknown operands as wildcards and is
/// therefore program-global, while resolution had already recorded
/// `Intrinsic(BuiltinOperator)` at the `-` occurrence itself.
#[test]
fn a_declared_nominal_operator_leaves_anonymous_arithmetic_builtin() {
    let source = r#"
        pub data Nat {
            case Zero;
            case Succ(prev: Nat);
        }

        operator - Nat::subtract(left: Nat, right: Nat) -> Nat;

        machine value() -> i32
        requires 7i32 == 7i32
        ensures 7i32 == 7i32
        {
            let landed: i32 = 0 - 7 / 2 * 2;
            0 - landed
        }
    "#;
    assert_eq!(
        execute(source, &[]),
        TerminalExecutionResult::Scalar(signed(7))
    );
}
