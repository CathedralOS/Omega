//! The lowered-form checker can be shown a source program without running
//! the program's Build and without the old checker
//! (`CompilationSession::typed_frontier_without_build`, the harness behind
//! `examples/route_probe.rs`). Two standalone programs pin it: one the
//! checker accepts, and the same program with a machine nothing calls whose
//! addition can overflow, which it rejects because a standalone source is its
//! own validation subject.
//!
//! A program that imports another package is judged with the dependency
//! edges its Build registers, stated by the caller. The kept program that
//! prints through the standard library's `Console` pins that, with the same
//! program refused at its import when the edge is left out.

#[path = "support/unbuilt_frontier.rs"]
mod unbuilt_frontier;

use terminal_psi_to_terminal_psi::RouteVerdict;

// Compilation shares process-wide counters; run one at a time.
static COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Every construct here is in the checker's scalar subset: a guarded
/// transition, a direct call whose `requires` the caller establishes, and
/// exact arithmetic the `requires` keeps in range.
const ACCEPTED: &str = "\
machine scaled(value: u32, factor: u32) -> u32 requires value <= 1000 && factor <= 10 {
    value * factor + 1
}
machine settle(total: u32) {
    transition total > 9000 {
        true -> done()
        false -> done()
    }
}
machine done() { }
machine main() {
    let total: u32 = 20;
    let product: u32 = scaled(total, 3);
    settle(product)
}
";

/// `main` never calls `unchecked`, whose `value + 1` overflows for the
/// largest `u32`.
const UNCALLED_OVERFLOW: &str = "\
machine unchecked(value: u32) -> u32 {
    value + 1
}
machine main() { }
";

/// One word per judged part of `source`, staged as a standalone `main.omg`.
fn verdicts(name: &str, source: &str) -> Vec<&'static str> {
    judged(name, source)
        .iter()
        .map(|verdict| match verdict {
            Ok(RouteVerdict::Accepted(_)) => "accepted",
            Ok(RouteVerdict::Rejected { .. }) => "rejected",
            Ok(RouteVerdict::Incomplete(_)) => "incomplete",
            Ok(RouteVerdict::FallBack(_)) => "not-supported",
            Err(_) => "defect",
        })
        .collect()
}

/// The route's verdict on each part of `source`, staged as a standalone
/// `main.omg`: the entry closure first.
fn judged(
    name: &str,
    source: &str,
) -> Vec<Result<RouteVerdict, terminal_psi_to_terminal_psi::CompilationRefusal>> {
    let _compilation = COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let frontier = unbuilt_frontier::staged_frontier(name, source);
    assert_eq!(
        frontier.entries.len(),
        1,
        "{name}: `main` is the bound entry"
    );
    judge(&frontier)
}

fn judge(
    frontier: &compiler::TypedFrontier,
) -> Vec<Result<RouteVerdict, terminal_psi_to_terminal_psi::CompilationRefusal>> {
    terminal_psi_to_terminal_psi::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &frontier.typed,
            typed_trees_to_terminal_psi::Compilation {
                entries: &frontier.entries,
                subjects: &[frontier.subject],
                purpose: source::DependencyScope::Product,
                selected: &frontier.selected,
                bindings: &frontier.bindings,
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Portable,
        )
        .program,
    )
}

#[test]
fn a_standalone_program_is_judged_without_its_build() {
    assert_eq!(verdicts("accepted", ACCEPTED), ["accepted"]);
}

/// A suffixed operator names its policy at one operation
/// (`wiki/spec/language/numeric_values.md`, "Policy operators"): `a +% b` on
/// two `u32` values "is exactly what `+` means for two `u32 in Wrapping`
/// operands", and its result is a plain `u32`. Each program is one machine
/// nothing calls beside an empty `main`.
///
/// The Wrapping and Saturating adds are total, so both are accepted. A
/// policy "does not license a zero divisor": `/%` with nothing known about
/// the divisor is rejected. A suffix excuses no other operation: the bare
/// `+` beside a `+%` is still Exact and still owes its range. `+!` is its own
/// crash site, which no type on this route carries yet: refused by name.
#[test]
fn an_operator_suffix_chooses_the_policy_of_one_operation() {
    let spare = |body: &str| {
        format!("machine spare(left: u32, right: u32) -> u32 {{ {body} }}\nmachine main() {{ }}\n")
    };
    for (name, body, expected) in [
        ("wrapping_add", "left +% right", "accepted"),
        ("wrapping_literal", "left *% 31 +% 1", "accepted"),
        ("saturating_add", "left +| right", "accepted"),
        ("wrapping_divide", "left /% right", "rejected"),
        (
            "exact_beside_wrapping",
            "let wrapped: u32 = left +% right; left + right",
            "rejected",
        ),
        ("trapping_add", "left +! right", "not-supported"),
    ] {
        assert_eq!(verdicts(name, &spare(body)), [expected], "{name}");
    }
}

