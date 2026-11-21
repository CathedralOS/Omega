//! Typed termination and crash obligations joined to the Psi control flow they constrain.
//!
//! Each case lowers one source package through `obligation_outputs/pipeline.rs`:
//! the prepared obligations are copied, `sources::lower_prepared` lowers the same
//! trees and prints the package fixture row, and the prepared and resolved
//! packages are dropped before anything is inspected. Each obligation is then
//! joined to the Psi record at its own declaration or exact source site:
//!
//! - `Termination(machine)` to that machine's ranking witness at the clause, and
//!   the witness to the actual terminal call edge it claims and the successor
//!   argument on that edge;
//! - `TotalCrashGuard` to the one published route guard at its site, in its
//!   machine and route position, with the predicate over the entry formals;
//! - `CrashCoverage { machine, cause }` to the call or trapping operation at its
//!   site inside the obligation's own caller, whose callee routes or primitive
//!   produce exactly the owed causes.
//!
//! Twins change one authored choice, and each twin's expectation is written from
//! the source. Every Psi record of the inspected form must be claimed by an
//! obligation and every obligation by a record, so coexistence earns nothing.
//! These tests do not judge an obligation; L6 does.
#[allow(dead_code)]
#[path = "obligation_outputs/pipeline.rs"]
mod pipeline;
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use PredicateOperator::{Equal, NotEqual};
use PredicateStep::{Apply, Formal, Integer};
use omega_lowering::{
    BlockTerminator, CallArgument, CallSite, CallTarget, CrashCause, IntegerPrimitive, LoweredMachine, MachineBody, OperationKind, Parameter,
    PredicateOperator, PredicateStep, RankComparison, RankExpression, RankOperand, Site, TrappingIntegerPrimitive, ValueId,
};
use omega_typed::ObligationKind;
use pipeline::{Lowered, operation_at, value_producer};

/// Receipt for the L5 readiness report, printed after the checks above it pass.
fn observed(label: &str) {
    if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
        println!("OMEGA_L5_ASSERT\t{label}");
    }
}

fn scalar_formals(body: &MachineBody) -> Vec<ValueId> {
    body.parameters
        .iter()
        .map(|parameter| match parameter {
            Parameter::Scalar(value) => *value,
            other => panic!("expected scalar formals, found {other:?}"),
        })
        .collect()
}

fn edge_site(
    body: &MachineBody,
    edge: omega_lowering::EdgeId,
) -> &Site {
    &body.diagnostics.edges[edge.get() - 1]
}

fn declared(
    machine: &LoweredMachine,
    package: &str,
    path: &str,
) -> bool {
    (machine.declaration.package.as_str(), machine.declaration.path.as_str()) == (package, path)
}

/// The one direct call, ordinary or terminal, whose retained site is exactly `site`.
fn call_at<'a>(
    body: &'a MachineBody,
    site: &Site,
) -> (CallSite, CallTarget, &'a [CallArgument]) {
    let mut found = Vec::new();
    for operation in &body.operations {
        match &operation.kind {
            OperationKind::Call { callee, arguments, .. } if &body.diagnostics.operations[operation.id.index()] == site => {
                found.push((CallSite::Operation(operation.id), *callee, arguments.as_slice()))
            }
            _ => {}
        }
    }
    for block in &body.blocks {
        match &block.terminator {
            BlockTerminator::TerminalCall { id, callee, arguments, .. } if edge_site(body, *id) == site => {
                found.push((CallSite::Edge(*id), *callee, arguments.as_slice()))
            }
            _ => {}
        }
    }
    let [call] = found.as_slice() else {
        panic!("expected one call at {site:?}, found {}", found.len())
    };
    *call
}

/// Every direct call of the package that selects `target`, with its owner and site.
fn calls_of<'a>(
    lowered: &'a Lowered<'_>,
    target: omega_lowering::MachineId,
) -> Vec<(&'a LoweredMachine, &'a Site)> {
    let mut found = Vec::new();
    for machine in &lowered.package.machines {
        let body = &machine.body;
        for operation in &body.operations {
            if matches!(&operation.kind, OperationKind::Call { callee: CallTarget::Machine(id), .. } if *id == target) {
                found.push((machine, &body.diagnostics.operations[operation.id.index()]));
            }
        }
        for block in &body.blocks {
            if let BlockTerminator::TerminalCall {
                id,
                callee: CallTarget::Machine(callee),
                ..
            } = &block.terminator
                && *callee == target
            {
                found.push((machine, edge_site(body, *id)));
            }
        }
    }
    found
}

