use super::*;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

const SOURCE: &str = "data Message { case Empty; case Data(value: u8); }
    data Other { case Empty; case Data(value: u8); }
    data Wrapper where message in Message::Data, { message: Message; }
    data Foreign { message: Other; }
    domain Message::First requires self in Message::Data;
    domain Message::Second requires self in Message::Data;
    domain Wrapper::Projected requires self.message in Message::Data;";

fn occurrence(program: &TypedTrees, facts: HandleSpan<ProofFact>) -> ExpressionHandle {
    let ProofFact::Expression(expression) = program.proof_facts.get(facts.start()) else {
        panic!("membership expression");
    };
    *expression
}

fn comparison(program: &TypedTrees, expression: ExpressionHandle) -> &TableBinaryExpression {
    let ExpressionNode::Binary(comparison) = program.expression_table.expression(expression) else {
        panic!("membership comparison");
    };
    comparison
}

#[test]
fn domain_self_and_fields_and_data_fields_keep_their_declared_carriers() {
    let program = typed(SOURCE);
    for domain in program.domain_definitions() {
        let expression = occurrence(&program, domain.facts);
        assert!(has_exact_domain_case_membership_meaning(
            &program,
            domain,
            expression,
            comparison(&program, expression)
        ));
    }
    let data = program
        .data_definitions()
        .iter()
        .find(|data| !data.where_facts.is_empty())
        .unwrap();
    let expression = occurrence(&program, data.where_facts);
    assert!(has_exact_data_case_membership_meaning(
        &program,
        data,
        expression,
        comparison(&program, expression)
    ));
}

#[test]
fn definition_membership_rejects_foreign_owner_and_unauthored_expression() {
    let program = typed(SOURCE);
    let first = &program.domain_definitions()[0];
    let second = &program.domain_definitions()[1];
    let expression = occurrence(&program, first.facts);
    assert!(!has_exact_domain_case_membership_meaning(
        &program,
        second,
        expression,
        comparison(&program, expression)
    ));
    let mut forged_owner = first.clone();
    forged_owner.target_type = program.domain_definitions()[2].target_type;
    assert!(!has_exact_domain_case_membership_meaning(
        &program,
        &forged_owner,
        expression,
        comparison(&program, expression)
    ));
    let mut altered = program.clone();
    let graft = altered
        .expression_table
        .insert(program.expression_table.expression(expression).clone());
    assert!(!has_exact_domain_case_membership_meaning(
        &altered,
        first,
        graft,
        comparison(&altered, graft)
    ));
    let mismatched = comparison(&program, occurrence(&program, second.facts));
    assert!(!has_exact_domain_case_membership_meaning(
        &program, first, expression, mismatched
    ));
}

#[test]
fn definition_membership_rejects_foreign_field_selection_and_forged_row_owner() {
    let program = typed(SOURCE);
    let domain = &program.domain_definitions()[2];
    let expression = occurrence(&program, domain.facts);
    let subject = comparison(&program, expression).left;
    let foreign = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Foreign")
        .unwrap();
    let wrapper = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Wrapper")
        .unwrap();
    let typed_trees::data::DataMember::Field(foreign_field) = &program.data_members(foreign)[0]
    else {
        panic!("foreign field");
    };
    let mut altered = program.clone();
    let ExpressionNode::Member(member) = altered.expression_table.expression_mut(subject) else {
        panic!("member");
    };
    member.member_symbol = foreign_field.symbol;
    assert!(!has_exact_domain_case_membership_meaning(
        &altered,
        domain,
        expression,
        comparison(&altered, expression)
    ));
    let mut altered = program.clone();
    let ExpressionNode::Member(member) = altered.expression_table.expression_mut(subject) else {
        panic!("member");
    };
    member.member_symbol = SymbolHandle::invalid();
    let typed_trees::data::DataMember::Field(field) =
        altered.data_members.get_mut(wrapper.members.start())
    else {
        panic!("wrapper field");
    };
    field.symbol = foreign_field.symbol;
    assert!(!has_exact_domain_case_membership_meaning(
        &altered,
        domain,
        expression,
        comparison(&altered, expression)
    ));
}