/// A suffixed operator in `requires` or `ensures` means what it means in a
/// body (`wiki/spec/language/domains.md`, "Operands": "The rule is the same
/// in executable code, `requires` and `ensures`"), and the Wrapping and
/// Saturating add, subtract and multiply are total, so each forms a term
/// (`wiki/spec/proofs/contracts.md`, "Total arithmetic").
///
/// A guarantee that states the Wrapping sum is established by a body that
/// computes it. A body that computes the Saturating sum does not establish
/// it, and a caller that knows nothing of its arguments does not establish a
/// `requires` over a Wrapping sum: the term is never read as the other
/// policy's or as Exact arithmetic. Neither failing program is `rejected`:
/// the prover has no rule that says what a Wrapping or Saturating operation
/// computes, so a goal over one that has no derivation is `incomplete`, as a
/// goal over a value a body computes by one already is.
///
/// Refused by name: `+!`, which "can transfer control" and "cannot form a
/// proof term"; `/%`, which still owes its divisor before it is total; and
/// an operand whose type carries a policy, since both operands of a suffixed
/// operator are unqualified.
#[test]
fn a_suffixed_operator_in_a_contract_is_the_operation_a_body_computes() {
    let pair = |contract: &str, body: &str| {
        format!(
            "machine spare(left: u32, right: u32) -> u32 {contract} {{ {body} }}
             machine main() {{ }}
"
        )
    };
    let gate = |clause: &str, caller: &str| {
        format!(
            "machine gate(left: u32, right: u32) requires {clause} {{ }}
             {caller}
"
        )
    };
    let mut wrong = Vec::new();
    for (name, source, expected) in [
        (
            "ensures_wrapping_sum",
            pair("ensures result == left +% right", "left +% right"),
            "accepted",
        ),
        (
            "ensures_wrapping_sum_of_saturating_body",
            pair("ensures result == left +% right", "left +| right"),
            "incomplete",
        ),
        (
            "ensures_saturating_product",
            pair("ensures result == left *| right", "left *| right"),
            "accepted",
        ),
        (
            "requires_wrapping_sum_established",
            gate(
                "left +% right == right +% left",
                "machine main() { gate(3, 4) }",
            ),
            "accepted",
        ),
        (
            "requires_wrapping_sum_unestablished",
            gate(
                "left +% right == 8",
                "machine spare(left: u32, right: u32) { gate(left, right) }
machine main() { }",
            ),
            "incomplete",
        ),
        (
            "ensures_trapping_sum",
            pair("ensures result == left +! right", "left +% right"),
            "rejected",
        ),
        (
            "ensures_wrapping_quotient",
            pair(
                "requires right > 0 ensures result == left /% right",
                "left /% right",
            ),
            "contract policy operator in a clause term that owes a condition before it is total",
        ),
        (
            "requires_policy_qualified_operand",
            "machine spare(left: u32 in Wrapping, right: u32) requires left +% right == right { }
             machine main() { }
"
            .to_string(),
            "contract policy operator on an operand that is not an unqualified integer",
        ),
    ] {
        // A refusal is compared with the reason it names.
        let judged: Vec<String> = judged(name, &source)
            .iter()
            .map(|verdict| match verdict {
                Ok(RouteVerdict::Accepted(_)) => "accepted".to_string(),
                Ok(RouteVerdict::Rejected { .. }) => "rejected".to_string(),
                Ok(RouteVerdict::Incomplete(_)) => "incomplete".to_string(),
                Ok(RouteVerdict::FallBack(refusal)) => refusal.detail.clone(),
                Err(_) => "defect".to_string(),
            })
            .collect();
        if judged != [expected] {
            wrong.push(format!("{name}: {judged:?}, expected {expected}"));
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// A sum's `requires` may bound each case's payload: `(self in
/// Packet::Alpha) && self.v >= 40 || (self in Packet::Beta) && self.v <= 5`
/// (`wiki/spec/language/data_and_literals.md`: "Field and case-payload
/// relationships belong to the data declaration's ordinary `requires`;
/// case-dependent projections require established case knowledge"). Both
/// cases name their payload `v`; under each case test `self.v` is that
/// case's field.
///
/// Constructing `Beta` within `Beta`'s bound is accepted. The twins differ
/// only in the constructed value: `Beta` at a value only `Alpha` admits, and
/// `Alpha` at a value only `Beta` admits, are each rejected.
#[test]
fn a_sum_bounds_each_cases_payload_under_that_case() {
    let program = |value: &str| {
        format!(
            "data Packet [copy]\n\
             requires ((self in Packet::Alpha) && self.v >= 40u64 && self.v <= 80u64) \
             || ((self in Packet::Beta) && self.v <= 5u64)\n\
             {{\n    case Alpha(v: u64);\n    case Beta(v: u64);\n}}\n\
             machine main() {{\n    let p: Packet = {value};\n}}\n"
        )
    };
    for (name, value, expected) in [
        ("beta_within", "Packet::Beta { v: 3u64 }", "accepted"),
        ("alpha_within", "Packet::Alpha { v: 60u64 }", "accepted"),
        (
            "beta_at_alphas_value",
            "Packet::Beta { v: 60u64 }",
            "rejected",
        ),
        (
            "alpha_at_betas_value",
            "Packet::Alpha { v: 3u64 }",
            "rejected",
        ),
    ] {
        assert_eq!(verdicts(name, &program(value)), [expected], "{name}");
    }
}

/// A machine with a value parameter, `clamp<const Limit: u64>`, applied at a
/// literal is judged as the machine typed preparation generates for that
/// value (`wiki/spec/language/generics.md`, "Value binders and const
/// requirements"): its contract and body with the literal where `Limit`
/// stood. `clamp<7>` and `clamp<200>` are two machines, so `150` is within
/// one and `5` within the other, and what `clamp<7>` guarantees keeps
/// `7 - small` exact.
///
/// The twin differs only in one argument: `clamp<7>(9)` breaks `9 <= 7`.
#[test]
fn a_value_parameter_application_is_judged_at_its_value() {
    let program = |small: &str| {
        format!(
            "machine clamp<const Limit: u64>(input: u64) -> u64\n\
             requires input <= Limit\n\
             ensures result <= Limit\n\
             {{ input }}\n\
             machine main() {{\n    \
             let small: u64 = clamp<7>({small});\n    \
             let room: u64 = 7u64 - small;\n    \
             let large: u64 = clamp<200>(150);\n}}\n"
        )
    };
    // The entry closure, then `clamp` checked once with `Limit` rigid.
    assert_eq!(verdicts("within", &program("5")), ["accepted", "accepted"]);
    assert_eq!(verdicts("beyond", &program("9")), ["rejected", "accepted"]);
}

#[test]
fn an_uncalled_defect_of_a_standalone_program_is_rejected() {
    assert_eq!(
        verdicts("uncalled_overflow", UNCALLED_OVERFLOW),
        ["rejected"]
    );
}

/// The source of a kept fixture program, read from the repository.
fn fixture(directory: &str) -> String {
    let main = unbuilt_frontier::repository(directory).join("main.omg");
    std::fs::read_to_string(&main).unwrap_or_else(|error| panic!("{}: {error}", main.display()))
}

/// `main` calls `relay<u8>` and `relay<bool>`, and `relay<T>` calls
/// `forward<T>`. Typed preparation generates the four machines those
/// applications select, so the entry closure is closed and judged; the two
/// generic machines are each judged once as written.
#[test]
fn a_program_applying_a_generic_machine_at_types_is_judged() {
    assert_eq!(
        verdicts(
            "type_parameter",
            &fixture("tests/fixtures/run/generics/type_parameter_contract")
        ),
        ["accepted", "accepted", "accepted"]
    );
}

/// Its twin differs only in the bound `relay` promises, which `forward`'s
/// contract does not give: `relay` is rejected as written, and so is the
/// entry closure holding the machines generated from it. `forward` stands.
#[test]
fn a_generic_machine_that_exceeds_its_callee_is_rejected_where_it_is_applied() {
    assert_eq!(
        verdicts(
            "type_parameter_exceeds",
            &fixture("tests/fixtures/fail/generics/type_parameter_contract_exceeds_callee")
        ),
        ["rejected", "accepted", "rejected"]
    );
}

/// A finite family: `pick<T>` admits `u8` and `u16` and is applied at both.
/// Each generated machine is the body under its own type, and each listed
/// type is judged once as an alternative of the generic machine.
#[test]
fn a_program_applying_a_finite_family_is_judged() {
    assert_eq!(
        verdicts(
            "finite_family",
            &fixture("tests/fixtures/run/generics/finite_family_contract")
        ),
        ["accepted", "accepted", "accepted"]
    );
}

/// Its twin differs only in what `pick` returns at `u16`: that alternative
/// and the entry closure holding `pick` at `u16` are rejected, and the `u8`
/// alternative stands.
#[test]
fn a_finite_family_alternative_that_breaks_its_contract_is_rejected() {
    assert_eq!(
        verdicts(
            "finite_family_wrong_alternative",
            &fixture("tests/fixtures/fail/generics/finite_family_contract_wrong_alternative")
        ),
        ["rejected", "accepted", "rejected"]
    );
}

/// `advance<Selected satisfies Step>` is applied at `Hold` and at `Shift`.
/// Typed preparation generates one machine per implementation, in which the
/// call through `Selected` enters that implementation's `apply`. Four parts
/// are judged: the entry closure holding both generated machines, `advance`
/// once as written against `Step::apply`'s contract, and for each
/// implementation that its `apply` refines `Step::apply`.
#[test]
fn a_program_applying_a_generic_machine_at_implementations_is_judged() {
    let parts = judged(
        "implementation_parameter",
        &fixture("tests/fixtures/run/generics/implementation_parameter_contract"),
    );
    let [Ok(RouteVerdict::Accepted(closure)), form, hold, shift] = &parts[..] else {
        panic!("four parts, the entry closure accepted: {parts:?}");
    };
    for part in [form, hold, shift] {
        assert!(matches!(part, Ok(RouteVerdict::Accepted(_))), "{part:?}");
    }
    // Each generated machine states the generic machine it came from and
    // commits to its own implementation, so the two do not commit alike.
    assert!(closure.elaboration.undeclared.is_empty());
    let commitments = closure
        .elaboration
        .module
        .machines
        .iter()
        .filter_map(|machine| machine.declaration.as_ref()?.specialization.as_ref())
        .map(|specialization| specialization.commitment)
        .collect::<Vec<_>>();
    let [at_hold, at_shift] = commitments[..] else {
        panic!("two generated machines: {commitments:?}");
    };
    assert_ne!(at_hold, at_shift);
}

/// Its twin differs only in the bound `advance` promises, 18, which holds
/// for both selections and not for `Step::apply`'s contract. The closure
/// and both implementations stand; `advance` as written is rejected, so the
/// selections a program makes do not make a generic body valid.
#[test]
fn a_generic_machine_valid_only_for_its_selections_is_rejected_as_written() {
    assert_eq!(
        verdicts(
            "implementation_parameter_only_selection",
            &fixture(
                "tests/fixtures/fail/generics/implementation_parameter_contract_only_holds_for_selection"
            )
        ),
        ["accepted", "rejected", "accepted", "accepted"]
    );
}

/// The same program where `Shift::apply` promises `result <= 25`, more than
/// `Step::apply`'s 20 allows. `advance` as written and `Hold` stand. The
/// witness that `Shift::apply` refines `Step::apply` is rejected, and so is
/// the closure: `advance` at `Shift` can no longer take the proof of the
/// generic body, and its own body returns up to 26.
#[test]
fn an_implementation_that_promises_less_than_its_requirement_is_rejected() {
    let source = fixture("tests/fixtures/run/generics/implementation_parameter_contract");
    let weaker = source.replace("ensures result <= 17", "ensures result <= 25");
    assert_ne!(source, weaker, "the fixture states Shift's promise");
    assert_eq!(
        verdicts("implementation_exceeds_requirement", &weaker),
        ["rejected", "accepted", "accepted", "rejected"]
    );
}

/// The kept program in `directory`, judged where it stands with `entry`
/// bound and the Product dependencies `(alias, directory)` its Build
/// registers. `Err` holds the messages that stopped it before the checker.
fn judged_with_dependencies(
    directory: &str,
    entry: &str,
    dependencies: &[(&str, &str)],
) -> Result<Vec<Result<RouteVerdict, terminal_psi_to_terminal_psi::CompilationRefusal>>, Vec<String>>
{
    let _compilation = COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    unbuilt_frontier::frontier(
        &unbuilt_frontier::repository(directory).join("main.omg"),
        Some(entry),
        dependencies,
    )
    .map(|frontier| judge(&frontier))
}

const CONSOLE_PROGRAM: &str = "tests/fixtures/run/providers/default_console_provider_reached";

/// The program writes two bytes and exits through `Console`, which its
/// Build reaches by registering the standard library as a dependency. With
/// that edge stated, the import selects the boundary trait, each call is
/// judged against its requirement's written contract, and the entry closure
/// is accepted.
#[test]
fn a_program_reaching_console_through_its_dependency_is_judged() {
    let parts = judged_with_dependencies(
        CONSOLE_PROGRAM,
        "entry_start",
        &[("omega_language_std", "source/library/std")],
    )
    .unwrap_or_else(|stopped| panic!("Psi 00 through 04 must complete: {stopped:?}"));
    assert!(
        matches!(&parts[..], [Ok(RouteVerdict::Accepted(_))]),
        "{parts:?}"
    );
}

/// The same program with no edge stated. Its first import segment is then no
/// package alias, so the path is looked up in the program's own package,
/// where nothing is named `omega_language_std::Console`.
#[test]
fn an_import_without_its_dependency_edge_selects_nothing() {
    let stopped = judged_with_dependencies(CONSOLE_PROGRAM, "entry_start", &[])
        .expect_err("the import has no package to select from");
    assert!(
        stopped.iter().any(|message| message
            .contains("import `omega_language_std::Console` does not select one declaration")),
        "{stopped:?}"
    );
}

const EXIT_PROGRAM: &str = "tests/fixtures/run/calls/process_exit_conditional_helper";

/// The verdict word of each judged part of the kept program in `directory`,
/// with `entry_start` bound and no dependency: core is bundled.
fn exit_verdicts(directory: &str) -> Vec<&'static str> {
    judged_with_dependencies(directory, "entry_start", &[])
        .unwrap_or_else(|stopped| panic!("Psi 00 through 04 must complete: {stopped:?}"))
        .iter()
        .map(|verdict| match verdict {
            Ok(RouteVerdict::Accepted(_)) => "accepted",
            Ok(RouteVerdict::Rejected { .. }) => "rejected",
            Ok(RouteVerdict::Incomplete(_)) => "incomplete",
            Ok(RouteVerdict::FallBack(_)) => "not-supported",
            Err(_) => "defect",
        })
        .collect()
}

/// The program ends through `ProcessExit::exit_process`, the boundary
/// requirement that has no normal return. The call is the last thing its
/// block does: `main` never returns, so the storage the entry lends it is
/// carried into that call, and the helper that exits on one arm owes its
/// guarantee on the other arm alone.
#[test]
fn a_program_that_ends_through_process_exit_is_judged() {
    let parts = judged_with_dependencies(EXIT_PROGRAM, "entry_start", &[])
        .unwrap_or_else(|stopped| panic!("Psi 00 through 04 must complete: {stopped:?}"));
    let [Ok(RouteVerdict::Accepted(closure))] = &parts[..] else {
        panic!("the entry closure accepted: {parts:?}");
    };
    // Each exit is a terminator that names the requirement, never a call
    // operation with a continuation, and the requirement's declaration
    // states the same completion.
    let module = &closure.elaboration.module;
    let exits = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .filter(|block| {
            matches!(
                block.terminator,
                terminal_psi::Terminator::ExternalCompletion {
                    kind: semantic_vocabulary::ExternalCompletionKind::ProcessExit,
                    ..
                }
            )
        })
        .count();
    assert_eq!(exits, 2, "`require_success` and `main` each exit once");
    assert!(module.boundary_machines.iter().all(|boundary| {
        boundary.completion
            == semantic_vocabulary::BoundaryCompletion::External(
                semantic_vocabulary::ExternalCompletionKind::ProcessExit,
            )
    }));
    assert!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .all(|operation| !matches!(
                operation.kind,
                terminal_psi::OperationKind::BoundaryCall { .. }
            )),
        "no exit is lowered as a call that returns"
    );
}

/// Its twin differs only in `main`'s ceiling, `reaches none`. The exit is
/// charged its requirement's `reaches ProcessExit` where every call is, so
/// the program is rejected.
#[test]
fn a_process_exit_outside_the_written_reach_is_rejected() {
    assert_eq!(
        exit_verdicts("tests/fixtures/fail/calls/process_exit_outside_reach"),
        ["rejected"]
    );
}

/// Its twin differs only in the helper's guard, so the arm that returns is
/// the one where the status is not zero. An arm that exits takes nothing
/// away from the arm that returns: that arm still owes `ensures status == 0`,
/// and the program is rejected.
#[test]
fn a_returning_arm_beside_a_process_exit_still_owes_its_guarantee() {
    assert_eq!(
        exit_verdicts("tests/fixtures/fail/calls/process_exit_returning_arm_breaks_guarantee"),
        ["rejected"]
    );
}

/// `finish` never returns, and the entry lends it two locals. Each loan is
/// formed at the call like any call's and carried into it: the call's edge
/// consumes exactly those two loans.
#[test]
fn a_call_that_never_returns_carries_the_storage_it_is_lent() {
    let parts = judged_with_dependencies(
        "tests/fixtures/run/calls/non_returning_call_lends_storage",
        "entry_start",
        &[],
    )
    .unwrap_or_else(|stopped| panic!("Psi 00 through 04 must complete: {stopped:?}"));
    let [Ok(RouteVerdict::Accepted(closure))] = &parts[..] else {
        panic!("the entry closure accepted: {parts:?}");
    };
    let consumed = closure
        .elaboration
        .tables
        .machines
        .values()
        .flat_map(|tables| tables.edges.values())
        .flat_map(|edge| &edge.dispositions)
        .filter(|disposition| disposition.fate == terminal_check_tables::edges::Fate::Consumed)
        .count();
    assert_eq!(consumed, 2, "one consumed loan per borrowed argument");
}

/// Its twin lends the same local twice, mutably. The loans of a call that
/// never returns are judged by the rule every call's are, so the second
/// conflicts with the first and the program is rejected.
#[test]
fn a_call_that_never_returns_may_not_be_lent_one_place_twice() {
    assert_eq!(
        exit_verdicts("tests/fixtures/fail/borrows/non_returning_call_lends_twice"),
        ["rejected"]
    );
}

/// A statement written after `ProcessExit::exit_process` is unreachable, not
/// an error (`wiki/spec/language/state_contracts.md`, "Statements after a
/// call that never returns"). The kept program's `let next: i32 = root.count
/// + 1;` after the exit is not lowered: `main` ends in the exit with no
/// addition before it, so the range the addition would owe is owed by
/// nobody and the program is accepted. The accepted closure carries one
/// warning at the `let` and one note at the call.
#[test]
fn a_statement_after_a_call_that_never_returns_is_accepted_with_one_warning() {
    let _compilation = COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let frontier = unbuilt_frontier::frontier(
        &unbuilt_frontier::repository("tests/fixtures/run/calls/statement_after_process_exit")
            .join("main.omg"),
        Some("entry_start"),
        &[],
    )
    .unwrap_or_else(|stopped| panic!("Psi 00 through 04 must complete: {stopped:?}"));
    let parts = judge(&frontier);
    let [Ok(RouteVerdict::Accepted(closure))] = &parts[..] else {
        panic!("the entry closure accepted: {parts:?}");
    };
    let operations = closure
        .elaboration
        .module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations);
    assert!(
        operations.clone().all(|operation| !matches!(
            operation.kind,
            terminal_psi::OperationKind::ExactIntegerAdd { .. }
        )),
        "the addition after the exit is not lowered"
    );
    let advisories = closure.advisories();
    let [warning, note] = &advisories[..] else {
        panic!("one warning and its note: {advisories:?}");
    };
    assert_eq!(warning.severity, diagnostics::DiagnosticSeverity::Warning);
    assert!(
        warning
            .message
            .starts_with("unreachable statement in machine `main` state `"),
        "{}",
        warning.message
    );
    assert_eq!(note.severity, diagnostics::DiagnosticSeverity::Note);
    assert_eq!(note.message, "this call never returns");
    // The warning is at the `let` and the note at the call before it.
    let (statement, call) = (
        warning.source_span.expect("the statement is authored"),
        note.source_span.expect("the call is authored"),
    );
    assert!(
        call.span.end <= statement.span.start,
        "{call:?} {statement:?}"
    );
}

