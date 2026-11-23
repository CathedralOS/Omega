//! Stage 05 must write each `requires`, `ensures` and `crashes` clause as a
//! proposition over the formals and the result it names, keeping the
//! operand order, the strictness and the sign the author wrote, and the
//! lowered-form checker must judge every call and return by that
//! proposition alone. Each test reads the clause stage 05 wrote, then
//! changes the source or the lowered clause and requires the checker's
//! verdict to follow.
//!
//! The spec rules: a call proves the callee's `requires` at its own
//! arguments and its caller relies on the callee's `ensures`
//! (`wiki/spec/language/machines.md`, "Contracts"); `a != b` is false
//! exactly when `a == b` (`wiki/spec/language/expressions.md`); and a
//! guarded `crashes` clause names the cause and the formals of the machine
//! that may crash (`wiki/spec/language/effects.md`, "Guarded crashes").

#[path = "../../06_lowered-program-to-checked-program/tests/support/source_handoff.rs"]
mod program;

use program::Program;
use semantic_vocabulary::{IntegerValue, Proposition, ScalarTerm, ValueId};
use terminal_psi::{CrashRouteBucket, CrashRouteGuard, OperationKind, TerminalMachine};

fn formals(machine: &TerminalMachine) -> Vec<ValueId> {
    machine
        .contract
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect()
}

/// The value a scalar term reads, or the signed or unsigned integer it is.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Term {
    Value(ValueId),
    Integer(IntegerValue),
    Boolean(bool),
}

fn term(term: &ScalarTerm) -> Term {
    match term {
        ScalarTerm::Value { id, .. } => Term::Value(*id),
        ScalarTerm::Integer { value, .. } => Term::Integer(*value),
        ScalarTerm::Boolean(value) => Term::Boolean(*value),
        other => panic!("a clause operand that is not a value or a literal: {other:?}"),
    }
}

/// A comparison clause as `(operator, left, right)`.
fn comparison(proposition: &Proposition) -> (&'static str, Term, Term) {
    match proposition {
        Proposition::LessThan(left, right) => ("<", term(left), term(right)),
        Proposition::LessOrEqual(left, right) => ("<=", term(left), term(right)),
        Proposition::Equal(left, right) => ("==", term(left), term(right)),
        other => panic!("not a comparison: {other:?}"),
    }
}

fn result(machine: &TerminalMachine) -> ValueId {
    machine
        .contract
        .result
        .scalar()
        .expect("a scalar result")
        .id
}

const ORDERED: &str = "machine forward(first: u8, second: u8) -> u8 \
        requires first < second ensures (result < second) { first } \
    machine swapped(first: u8, second: u8) -> u8 \
        requires second < first ensures (result < first) { second } \
    machine inclusive(first: u8, second: u8) -> u8 \
        requires first <= second ensures (result <= second) { first }";

/// Each clause names the formals by the position the author wrote and
/// keeps `<` apart from `<=`, and `result` is the machine's result value.
/// `swapped` writes the same comparison with the formals exchanged.
#[test]
fn a_clause_names_its_formals_and_result_in_the_order_written() {
    let program = Program::of(ORDERED);
    let lowered = program.lower();
    for (path, requires, ensures) in [
        ("forward", ("<", 0, 1), ("<", 1)),
        ("swapped", ("<", 1, 0), ("<", 0)),
        ("inclusive", ("<=", 0, 1), ("<=", 1)),
    ] {
        let machine = lowered.machine(path);
        let formal = formals(machine);
        let [requirement] = machine.contract.requires.as_slice() else {
            panic!("{path}: one requirement")
        };
        assert_eq!(
            comparison(requirement),
            (
                requires.0,
                Term::Value(formal[requires.1]),
                Term::Value(formal[requires.2])
            ),
            "{path}"
        );
        let [guarantee] = machine.contract.ensures.as_slice() else {
            panic!("{path}: one guarantee")
        };
        assert_eq!(
            comparison(&guarantee.proposition),
            (
                ensures.0,
                Term::Value(result(machine)),
                Term::Value(formal[ensures.1])
            ),
            "{path}"
        );
    }
    program.judged(|_| {}).accepted();
}

