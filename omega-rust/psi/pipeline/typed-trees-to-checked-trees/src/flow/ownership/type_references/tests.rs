//! `intrinsic_enum_equality` decides whether `==`/`!=` on a payload-free
//! nominal sum reads only tags, which pins the operands' custody records
//! instead of an ownership transfer. The nominal premise is an exact
//! declaration identity: when a second declaration row bears the operand's
//! symbol, the member scan cannot be re-derived for the exact subject, so
//! the classification must decline rather than let the first same-shaped
//! row mint evidence.
use super::intrinsic_enum_equality;
use typed_trees::statement::StatementNode;

fn typed_source(source: &str) -> typed_trees::TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize enum-equality fixture");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse enum-equality fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve enum-equality fixture");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type enum-equality fixture")
}

/// `decide` compares two `Choice` parameters: the equality's custody
/// classification depends entirely on resolving the shared nominal symbol to
/// one declaration whose members are all payload-free variants.
fn equality_is_intrinsic(source: &str, mutate: impl FnOnce(&mut typed_trees::TypedTrees)) -> bool {
    let mut program = typed_source(source);
    mutate(&mut program);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "decide")
        .expect("fixture machine");
    let state = &program.machine_states(machine)[0];
    let StatementNode::Expression(expression) =
        program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("fixture body must be one result expression")
    };
    intrinsic_enum_equality(&program, state.symbol, expression)
}

fn duplicate_choice(program: &mut typed_trees::TypedTrees) {
    let duplicate = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Choice")
        .expect("fixture Choice declaration")
        .clone();
    program.push_data_definition(duplicate);
}

#[test]
fn payload_free_nominal_equality_is_intrinsic() {
    assert!(equality_is_intrinsic(
        "data Choice { case A; case B; }
         machine decide(x: Choice, y: Choice) -> bool { x == y }",
        |_| {},
    ));
}

#[test]
fn duplicated_nominal_identity_declines_equality_classification() {
    // A second row bearing the operand's symbol makes the nominal premise
    // ambiguous: the member scan could name the wrong declaration, so the
    // equality cannot be certified intrinsic for the exact subject.
    assert!(!equality_is_intrinsic(
        "data Choice { case A; case B; }
         machine decide(x: Choice, y: Choice) -> bool { x == y }",
        duplicate_choice,
    ));
}
