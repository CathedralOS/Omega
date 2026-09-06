use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::data::DataMember;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

fn assert_indexed_member_identities(collection: &str, receiver: &str) {
    let source = format!(
        "data Decoy {{ end: i64; other: i64; }}
         data Endpoint {{ end: i64; other: i64; }}
         machine window(cells: &mut {collection}) -> i64 {{
             let end: i64 = {receiver}.end;
             let other: i64 = {receiver}.other;
             {receiver}.other = 1;
             end
         }}"
    );
    assert_source_member_identities(&source, "window", receiver);
}

fn assert_source_member_identities(source: &str, machine_name: &str, receiver: &str) {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let endpoint = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Endpoint")
        .expect("element declaration");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .expect("window machine");
    let state = &typed.machine_states(machine)[0];
    let mut checked_members = 0;
    for statement in typed.statement_table.statements(state.statement_nodes) {
        let (expression, field_name) = match statement {
            StatementNode::LocalData(local) if local.name.as_str() == "end" => {
                (local.initial_value, "end")
            }
            StatementNode::LocalData(local) if local.name.as_str() == "other" => {
                (local.initial_value, "other")
            }
            StatementNode::Assignment(assignment) => (assignment.target, "other"),
            _ => continue,
        };
        let expected = typed
            .data_members(endpoint)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == field_name => Some(field.symbol),
                _ => None,
            })
            .expect("exact element field");
        assert!(expected.is_valid());
        let ExpressionNode::Member(member) = typed.expression_table.expression(expression) else {
            panic!("expected member expression: {source}");
        };
        assert!(matches!(
            typed.expression_table.expression(member.receiver),
            ExpressionNode::Indexed(_)
        ));
        assert_eq!(
            member.member_symbol, expected,
            "{receiver}.{field_name} must bind the element declaration's exact field"
        );
        checked_members += 1;
    }
    assert_eq!(checked_members, 3, "both reads and the assignment target");
}

#[test]
fn indexed_members_bind_exact_element_field_symbols() {
    assert_indexed_member_identities("[Endpoint; 2]", "cells[0]");
}

#[test]
fn nested_indexed_members_bind_exact_element_field_symbols() {
    assert_indexed_member_identities("[[Endpoint; 2]; 2]", "cells[0][1]");
}

#[test]
fn slice_members_bind_exact_element_field_symbols() {
    assert_indexed_member_identities("[Endpoint]", "cells[0]");
}

#[test]
fn attached_indexed_members_bind_exact_element_field_symbols() {
    for receiver in ["self.cells[0]", "cells[0]"] {
        let source = format!(
            "data Decoy {{ end: i64; other: i64; }}
             data Endpoint {{ end: i64; other: i64; }}
             data Holder {{ cells: [Endpoint; 2]; }}
             machine Holder::window(&mut self) -> i64 {{
                 let end: i64 = {receiver}.end;
                 let other: i64 = {receiver}.other;
                 {receiver}.other = 1;
                 end
             }}"
        );
        assert_source_member_identities(&source, "Holder::window", receiver);
    }
}

fn resolved_member_fixture() -> symbol_resolved_trees::SymbolResolvedTrees {
    let source = "data Decoy { end: i64; }
        data Endpoint { end: i64; }
        machine read(cells: &[Endpoint; 2]) -> i64 { cells[0].end }";
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    lower_syntax_trees(&syntax).expect("resolve")
}

fn only_member_symbol(typed: &typed_trees::TypedTrees) -> symbols::SymbolHandle {
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "read")
        .expect("read machine");
    let state = &typed.machine_states(machine)[0];
    let expression = typed
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::Expression(expression) => Some(*expression),
            _ => None,
        })
        .expect("read result");
    let ExpressionNode::Member(member) = typed.expression_table.expression(expression) else {
        panic!("member result");
    };
    member.member_symbol
}

#[test]
fn explicit_indexed_member_selections_are_not_rebound() {
    use symbol_resolved_trees::data::DataMember as ResolvedDataMember;
    use symbol_resolved_trees::expression::ExpressionNode as ResolvedExpression;

    let original = resolved_member_fixture();
    let field = |owner: &str| {
        let definition = original
            .data_definitions
            .iter()
            .find(|definition| definition.name.as_str() == owner)
            .expect("owner");
        original
            .data_members(definition.members)
            .iter()
            .find_map(|member| match member {
                ResolvedDataMember::Field(field) => Some(field.symbol),
                _ => None,
            })
            .expect("field")
    };
    let exact = field("Endpoint");
    let foreign = field("Decoy");
    let stale = symbols::SymbolHandle::from_parts(exact.arena_index(), exact.generation() + 1);
    let expression = original
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .find_map(|(handle, node)| matches!(node, ResolvedExpression::Member(_)).then_some(handle))
        .expect("member");
    for selected in [exact, foreign, stale] {
        let mut resolved = original.clone();
        let ResolvedExpression::Member(member) = resolved
            .tables
            .bodies
            .expressions
            .expression_mut(expression)
        else {
            panic!("member");
        };
        member.member_symbol = selected;
        let typed = lower_symbol_resolved_trees(&resolved).expect("retain explicit selection");
        assert_eq!(only_member_symbol(&typed), selected);
    }
}

#[test]
fn missing_or_stale_collection_identity_cannot_bind_by_spelling() {
    use symbol_resolved_trees::expression::ExpressionNode as ResolvedExpression;

    let original = resolved_member_fixture();
    let (expression, symbol) = original
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .find_map(|(handle, node)| match node {
            ResolvedExpression::Name(path)
                if original.tables.bodies.expressions.display_name(handle) == "cells" =>
            {
                Some((handle, path.symbol))
            }
            _ => None,
        })
        .expect("collection");
    let stale = symbols::SymbolHandle::from_parts(symbol.arena_index(), symbol.generation() + 1);
    for replacement in [symbols::SymbolHandle::invalid(), stale] {
        let mut resolved = original.clone();
        let ResolvedExpression::Name(path) = resolved
            .tables
            .bodies
            .expressions
            .expression_mut(expression)
        else {
            panic!("collection");
        };
        path.head_symbol = replacement;
        path.symbol = replacement;
        path.member_symbols = arena::HandleSpan::empty();
        // Earlier identity validation may reject the malformed tree; successful
        // lowering must never repair its field from the unchanged spelling.
        if let Ok(typed) = lower_symbol_resolved_trees(&resolved) {
            assert!(!only_member_symbol(&typed).is_valid());
        }
    }
}
