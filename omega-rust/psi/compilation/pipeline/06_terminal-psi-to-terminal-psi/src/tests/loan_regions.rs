//! One lifetime given the loans of two places. Read from source and run in
//! the Psi interpreter: the products of `samples/cli/facts/
//! build_activation_borrow` choose between two cells their caller lent
//! separately, `selected<'storage>(left: &'storage mut Cell, right: &'storage
//! mut Cell, first)`.
//!
//! A loan is formed where the program forms it and lives within the storage
//! it lends (loans.md: a loan resource has its "captured place" and its
//! "parent lifetime"), so its region is some region inside the lent place's.
//! Two loans given to one lifetime share a region inside both places, and
//! lifetimes.md, "Returned views", keeps both as sources: "every reference
//! leaf carrying the selected lifetime remains a possible source". No named
//! region is ordered against another by this: a loan passes for a named
//! lifetime only when that lifetime is the lent place's own.

use super::build_purpose::judge_build;
use super::*;

const CELLS: &str = r#"
data Cell { before: u64; value: u64; after: u64; }
data Carrier<'storage> { cell: &'storage mut Cell; }
data View<'storage> { cell: &'storage Cell; }
"#;

/// The sample's chooser: a carrier of whichever cell `first` selects.
const SELECTED: &str = r#"
machine selected<'storage>(left: &'storage mut Cell, right: &'storage mut Cell, first: bool) -> Carrier<'storage> {
    transition first {
        true -> Carrier { cell: left }
        false -> Carrier { cell: right }
    }
}
"#;

/// The entry every program here ends with: two cells with distinct
/// neighbours, `journey` run once for each choice, and a trap unless each
/// run wrote 73 to the cell it chose and nothing else changed. An inverted
/// choice, a write to the other cell, or a write confined to a copy takes
/// the trapping arm.
const ENTRY: &str = r#"
machine entry() crashes Trap {
    let mut left: Cell = Cell { before: 11, value: 5, after: 17 };
    let mut right: Cell = Cell { before: 23, value: 9, after: 29 };
    journey(&mut left, &mut right, true);
    let first_correct: bool = left.value == 73 && right.value == 9
        && left.before == 11 && left.after == 17 && right.before == 23 && right.after == 29;
    journey(&mut left, &mut right, false);
    transition first_correct && left.value == 73 && right.value == 73
        && left.before == 11 && left.after == 17 && right.before == 23 && right.after == 29 {
        true -> done()
        false -> bad()
    }
    machine done() {}
    machine bad() { crash Trap; }
}
"#;

/// The Product `text` judged from its `entry`, published and run: `Ok` with
/// how it ended, or the refusal's text.
fn run(text: &str) -> Result<terminal_interpreter::TerminalExecutionStatus, String> {
    let typed = super::boundary_calls::typed(text);
    let entry = super::boundary_calls::entry(&typed);
    let elaborated = typed_trees_to_terminal_psi::elaborate_for(
        &typed,
        typed_trees_to_terminal_psi::Compilation {
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    );
    let accepted = match crate::judge(elaborated) {
        Ok(RouteVerdict::Accepted(accepted)) => accepted,
        Ok(RouteVerdict::FallBack(refusal)) | Err(refusal) => return Err(refusal.detail),
        Ok(other) => return Err(format!("{other:?}")),
    };
    let publication = accepted.publish_invocation().unwrap();
    let program = terminal_interpreter::TerminalProgram::borrow(publication.artifact.view());
    let mut instance = program.instantiate(Default::default()).unwrap();
    Ok(instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(10_000))
        .unwrap())
}

fn completes(journey: &str) {
    assert_eq!(
        run(&format!("{CELLS}{journey}{ENTRY}")),
        Ok(terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        )),
        "{journey}"
    );
}

/// What the checker says of the Build program `build`: `accepted`, or the
/// refusal's text.
fn verdict(build: &str) -> String {
    match judge_build(build) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
        Ok(RouteVerdict::Accepted(accepted)) => {
            accepted.publish_invocation().unwrap();
            "accepted".to_owned()
        }
        other => panic!("{other:?}"),
    }
}

