//! One generic body goes the whole way on the lowered form, from source:
//! it is checked once with its parameters rigid, and its certificates are
//! decided again by the proof kernel, with no search, in each machine stage
//! 04 generated from it. `wiki/drafts/designs/check_on_lowered_form.md`,
//! condition 3 of the gate, asks for that and for the negative twin to be
//! rejected.
//!
//! Each fixture is checked by the current pipeline first, which is the only
//! way to obtain the frozen typed trees; the route's three steps then run on
//! those trees (`support/generic_certificate_carry.rs`).
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;
#[path = "support/generic_certificate_carry.rs"]
mod generic_certificate_carry;

use fixture_package_inputs::repo_root;

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The program line's outcome and every per-obligation line, tab-split
/// after the fixture path: `[label, specialization, obligation, kind,
/// outcome]`.
fn carried(fixture: &str) -> (String, Vec<Vec<String>>) {
    let _guard = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let main = repo_root().join(fixture).join("main.omg");
    // The current pipeline needs a compile worker's stack.
    let (outcome, lines) = std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let mut lines = Vec::new();
            let outcome = generic_certificate_carry::probe(&main, &mut lines);
            (outcome, lines)
        })
        .expect("spawn a compile worker")
        .join()
        .expect("the carry does not panic")
    });
    let rows = lines
        .iter()
        .map(|line| {
            let mut fields: Vec<String> = line.split('\t').map(str::to_owned).collect();
            fields.remove(1);
            fields.truncate(5);
            fields
        })
        .collect();
    (outcome, rows)
}

fn count(rows: &[Vec<String>], label: &str, outcome: &str) -> usize {
    rows.iter()
        .filter(|row| row[0] == label && row[4] == outcome)
        .count()
}

/// An implementation parameter: `advance<Selected satisfies Step>` is
/// proved once against `Step::apply`'s contract and carried into
/// `advance<Hold>` and `advance<Shift>`.
#[test]
fn an_implementation_parameter_body_is_checked_once_and_carried() {
    let (outcome, rows) = carried("tests/omega/run/generics/implementation_parameter_contract");
    assert_eq!(outcome, "current=accepted\troute=carried-accepted");
    assert_eq!(count(&rows, "generic", "proved"), 3);
    assert_eq!(count(&rows, "generic", "refused"), 0);
    assert_eq!(count(&rows, "carried", "accepted"), 6);
    assert_eq!(count(&rows, "carried", "refused"), 0);
}

/// Its twin promises `result <= 18`, which only the implementations in use
/// give: a search over each specialization proves it, the generic check
/// refuses it, and nothing is then carried for that guarantee.
#[test]
fn a_bound_only_the_selected_implementations_give_is_refused_generically() {
    let (outcome, rows) = carried(
        "tests/omega/fail/generics/implementation_parameter_contract_only_holds_for_selection",
    );
    assert_eq!(outcome, "current=rejected\troute=carried-refused");
    assert_eq!(count(&rows, "concrete", "refused"), 0);
    assert_eq!(count(&rows, "generic", "refused"), 1);
    assert_eq!(count(&rows, "carried", "refused"), 2);
}

/// A value parameter, with an implementation parameter beside it:
/// `Selected::value<const Units: u64>` is proved once with `Units` a formal
/// and carried into `Selected::value<7>`; `selected<Implementation
/// satisfies Bounded>` is proved once against `Bounded::value`'s contract
/// at `Units == 7` and carried into `selected<Selected>`.
#[test]
fn a_value_parameter_body_is_checked_once_and_carried() {
    let (outcome, rows) =
        carried("tests/omega/pass/facts/lexical_generic_bound_through_local_exit");
    assert_eq!(outcome, "current=accepted\troute=carried-accepted");
    // `selected`: the call's requirement and its guarantee. `value`: its
    // guarantee.
    assert_eq!(count(&rows, "generic", "proved"), 3);
    assert_eq!(count(&rows, "generic", "refused"), 0);
    assert_eq!(count(&rows, "witness", "refused"), 0);
    assert_eq!(count(&rows, "carried", "accepted"), 3);
    assert_eq!(count(&rows, "carried", "refused"), 0);
}

/// Its twin differs only in the requirement's own contract (`requires
/// true`): the member then asks `input <= Units`, which the requirement
/// does not give. The one call passes 7, so a search over the
/// specialization proves the call's requirement; the member's refinement
/// proof does not exist, and the carried requirement is refused.
#[test]
fn a_member_that_asks_more_than_its_requirement_is_refused() {
    let (outcome, rows) =
        carried("tests/omega/fail/facts/lexical_generic_bound_through_local_rejected");
    assert_eq!(outcome, "current=rejected\troute=carried-refused");
    assert_eq!(count(&rows, "concrete", "refused"), 0);
    let refused: Vec<_> = rows
        .iter()
        .filter(|row| row[4] == "refused")
        .map(|row| (row[0].as_str(), row[3].as_str()))
        .collect();
    assert_eq!(
        refused,
        [("witness", "requirement"), ("carried", "requirement")]
    );
}

/// An opaque type parameter: `relay<T>` and `forward<T>` are each proved
/// once with their `T` values rigid, `relay` against `forward`'s contract,
/// and carried into the machines generated for `u8` and for `bool`.
#[test]
fn a_type_parameter_body_is_checked_once_and_carried() {
    let (outcome, rows) = carried("tests/omega/run/generics/type_parameter_contract");
    assert_eq!(outcome, "current=accepted\troute=carried-accepted");
    // `relay`: the call's requirement, the add and the guarantee.
    // `forward`: its guarantee.
    assert_eq!(count(&rows, "generic", "proved"), 4);
    assert_eq!(count(&rows, "generic", "refused"), 0);
    // Two types, each with both machines.
    assert_eq!(count(&rows, "carried", "accepted"), 8);
    assert_eq!(count(&rows, "carried", "refused"), 0);
}

/// Its twin differs only in the bound `relay` promises, `result <= 10`,
/// which `forward`'s contract does not give after the add: refused once, as
/// a generic body, and then in each generated machine with nothing to
/// carry.
#[test]
fn a_type_parameter_body_that_exceeds_its_callee_is_refused() {
    let (outcome, rows) =
        carried("tests/omega/fail/generics/type_parameter_contract_exceeds_callee");
    assert_eq!(outcome, "current=rejected\troute=carried-refused");
    let refused: Vec<_> = rows
        .iter()
        .filter(|row| row[4] == "refused" && row[0] != "concrete")
        .map(|row| (row[0].as_str(), row[3].as_str()))
        .collect();
    assert_eq!(
        refused,
        [
            ("generic", "guarantee"),
            ("carried", "guarantee"),
            ("carried", "guarantee")
        ]
    );
}

/// The design's witness for an opaque `T`: `same<T>` and `compare<T>` own no
/// contract clause, so nothing is proved or carried, and what is shown is
/// that both generic forms lower with their `T` values rigid and that each
/// generated machine is that body at `u64`. Its `main` is outside the
/// subset, so each generated machine is the root of its own closure.
#[test]
fn the_comparison_witness_lowers_and_corresponds() {
    let (outcome, rows) =
        carried("tests/omega/pass/facts/generic_comparison_body_through_match_join_exit");
    assert_eq!(outcome, "current=accepted\troute=carried-accepted");
    assert_eq!(count(&rows, "generic", "refused"), 0);
    assert_eq!(count(&rows, "carried", "refused"), 0);
}
