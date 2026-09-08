use source::SourceMap;
use source_files_to_tokens::Lexer;
use std::{path::PathBuf, sync::Arc};
use symbol_resolved_trees::data::DataMember;
use symbol_resolved_trees::expression::ExpressionNode;
use symbol_resolved_trees::types::TypeReference;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources;
use tokens_to_syntax_trees::parse_syntax_trees_with_id;

#[test]
fn declared_module_retains_namespace_for_local_references() {
    assert_module_reference_symbols(
        r#"
        module dungeon::combat;

        data Damage { amount: u64; }
        data Attack { local: Damage; }

        machine damage() -> u64 { 1 }
        machine local_attack() -> u64 { damage() }
        "#,
        1,
        1,
    );
}

#[test]
fn declared_module_qualified_and_local_references_share_exact_symbols() {
    assert_module_reference_symbols(
        r#"
        module dungeon::combat;

        data Damage { amount: u64; }
        data Attack {
            local: Damage;
            qualified: dungeon::combat::Damage;
        }

        machine damage() -> u64 { 1 }
        machine local_attack() -> u64 { damage() }
        machine qualified_attack() -> u64 { dungeon::combat::damage() }
        "#,
        2,
        2,
    );
}

fn assert_module_reference_symbols(
    source: &str,
    expected_field_count: usize,
    expected_call_count: usize,
) {
    let mut sources = SourceMap::default();
    let source_id = sources
        .add(PathBuf::from("combat.omg"), source.to_owned())
        .source_id;
    let tokens = Lexer::new(source).tokenize().expect("tokenize module");
    let syntax = parse_syntax_trees_with_id(source_id, &tokens).expect("parse module");
    let program = lower_syntax_trees_with_sources(&syntax, Arc::new(sources))
        .expect("resolve declared module references");

    let damage = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Damage")
        .expect("Damage declaration");
    assert_eq!(
        program.symbols.display_path(damage.symbol, "::"),
        "dungeon::combat::Damage"
    );
    let attack = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Attack")
        .expect("Attack declaration");
    let fields = program.data_members(attack.members);
    assert_eq!(fields.len(), expected_field_count);
    for field in fields {
        let DataMember::Field(field) = field else {
            panic!("Attack contains only fields");
        };
        let TypeReference::Named { symbol, .. } = &field.type_reference else {
            panic!("Damage field retains nominal identity");
        };
        assert_eq!(*symbol, damage.symbol);
    }

    let damage_machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "damage")
        .expect("damage machine");
    assert_eq!(
        program.symbols.display_path(damage_machine.symbol, "::"),
        "dungeon::combat::damage"
    );
    let [entry] = program
        .tables
        .declarations
        .machine_state_handles
        .span_or_empty(damage_machine.states)
    else {
        panic!("one damage entry state");
    };
    let entry_symbol = program
        .tables
        .declarations
        .machine_states
        .get(*entry)
        .symbol;
    let mut call_count = 0;
    for (_, expression) in program.tables.bodies.expressions.iter_expressions() {
        if let ExpressionNode::Call(call) = expression {
            assert_eq!(call.target_symbol, entry_symbol);
            call_count += 1;
        }
    }
    assert_eq!(call_count, expected_call_count);
}
