use source_files_to_tokens::Lexer;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::data::DataMember;
use symbol_resolved_trees::domain::ProofFact;
use symbol_resolved_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
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
    let program = resolve(ResolutionRequest::new(syntax)).expect("resolve data invariant");

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
        let program = resolve(ResolutionRequest::new(syntax)).expect("retain open template facts");
        let definition = program.data_definitions.iter().next().expect("template");
        assert!(
            definition.zero_gated,
            "an unresolved fact cannot authorize zero construction"
        );
        assert_eq!(program.proof_facts(definition.where_facts).len(), 1);
    }
}

#[test]
fn data_where_zero_classification_retains_unknown_predicates() {
    for (carrier, fact, zero_gated) in [
        ("u64", "value >= 0 && value <= 8", false),
        ("u64", "value > 0", true),
        ("u64", "value / 2 >= 0", true),
        ("f32", "value >= 0.0 && value <= 1.0", true),
        ("f64", "value > 0.0", true),
        // A positive exact decimal can round to zero at the field's format.
        // Resolution must not decide this strict comparison as a rational.
        ("f32", "value < 1e-50", true),
    ] {
        let source = format!("data Sample where {fact}, {{ value: {carrier}; }}");
        let tokens = Lexer::new(&source).tokenize().expect("tokenize predicate");
        let syntax = parse_syntax_trees(&tokens).expect("parse predicate");
        let program = resolve(ResolutionRequest::new(syntax)).expect("retain predicate");
        let definition = program.data_definitions.iter().next().expect("sample");
        assert_eq!(definition.zero_gated, zero_gated, "{carrier}: {fact}");
        let [ProofFact::Expression(_)] = program.proof_facts(definition.where_facts) else {
            panic!("resolution must retain the whole predicate: {fact}");
        };
    }
}

#[test]
fn unresolved_zero_predicates_keep_exact_callable_identity() {
    let tokens = Lexer::new(
        "machine accepts_zero() -> bool { true } \
         data Sample where accepts_zero(), { value: f32; }",
    )
    .tokenize()
    .expect("tokenize callable predicate");
    let syntax = parse_syntax_trees(&tokens).expect("parse callable predicate");
    let program = resolve(ResolutionRequest::new(syntax)).expect("resolve callable predicate");
    let definition = program.data_definitions.iter().next().expect("sample");
    assert!(
        definition.zero_gated,
        "resolution does not execute predicates"
    );
    let [ProofFact::Expression(expression)] = program.proof_facts(definition.where_facts) else {
        panic!("one retained predicate");
    };
    let ExpressionNode::Call(call) = program.tables.bodies.expressions.expression(*expression)
    else {
        panic!("retained call");
    };
    let machine = program.machines.iter().next().expect("predicate machine");
    let [entry] = program
        .tables
        .declarations
        .machine_state_handles
        .span_or_empty(machine.states)
    else {
        panic!("one predicate entry");
    };
    assert_eq!(
        call.target_symbol,
        program
            .tables
            .declarations
            .machine_states
            .get(*entry)
            .symbol
    );
}

#[test]
fn data_zero_gating_preserves_membership_and_first_case_contributions() {
    let tokens = Lexer::new(
        r#"
        domain u64::Positive requires self > 0;
        data Membership where value >= 0, value in u64::Positive, { value: u64; }
        data FirstRejected { case First where false; case Second; }
        data LaterRejected { case First; case Second where false; }
        "#,
    )
    .tokenize()
    .expect("tokenize independent gates");
    let syntax = parse_syntax_trees(&tokens).expect("parse independent gates");
    let program = resolve(ResolutionRequest::new(syntax)).expect("resolve independent gates");
    for (name, zero_gated) in [
        ("Membership", true),
        ("FirstRejected", true),
        ("LaterRejected", false),
    ] {
        let definition = program
            .data_definitions
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .expect("named definition");
        assert_eq!(definition.zero_gated, zero_gated, "{name}");
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
