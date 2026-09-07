use source_files_to_tokens::Lexer;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::data::DataMember;
use symbol_resolved_trees::domain::ProofFact;
use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;

#[test]
fn data_where_fields_and_static_parameters_receive_exact_local_symbols() {
    let tokens = Lexer::new(
        r#"
        domain u64::Small
        requires self <= 8;

        machine sample() -> u64 { 0 }

        pub data Buffer<const N: u64>
        where
            count <= N,
            count in u64::Small,
            sample() in u64::Small,
        {
            count: u64;
        }
        "#,
    )
    .tokenize()
    .expect("tokenize data invariant");
    let syntax = parse_syntax_trees(&tokens).expect("parse data invariant");
    let program = lower_syntax_trees(&syntax).expect("resolve data invariant");

    let definition = program
        .data_definitions
        .iter()
        .next()
        .expect("one data definition");
    assert_eq!(program.data_definitions.len(), 1);
    let [parameter] = program.data_type_parameters(definition.type_parameters) else {
        panic!("one static parameter")
    };
    let [DataMember::Field(field)] = program.data_members(definition.members) else {
        panic!("one direct field")
    };
    let [
        ProofFact::Expression(expression),
        ProofFact::Membership(membership),
        ProofFact::Membership(call_membership),
    ] = program.proof_facts(definition.where_facts)
    else {
        panic!("one expression and two membership invariants")
    };
    let ExpressionNode::Binary(binary) = program.tables.bodies.expressions.expression(*expression)
    else {
        panic!("binary invariant")
    };

    assert_name_identity(&program, binary.left, field.symbol);
    assert_name_identity(&program, binary.right, parameter.symbol);
    assert_name_identity(&program, membership.value, field.symbol);

    let sample = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "sample")
        .expect("sample machine");
    let [entry] = program
        .tables
        .declarations
        .machine_state_handles
        .span_or_empty(sample.states)
    else {
        panic!("one sample entry state")
    };
    let entry = program.tables.declarations.machine_states.get(*entry);
    let ExpressionNode::Call(call) = program
        .tables
        .bodies
        .expressions
        .expression(call_membership.value)
    else {
        panic!("membership value call")
    };
    assert_eq!(call.target_symbol, entry.symbol);
}

#[test]
fn unresolved_template_facts_stay_gated_until_concrete_instantiation() {
    for fact in ["N / 2 > 0", "count / 2 <= N"] {
        let source = format!("data Buffer<const N: u64> where {fact}, {{ count: u64; }}");
        let tokens = Lexer::new(&source).tokenize().expect("tokenize template");
        let syntax = parse_syntax_trees(&tokens).expect("parse template");
        let program = lower_syntax_trees(&syntax).expect("retain open template facts");
        let definition = program.data_definitions.iter().next().expect("template");
        assert!(
            definition.zero_gated,
            "an unresolved fact cannot authorize zero construction"
        );
        assert_eq!(program.proof_facts(definition.where_facts).len(), 1);
    }
}

fn assert_name_identity(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
    expected: symbols::SymbolHandle,
) {
    let ExpressionNode::Name(path) = program.tables.bodies.expressions.expression(expression)
    else {
        panic!("name expression")
    };
    assert_eq!(path.head_symbol, expected);
    assert_eq!(path.symbol, expected);
    assert_eq!(
        program
            .tables
            .bodies
            .expressions
            .name_path_member_symbols(path.member_symbols),
        &[expected]
    );
}