fn forward_caller(arguments: &str) -> Program {
    Program::of(&format!(
        "machine forward(first: u8, second: u8) -> u8 requires first < second {{ first }} \
         machine caller() -> u8 {{ forward({arguments}) }}"
    ))
}

/// `forward(1u8, 2u8)` meets `first < second`; `forward(2u8, 1u8)` does not.
/// Exchanging the clause's two operands in the lowered program, as a stage
/// 05 that read the operands in the wrong order would, rejects the call
/// that meets the written clause, exactly as the source twin is rejected.
#[test]
fn a_call_is_judged_by_the_clause_in_its_written_order() {
    let program = forward_caller("1u8, 2u8");
    program.judged(|_| {}).accepted();
    let twin = forward_caller("2u8, 1u8").judged(|_| {}).rejected();
    assert_eq!(
        twin.unproved,
        ["CallRequirement { invocation: InvocationId(1), clause: 0 }"],
        "{twin:?}"
    );
    let exchanged = program
        .judged_in("forward", |machine| {
            let Proposition::LessThan(left, right) = &mut machine.contract.requires[0] else {
                unreachable!()
            };
            std::mem::swap(left, right);
        })
        .rejected();
    assert_eq!(exchanged.unproved, twin.unproved, "{exchanged:?}");
}

/// A negative bound keeps its sign whether it is grouped or not, and a
/// folded `1 - 2` lands as the one constant -1 of the formal's type, with
/// or without parentheses.
#[test]
fn a_negative_or_folded_bound_keeps_its_signed_value() {
    let program = Program::of(
        "public machine bounded(value: i8) -> i8 requires value >= (-128) \
             ensures result >= -1 { 0i8 } \
         machine grouped(x: i8) -> bool requires x > (1 - 2) { true } \
         machine plain(x: i8) -> bool requires x > 1 - 2 { true }",
    );
    let lowered = program.lower();
    let bounded = lowered.machine("bounded");
    assert_eq!(
        comparison(&bounded.contract.requires[0]),
        (
            "<=",
            Term::Integer(IntegerValue::Signed(-128)),
            Term::Value(formals(bounded)[0])
        )
    );
    assert_eq!(
        comparison(&bounded.contract.ensures[0].proposition),
        (
            "<=",
            Term::Integer(IntegerValue::Signed(-1)),
            Term::Value(result(bounded))
        )
    );
    for path in ["grouped", "plain"] {
        let machine = lowered.machine(path);
        assert_eq!(
            comparison(&machine.contract.requires[0]),
            (
                "<",
                Term::Integer(IntegerValue::Signed(-1)),
                Term::Value(formals(machine)[0])
            ),
            "{path}"
        );
    }
    program.judged(|_| {}).accepted();
}

