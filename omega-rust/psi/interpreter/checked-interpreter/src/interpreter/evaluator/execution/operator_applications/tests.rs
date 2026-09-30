//! Boundary operator applications execute the realization checked Psi
//! selects for their exact overload, in value and cast position.

use crate::{BuildMachineEntry, InterpretOptions, InterpretOutcome, interpret_entry};
use checked_trees::CheckedTrees;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::types::PrimitiveType;

fn run(checked: &CheckedTrees) -> InterpretOutcome {
    interpret_entry(
        checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    )
}

fn float_policy_program(qualification: &str) -> CheckedTrees {
    crate::front_end::checked_program(&format!(
        "pub boundary machine * Float::multiply(left: f32, right: f32) -> f32;
             pub data FloatNativeProvider {{}}
             machine FloatNativeProvider::multiply32(left: f32, right: f32) -> f32
                 satisfies Float::multiply via ForeignBinding::CompilerIntrinsic;
             machine main() -> i32 {{
                 let maximum: f32 {qualification} = 340282346638528859811704183484516925440.0f32;
                 let two: f32 {qualification} = 2.0f32;
                 let product: f32 = (maximum * two) as f32;
                 transition product == maximum {{ true -> (70) false -> (71) }}
             }}"
    ))
}

#[test]
fn requirement_call_preserves_its_checked_float_policy() {
    for (qualification, expected) in [("in Saturating", 70), ("", 71)] {
        let checked = float_policy_program(qualification);
        let outcome = run(&checked);
        assert_eq!(outcome.error, None, "{qualification}");
        assert_eq!(outcome.exit_code, expected, "{qualification}");
    }
}

#[test]
fn requirement_call_traps_on_its_checked_overflow_policy() {
    let outcome = run(&float_policy_program("in Trapping"));
    assert_eq!(
        outcome.error.as_deref(),
        Some("float overflow in Trapping domain")
    );
}

#[test]
fn requirement_call_rejects_missing_or_ambiguous_selection() {
    let checked = float_policy_program("in Saturating");
    let selected = *checked
        .facts
        .operators
        .named_requirement_uses()
        .next()
        .expect("a bound requirement retains its checked policy");
    let mut missing = checked.clone();
    missing.facts.operators.named_requirement_uses = Default::default();
    assert_eq!(
        run(&missing).error.as_deref(),
        Some("operator application has no checked expression selection")
    );
    let mut duplicated = checked;
    duplicated
        .facts
        .operators
        .named_requirement_uses
        .append(selected);
    assert_eq!(
        run(&duplicated).error.as_deref(),
        Some("operator application has conflicting checked selections")
    );
}

/// The `I32::from_f64` overload whose `i32` result is in `domain`.
fn overload(checked: &CheckedTrees, domain: ArithmeticDomain) -> symbols::SymbolHandle {
    checked
        .operators()
        .iter()
        .find(|operator| {
            checked.primitive_type_reference(operator.return_type) == Some(PrimitiveType::I32)
                && checked
                    .type_reference_table
                    .arithmetic_domain(operator.return_type)
                    == domain
        })
        .unwrap_or_else(|| panic!("{domain:?} overload"))
        .symbol
}

const CHECKED_OFFSET: &str = "
    data CheckedMath {}
    boundary operator CheckedMath::offset_seven(value: i32 in Wrapping) -> i32 in Wrapping
    ensures result == value + 7;
    data CheckedMathProvider {}
    machine CheckedMathProvider::offset_seven_impl(input: i32 in Wrapping) -> i32 in Wrapping
    satisfies CheckedMath::offset_seven
    ensures result == input + 7
    {
        transition { _ -> (input + 7) }
    }
";

// `pass/providers/checked_boundary_operator_physical_custody`: the operator
// symbol owns no state, and its single checked satisfier is the realization
// the Build selects. Before this route the call declined with "qualified call
// target `offset_seven` has no resolved state".
#[test]
fn checked_satisfier_body_runs_for_a_named_operator_application() {
    let checked = crate::front_end::checked_program(&format!(
        "{CHECKED_OFFSET}
         machine main() -> i32 {{
             let result: i32 in Wrapping = CheckedMath::offset_seven(63);
             result as i32
         }}"
    ));
    let outcome = run(&checked);
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}

