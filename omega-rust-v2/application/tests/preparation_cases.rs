//! CLI diagnostic twins for refusals owned by L4 preparation.
//!
//! Each test checks one refusal family from `layers/04_typed-trees-to-prepared-trees/CONTRACT.md`.
//! The positive twin must pass `source prepare`. The negative twin must stop in preparation at
//! the exact source occurrence, and `source lower` must report the same stop. The sources and
//! reasons come from the V2 preparation crate and its tests. These cases cover selected
//! diagnostic families, not every place where preparation can refuse.
#[path = "support/diagnostic_twins.rs"]
mod diagnostic_twins;
use diagnostic_twins::Case;
use omega_preparation::{PreparationFailure, PreparationReason, RefusalCause};

const ADMISSION: &str = "L4 specializes the selected body's published effects";
const TERMINATION: &str = "A local declaration after a tail expression";
const DISCARD: &str = "An explicit discard evaluates its operand";
const FROZEN_VALUES: &str = "L4 evaluates integers, Booleans, floating-point values";
const NAMED_READS: &str = "Named reads connect each prepared constant";
const TYPED_CAUSE: &str = "Every refusal L4 renders carries its typed cause";

fn check_kind(expected: impl Into<RefusalCause>, case: &Case<'_>) {
    let package = diagnostic_twins::resolve_root_package(case.bad).unwrap();
    let failure = omega_preparation::prepare_source_package_diagnosed(&package).expect_err("negative twin prepared");
    assert!(matches!(failure, PreparationFailure::Preparation(_)), "{failure:?}");
    let source = case.bad.iter().find(|(file, _)| *file == case.file).unwrap().1;
    let start = source.rfind(case.occurrence).unwrap();
    spec_corpus::assert_preparation_refusal!(
        failure, expected.into(), "root", case.file, (start, start + case.occurrence.len()), case.reason,
    );
}

fn check(expected: impl Into<RefusalCause>, case: Case<'_>) {
    check_kind(expected, &case);
    diagnostic_twins::check(case);
}

// Preparation consumes this request while standalone typing requires a closed identity.
fn check_pending(
    expected: PreparationReason,
    case: Case<'_>,
    occurrence: &str,
) {
    for files in [case.good, case.bad] {
        let [(file, text)] = files else {
            panic!("expected a single-source demand witness")
        };
        let package = diagnostic_twins::resolve_root_package(files).unwrap();
        let progress = omega_typer::type_source_package_for_preparation(&package, &Default::default()).unwrap();
        let omega_typer::TypingProgress::Pending(pending) = progress else {
            panic!("expected a pending evaluation")
        };
        let [omega_typer::StaticValueDemand::Closed(request)] = pending.demands() else {
            panic!("unexpected demands: {:?}", pending.demands())
        };
        let site = &request.expression;
        let start = text.rfind(occurrence).unwrap();
        assert_eq!((site.package.as_str(), site.file.as_str()), ("root", *file));
        assert_eq!((site.span.get_start_offset(), site.span.get_end_offset()), (start, start + occurrence.len()));
    }
    check_kind(expected, &case);
    diagnostic_twins::check_input(case, Some(occurrence));
}

/// Format the exact reason for a refusal made by L4's static evaluator.
macro_rules! refusal {
    ($reason:literal) => {
        concat!("static evaluation cannot admit this invocation: ", $reason)
    };
}

// Preconditions run before the body. A false integer requirement refuses the call (see
// omega-preparation integer_constants::constants_and_indices_refuse_the_same_call_at_its_occurrence).
#[test]
fn false_integer_precondition_refuses_the_call() {
    check(PreparationReason::IntegerPreconditionFalse, Case {
        layer: "L4",
        producer: "preparation",
        good: &[(
            "admission.omg",
            "machine width(n:u8)->u8 requires n < 8 reaches none crashes none terminates {n +% 1}\nconst SIZE:u8=width(7);",
        )],
        bad: &[(
            "admission.omg",
            "machine width(n:u8)->u8 requires n < 8 reaches none crashes none terminates {n +% 1}\nconst SIZE:u8=width(8);",
        )],
        file: "admission.omg",
        occurrence: "width(8)",
        reason: refusal!("the selected integer precondition is false for the supplied arguments"),
        rule: ADMISSION,
    });
}