/// Its twin calls a helper that exits on one arm and returns on the other.
/// "The call can return on some paths: the continuation is checked on those
/// returning paths", so the same `let` is lowered, its addition owes its
/// range, nothing bounds `root.count`, and the program is rejected. Nothing
/// is reported as unreachable.
#[test]
fn a_statement_after_a_call_that_may_return_is_checked() {
    assert_eq!(
        exit_verdicts("tests/fixtures/fail/calls/statement_after_conditional_exit"),
        ["rejected"]
    );
}

/// Its other twin names a machine nothing declares in the statement after
/// the exit. "Unreachable statements are still source": leaving the
/// statement out of the lowered form does not excuse the name, and the
/// program is not accepted. It is refused where an unknown call name is
/// refused anywhere today: the reader raises the row typing left on the
/// declaration that holds the call (`typed_trees::deferred_refusals`).
#[test]
fn an_unknown_machine_after_a_call_that_never_returns_is_not_accepted() {
    let parts = judged_with_dependencies(
        "tests/fixtures/fail/calls/unknown_machine_after_process_exit",
        "entry_start",
        &[],
    )
    .unwrap_or_else(|stopped| panic!("Psi 00 through 04 must complete: {stopped:?}"));
    let [Ok(RouteVerdict::FallBack(refusal))] = &parts[..] else {
        panic!("the unknown name refuses the program: {parts:?}");
    };
    assert_eq!(
        refusal.detail,
        "declaration holding a call whose name resolves to no declaration"
    );
}

