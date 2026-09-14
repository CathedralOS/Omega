use source::SourceMap;
use source_files_to_tokens::Lexer;
use std::{path::PathBuf, sync::Arc};
use symbol_resolved_trees::SymbolResolvedTrees;
use symbol_resolved_trees::data::DataMember;
use symbol_resolved_trees::expression::ExpressionNode;
use symbol_resolved_trees::types::TypeReference;
use syntax_trees::SyntaxTrees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources;
use tokens_to_syntax_trees::{parse_syntax_trees_into_with_id, parse_syntax_trees_with_id};

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

fn lower_multi(sources: &[(&str, &str)]) -> SymbolResolvedTrees {
    let mut map = SourceMap::default();
    let mut syntax = SyntaxTrees::default();
    for (name, text) in sources {
        let id = map.add(PathBuf::from(name), (*text).to_owned()).source_id;
        let tokens = Lexer::new(text).tokenize().expect("tokenize");
        parse_syntax_trees_into_with_id(&mut syntax, id, &tokens).expect("parse");
    }
    lower_syntax_trees_with_sources(&syntax, Arc::new(map)).expect("lower")
}

fn membership_domain_symbols(program: &SymbolResolvedTrees) -> Vec<symbols::SymbolHandle> {
    program
        .tables
        .bodies
        .expressions
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            ExpressionNode::Membership(membership) => Some(membership.domain_symbol),
            _ => None,
        })
        .collect()
}

#[test]
fn module_owned_domain_and_operator_home_retain_exact_namespace() {
    let mut program = lower_multi(&[(
        "units.omg",
        r#"
        module units;
        pub domain u64::Distance requires self > 0;
        pub operator + u64::Distance::add(left: u64 in u64::Distance, right: u64 in u64::Distance) -> u64 in u64::Distance;
        "#,
    )]);
    let qualified_id = program.semantic_domains.intern("units::u64::Distance");
    let domain = program
        .domain_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "u64::Distance")
        .expect("module domain");
    assert_eq!(
        program.symbols.display_path(domain.symbol, "::"),
        "units::u64::Distance"
    );
    assert_eq!(
        domain.semantic_id, qualified_id,
        "module-owned domains intern under their complete logical path"
    );
    let [operator] = program.operator_definitions(domain.operators) else {
        panic!("the operator homed into the module domain")
    };
    assert!(
        program.operators.is_empty(),
        "no root operator remains after domain homing"
    );
    assert_eq!(
        program.symbols.display_path(operator.symbol, "::"),
        "units::u64::Distance::add"
    );
}

#[test]
fn module_domain_membership_selects_relative_and_qualified_names() {
    let program = lower_multi(&[(
        "units.omg",
        r#"
        module units;
        pub domain u64::Distance requires self > 0;
        machine near(value: u64) -> bool { value in u64::Distance }
        machine far(value: u64) -> bool { value in units::u64::Distance }
        "#,
    )]);
    let domain = program
        .domain_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "u64::Distance")
        .expect("module domain");
    let selections = membership_domain_symbols(&program);
    assert_eq!(selections.len(), 2, "both memberships resolve");
    for selected in selections {
        assert_eq!(selected, domain.symbol);
    }
}

#[test]
fn foreign_module_domain_needs_exact_or_imported_spelling() {
    let unimported = lower_multi(&[
        (
            "units.omg",
            "module units; pub domain u64::Distance requires self > 0;",
        ),
        (
            "check.omg",
            "machine check(value: u64) -> bool { value in u64::Distance }",
        ),
    ]);
    let [selection] = membership_domain_symbols(&unimported)
        .try_into()
        .expect("one membership");
    assert!(
        !selection.is_valid(),
        "a relative spelling must not reach a foreign module's domain"
    );

    let qualified = lower_multi(&[
        (
            "units.omg",
            "module units; pub domain u64::Distance requires self > 0;",
        ),
        (
            "check.omg",
            "machine check(value: u64) -> bool { value in units::u64::Distance }",
        ),
    ]);
    let domain = qualified
        .domain_definitions
        .iter()
        .next()
        .expect("module domain");
    let [selection] = membership_domain_symbols(&qualified)
        .try_into()
        .expect("one membership");
    assert_eq!(
        selection, domain.symbol,
        "a fully qualified path selects the exact module domain"
    );
}

#[test]
fn module_domain_proof_facts_select_their_own_domain() {
    let program = lower_multi(&[(
        "units.omg",
        r#"
        module units;
        domain u64::Long requires self > 0;
        domain u64::Distance requires self in u64::Long;
        "#,
    )]);
    let long = program
        .domain_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "u64::Long")
        .expect("u64::Long domain");
    let domain = program
        .domain_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "u64::Distance")
        .expect("u64::Distance domain");
    let [fact] = program.proof_facts(domain.facts) else {
        panic!("one proof fact")
    };
    let symbol_resolved_trees::domain::ProofFact::Membership(membership) = fact else {
        panic!("membership fact")
    };
    assert_eq!(
        membership.domain_symbol, long.symbol,
        "a relative proof-fact path selects the same-module domain"
    );
}