// A crash guard that holds for the entry operands leaves the declared route possible (see
// integer_constants::closed_crash_guards_use_the_selected_callees_entry_values).
#[test]
fn possible_trap_route_refuses_the_call() {
    check_pending(
        PreparationReason::TrapRoutePossible,
        Case {
            layer: "L4",
            producer: "preparation",
            good: &[(
                "crash.omg",
                "machine divide(n:u64,d:u64)->u64 crashes Trap (d == 0) {n /! d}\nconst SIZE:u64=divide(12,3);\nmachine carry(x:[u8; SIZE])->[u8;4]{x}",
            )],
            bad: &[(
                "crash.omg",
                "machine divide(n:u64,d:u64)->u64 crashes Trap (d == 0) {n /! d}\nconst SIZE:u64=divide(12,0);\nmachine carry(x:[u8; SIZE])->[u8;4]{x}",
            )],
            file: "crash.omg",
            occurrence: "divide(12,0)",
            reason: refusal!("the selected Trap route remains possible for the supplied arguments"),
            rule: ADMISSION,
        },
        "SIZE",
    );
}

// The ordinary source command has no checker judgment, so a postcondition cannot be
// established for the result (see admission/proofs/contract_tests.rs).
#[test]
fn postcondition_without_a_checked_proof_refuses_the_call() {
    check(PreparationReason::PostconditionNeedsProof, Case {
        layer: "L4",
        producer: "preparation",
        good: &[("ensures.omg", "machine seven(n:u64)->u64 {7u64}\nconst VALUE:u64=seven(3u64);")],
        bad: &[(
            "ensures.omg",
            "machine seven(n:u64)->u64 ensures result == 7u64 {7u64}\nconst VALUE:u64=seven(3u64);",
        )],
        file: "ensures.omg",
        occurrence: "seven(3u64)",
        reason: refusal!("the selected postcondition requires a checked proof for the arguments and result"),
        rule: ADMISSION,
    });
}

// The resolved export status decides whether a termination promise is required (see
// closed_indices::public_machine_needs_a_declared_termination_promise).
#[test]
fn public_machine_without_a_termination_promise_is_refused() {
    check_pending(
        PreparationReason::PublicMachineWithoutTermination,
        Case {
            layer: "L4",
            producer: "preparation",
            good: &[(
                "public.omg",
                "data Buffer<const N:u8>{}\npublic machine width(n:u8)->u8 reaches none crashes none terminates {n +% 1}\nmachine computed(x:Buffer<width(7)>){}",
            )],
            bad: &[(
                "public.omg",
                "data Buffer<const N:u8>{}\npublic machine width(n:u8)->u8 reaches none crashes none {n +% 1}\nmachine computed(x:Buffer<width(7)>){}",
            )],
            file: "public.omg",
            occurrence: "width(7)",
            reason: refusal!("the selected public machine does not declare termination"),
            rule: TERMINATION,
        },
        "width(7)",
    );
}

// The ranking is required at entry, even when the invocation takes the base case (see
// integer_constants::preparation_requires_proof_before_entering_a_recursive_body).
#[test]
fn recursive_component_without_a_checked_ranking_is_refused() {
    check(PreparationReason::CyclicInvocation, Case {
        layer: "L4",
        producer: "preparation",
        good: &[(
            "recursion.omg",
            "machine count(n:u64)->u64 {match n == 0u64 {true -> 7u64, false -> n - 1u64,}}\nconst VALUE:u64=count(0u64);",
        )],
        bad: &[(
            "recursion.omg",
            "machine count(n:u64)->u64 {match n == 0u64 {true -> 7u64, false -> count(n - 1u64),}}\nconst VALUE:u64=count(0u64);",
        )],
        file: "recursion.omg",
        occurrence: "count(n - 1u64)",
        reason: refusal!("a cyclic static invocation requires a checked termination proof\nrecursive component: count -> count"),
        rule: TERMINATION,
    });
}

// A discarded operand still runs, so its failed precondition still refuses preparation (see
// boolean_calls::a_discarded_operand_still_refuses_its_failed_precondition).
#[test]
fn discarded_operand_still_refuses_its_failed_precondition() {
    check(PreparationReason::IntegerPreconditionFalse, Case {
        layer: "L4",
        producer: "preparation",
        good: &[(
            "discard.omg",
            "machine divide(value:u8)->u8 requires value != 0 {12u8 / value}\nmachine caller(x:u8)->u8{_ = divide(x); x}\nconst V:u8=caller(3u8);",
        )],
        bad: &[(
            "discard.omg",
            "machine divide(value:u8)->u8 requires value != 0 {12u8 / value}\nmachine caller(x:u8)->u8{_ = divide(x); x}\nconst V:u8=caller(0u8);",
        )],
        file: "discard.omg",
        occurrence: "divide(x)",
        reason: refusal!("the selected integer precondition is false for the supplied arguments\ncall chain: caller -> divide"),
        rule: DISCARD,
    });
}