/// An integer `a != b` is `a < b or b < a`, and `a == b` stays an equality.
/// A Boolean has no order to split on, so `result != false` is the negation
/// `!(result == false)`, the same clause the negation spells. The checker
/// proves `ensures result != false` for a body that returns `true` and
/// refuses it for one that returns `false`.
#[test]
fn equality_and_disequality_keep_their_own_meaning() {
    let program = Program::of(
        "machine equal(a: u8, b: u8) -> bool requires a == b ensures result == false { false } \
         machine unequal(a: u8, b: u8) -> bool requires a != b ensures result != false { true } \
         machine negated(a: u8, b: u8) -> bool ensures !(result == false) { true }",
    );
    let lowered = program.lower();
    let equal = lowered.machine("equal");
    let [a, b] = formals(equal)[..] else {
        panic!("two formals")
    };
    assert_eq!(
        comparison(&equal.contract.requires[0]),
        ("==", Term::Value(a), Term::Value(b))
    );
    assert_eq!(
        comparison(&equal.contract.ensures[0].proposition),
        ("==", Term::Value(result(equal)), Term::Boolean(false))
    );

    let unequal = lowered.machine("unequal");
    let [a, b] = formals(unequal)[..] else {
        panic!("two formals")
    };
    let Proposition::Disjunction(either) = &unequal.contract.requires[0] else {
        panic!("{:?}", unequal.contract.requires)
    };
    let either: Vec<_> = either.iter().map(comparison).collect();
    assert_eq!(
        either,
        [
            ("<", Term::Value(a), Term::Value(b)),
            ("<", Term::Value(b), Term::Value(a)),
        ]
    );
    let Proposition::Implication {
        premise,
        conclusion,
    } = &unequal.contract.ensures[0].proposition
    else {
        panic!("{:?}", unequal.contract.ensures)
    };
    assert_eq!(**conclusion, Proposition::Falsehood);
    assert_eq!(
        comparison(premise),
        ("==", Term::Value(result(unequal)), Term::Boolean(false))
    );
    assert_eq!(
        unequal.contract.ensures[0].proposition,
        lowered.machine("negated").contract.ensures[0].proposition
    );
    program.judged(|_| {}).accepted();

    let returns_false = Program::of(
        "machine unequal(a: u8, b: u8) -> bool requires a != b ensures result != false { false }",
    )
    .judged(|_| {})
    .rejected();
    assert_eq!(returns_false.unproved.len(), 1, "{returns_false:?}");
    assert!(
        returns_false.unproved[0].starts_with("NormalGuarantee {"),
        "{returns_false:?}"
    );
}

fn guarded_by(body: &str) -> Program {
    Program::of(&format!(
        "machine predicate() -> bool reaches none crashes none terminates {{ {body} }} \
         machine guarded() requires predicate() {{}} \
         machine caller() {{ guarded(); }}"
    ))
}

/// `requires predicate()` calls a machine with no arguments that always
/// terminates and cannot crash, so stage 05 replaces the call with the value
/// it returns: `true` makes the clause hold and the caller is accepted;
/// `false` makes it fail and the call is rejected.
#[test]
fn a_parameterless_predicate_clause_is_the_value_its_machine_returns() {
    let holds = guarded_by("true");
    assert_eq!(
        holds.lower().machine("guarded").contract.requires,
        [Proposition::Truth]
    );
    holds.judged(|_| {}).accepted();

    let fails = guarded_by("false");
    assert_eq!(
        fails.lower().machine("guarded").contract.requires,
        [Proposition::Falsehood]
    );
    let rejection = fails.judged(|_| {}).rejected();
    assert_eq!(
        rejection.unproved,
        ["CallRequirement { invocation: InvocationId(1), clause: 0 }"],
        "{rejection:?}"
    );
}

/// The routes as `(cause, guards)`, each guard either `"always"` or the
/// formal it compares with zero.
fn routes(machine: &TerminalMachine) -> Vec<(String, Vec<Option<ValueId>>)> {
    machine
        .contract
        .crash_routes
        .iter()
        .map(
            |CrashRouteBucket {
                 cause,
                 alternatives,
             }| {
                (
                    format!("{cause:?}"),
                    alternatives
                        .iter()
                        .map(|guard| match guard {
                            CrashRouteGuard::Truth => None,
                            CrashRouteGuard::Predicate(predicate) => {
                                let Proposition::Equal(ScalarTerm::Value { id, .. }, _) =
                                    predicate.proposition()
                                else {
                                    panic!("{predicate:?}")
                                };
                                Some(*id)
                            }
                        })
                        .collect(),
                )
            },
        )
        .collect()
}