/// A machine that declares a lifetime is one machine, judged once
/// (`wiki/spec/language/lifetimes.md`: "Lifetime arguments are erased
/// regions"): it may return the view it was given for the lifetime its
/// result names, and a call gives that lifetime the region of the view it
/// passes. The twin returns the view of the other lifetime ("A signature
/// does not authorize returning unrelated storage") and is rejected.
#[test]
fn a_machine_with_a_lifetime_returns_a_view_of_that_lifetime_only() {
    let program = |returned: &str| {
        format!(
            "machine pick<'a, 'b>(view: &'a [u8], other: &'b [u8]) -> &'a [u8] {{ {returned} }}\n\
             machine relay(name: &[u8], second: &[u8]) {{ let kept: &[u8] = pick(name, second); }}\n\
             machine main() {{ }}\n"
        )
    };
    assert_eq!(verdicts("lifetime_view", &program("view")), ["accepted"]);
    assert_eq!(
        verdicts("lifetime_other_view", &program("other")),
        ["rejected"]
    );
}

/// A case payload is a place that exists while its case is selected
/// (`wiki/spec/language/patterns.md`: "The case fact belongs to the
/// place"). A match arm lends the view its case holds, and a payload named
/// directly is lent where the construction before it selected the case.
/// The twin lends a payload of the other case and is rejected.
#[test]
fn a_case_payload_is_lent_only_where_its_case_is_selected() {
    let program = |lent: &str| {
        format!(
            "data Source<'storage> {{\n\
                 case Path(location: &'storage [u8]);\n\
                 case Git(repository: &'storage [u8], revision: u32);\n\
             }}\n\
             machine measure(view: &[u8]) -> u32 {{ 1 }}\n\
             machine route(source: Source) -> u32 {{\n\
                 match source {{\n\
                     Source::Path {{ location }} -> measure(location)\n\
                     Source::Git {{ repository, revision }} -> measure(repository)\n\
                 }}\n\
             }}\n\
             machine main() {{\n\
                 let first: Source = Source::Path {{ location: \"here\" }};\n\
                 let routed: u32 = route(first);\n\
                 let held: Source = Source::Path {{ location: \"here\" }};\n\
                 let direct: u32 = measure(held.{lent});\n\
             }}\n"
        )
    };
    assert_eq!(
        verdicts("case_payload_lent", &program("location")),
        ["accepted"]
    );
    assert_eq!(
        verdicts("case_payload_other_case", &program("repository")),
        ["rejected"]
    );
}