/// The caller's two formals elide their lifetimes, so they are two regions,
/// and the call gives `'storage` a loan of each. The callee writes both
/// cells, and the entry sees both writes in the cells it lent.
#[test]
fn a_call_gives_one_lifetime_the_loans_of_two_places() {
    completes(
        r#"
machine both<'storage>(left: &'storage mut Cell, right: &'storage mut Cell, first: bool) {
    transition first {
        true -> write(left)
        false -> write(right)
    }
    machine write(cell: &mut Cell) { cell.value = 73; }
}
machine journey(left: &mut Cell, right: &mut Cell, first: bool) {
    both(left, right, first);
}
"#,
    );
}

/// A reference field given a loan of one formal holds a loan of that
/// formal's region only when the two are stated to be one region: elided,
/// the holder's region is its own and the loan is within the other formal's.
/// lifetimes.md: "A signature does not authorize returning unrelated storage
/// merely because its type has the same lifetime spelling".
#[test]
fn a_loan_is_stored_only_where_its_place_region_is_the_holders() {
    let stashed = |binders: &str, holder: &str, cell: &str| {
        format!(
            r#"{CELLS}
machine build_entry{binders}(holder: &mut View{holder}, cell: &{cell}Cell) {{
    holder.cell = cell;
}}
"#
        )
    };
    assert_eq!(
        verdict(&stashed("", "", "")),
        "typing assigned value type while reading the body of `build_entry`"
    );
    // Stated as one region the store is typed; what stops it then is the
    // checker's own question about a reference kept in storage the caller
    // lent, which no slice answers yet.
    assert_eq!(
        verdict(&stashed("<'s>", "<'s>", "'s ")),
        "a reference retained in caller storage needs its checked output-loan correspondence"
    );
}

/// What the shared region is not: it is no named lifetime of the machine. A
/// view that may hold a loan of `right` is not returned as a view of
/// `left`'s lifetime.
#[test]
fn a_shared_loan_region_is_no_named_lifetime_of_the_machine() {
    assert_eq!(
        verdict(&format!(
            r#"{CELLS}
machine pick<'a, 'b>(left: &'a Cell, right: &'b Cell) -> View<'a> {{
    let mut view: View = View {{ cell: left }};
    view.cell = right;
    view
}}
machine build_entry(left: &Cell, right: &Cell) {{
    let view: View = pick(left, right);
}}
"#
        )),
        "typing arm value type while reading the body of `pick`"
    );
}

/// A loan of a local is within that local's storage, which no lifetime of
/// the machine names: it is not returned for one.
#[test]
fn a_loan_of_a_local_is_not_returned_for_a_lifetime_of_the_machine() {
    assert_ne!(
        verdict(&format!(
            r#"{CELLS}
machine leak<'a>(cell: &'a Cell) -> View<'a> {{
    let local: Cell = Cell {{ before: 1, value: 2, after: 3 }};
    View {{ cell: &local }}
}}
machine build_entry(cell: &Cell) {{
    let view: View = leak(cell);
    let observed: bool = view.cell.value == 2;
}}
"#
        )),
        "accepted"
    );
}

/// Where a reference that may be backed by either of two inputs still
/// stops. The reader types each of these; the checker's place model
/// resolves a reference to one backing and the published result contract
/// names one input root, so a value that may borrow either is refused by
/// name until both carry every possible source (lifetimes.md: "an unresolved
/// dynamic selection conservatively retains all possible sources").
#[test]
fn a_reference_backed_by_either_of_two_inputs_is_refused_by_name() {
    let returned = run(&format!(
        r#"{CELLS}{SELECTED}
machine journey(left: &mut Cell, right: &mut Cell, first: bool) {{
    let mut carrier: Carrier = selected(left, right, first);
    carrier.cell.value = 73;
}}
{ENTRY}"#
    ));
    assert_eq!(
        returned,
        Err("call reference result has alternatives from different input roots".to_owned())
    );
    let joined = run(&format!(
        r#"{CELLS}
machine journey(left: &mut Cell, right: &mut Cell, first: bool) {{
    let mut carrier: Carrier = match first {{
        true -> Carrier {{ cell: left }}
        false -> Carrier {{ cell: right }}
    }};
    carrier.cell.value = 73;
}}
{ENTRY}"#
    ));
    assert_eq!(
        joined,
        Err("reference use without exact live backing correspondence".to_owned())
    );
}
