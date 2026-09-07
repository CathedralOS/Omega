//! Ordinary contract members retain declaration identities before checking.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

fn resolved(requirement: &str) -> symbol_resolved_trees::SymbolResolvedTrees {
    let source = format!(
        "data Decoy {{ flag: bool; }}
         data Inner {{ flag: bool; }}
         data Record {{ flag: bool; inner: Inner; }}
         machine inspect(record: &Record) -> bool
         requires {requirement}
         {{ true }}"
    );
    let tokens = Lexer::new(&source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    lower_syntax_trees(&syntax).expect("resolve")
}

fn requirement(program: &typed_trees::TypedTrees) -> ExpressionHandle {
    program
        .machine_contracts(&program.machines()[0])
        .iter()
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .find_map(|fact| match fact {
            typed_trees::domain::ProofFact::Expression(expression) => Some(*expression),
            _ => None,
        })
        .expect("source requirement expression")
}

fn field(program: &typed_trees::TypedTrees, owner: &str, name: &str) -> symbols::SymbolHandle {
    let owner = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == owner)
        .expect("owner declaration");
    program
        .data_members(owner)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == name => Some(field.symbol),
            _ => None,
        })
        .expect("exact field declaration")
}

#[test]
fn ordinary_requires_member_retains_the_exact_declared_field() {
    let program = lower_symbol_resolved_trees(&resolved("record.flag")).expect("type");
    let ExpressionNode::Member(member) = program.expression_table.expression(requirement(&program))
    else {
        panic!("the requirement is a member predicate")
    };
    assert_eq!(member.member_symbol, field(&program, "Record", "flag"));
    assert_ne!(member.member_symbol, field(&program, "Decoy", "flag"));
    let ExpressionNode::Name(receiver) = program.expression_table.expression(member.receiver)
    else {
        panic!("the receiver is the exact entry formal")
    };
    let parameter =
        &program.state_parameters(&program.machine_states(&program.machines()[0])[0])[0];
    assert_eq!(receiver.symbol, parameter.symbol);
    assert_eq!(receiver.head_symbol, parameter.symbol);
}

#[test]
fn nested_requires_members_retain_each_exact_declared_field() {
    let program = lower_symbol_resolved_trees(&resolved("record.inner.flag")).expect("type");
    let ExpressionNode::Member(leaf) = program.expression_table.expression(requirement(&program))
    else {
        panic!("the requirement is a nested member predicate")
    };
    assert_eq!(leaf.member_symbol, field(&program, "Inner", "flag"));
    assert_ne!(leaf.member_symbol, field(&program, "Record", "flag"));
    let ExpressionNode::Member(inner) = program.expression_table.expression(leaf.receiver) else {
        panic!("the inner receiver is itself a member")
    };
    assert_eq!(inner.member_symbol, field(&program, "Record", "inner"));
}

#[test]
fn explicit_ordinary_member_selections_are_not_rebound() {
    use symbol_resolved_trees::data::DataMember as ResolvedDataMember;
    use symbol_resolved_trees::expression::ExpressionNode as ResolvedExpression;

    let original = resolved("record.flag");
    let field = |owner: &str| {
        let definition = original
            .data_definitions
            .iter()
            .find(|definition| definition.name.as_str() == owner)
            .expect("owner declaration");
        original
            .data_members(definition.members)
            .iter()
            .find_map(|member| match member {
                ResolvedDataMember::Field(field) if field.name.as_str() == "flag" => {
                    Some(field.symbol)
                }
                _ => None,
            })
            .expect("field declaration")
    };
    let exact = field("Record");
    let foreign = field("Decoy");
    let stale = symbols::SymbolHandle::from_parts(exact.arena_index(), exact.generation() + 1);
    let expression = original
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .find_map(|(handle, node)| matches!(node, ResolvedExpression::Member(_)).then_some(handle))
        .expect("requirement member");
    for selected in [exact, foreign, stale] {
        let mut resolved = original.clone();
        let ResolvedExpression::Member(member) = resolved
            .tables
            .bodies
            .expressions
            .expression_mut(expression)
        else {
            panic!("requirement member");
        };
        member.member_symbol = selected;
        let typed = lower_symbol_resolved_trees(&resolved).expect("retain explicit selection");
        let ExpressionNode::Member(member) = typed.expression_table.expression(requirement(&typed))
        else {
            panic!("typed requirement member");
        };
        assert_eq!(member.member_symbol, selected);
    }
}

#[test]
fn missing_or_stale_ordinary_receiver_cannot_bind_by_spelling() {
    use symbol_resolved_trees::expression::ExpressionNode as ResolvedExpression;

    for predicate in ["record.flag", "record.inner.flag"] {
        let original = resolved(predicate);
        let (expression, symbol) = original
            .tables
            .bodies
            .expressions
            .iter_expressions()
            .find_map(|(handle, node)| match node {
                ResolvedExpression::Name(path)
                    if original.tables.bodies.expressions.display_name(handle) == "record" =>
                {
                    Some((handle, path.symbol))
                }
                _ => None,
            })
            .expect("entry receiver");
        let stale =
            symbols::SymbolHandle::from_parts(symbol.arena_index(), symbol.generation() + 1);
        for replacement in [symbols::SymbolHandle::invalid(), stale] {
            let mut resolved = original.clone();
            let ResolvedExpression::Name(path) = resolved
                .tables
                .bodies
                .expressions
                .expression_mut(expression)
            else {
                panic!("entry receiver");
            };
            path.head_symbol = replacement;
            path.symbol = replacement;
            path.member_symbols = arena::HandleSpan::empty();
            // Earlier validation may reject corruption. If lowering succeeds,
            // the unchanged spelling must not restore a declaration identity.
            if let Ok(typed) = lower_symbol_resolved_trees(&resolved) {
                let ExpressionNode::Member(member) =
                    typed.expression_table.expression(requirement(&typed))
                else {
                    panic!("typed requirement member");
                };
                assert!(!member.member_symbol.is_valid());
            }
        }
    }
}