/// A built-in integer name is a spelling of a sign and a width
/// (`wiki/spec/language/numeric_values.md`, "Width-indexed integers": "the
/// existing fixed-width names abbreviate those applications"), so the front
/// end and the checker are told the carrier, never one of eight names. Each
/// name is shown here as a parameter type, a result type and a literal
/// suffix, and each is judged at its own range: "the unsigned range is
/// `0 <= value < 2^N`; the signed range is `-2^(N-1) <= value < 2^(N-1)`".
///
/// For every name, `value + 1` is accepted when `requires` keeps `value`
/// below that carrier's greatest value and rejected when it allows the
/// greatest value itself; the two programs differ only in that bound. The
/// suffixed literal has the parameter's carrier, so the Wrapping add of the
/// two is accepted with nothing known about `value`.
#[test]
fn each_built_in_integer_name_is_judged_at_its_own_sign_and_width() {
    let mut wrong = Vec::new();
    for (carrier, greatest) in [
        ("u8", "255"),
        ("u16", "65535"),
        ("u32", "4294967295"),
        ("u64", "18446744073709551615"),
        ("i8", "127"),
        ("i16", "32767"),
        ("i32", "2147483647"),
        ("i64", "9223372036854775807"),
    ] {
        let bounded = |relation: &str| {
            format!(
                "machine next(value: {carrier}) -> {carrier} requires value {relation} {greatest} {{ value + 1 }}
                 machine main() {{ }}
"
            )
        };
        let wrapped = format!(
            "machine next(value: {carrier}) -> {carrier} {{ value +% 1{carrier} }}
             machine main() {{ }}
"
        );
        for (case, source, expected) in [
            ("below_greatest", bounded("<"), "accepted"),
            ("at_greatest", bounded("<="), "rejected"),
            ("suffixed_literal", wrapped, "accepted"),
        ] {
            let name = format!("{carrier}_{case}");
            let found = verdicts(&name, &source);
            if found != [expected] {
                wrong.push(format!("{name}: {found:?}, expected {expected}"));
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// An integer type is a sign and a width at every width, not only the eight
/// conventional ones (`wiki/spec/language/numeric_values.md`, "Width-indexed
/// integers": "All existing fixed-integer operations apply at N, not at a
/// convenient machine register width"). Each row is a carrier no machine
/// register has, with its greatest value.
///
/// As for the built-in names, `value + 1` is accepted when `requires` keeps
/// `value` below the carrier's greatest value and rejected when it allows
/// the greatest value itself, and the Wrapping add of a literal suffixed
/// with the same carrier (`1u13`) is accepted with nothing known about
/// `value`. `u1` holds zero and one and is an integer, not `bool`.
#[test]
fn an_integer_of_any_width_is_judged_at_its_own_sign_and_width() {
    let mut wrong = Vec::new();
    for (carrier, greatest) in [
        ("u1", "1"),
        ("u13", "8191"),
        ("i13", "4095"),
        ("u24", "16777215"),
        ("i48", "140737488355327"),
        ("u63", "9223372036854775807"),
    ] {
        let bounded = |relation: &str| {
            format!(
                "machine next(value: {carrier}) -> {carrier} requires value {relation} {greatest} {{ value + 1 }}
                 machine main() {{ }}
"
            )
        };
        let wrapped = format!(
            "machine next(value: {carrier}) -> {carrier} {{ value +% 1{carrier} }}
             machine main() {{ }}
"
        );
        for (case, source, expected) in [
            ("below_greatest", bounded("<"), "accepted"),
            ("at_greatest", bounded("<="), "rejected"),
            ("suffixed_literal", wrapped, "accepted"),
        ] {
            let name = format!("{carrier}_{case}");
            let found = verdicts(&name, &source);
            if found != [expected] {
                wrong.push(format!("{name}: {found:?}, expected {expected}"));
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// `u<N>` and `i<N>` are the same types their decimal shorthand names
/// ("`u32` and `u<32>` are exactly the same type, not convertible
/// wrappers"). A parameter written `u<13>` is returned as a `u13` and judged
/// at thirteen bits: accepted below the greatest value, rejected at it.
///
/// A width is "a positive proof-static `u64`": "Zero widths and runtime
/// width arguments reject", each before typing and for its own reason.
///
/// A width that is a const parameter is kept for the specialization that
/// substitutes it. The lowered-form checker has no integer type whose width
/// is a parameter yet, so such a program is refused by name, not judged at
/// some other width.
#[test]
fn an_indexed_width_is_the_type_its_shorthand_names() {
    let spare = |declaration: &str| format!("{declaration}\nmachine main() {{ }}\n");
    for (name, declaration, expected) in [
        (
            "indexed_conventional",
            "machine next(value: u<32>) -> u32 { value }",
            "accepted",
        ),
        (
            "indexed_below_greatest",
            "machine next(value: u<13>) -> u13 requires value < 8191 { value + 1 }",
            "accepted",
        ),
        (
            "indexed_at_greatest",
            "machine next(value: u<13>) -> u13 requires value <= 8191 { value + 1 }",
            "rejected",
        ),
        (
            "indexed_signed_above_least",
            "machine next(value: i<13>) -> i13 requires value > -4096 { value - 1 }",
            "accepted",
        ),
        (
            "indexed_signed_at_least",
            "machine next(value: i<13>) -> i13 requires value >= -4096 { value - 1 }",
            "rejected",
        ),
    ] {
        assert_eq!(verdicts(name, &spare(declaration)), [expected], "{name}");
    }
    /* The empty `main` is judged; the generic form of `next` is what is refused. */
    assert_eq!(
        verdicts(
            "symbolic_width",
            &spare("machine next<const N: u64>(value: u<N>) -> u<N> { value }")
        ),
        ["accepted", "not-supported"]
    );
    for (name, declaration, reason) in [
        (
            "zero_width",
            "machine next(value: u<0>) -> u32 { 1 }",
            "zero width",
        ),
        (
            "runtime_width",
            "machine next(count: u64, value: u<count>) -> u32 { 1 }",
            "runtime value",
        ),
    ] {
        let stopped = unbuilt_frontier::staged(name, &spare(declaration))
            .err()
            .unwrap_or_else(|| panic!("{name}: a malformed width must not be typed"));
        assert!(
            stopped.iter().any(|message| message.contains(reason)),
            "{name}: {stopped:?}"
        );
    }
}

/// Saturating arithmetic and exact conversion are judged at the carrier's
/// own width ("Exact numerical conversion preserves value and proves
/// destination fit"). `u16` to `u13` is accepted when `requires` keeps the
/// value within thirteen bits and rejected when it does not; the widening
/// `u13` to `u16` and the Saturating add at `u13` owe nothing.
///
/// A width the checker cannot state is refused by that width, never read as
/// a narrower one: its statements hold integers up to 128 bits, so `u128`
/// is judged and `u257` is not supported yet.
#[test]
fn conversions_and_policies_are_judged_at_the_width_the_program_names() {
    let spare = |declaration: &str| format!("{declaration}\nmachine main() {{ }}\n");
    for (name, declaration, expected) in [
        (
            "narrowing_within",
            "machine next(value: u16) -> u13 requires value <= 8191 { value as u13 }",
            "accepted",
        ),
        (
            "narrowing_beyond",
            "machine next(value: u16) -> u13 requires value <= 8192 { value as u13 }",
            "rejected",
        ),
        (
            "widening",
            "machine next(value: u13) -> u16 { value as u16 }",
            "accepted",
        ),
        (
            "saturating_add",
            "machine next(value: u13) -> u13 { value +| 8191u13 }",
            "accepted",
        ),
        (
            "beyond_a_register_below_greatest",
            "machine next(value: u65) -> u65 requires value < 5 { value + 1 }",
            "accepted",
        ),
        (
            "beyond_a_register_unbounded",
            "machine next(value: u65) -> u65 { value + 1 }",
            "rejected",
        ),
        (
            "beyond_the_checker",
            "machine next(value: u257) -> u257 { value }",
            "not-supported",
        ),
    ] {
        assert_eq!(verdicts(name, &spare(declaration)), [expected], "{name}");
    }
}

/// A dispatch over an owned sum that is not `[copy]`. The `Root` arm names
/// nothing of the subject, so the subject is discarded on that case's edge
/// (`wiki/spec/terminal-psi/control_flow.md`: "Selected payload fields become
/// scalar parameters on that case's edge before cleanup."); the `Named` arm
/// keeps it for the view it lends. The twin hands the same owned sum to two
/// calls and is rejected: it was moved by the first.
#[test]
fn an_owned_sum_is_dispatched_and_disposed_of_once() {
    let program = |again: &str| {
        format!(
            "data Selection<'storage> {{\n\
                 case Root;\n\
                 case Named(package: &'storage [u8]);\n\
             }}\n\
             machine take(view: &[u8]) {{ }}\n\
             machine select(view: &[u8], selection: Selection) {{\n\
                 transition selection {{\n\
                     Selection::Root -> take(view)\n\
                     Selection::Named {{ package }} -> take(package)\n\
                 }}\n\
             }}\n\
             machine main() {{\n\
                 let chosen: Selection = Selection::Named {{ package: \"here\" }};\n\
                 let other: Selection = Selection::Root;\n\
                 select(\"first\", chosen);\n\
                 select(\"second\", {again});\n\
             }}\n"
        )
    };
    assert_eq!(
        verdicts("owned_sum_dispatched", &program("other")),
        ["accepted"]
    );
    assert_eq!(
        verdicts("owned_sum_moved_twice", &program("chosen")),
        ["rejected"]
    );
}