// Two checked satisfiers leave the Build's provider choice open, and checked
// Psi does not retain it. The application declines instead of running either
// body or one found by its leaf spelling.
#[test]
fn several_checked_satisfiers_decline_without_choosing() {
    let checked = crate::front_end::checked_program(&format!(
        "{CHECKED_OFFSET}
         machine CheckedMathProvider::offset_seven_again(input: i32 in Wrapping) -> i32 in Wrapping
         satisfies CheckedMath::offset_seven
         ensures result == input + 7
         {{
             transition {{ _ -> (input + 7) }}
         }}
         machine main() -> i32 {{
             let result: i32 in Wrapping = CheckedMath::offset_seven(63);
             result as i32
         }}"
    ));
    let outcome = run(&checked);
    assert_eq!(
        outcome.error.as_deref(),
        Some("qualified call target `offset_seven` has no resolved state")
    );
}

// Destination-owned conversion overloads realized only by compiler
// intrinsics, the shape of `core::float_operations` and its per-target
// `FloatNativeProvider` leaves.
const CONVERSIONS: &str = "
    pub data I32 {}
    pub data U64 {}
    pub data F32 {}
    pub data F64 {}
    pub boundary operator I32::from_f64(value: f64) -> i32
    requires value == value && value > -2147483649.0 && value < 2147483648.0;
    pub boundary operator I32::from_f64(value: f64) -> i32 in Trapping;
    pub boundary operator I32::from_f64(value: f64) -> i32 in Saturating;
    pub boundary operator U64::from_f64(value: f64) -> u64 in Saturating;
    pub boundary operator F32::from_f64(value: f64) -> f32;
    pub boundary operator F64::from_i32(value: i32) -> f64;
    pub data FloatNativeProvider {}
    machine FloatNativeProvider::to_i32_exact(value: f64) -> i32
        satisfies I32::from_f64 via ForeignBinding::CompilerIntrinsic;
    machine FloatNativeProvider::to_i32_trapping(value: f64) -> i32 in Trapping
        satisfies I32::from_f64 via ForeignBinding::CompilerIntrinsic;
    machine FloatNativeProvider::to_i32_saturating(value: f64) -> i32 in Saturating
        satisfies I32::from_f64 via ForeignBinding::CompilerIntrinsic;
    machine FloatNativeProvider::to_u64_saturating(value: f64) -> u64 in Saturating
        satisfies U64::from_f64 via ForeignBinding::CompilerIntrinsic;
    machine FloatNativeProvider::to_f32(value: f64) -> f32
        satisfies F32::from_f64 via ForeignBinding::CompilerIntrinsic;
    machine FloatNativeProvider::from_i32(value: i32) -> f64
        satisfies F64::from_i32 via ForeignBinding::CompilerIntrinsic;
";

// Each overload executes its own exact meaning: the unqualified overload and
// Trapping truncate toward zero, Saturating clamps to the carrier and maps NaN
// to zero, and format and integer-to-float conversions round nearest-even.
#[test]
fn compiler_intrinsic_conversions_execute_their_selected_overload() {
    let checked = crate::front_end::checked_program(&format!(
        "{CONVERSIONS}
         machine main() -> i32 {{
             let exact: i32 = I32::from_f64(-321.75);
             let trapped: i32 in Trapping = I32::from_f64(70.75);
             let huge: f64 = 100000000000000000000.0;
             let clamped: i32 in Saturating = I32::from_f64(huge);
             let nan: f64 = 0.0f64 / 0.0;
             let zeroed: i32 in Saturating = I32::from_f64(nan);
             let negative: u64 in Saturating = U64::from_f64(0.0 - huge);
             let halfway: f64 = 1.000000059604644775390625;
             let down: f32 = F32::from_f64(halfway);
             let widened: f64 = F64::from_i32(-7);
             transition exact == -321
                 && (trapped as i32) == 70
                 && (clamped as i32) == 2147483647
                 && (zeroed as i32) == 0
                 && (negative as u64) == 0
                 && down == 1.0f32
                 && widened == -7.0 {{
                 true -> 70
                 false -> 1
             }}
         }}"
    ));
    let outcome = run(&checked);
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}

// `pass/float/named_float_integer_lower_boundary_conversion_traps`: exactly
// one below the i32 minimum cannot truncate, so the Trapping overload traps
// where native execution traps.
#[test]
fn trapping_conversion_traps_on_a_value_that_cannot_truncate() {
    let checked = crate::front_end::checked_program(&format!(
        "{CONVERSIONS}
         machine main() -> i32 {{
             let converted: i32 in Trapping = I32::from_f64(-2147483649.0);
             converted as i32
         }}"
    ));
    let outcome = run(&checked);
    assert_eq!(
        outcome.error.as_deref(),
        Some(
            "float-to-int conversion failed in Trapping domain: the truncated value is out of range for I32"
        )
    );
}

