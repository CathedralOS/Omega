use super::{fact_only_theorem, is_fact_only_citation};
use typed_trees::TypedTrees;
use typed_trees::statement::StatementNode;

const THEOREMS: &str = r#"
machine positive(value: u64, bound: &[u64; 2])
requires value < 8
ensures value < 9
{
}

machine counted(value: u64)
requires value < 8
ensures value < 9
{
    transition { _ -> done(value) }
    state done(value: u64) { }
}

machine writes(cells: &mut [u64; 2])
requires cells[0] < 8
ensures cells[0] < 9
{
}

machine valued(value: u64) -> u64
requires value < 8
{
    value
}

machine barrier(value: u64)
requires value < 8
{
}
"#;

fn program(caller: &str) -> TypedTrees {
    crate::front_end::typed_program(&format!("{THEOREMS}\n{caller}"))
}

fn named<'program>(
    program: &'program TypedTrees,
    name: &str,
) -> &'program typed_trees::machine::Machine {
    program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .expect("fixture machine")
}

fn caller_statements(program: &TypedTrees) -> Vec<&StatementNode> {
    let caller = named(program, "caller");
    program
        .statement_table
        .statements(program.machine_states(caller)[0].statement_nodes)
        .iter()
        .collect()
}

#[test]
fn an_empty_unit_theorem_over_read_only_parameters_is_fact_only() {
    let program = program("machine caller() { }");
    assert!(fact_only_theorem(&program, named(&program, "positive").symbol).is_some());
    // States, a mutable parameter, a Type result or no conclusion each keep
    // an ordinary call.
    for name in ["counted", "writes", "valued", "barrier"] {
        assert!(
            fact_only_theorem(&program, named(&program, name).symbol).is_none(),
            "{name} is not theorem-only"
        );
    }
}

#[test]
fn a_citation_with_place_and_literal_arguments_is_erased() {
    let program = program(
        "data Main { bound: [u64; 2]; count: u64; }
         machine caller(main: &mut Main) {
             positive(3, &main.bound);
             positive(main.count, &main.bound);
             counted(3);
         }",
    );
    let statements = caller_statements(&program);
    assert!(is_fact_only_citation(&program, statements[0]));
    assert!(is_fact_only_citation(&program, statements[1]));
    assert!(!is_fact_only_citation(&program, statements[2]));
}

#[test]
fn a_citation_that_computes_an_argument_stays_a_call() {
    let program = program(
        "data Main { bound: [u64; 2]; count: u64; }
         machine caller(main: &mut Main)
         requires main.count < 4
         {
             positive(main.count + 1, &main.bound);
         }",
    );
    let statements = caller_statements(&program);
    assert!(!is_fact_only_citation(&program, statements[0]));
}