// Exact integer operations must stay within the selected carrier (see
// integer_constants::exact_intermediate_overflow_refuses_the_call_before_its_parent_operation).
#[test]
fn exact_intermediate_overflow_refuses_the_call() {
    check(PreparationReason::AdditionOutsideCarrier, Case {
        layer: "L4",
        producer: "preparation",
        good: &[("overflow.omg", "machine compute(n:i8)->i8{(n + 1) - 1}\nconst RESULT:i8=compute(1);")],
        bad: &[("overflow.omg", "machine compute(n:i8)->i8{(n + 1) - 1}\nconst RESULT:i8=compute(127);")],
        file: "overflow.omg",
        occurrence: "compute(127)",
        reason: refusal!("exact addition exceeds its selected integer carrier"),
        rule: FROZEN_VALUES,
    });
}

#[test]
fn an_open_callable_has_no_runtime_descriptor_layout() {
    let good = r"use omega\language\core; data F=machine(x:u8)->u8 reaches none crashes none; const SIZE:u64=core\size_of::<F>();";
    let bad = good.replace(" reaches none", "");
    check(RefusalCause::TargetLayout("the selected layout query needs a data type with runtime storage"), Case {
        layer: "L4",
        producer: "preparation",
        good: &[("callback.omg", good)],
        bad: &[("callback.omg", &bad)],
        file: "callback.omg",
        occurrence: r"core\size_of::<F>()",
        reason: refusal!("the selected layout query needs a data type with runtime storage"),
        rule: TYPED_CAUSE,
    });
}

#[test]
fn a_proof_only_layout_query_cannot_be_deferred_for_target_selection() {
    let good = r"use omega\language\core; const VALUE:u64=core\size_of::<u8>();";
    let bad = good.replace("::<u8>", r"::<core\Squash<u8>>");
    check(RefusalCause::TargetLayout("the selected layout query needs a data type with runtime storage"), Case {
        layer: "L4",
        producer: "preparation",
        good: &[("geometry.omg", good)],
        bad: &[("geometry.omg", &bad)],
        file: "geometry.omg",
        occurrence: r"core\size_of::<core\Squash<u8>>()",
        reason: refusal!("the selected layout query needs a data type with runtime storage"),
        rule: TYPED_CAUSE,
    });
}

#[test]
fn a_shift_count_at_the_carrier_width_forwards_the_integer_refusal() {
    let good = "machine shift(n:u7,k:u64)->u7{n << k}\nconst VALUE:u7=shift(1,6);";
    let bad = good.replace("shift(1,6)", "shift(1,7)");
    check(RefusalCause::IntegerPrimitive("static shift count outside carrier width"), Case {
        layer: "L4",
        producer: "preparation",
        good: &[("shift.omg", good)],
        bad: &[("shift.omg", &bad)],
        file: "shift.omg",
        occurrence: "shift(1,7)",
        reason: refusal!("static shift count outside carrier width"),
        rule: TYPED_CAUSE,
    });
}

// Small values fit even this carrier; only wrapping below zero needs its enormous modulus.
#[test]
fn a_forwarded_integer_refusal_identifies_the_nested_call_in_its_defining_file() {
    let helpers = ("helpers.omg", "machine shift(n:u7,k:u64)->u7{n << k}\nmachine via(k:u64)->u7{shift(1,k)}");
    check(RefusalCause::IntegerPrimitive("static shift count outside carrier width"), Case {
        layer: "L4",
        producer: "preparation",
        good: &[helpers, ("use.omg", "const VALUE:u7=via(6);")],
        bad: &[helpers, ("use.omg", "const VALUE:u7=via(7);")],
        file: "helpers.omg",
        occurrence: "shift(1,k)",
        reason: refusal!("static shift count outside carrier width\ncall chain: via -> shift"),
        rule: TYPED_CAUSE,
    });
}

#[test]
fn wrapping_below_zero_forwards_the_integer_allocation_budget_refusal() {
    let good = "machine prev(x:u18446744073709551615)->u18446744073709551615{x -% 1}\nconst VALUE:u18446744073709551615=prev(1);";
    let bad = good.replace("prev(1)", "prev(0)");
    check(RefusalCause::IntegerPrimitive("static evaluation exceeds the integer allocation budget"), Case {
        layer: "L4",
        producer: "preparation",
        good: &[("budget.omg", good)],
        bad: &[("budget.omg", &bad)],
        file: "budget.omg",
        occurrence: "prev(0)",
        reason: refusal!("static evaluation exceeds the integer allocation budget"),
        rule: TYPED_CAUSE,
    });
}