// A policy cast is the ordinary call its author could have written: checking
// binds `value as i32 in Saturating` to the Saturating overload, and the
// interpreter runs that overload by identity.
#[test]
fn policy_cast_runs_the_conversion_overload_it_was_bound_to() {
    let checked = crate::front_end::checked_program(&format!(
        "{CONVERSIONS}
         machine main() -> i32 {{
             let nan: f64 = 0.0f64 / 0.0;
             let zeroed: i32 = (nan as i32 in Saturating) as i32;
             let huge: f64 = 0.0 - 100000000000000000000.0;
             let clamped: i32 = (huge as i32 in Saturating) as i32;
             transition zeroed == 0 && clamped == -2147483648 {{
                 true -> 70
                 false -> 1
             }}
         }}"
    ));
    let saturating = overload(&checked, ArithmeticDomain::Saturating);
    let bound = checked
        .expression_table
        .iter_expressions()
        .filter(|(_, expression)| {
            matches!(
                expression,
                typed_trees::expression::ExpressionNode::Call(call)
                    if call.target_symbol == saturating
            )
        })
        .count();
    assert_eq!(bound, 2, "both policy casts bind the Saturating overload");
    let outcome = run(&checked);
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}

// The call and its checked named-use fact must name the same overload. A call
// retargeted after checking traps instead of running the substituted overload.
#[test]
fn application_retargeted_after_checking_traps() {
    let mut checked = crate::front_end::checked_program(&format!(
        "{CONVERSIONS}
         machine main() -> i32 {{
             let clamped: i32 in Saturating = I32::from_f64(1.5);
             clamped as i32
         }}"
    ));
    let trapping = overload(&checked, ArithmeticDomain::Trapping);
    let call = checked
        .facts
        .operators
        .named_uses()
        .next()
        .expect("checked named use")
        .expression;
    let typed_trees::expression::ExpressionNode::Call(call) =
        checked.typed.expression_table.expression_mut(call)
    else {
        panic!("the named use is a call");
    };
    call.target_symbol = trapping;
    let outcome = run(&checked);
    assert_eq!(
        outcome.error.as_deref(),
        Some("operator application differs from its checked named selection")
    );
}

// Spelled float arithmetic binds to a call on the bodyless token-bearing
// `Float` requirement (`operators/call_binding.rs`), the shape every program
// importing `core::float_operations` has. Its compiler-intrinsic realization
// executes the sealed primitive slot: exact arithmetic rounded once through
// the format. Before this route `0.1 + 0.2` declined with "call target
// `Float::add` is a bodyless requirement or external declaration".
#[test]
fn spelled_float_arithmetic_runs_its_bound_primitive_requirement() {
    let checked = crate::front_end::checked_program(
        "pub boundary machine + Float::add(left: f64, right: f64) -> f64;
         pub boundary machine / Float::divide(left: f64, right: f64) -> f64;
         pub boundary machine * Float::multiply(left: f32, right: f32) -> f32;
         pub data FloatNativeProvider {}
         machine FloatNativeProvider::add64(left: f64, right: f64) -> f64
             satisfies Float::add via ForeignBinding::CompilerIntrinsic;
         machine FloatNativeProvider::divide64(left: f64, right: f64) -> f64
             satisfies Float::divide via ForeignBinding::CompilerIntrinsic;
         machine FloatNativeProvider::multiply32(left: f32, right: f32) -> f32
             satisfies Float::multiply via ForeignBinding::CompilerIntrinsic;
         machine main() -> i32 {
             let tenth: f64 = 0.1;
             let sum: f64 = tenth + 0.2;
             let zero: f64 = 0.0;
             let nan: f64 = zero / zero;
             let third: f32 = 0.33333334f32;
             let product: f32 = third * 3.0f32;
             transition sum == 0.30000000000000004 && nan != nan && product == 1.0f32 {
                 true -> 70
                 false -> 1
             }
         }",
    );
    let requirements = checked
        .machines()
        .iter()
        .filter(|machine| !machine.body_is_present)
        .filter_map(|machine| checked.machine_states(machine).first())
        .map(|entry| entry.symbol)
        .collect::<Vec<_>>();
    let bound = checked
        .expression_table
        .iter_expressions()
        .filter(|(_, expression)| {
            matches!(
                expression,
                typed_trees::expression::ExpressionNode::Call(call)
                    if requirements.contains(&call.target_symbol)
            )
        })
        .count();
    assert_eq!(bound, 3, "each spelled operation binds its requirement");
    let outcome = run(&checked);
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 70);
}
