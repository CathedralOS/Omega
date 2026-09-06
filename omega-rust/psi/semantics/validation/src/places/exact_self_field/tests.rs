use super::*;
use source::SourceMap;
use source_files_to_tokens::Lexer;
use symbols::SymbolHandle;

fn fixture() -> TypedTrees {
    fixture_with_sources(true)
}

fn fixture_with_sources(retain_sources: bool) -> TypedTrees {
    let source = "data Counter { index: i32; spare: i32; }
        machine Counter::read(&self, other: &Counter) -> i32 {
            let saved: i32 = other.index;
            self.index
        }";
    let tokens = Lexer::new(source).tokenize().expect("tokenize counters");
    let mut sources = SourceMap::default();
    let source_id = sources
        .add("counter_fields.omg".into(), source.to_owned())
        .source_id;
    let syntax = tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens)
        .expect("parse counters");
    let resolved = if retain_sources {
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
            &syntax,
            std::sync::Arc::new(sources),
        )
    } else {
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
    }
    .expect("resolve counters");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type counters")
}

fn member(program: &TypedTrees, label: &str) -> ExpressionHandle {
    program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, expression)| {
            (matches!(expression, ExpressionNode::Member(_))
                && program.expression_table.display_name(handle) == label)
                .then_some(handle)
        })
        .expect("fixture member")
}

#[test]
fn exact_self_field_is_not_another_receiver_of_the_same_type() {
    let mut program = fixture();
    let own = member(&program, "self.index");
    let other = member(&program, "other.index");
    let field = exact_self_field(&program, &program.machines()[0], own)
        .expect("own field")
        .symbol;
    let ExpressionNode::Member(other_member) = program.expression_table.expression_mut(other)
    else {
        unreachable!()
    };
    // Even the correct declaration's field symbol cannot change its receiver.
    other_member.member_symbol = field;
    assert!(exact_self_field(&program, &program.machines()[0], other).is_none());
}

#[test]
fn inherited_field_slots_do_not_require_source_span_identity() {
    let program = fixture_with_sources(false);
    let own = member(&program, "self.index");
    assert!(exact_self_field(&program, &program.machines()[0], own).is_some());
}

#[test]
fn missing_field_symbol_resolves_only_under_the_exact_self_owner() {
    let mut program = fixture();
    let own = member(&program, "self.index");
    let expected = exact_self_field(&program, &program.machines()[0], own)
        .unwrap()
        .symbol;
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(own) else {
        unreachable!()
    };
    member.member_symbol = SymbolHandle::invalid();
    assert_eq!(
        exact_self_field(&program, &program.machines()[0], own)
            .unwrap()
            .symbol,
        expected
    );

    let mut detached = program.machines()[0].clone();
    detached.attached_data_symbol = SymbolHandle::invalid();
    assert!(exact_self_field(&program, &detached, own).is_none());
}

#[test]
fn conflicting_field_symbol_cannot_use_the_old_member_name() {
    let mut program = fixture();
    let own = member(&program, "self.index");
    let conflicting = program.machines()[0].symbol;
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(own) else {
        unreachable!()
    };
    member.member_symbol = conflicting;
    assert!(exact_self_field(&program, &program.machines()[0], own).is_none());
}

#[test]
fn another_inherited_field_slot_cannot_use_the_old_member_name() {
    let mut program = fixture();
    let own = member(&program, "self.index");
    let other = program
        .symbols
        .child_handles(program.machines()[0].symbol)
        .unwrap()
        .filter(|symbol| program.symbols.get(*symbol).kind == SymbolKind::Field)
        .nth(1)
        .expect("inherited spare field");
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(own) else {
        unreachable!()
    };
    member.member_symbol = other;
    assert!(exact_self_field(&program, &program.machines()[0], own).is_none());
}

#[test]
fn absent_symbols_do_not_merge_distinct_fields_of_the_same_owner() {
    let mut program = fixture();
    let own = member(&program, "self.index");
    let expected = exact_self_field(&program, &program.machines()[0], own)
        .unwrap()
        .symbol;
    let ExpressionNode::Member(mut spare) = program.expression_table.expression(own).clone() else {
        unreachable!()
    };
    spare.member_symbol = SymbolHandle::invalid();
    spare.member = typed_trees::name::Identifier::generated("spare");
    let spare = program
        .expression_table
        .insert(ExpressionNode::Member(spare));
    let actual = exact_self_field(&program, &program.machines()[0], spare)
        .unwrap()
        .symbol;
    assert!(actual.is_valid());
    assert_ne!(actual, expected);
}

#[test]
fn missing_or_conflicting_self_root_cannot_use_its_spelling() {
    for clear_head in [false, true] {
        let mut program = fixture();
        let own = member(&program, "self.index");
        let ExpressionNode::Member(member) = program.expression_table.expression(own) else {
            unreachable!()
        };
        let receiver = member.receiver;
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(receiver) else {
            unreachable!()
        };
        if clear_head {
            path.head_symbol = SymbolHandle::invalid();
        } else {
            path.symbol = SymbolHandle::invalid();
        }
        assert!(exact_self_field(&program, &program.machines()[0], own).is_none());
    }
}

#[test]
fn case_projection_is_not_a_direct_attached_field() {
    let mut program = fixture();
    let own = member(&program, "self.index");
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(own) else {
        unreachable!()
    };
    member.case_variant = Some(typed_trees::name::Identifier::generated("Case"));
    assert!(exact_self_field(&program, &program.machines()[0], own).is_none());
}