/// A route without a guard may always be taken. Routes are grouped by
/// cause, so writing `crashes Trap` before or after `crashes Abort b == 0`
/// publishes the same contract, and `crashes none` publishes no route.
#[test]
fn routes_are_kept_per_cause_whatever_order_they_are_written_in() {
    let program = Program::of(
        "machine pick(a: u8, b: u8) -> u8 crashes Abort b == 0 crashes Trap { a } \
         machine swapped(a: u8, b: u8) -> u8 crashes Trap crashes Abort b == 0 { a } \
         machine never(a: u8, b: u8) -> u8 crashes none { a }",
    );
    let lowered = program.lower();
    let pick = lowered.machine("pick");
    let b = formals(pick)[1];
    assert_eq!(
        routes(pick),
        [
            ("Trap".to_owned(), vec![None]),
            ("Abort".to_owned(), vec![Some(b)])
        ]
    );
    assert_eq!(
        lowered.machine("swapped").contract.crash_routes,
        pick.contract.crash_routes
    );
    let never = lowered.machine("never");
    assert!(never.contract.crash_routes.is_empty());
    assert_eq!(never.contract.declared_crash_routes, Some(Vec::new()));
    program.judged(|_| {}).accepted();
}

fn generic(ceiling: &str) -> Program {
    Program::of(&format!(
        "machine keep<T>(value: T, count: u8) -> T crashes Trap count == 0 {{ value }} \
         machine run(x: u8, n: u8) -> u8 {ceiling} {{ keep::<u8>(x, n) }}"
    ))
}

/// `keep::<u8>(x, n)` calls the `u8` instance of `keep`, which keeps the
/// route its declaration wrote over its own second formal, and passes `x`
/// and `n` in that order. `run` must cover the route: `crashes Trap`
/// does, and `crashes none` is rejected at the call.
#[test]
fn a_generic_instance_keeps_the_route_of_its_declaration() {
    let program = generic("crashes Trap");
    let lowered = program.lower();
    let run = lowered.machine("run");
    let [(_, call)] = lowered.operations("run")[..] else {
        panic!("one call")
    };
    let OperationKind::Call {
        callee, arguments, ..
    } = &call.kind
    else {
        panic!("{call:?}")
    };
    assert_eq!(arguments.as_slice(), formals(run));
    let instance = lowered
        .elaboration
        .module
        .machines
        .iter()
        .find(|machine| machine.id == *callee)
        .unwrap();
    assert_eq!(
        routes(instance),
        [("Trap".to_owned(), vec![Some(formals(instance)[1])])]
    );
    program.judged(|_| {}).accepted();

    let uncovered = generic("crashes none").judged(|_| {}).rejected();
    assert_eq!(
        uncovered.unproved,
        ["CrashCoverage { site: Operation(OperationId(1)), cause: Trap }"],
        "{uncovered:?}"
    );
}

fn member(route: &str) -> Program {
    Program::of(&format!(
        "trait Step {{ machine next(v: u8) -> u8; }} \
         machine risky(v: u8) -> u8 {route} {{ v }} \
         Local satisfies Step {{ Step::next = risky; }} \
         machine use_step<const E: Step>(n: u8) -> u8 {{ E::next(n) }} \
         machine outer(a: u8) -> u8 crashes none {{ use_step::<Local>(a) }}"
    ))
}

/// `outer` promises `crashes none` and reaches `risky` only through the
/// conformance `Local` supplies for `Step::next`. Without a route on
/// `risky` the program is accepted; with `crashes Trap v == 0` the
/// selected member may trap on `outer`'s own argument, and the call is
/// rejected.
#[test]
fn a_conformance_member_brings_the_route_of_the_machine_it_selects() {
    member("").judged(|_| {}).accepted();
    let routed = member("crashes Trap v == 0").judged(|_| {}).rejected();
    assert_eq!(
        routed.unproved,
        ["CrashCoverage { site: Operation(OperationId(1)), cause: Trap }"],
        "{routed:?}"
    );
}
