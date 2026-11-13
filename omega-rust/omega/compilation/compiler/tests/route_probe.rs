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

use lowered_discharge::RouteVerdict;

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
) -> Vec<Result<RouteVerdict, lowered_discharge::CompilationRefusal>> {
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
) -> Vec<Result<RouteVerdict, lowered_discharge::CompilationRefusal>> {
    lowered_discharge::judge_compilation(
        &frontier.typed,
        structure_elaboration::Compilation {
            entries: &frontier.entries,
            subjects: &[frontier.subject],
            purpose: source::DependencyScope::Product,
            selected: &frontier.selected,
            bindings: &frontier.bindings,
        },
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
) -> Result<Vec<Result<RouteVerdict, lowered_discharge::CompilationRefusal>>, Vec<String>> {
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