// The complete constant type must permit copying (see
// owned_records::freezing_checks_the_type_of_inactive_payloads).
#[test]
fn linear_constant_type_cannot_freeze() {
    check(PreparationReason::ConstantTypeNotCopyable, Case {
        layer: "L4",
        producer: "preparation",
        good: &[(
            "copy.omg",
            "data Handle [copy] {}\ndata Choice [copy] {case Empty; case Used(handle:Handle);}\nconst VALUE:Choice=Choice::Empty;",
        )],
        bad: &[(
            "copy.omg",
            "data Handle [linear] {}\ndata Choice [linear] {case Empty; case Used(handle:Handle);}\nconst VALUE:Choice=Choice::Empty;",
        )],
        file: "copy.omg",
        occurrence: "Choice::Empty",
        reason: refusal!("the complete constant type must permit copying"),
        rule: FROZEN_VALUES,
    });
}

// The type walk includes inactive payloads. Here only the unused `Used` case has a cleanup
// obligation (see owned_records::freezing_checks_the_type_of_inactive_payloads).
#[test]
fn cleanup_in_an_inactive_payload_prevents_freezing() {
    check(PreparationReason::ConstantTypeHasCleanup, Case {
        layer: "L4",
        producer: "preparation",
        good: &[(
            "cleanup.omg",
            "data Handle [copy] {}\nmachine dispose(handle:Handle) {}\ndata Choice<T [copy]> [copy] {case Empty; case Used(handle:T);}\nconst VALUE:Choice<Handle> = Choice::Empty;",
        )],
        bad: &[(
            "cleanup.omg",
            "data Handle [copy] dropped by dispose {}\nmachine dispose(handle:Handle) {}\ndata Choice<T [copy]> [copy] {case Empty; case Used(handle:T);}\nconst VALUE:Choice<Handle> = Choice::Empty;",
        )],
        file: "cleanup.omg",
        occurrence: "Choice::Empty",
        reason: refusal!("the complete constant type has a cleanup obligation"),
        rule: FROZEN_VALUES,
    });
}

// Constants are materialized in source order. FIRST reads SECOND, and SECOND's read of the active
// FIRST closes the cycle at that initializer (see
// integer_constants::annotated_integer_constants_cannot_supply_each_other_through_a_cycle).
#[test]
fn cyclic_named_constant_reads_are_refused() {
    check(PreparationReason::CyclicConstantInitializer, Case {
        layer: "L4",
        producer: "preparation",
        good: &[("cycle.omg", "const FIRST:u8=SECOND;\nconst SECOND:u8=7;")],
        bad: &[("cycle.omg", "const FIRST:u8=SECOND;\nconst SECOND:u8=FIRST;")],
        file: "cycle.omg",
        occurrence: "FIRST",
        reason: refusal!("cyclic constant initializer"),
        rule: NAMED_READS,
    });
}

// Every declared crash route is checked before the body. Here the Trap route is closed but the
// Abort guard holds (see integer_constants::every_crash_alternative_and_the_operation_itself_must_be_safe).
#[test]
fn possible_abort_route_refuses_the_call() {
    check(PreparationReason::AbortRoutePossible, Case {
        layer: "L4",
        producer: "preparation",
        good: &[(
            "abort.omg",
            "machine f(n:i8)->i8 crashes Trap false crashes Abort (n == 9) { 12i8 /! n }\nconst SIZE:i8=f(3);",
        )],
        bad: &[(
            "abort.omg",
            "machine f(n:i8)->i8 crashes Trap false crashes Abort (n == 9) { 12i8 /! n }\nconst SIZE:i8=f(9);",
        )],
        file: "abort.omg",
        occurrence: "f(9)",
        reason: refusal!("the selected Abort route remains possible for the supplied arguments"),
        rule: ADMISSION,
    });
}

// A Boolean callee selects the Boolean precondition variant. The refusal names the nested call and
// the chain that reached it (see boolean_calls::a_selected_predicate_checks_the_bound_value_before_running_the_callee).
#[test]
fn false_boolean_precondition_refuses_the_nested_call() {
    let machines = "machine predicate(value:bool)->bool reaches none crashes none terminates {value}\nmachine guarded(value:bool)->bool requires predicate(value) {value}\nmachine wrapped(value:bool)->bool {guarded(value)}";
    let good = format!("{machines}\nconst VALUE:bool=wrapped(true);");
    let bad = format!("{machines}\nconst VALUE:bool=wrapped(false);");
    check(PreparationReason::BooleanPreconditionFalse, Case {
        layer: "L4",
        producer: "preparation",
        good: &[("boolean.omg", good.as_str())],
        bad: &[("boolean.omg", bad.as_str())],
        file: "boolean.omg",
        occurrence: "guarded(value)",
        reason: refusal!("the selected Boolean precondition is false for the supplied arguments\ncall chain: wrapped -> guarded"),
        rule: ADMISSION,
    });
}

