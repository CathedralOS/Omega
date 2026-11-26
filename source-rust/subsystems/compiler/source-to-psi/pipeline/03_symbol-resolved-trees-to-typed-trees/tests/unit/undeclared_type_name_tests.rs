//! A written type name that selects no declaration is a violation recorded
//! on the declaration that writes it (`undeclared_type_names.rs`). The twin
//! fixtures under `tests/fixtures/type_names` differ only in the name of the
//! declaration the program uses.

use crate::source_state::TypedTrees;
use crate::source_state::deferred_refusals::DeferredRequest;

const DECLARED: &str = include_str!("../fixtures/type_names/run/declared/main.omg");
const UNDECLARED: &str = include_str!("../fixtures/type_names/fail/undeclared/main.omg");

/// Each declaration holding an undeclared type name, with the `(line,
/// column)` of the name its row reports.
fn undeclared(program: &TypedTrees, text: &str) -> Vec<(String, (usize, usize), String)> {
    let mut rows: Vec<_> = program
        .deferred_refusals
        .iter()
        .filter(|row| row.request == DeferredRequest::UndeclaredTypeName)
        .map(|row| {
            let start = row.source_span.span.start;
            let line = text[..start].matches('\n').count() + 1;
            let column = start - text[..start].rfind('\n').map_or(0, |at| at + 1) + 1;
            (
                program.symbols.display_path(row.declaration, "::"),
                (line, column),
                row.detail.clone(),
            )
        })
        .collect();
    rows.sort();
    rows
}

#[test]
fn a_program_whose_type_names_select_declarations_holds_no_row() {
    let program = super::qualified_operator_home_tests::checked(&[("main.omg", DECLARED)])
        .expect("the declared twin types");
    assert_eq!(undeclared(&program, DECLARED), []);
}

#[test]
fn each_declaration_writing_an_undeclared_type_name_holds_a_violation_at_the_name() {
    let program = super::qualified_operator_home_tests::checked(&[("main.omg", UNDECLARED)])
        .expect("typing records the missing type instead of refusing the whole program");
    let rows = undeclared(&program, UNDECLARED);
    let at = |declaration: &str, position| (declaration.to_owned(), position, "`Pick`".to_owned());
    // One row per declaration, at its first written `Pick`: the payload of
    // `Choice`, the parameter of `select`, the parameter of `second` and the
    // local of `main`.
    assert_eq!(
        rows,
        [
            at("Choice", (5, 39)),
            at("main", (12, 17)),
            at("second", (9, 25)),
            at("select", (7, 19)),
        ]
    );
    assert!(DeferredRequest::UndeclaredTypeName.is_violation());
}

/// Names another stage binds are not recorded: a domain written as a type,
/// and a generic argument, which typing refuses itself when it names nothing.
#[test]
fn a_name_that_selects_another_kind_of_declaration_is_not_recorded() {
    let text = "domain u8::Small requires self < 10u8;\n\
                machine keep(x: u8 in Small) -> u8 in Small { x }\n";
    let program = super::qualified_operator_home_tests::checked(&[("main.omg", text)])
        .expect("the domain program types");
    assert_eq!(undeclared(&program, text), []);
}