fn same_causes(
    left: &[CrashCause],
    right: &[CrashCause],
) -> bool {
    let count = |causes: &[CrashCause], cause| causes.iter().filter(|&&c| c == cause).count();
    left.len() == right.len()
        && [CrashCause::Trap, CrashCause::Abort]
            .into_iter()
            .all(|cause| count(left, cause) == count(right, cause))
}

/// An actual argument written from the source: a caller formal, or the value
/// produced by the operation spelled exactly so.
#[derive(Clone, Copy, Debug)]
enum Actual {
    Formal(usize),
    Produced(&'static str),
}

fn check_actual(
    lowered: &Lowered<'_>,
    machine: &LoweredMachine,
    value: ValueId,
    expected: Actual,
    context: &str,
) {
    match expected {
        Actual::Formal(slot) => assert_eq!(value, scalar_formals(&machine.body)[slot], "{context}: actual {expected:?}"),
        Actual::Produced(text) => {
            let producer = value_producer(&machine.body, value);
            assert_eq!(
                lowered.spelling(&machine.body.diagnostics.operations[producer.id.index()]),
                text,
                "{context}: actual {expected:?}"
            );
        }
    }
}

const RANKED: &str = "terminates by n";
const DOWN: &str = "transition n == 0 {true -> leaf(n), false -> down(n - 1),}";

/// A `Termination` obligation names the machine whose `terminates by` clause
/// owes it. Psi publishes the ranking witness at that same clause site on that
/// machine, and the witness claims exactly the terminal call edge back into the
/// component: the edge is spelled as the recursive call, its block ends in that
/// terminal call, and the successor rank is the actual argument on it. Twins move
/// the clause to the other recursive machine or to both, exchange the dispatch
/// arms, or preserve the argument instead of decrementing it. Machines without a
/// clause owe nothing and carry neither a witness nor a promise. A bare
/// `terminates` keeps only the machine's promise.
#[test]
fn termination_names_the_ranked_machine_and_the_exact_terminal_edge_it_claims() {
    // (down clause, down dispatch, skip clause,
    //  ranked machines: (path, claimed recursive call, decremented actual or the formal, comparison))
    type Claim = (&'static str, &'static str, Option<&'static str>, RankComparison);
    let cases: [(&str, &str, &str, &[Claim]); 5] = [
        (RANKED, DOWN, "", &[("down", "down(n - 1)", Some("n - 1"), RankComparison::Strict)]),
        ("", DOWN, RANKED, &[("skip", "skip(n - 2)", Some("n - 2"), RankComparison::Strict)]),
        (
            RANKED,
            DOWN,
            RANKED,
            &[
                ("down", "down(n - 1)", Some("n - 1"), RankComparison::Strict),
                ("skip", "skip(n - 2)", Some("n - 2"), RankComparison::Strict),
            ],
        ),
        (
            RANKED,
            "transition n != 0 {true -> down(n - 1), false -> leaf(n),}",
            "",
            &[("down", "down(n - 1)", Some("n - 1"), RankComparison::Strict)],
        ),
        (
            RANKED,
            "transition n == 0 {true -> leaf(n), false -> down(n),}",
            "",
            &[("down", "down(n)", None, RankComparison::Preserving)],
        ),
    ];
    for (down_clause, down_body, skip_clause, claims) in cases {
        let source = format!(
            "machine leaf(x:u64)->u64 {{x}}
             machine down(n:u64)->u64 {down_clause} {{{down_body}}}
             machine skip(n:u64)->u64 {skip_clause} {{transition n < 2 {{true -> leaf(n), false -> skip(n - 2),}}}}"
        );
        let lowered = pipeline::lower(&source);
        let obligations = lowered.obligations("Termination");
        assert_eq!(obligations.len(), claims.len(), "{source}: one obligation per clause");
        for &(path, call, decremented, comparison) in claims {
            let owned: Vec<_> = obligations
                .iter()
                .filter(|obligation| matches!(&obligation.kind, ObligationKind::Termination(owner) if owner.path == path))
                .collect();
            let [obligation] = owned.as_slice() else {
                panic!("{source}: expected one termination obligation owned by {path}, found {owned:?}")
            };
            let ObligationKind::Termination(owner) = &obligation.kind else {
                unreachable!()
            };
            assert_eq!(owner.package, pipeline::PACKAGE);
            assert_eq!(lowered.spelling(&obligation.site), RANKED, "{source}");

            let machine = lowered.machine(path);
            assert!(declared(machine, &owner.package, &owner.path));
            assert!(machine.contract.promises_termination, "{source}: {path}");
            let witness = machine
                .contract
                .ranking
                .as_ref()
                .unwrap_or_else(|| panic!("{source}: {path} publishes no ranking witness"));
            assert_eq!(witness.site, obligation.site, "{source}: the witness is the obligated clause");
            assert_eq!(witness.rank, RankExpression::Operand(RankOperand::Input(0)), "{source}");
            assert_eq!(witness.component, [machine.id], "{source}: a self-recursive component");

            let body = &machine.body;
            let [edge] = witness.edges.as_slice() else {
                panic!("{source}: {path} claims {} edges", witness.edges.len())
            };
            let omega_lowering::CallSite::Edge(call_edge) = edge.call else { panic!("expected terminal call") };
            assert_eq!(
                lowered.spelling(edge_site(body, call_edge)),
                call,
                "{source}: the claimed edge is the recursive call"
            );
            let block = body
                .blocks
                .iter()
                .find(|block| block.id == edge.block)
                .expect("the claimed block exists");
            let BlockTerminator::TerminalCall { id, callee, arguments, .. } = &block.terminator else {
                panic!("{source}: the claimed block ends in {:?}", block.terminator)
            };
            assert_eq!(
                (*id, *callee, edge.target),
                (call_edge, CallTarget::Machine(machine.id), machine.id),
                "{source}"
            );
            let [CallArgument::Scalar(actual)] = arguments[..] else {
                panic!("{source}: expected one scalar actual, found {arguments:?}")
            };
            assert_eq!(
                edge.successor,
                Some(RankExpression::Operand(RankOperand::Input(actual))),
                "{source}: the successor is the actual argument"
            );
            assert_eq!(edge.comparison, comparison, "{source}");
            let n = scalar_formals(body)[0];
            match decremented {
                Some(text) => {
                    check_actual(&lowered, machine, actual, Actual::Produced(text), &source);
                    let OperationKind::Integer { operation, operands } = &value_producer(body, actual).kind else {
                        panic!("{source}: the successor is an integer operation")
                    };
                    assert_eq!((operation.primitive, operands.first()), (IntegerPrimitive::Subtract, Some(&n)), "{source}");
                }
                None => assert_eq!(actual, n, "{source}: the formal is forwarded unchanged"),
            }
            // The exit to `leaf` stays unclaimed.
            for block in &body.blocks {
                if let BlockTerminator::TerminalCall { id, callee, .. } = &block.terminator
                    && *id != call_edge
                {
                    assert_eq!(
                        *callee,
                        CallTarget::Machine(lowered.machine("leaf").id),
                        "{source}: only the exit edge is unclaimed"
                    );
                }
            }
        }
        for path in ["leaf", "down", "skip"] {
            if claims.iter().all(|claim| claim.0 != path) {
                let machine = lowered.machine(path);
                assert!(machine.contract.ranking.is_none(), "{source}: {path} has no clause");
                assert!(!machine.contract.promises_termination, "{source}: {path} has no clause");
            }
        }
    }

    // A bare clause owes the same obligation; Psi keeps only the owner's promise.
    for (visit, idle, owner) in [
        ("terminates", "", "visit"),
        ("", "terminates", "idle"),
        ("terminates by n in 0..=9", "", "visit"),
    ] {
        let source = format!("machine visit(n:u64) {visit} {{}} machine idle(n:u64) {idle} {{}}");
        let lowered = pipeline::lower(&source);
        let obligations = lowered.obligations("Termination");
        let [obligation] = obligations.as_slice() else {
            panic!("{source}: expected one termination obligation, found {obligations:?}")
        };
        let ObligationKind::Termination(declaration) = &obligation.kind else {
            unreachable!()
        };
        assert_eq!(declaration.path, owner, "{source}");
        let clause = if owner == "visit" { visit } else { idle };
        assert_eq!(lowered.spelling(&obligation.site), clause, "{source}");
        for path in ["visit", "idle"] {
            let machine = lowered.machine(path);
            assert_eq!(machine.contract.promises_termination, path == owner, "{source}: {path}");
            match &machine.contract.ranking {
                Some(witness) => {
                    assert_eq!(
                        (path, clause),
                        (owner, "terminates by n in 0..=9"),
                        "{source}: only an authored witness is published"
                    );
                    assert_eq!(witness.site, obligation.site, "{source}");
                    assert!(witness.edges.is_empty(), "{source}: an acyclic witness claims no edge");
                }
                None => assert!(path != owner || clause == "terminates", "{source}: {path}"),
            }
        }
    }
    observed("obligation-control-flow/termination");
}

fn guard_of(
    formal: usize,
    value: i32,
    operator: PredicateOperator,
) -> Vec<PredicateStep> {
    vec![Formal(formal), Integer(value.into()), Apply(operator)]
}

/// Each `TotalCrashGuard` sits at one guard expression. Psi publishes exactly
/// one route guard at that site, on the machine and at the route position that
/// authored it, with its cause and predicate over the entry formals. Every
/// published guard has exactly one such obligation, and unguarded routes have
/// none. Twins reorder routes, guard the other formal, move the guarded route to
/// the other machine, add a second guard, or publish no route.
#[test]
fn total_crash_guard_is_the_published_guard_of_its_own_route() {
    // (first clauses, second clauses, guarded routes: (machine, route index, cause, guard, predicate))
    type Guarded = (&'static str, usize, CrashCause, &'static str, Vec<PredicateStep>);
    let cases: Vec<(&str, &str, Vec<Guarded>)> = vec![
        (
            "crashes Abort b == 0 crashes Trap",
            "",
            vec![("first", 0, CrashCause::Abort, "b == 0", guard_of(1, 0, Equal))],
        ),
        (
            "crashes Trap crashes Abort b == 0",
            "",
            vec![("first", 1, CrashCause::Abort, "b == 0", guard_of(1, 0, Equal))],
        ),
        (
            "crashes Abort a == 0 crashes Trap",
            "",
            vec![("first", 0, CrashCause::Abort, "a == 0", guard_of(0, 0, Equal))],
        ),
        (
            "crashes Trap",
            "crashes Abort b != 7",
            vec![("second", 0, CrashCause::Abort, "b != 7", guard_of(1, 7, NotEqual))],
        ),
        (
            "crashes Abort b == 0 crashes Trap a == 1",
            "",
            vec![
                ("first", 0, CrashCause::Abort, "b == 0", guard_of(1, 0, Equal)),
                ("first", 1, CrashCause::Trap, "a == 1", guard_of(0, 1, Equal)),
            ],
        ),
        ("crashes Trap", "crashes none", vec![]),
    ];
    for (first, second, guarded) in cases {
        let source = format!("machine first(a:u8, b:u8)->u8 {first} {{a}} machine second(a:u8, b:u8)->u8 {second} {{b}}");
        let lowered = pipeline::lower(&source);
        let obligations = lowered.obligations("TotalCrashGuard");
        assert_eq!(obligations.len(), guarded.len(), "{source}: one obligation per guard");

        // Every published guard is claimed by exactly one obligation at its site.
        let mut published = 0;
        for machine in &lowered.package.machines {
            for route in &machine.contract.crash_routes {
                if let Some(guard) = &route.guard {
                    published += 1;
                    let owing = obligations
                        .iter()
                        .filter(|obligation| obligation.site == guard.site)
                        .count();
                    assert_eq!(owing, 1, "{source}: guard {:?} needs exactly one obligation", lowered.spelling(&guard.site));
                }
            }
        }
        assert_eq!(published, guarded.len(), "{source}: no unobligated guard");

        for (path, index, cause, text, predicate) in &guarded {
            let at: Vec<_> = obligations
                .iter()
                .filter(|obligation| lowered.spelling(&obligation.site) == *text)
                .collect();
            let [obligation] = at.as_slice() else {
                panic!("{source}: expected one guard obligation at {text}")
            };
            let routes: Vec<_> = lowered
                .package
                .machines
                .iter()
                .flat_map(|machine| {
                    machine
                        .contract
                        .crash_routes
                        .iter()
                        .enumerate()
                        .map(move |(position, route)| (machine, position, route))
                })
                .filter(|(_, _, route)| {
                    route
                        .guard
                        .as_ref()
                        .is_some_and(|guard| guard.site == obligation.site)
                })
                .collect();
            let [(machine, position, route)] = routes.as_slice() else {
                panic!("{source}: {text} guards {} routes", routes.len())
            };
            assert!(declared(machine, pipeline::PACKAGE, path), "{source}: {text} belongs to {path}");
            assert_eq!(*position, *index, "{source}: {text} keeps its authored route position");
            assert_eq!(route.cause, *cause, "{source}: {text}");
            assert_eq!(&route.guard.as_ref().unwrap().predicate, predicate, "{source}: {text}");
            assert!(
                lowered.spelling(&route.site).ends_with(text),
                "{source}: the route clause {:?} ends with its guard",
                lowered.spelling(&route.site)
            );
        }
    }
    observed("obligation-control-flow/total-crash-guard");
}

/// Each call-site `CrashCoverage { machine, cause }` names the caller that owes
/// it, and its site is one direct call in that caller's body, ordinary or
/// terminal. That call selects the routed callee; the obligation causes at the
/// site are exactly the callee's published route causes, and the call keeps the
/// ordered actuals the routes' guards are instantiated with. Every call of the
/// routed callee is such a site. Twins nest the routed call inside or around
/// another call, swap the actuals, change or add a cause, move the call to the
/// other caller, make it a terminal edge, or publish no route.
#[test]
fn call_crash_coverage_names_its_caller_call_callee_routes_and_actuals() {
    struct Case {
        routed: &'static str,
        outer: (&'static str, &'static str),
        other: (&'static str, &'static str),
        caller: &'static str,
        call: &'static str,
        terminal: bool,
        actuals: [Actual; 2],
        causes: &'static [CrashCause],
    }
    use Actual::{Formal as F, Produced as P};
    const TRAP: &[CrashCause] = &[CrashCause::Trap];
    let trap = TRAP;
    let cases = [
        Case {
            routed: "crashes Trap y == 0",
            outer: ("crashes Trap", "routed(plain(a), b)"),
            other: ("crashes none", "plain(b)"),
            caller: "outer",
            call: "routed(plain(a), b)",
            terminal: false,
            actuals: [P("plain(a)"), F(1)],
            causes: trap,
        },
        Case {
            routed: "crashes Trap y == 0",
            outer: ("crashes Trap", "plain(routed(a, b))"),
            other: ("crashes none", "plain(b)"),
            caller: "outer",
            call: "routed(a, b)",
            terminal: false,
            actuals: [F(0), F(1)],
            causes: trap,
        },
        Case {
            routed: "crashes Trap y == 0",
            outer: ("crashes Trap", "routed(b, a)"),
            other: ("crashes none", "plain(b)"),
            caller: "outer",
            call: "routed(b, a)",
            terminal: false,
            actuals: [F(1), F(0)],
            causes: trap,
        },
        Case {
            routed: "crashes Abort y == 0",
            outer: ("crashes Abort", "routed(a, b)"),
            other: ("crashes none", "plain(b)"),
            caller: "outer",
            call: "routed(a, b)",
            terminal: false,
            actuals: [F(0), F(1)],
            causes: &[CrashCause::Abort],
        },
        Case {
            routed: "crashes Abort y == 0 crashes Trap",
            outer: ("crashes Abort crashes Trap", "routed(a, b)"),
            other: ("crashes none", "plain(b)"),
            caller: "outer",
            call: "routed(a, b)",
            terminal: false,
            actuals: [F(0), F(1)],
            causes: &[CrashCause::Abort, CrashCause::Trap],
        },
        Case {
            routed: "crashes Trap y == 0",
            outer: ("crashes none", "plain(a)"),
            other: ("crashes Trap", "routed(b, a)"),
            caller: "other",
            call: "routed(b, a)",
            terminal: false,
            actuals: [F(1), F(0)],
            causes: trap,
        },
        Case {
            routed: "crashes Trap y == 0",
            outer: ("crashes Trap", "transition {_ -> routed(a, b),}"),
            other: ("crashes none", "plain(b)"),
            caller: "outer",
            call: "routed(a, b)",
            terminal: true,
            actuals: [F(0), F(1)],
            causes: trap,
        },
        Case {
            routed: "crashes none",
            outer: ("crashes none", "routed(a, b)"),
            other: ("crashes none", "plain(b)"),
            caller: "outer",
            call: "routed(a, b)",
            terminal: false,
            actuals: [F(0), F(1)],
            causes: &[],
        },
    ];
    for case in cases {
        let source = format!(
            "machine plain(x:u8)->u8 crashes none {{x}}
             machine routed(x:u8, y:u8)->u8 {} {{x}}
             machine outer(a:u8, b:u8)->u8 {} {{{}}}
             machine other(a:u8, b:u8)->u8 {} {{{}}}",
            case.routed, case.outer.0, case.outer.1, case.other.0, case.other.1
        );
        let lowered = pipeline::lower(&source);
        let coverage = lowered.obligations("CrashCoverage");
        assert_eq!(coverage.len(), case.causes.len(), "{source}: one obligation per callee route");
        let mut owed = Vec::new();
        for obligation in &coverage {
            let ObligationKind::CrashCoverage { machine, cause } = &obligation.kind else {
                unreachable!()
            };
            assert_eq!(
                (machine.package.as_str(), machine.path.as_str()),
                (pipeline::PACKAGE, case.caller),
                "{source}: the caller owes coverage"
            );
            assert_eq!(lowered.spelling(&obligation.site), case.call, "{source}");
            owed.push(*cause);
        }
        assert!(same_causes(&owed, case.causes), "{source}: owed {owed:?}");

        let routed = lowered.machine("routed");
        let route_causes: Vec<_> = routed
            .contract
            .crash_routes
            .iter()
            .map(|route| route.cause)
            .collect();
        assert!(same_causes(&route_causes, case.causes), "{source}: callee routes {route_causes:?}");

        let caller = lowered.machine(case.caller);
        let diagnostics = &caller.body.diagnostics;
        let site = &diagnostics
            .operations
            .iter()
            .chain(&diagnostics.edges)
            .find(|site| lowered.spelling(site) == case.call)
            .expect("the caller retains the call site")
            .clone();
        if let Some(obligation) = coverage.first() {
            assert_eq!(site, &obligation.site, "{source}");
        }
        let (at, callee, arguments) = call_at(&caller.body, site);
        assert_eq!(matches!(at, CallSite::Edge(_)), case.terminal, "{source}: call form {at:?}");
        assert_eq!(callee, CallTarget::Machine(routed.id), "{source}: the covered call selects the routed callee");
        assert_eq!(arguments.len(), 2, "{source}");
        for (argument, expected) in arguments.iter().zip(case.actuals) {
            let CallArgument::Scalar(value) = *argument else {
                panic!("{source}: expected a scalar actual, found {argument:?}")
            };
            check_actual(&lowered, caller, value, expected, &source);
            if let Actual::Produced(_) = expected {
                let plain = lowered.machine("plain").id;
                assert!(
                    matches!(&value_producer(&caller.body, value).kind, OperationKind::Call { callee: CallTarget::Machine(id), .. } if *id == plain),
                    "{source}: the actual is the inner plain call"
                );
            }
        }

        // The routed callee is called only at the covered site of the owing caller.
        let calls = calls_of(&lowered, routed.id);
        let [(owner, call_site)] = calls.as_slice() else {
            panic!("{source}: routed is called {} times", calls.len())
        };
        assert_eq!((owner.id, *call_site), (caller.id, site), "{source}");
        // A callee without routes is never owed coverage.
        let plain = lowered.machine("plain");
        assert!(plain.contract.crash_routes.is_empty() && plain.contract.has_empty_crash_ceiling);
    }
    observed("obligation-control-flow/call-crash-coverage");
}

/// A trapping operator owes `CrashCoverage { machine, Trap }` at its own
/// occurrence. The Psi operation at that site in the obligation's machine is the
/// trapping operation with the authored primitive and ordered operands. Every
/// trapping operation in the package has exactly one such obligation, and an
/// exact division owes none. Twins swap operands, change the primitive, nest one
/// trapping operation inside another, or move the operation to the other machine.
#[test]
fn trap_crash_coverage_names_its_machine_and_trapping_operation() {
    use Actual::{Formal as F, Produced as P};
    use TrappingIntegerPrimitive::{Add, Divide, Multiply, Subtract};
    type Trapping = (&'static str, TrappingIntegerPrimitive, Actual, Actual);
    let cases: [(&str, &str, &str, &[Trapping]); 5] = [
        ("a /! b", "a", "apply", &[("a /! b", Divide, F(0), F(1))]),
        ("b /! a", "a", "apply", &[("b /! a", Divide, F(1), F(0))]),
        ("a -! b", "a", "apply", &[("a -! b", Subtract, F(0), F(1))]),
        (
            "(a /! b) +! a",
            "a",
            "apply",
            &[
                ("a /! b", Divide, F(0), F(1)),
                ("(a /! b) +! a", Add, P("a /! b"), F(0)),
            ],
        ),
        ("a", "b *! a", "twin", &[("b *! a", Multiply, F(1), F(0))]),
    ];
    for (apply, twin, owner, operations) in cases {
        let source = format!(
            "machine apply(a:u8, b:u8)->u8 crashes Trap {{{apply}}}
             machine twin(a:u8, b:u8)->u8 crashes Trap {{{twin}}}
             machine calm(a:u8, b:u8)->u8 requires b != 0 crashes none {{a / b}}"
        );
        let lowered = pipeline::lower(&source);
        let coverage = lowered.obligations("CrashCoverage");
        assert_eq!(coverage.len(), operations.len(), "{source}: one obligation per trapping occurrence");
        let machine = lowered.machine(owner);
        for &(text, primitive, left, right) in operations {
            let at: Vec<_> = coverage
                .iter()
                .filter(|obligation| lowered.spelling(&obligation.site) == text)
                .collect();
            let [obligation] = at.as_slice() else {
                panic!("{source}: expected one coverage obligation at {text}")
            };
            let ObligationKind::CrashCoverage { machine: declaration, cause } = &obligation.kind else {
                unreachable!()
            };
            assert_eq!(*cause, CrashCause::Trap, "{source}: {text}");
            assert!(
                declared(machine, &declaration.package, &declaration.path),
                "{source}: {text} is owed by {owner}"
            );
            let operation = operation_at(&machine.body, &obligation.site);
            let OperationKind::TrappingInteger {
                primitive: selected,
                left: first,
                right: second,
            } = operation.kind
            else {
                panic!("{source}: {text} lowers to {:?}", operation.kind)
            };
            assert_eq!(selected, primitive, "{source}: {text}");
            check_actual(&lowered, machine, first, left, &format!("{source}: {text} left"));
            check_actual(&lowered, machine, second, right, &format!("{source}: {text} right"));
        }
        // Every trapping operation is an obligated occurrence of the owner.
        let mut trapping = 0;
        for candidate in &lowered.package.machines {
            for operation in &candidate.body.operations {
                if matches!(operation.kind, OperationKind::TrappingInteger { .. }) {
                    trapping += 1;
                    assert_eq!(candidate.id, machine.id, "{source}: trapping operation outside {owner}");
                    let site = &candidate.body.diagnostics.operations[operation.id.index()];
                    assert_eq!(
                        coverage
                            .iter()
                            .filter(|obligation| &obligation.site == site)
                            .count(),
                        1,
                        "{source}"
                    );
                }
            }
        }
        assert_eq!(trapping, operations.len(), "{source}");
        let calm = lowered.machine("calm");
        let [division] = calm.body.operations.as_slice() else {
            panic!("{source}: calm has one division")
        };
        assert!(
            matches!(&division.kind, OperationKind::Integer { operation, .. } if operation.primitive == IntegerPrimitive::Divide),
            "{source}: exact division is not trapping"
        );
        assert!(calm.contract.crash_routes.is_empty());
    }
    observed("obligation-control-flow/trap-crash-coverage");
}