#[test]
fn domain_membership_rejects_a_sibling_self_root_grafted_into_its_predicate() {
    let mut program = typed(SOURCE);
    let first = program.domain_definitions()[0].clone();
    let second = program.domain_definitions()[1].clone();
    let expression = occurrence(&program, first.facts);
    let foreign = occurrence(&program, second.facts);
    let foreign_self = comparison(&program, foreign).left;
    assert!(has_exact_domain_case_membership_meaning(
        &program,
        &first,
        expression,
        comparison(&program, expression),
    ));
    assert!(has_exact_domain_case_membership_meaning(
        &program,
        &second,
        foreign,
        comparison(&program, foreign),
    ));
    let ExpressionNode::Binary(changed) = program.expression_table.expression_mut(expression)
    else {
        panic!("membership");
    };
    changed.left = foreign_self;
    assert!(!has_exact_domain_case_membership_meaning(
        &program,
        &first,
        expression,
        comparison(&program, expression),
    ));
    // Equal carriers do not give either owner exclusive custody of the now
    // shared reserved occurrence.
    assert!(!has_exact_domain_case_membership_meaning(
        &program,
        &second,
        foreign,
        comparison(&program, foreign),
    ));
}

#[test]
fn data_membership_rejects_foreign_bare_field_and_changed_nominal_carrier() {
    let program = typed(SOURCE);
    let wrapper = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Wrapper")
        .unwrap();
    let foreign = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Foreign")
        .unwrap();
    let typed_trees::data::DataMember::Field(foreign_field) = &program.data_members(foreign)[0]
    else {
        panic!("foreign field");
    };
    let expression = occurrence(&program, wrapper.where_facts);
    let subject = comparison(&program, expression).left;
    let mut altered = program.clone();
    let ExpressionNode::Name(path) = altered.expression_table.expression_mut(subject) else {
        panic!("bare field");
    };
    path.symbol = foreign_field.symbol;
    path.head_symbol = foreign_field.symbol;
    path.member_symbols = HandleSpan::empty();
    let mut path = *path;
    altered
        .expression_table
        .push_name_path_member_symbol(&mut path.member_symbols, foreign_field.symbol);
    *altered.expression_table.expression_mut(subject) = ExpressionNode::Name(path);
    assert!(!has_exact_data_case_membership_meaning(
        &altered,
        wrapper,
        expression,
        comparison(&altered, expression)
    ));
    let mut altered = program.clone();
    let typed_trees::data::DataMember::Field(field) =
        altered.data_members.get_mut(wrapper.members.start())
    else {
        panic!("wrapper field");
    };
    field.type_reference = foreign_field.type_reference;
    assert!(!has_exact_data_case_membership_meaning(
        &altered,
        wrapper,
        expression,
        comparison(&altered, expression)
    ));
}

#[test]
fn nested_definition_fields_compose_without_a_machine_owner() {
    let program = typed(
        "data Message { case Empty; case Data(value: u8); }
        data Inner { message: Message; }
        data Wrapper where inner.message in Message::Data, { inner: Inner; }
        domain Wrapper::Ready requires self.inner.message in Message::Data;",
    );
    let domain = &program.domain_definitions()[0];
    let expression = occurrence(&program, domain.facts);
    assert!(has_exact_domain_case_membership_meaning(
        &program,
        domain,
        expression,
        comparison(&program, expression)
    ));
    let data = program
        .data_definitions()
        .iter()
        .find(|data| !data.where_facts.is_empty())
        .unwrap();
    let expression = occurrence(&program, data.where_facts);
    assert!(has_exact_data_case_membership_meaning(
        &program,
        data,
        expression,
        comparison(&program, expression)
    ));
}