// An acyclic call graph needs no termination proof. Closing it into a two-machine cycle does, and
// the refusal names both the entry chain and the component (see
// integer_constants::preparation_requires_proof_before_entering_a_recursive_body).
#[test]
fn mutual_recursion_without_a_checked_ranking_is_refused() {
    let program = |second: &str| {
        format!(
            "machine first(n:u64)->u64 {{match n == 0u64 {{true -> 7u64, false -> second(n - 1u64),}}}}\nmachine second(n:u64)->u64 {{{second}}}\nmachine outer()->u64 {{first(0u64)}}\nconst VALUE:u64=outer();"
        )
    };
    let (good, bad) = (program("n"), program("first(n)"));
    check(PreparationReason::CyclicInvocation, Case {
        layer: "L4",
        producer: "preparation",
        good: &[("mutual.omg", good.as_str())],
        bad: &[("mutual.omg", bad.as_str())],
        file: "mutual.omg",
        occurrence: "first(n)",
        reason: refusal!("a cyclic static invocation requires a checked termination proof\ncall chain: outer -> first\nrecursive component: first -> second -> first"),
        rule: TERMINATION,
    });
}

#[test]
fn a_cycle_points_to_the_closing_read_in_the_second_file() {
    let first = ("a.omg", "const FIRST:bool=SECOND;");
    check(PreparationReason::CyclicConstantInitializer, Case {
        layer: "L4",
        producer: "preparation",
        good: &[first, ("b.omg", "const SECOND:bool=!false;")],
        bad: &[first, ("b.omg", "const SECOND:bool=!FIRST;")],
        file: "b.omg",
        occurrence: "FIRST",
        reason: refusal!("cyclic constant initializer"),
        rule: NAMED_READS,
    });
}

#[test]
fn an_abort_disproof_is_required_at_the_call_inside_the_sum() {
    let source = "let exceeds(n:u64,d:u64):bool=n < d;
machine f(n:u64,d:u64)->u64 crashes Abort (n < d) {7u64}
const N:u64=1u64 +% f(12u64,3u64);";
    let proof_guard = source.replace("Abort (n < d)", "Abort (exceeds(n,d))");
    check(PreparationReason::AbortRouteNeedsDisproof, Case {
        layer: "L4",
        producer: "preparation",
        good: &[("sum.omg", source)],
        bad: &[("sum.omg", &proof_guard)],
        file: "sum.omg",
        occurrence: "f(12u64,3u64)",
        reason: refusal!("the selected Abort route requires a checked disproof for the supplied arguments"),
        rule: ADMISSION,
    });
}

#[test]
fn a_nested_omission_reports_the_constructor_that_requires_the_proof() {
    let good = "data Part [copy] {value:u8;}\ndata Inner [copy] {part:Part;}\ndata Outer [copy] {inner:Inner;}\nconst VALUE:Outer=Outer{inner:Inner{..}};";
    let bad = good.replace("Part [copy] {", "Part [copy] requires false {");
    check(
        PreparationReason::OmittedFieldZeroNeedsProof,
        Case {
            layer: "L4",
            producer: "preparation",
            good: &[("nested.omg", good)],
            bad: &[("nested.omg", bad.as_str())],
            file: "nested.omg",
            occurrence: "Inner{..}",
            reason: refusal!("the omitted field's zero value requires proof of the data predicate"),
            rule: FROZEN_VALUES,
        },
    );
}

#[test]
fn an_out_of_bounds_read_identifies_the_failed_subscript_in_the_initializer() {
    check(PreparationReason::ArrayIndexOutOfBounds, Case {
        layer: "L4",
        producer: "preparation",
        good: &[("array.omg", "const TABLE:[u8;2]=[4u8,5u8]; const ANSWER:u8=TABLE[0u64] +% TABLE[1u64];")],
        bad: &[("array.omg", "const TABLE:[u8;2]=[4u8,5u8]; const ANSWER:u8=TABLE[0u64] +% TABLE[2u64];")],
        file: "array.omg",
        occurrence: "TABLE[2u64]",
        reason: refusal!("selected array read requires index < items.length"),
        rule: FROZEN_VALUES,
    });
}
