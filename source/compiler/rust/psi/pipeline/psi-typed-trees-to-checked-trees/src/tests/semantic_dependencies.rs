use super::{
    Lexer, lower_symbol_resolved_trees, lower_syntax_trees, lower_typed_trees, parse_syntax_trees,
};
use psi_checked_trees::{
    CheckedSemanticDependencyExposure as Exposure, CheckedSemanticDependencyKind as Kind,
};

fn checked(source: &str) -> psi_checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("check")
}

#[test]
fn retains_nominal_semantics_carried_through_a_nested_call_result() {
    let checked = checked(
        r#"
        data Token { value: u64; }
        machine make() -> Token { Token { value: 7u64 } }
        machine consume(value: Token) {}
        machine relay() { consume(make()); }
        "#,
    );
    let relay = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "relay")
        .expect("relay machine")
        .symbol;
    let token = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Token")
        .expect("Token data")
        .symbol;
    let rows = &checked.facts.flow.semantic_dependencies.rows;

    for kind in [Kind::NominalIdentity, Kind::Layout, Kind::OwnershipBehavior] {
        assert!(
            rows.iter().any(|row| {
                row.consumer_machine == relay
                    && row.dependency == token
                    && row.exposure == Exposure::PrivateImplementation
                    && row.kind == kind
            }),
            "missing {kind:?}: {rows:#?}"
        );
    }
}

#[test]
fn public_machine_head_promotes_carried_nominals_to_public_interface() {
    let checked = checked(
        r#"
        pub data Token { value: u64; }
        pub machine make() -> Token { Token { value: 7u64 } }
        "#,
    );
    let make = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "make")
        .expect("make machine")
        .symbol;
    let token = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Token")
        .expect("Token data")
        .symbol;

    for kind in [Kind::NominalIdentity, Kind::Layout, Kind::OwnershipBehavior] {
        assert!(
            checked
                .facts
                .flow
                .semantic_dependencies
                .rows
                .iter()
                .any(|row| {
                    row.consumer_machine == make
                        && row.dependency == token
                        && row.exposure == Exposure::PublicInterface
                        && row.kind == kind
                })
        );
    }
}

#[test]
fn retains_the_exact_compiler_selected_cleanup_machine() {
    let checked = checked(
        r#"
        pub data Token { ready: bool; }
        machine Token::drop(&mut self) {}
        pub machine cleanup(value: Token) {}
        "#,
    );
    let cleanup = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "cleanup")
        .expect("cleanup consumer")
        .symbol;
    let drop_machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Token::drop")
        .expect("Token::drop")
        .symbol;
    let token = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Token")
        .expect("Token data")
        .symbol;
    let rows = &checked.facts.flow.semantic_dependencies.rows;

    assert!(
        rows.iter().any(|row| {
            row.consumer_machine == cleanup
                && row.dependency == token
                && row.exposure == Exposure::PublicInterface
                && row.kind == Kind::AutomaticCleanup
        }),
        "missing public automatic-cleanup type dependency: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.consumer_machine == cleanup
                && row.dependency == drop_machine
                && row.exposure == Exposure::PublicInterface
                && row.kind == Kind::AutomaticCleanupMachine
        }),
        "missing exact cleanup-machine dependency: {rows:#?}"
    );
}

#[test]
fn unrelated_drop_spelling_cannot_supply_automatic_cleanup() {
    let checked = checked(
        r#"
        data Token { ready: bool; }
        data Other { ready: bool; }
        machine Other::drop(&mut self) {}
        machine cleanup(value: Token) {}
        "#,
    );
    let cleanup = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "cleanup")
        .expect("cleanup consumer")
        .symbol;
    let unrelated_drop = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Other::drop")
        .expect("Other::drop")
        .symbol;

    assert!(
        !checked
            .facts
            .flow
            .semantic_dependencies
            .rows
            .iter()
            .any(|row| {
                row.consumer_machine == cleanup
                    && row.dependency == unrelated_drop
                    && row.kind == Kind::AutomaticCleanupMachine
            })
    );
}
