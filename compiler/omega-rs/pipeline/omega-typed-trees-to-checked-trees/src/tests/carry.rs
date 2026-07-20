use super::{
    Lexer, lower_symbol_resolved_trees, lower_syntax_trees, lower_typed_trees, parse_syntax_trees,
};

#[test]
fn checked_facts_store_declared_and_effective_carry_separately() {
    let source = r#"
        data Inner { value: i32; }
        data Outer [carry(
            suspension: allowed,
            cpu: any,
            thread: any,
            address: movable,
        )] { inner: Inner; }
        data Envelope<T> { value: T; }
        data Concrete { value: Envelope<i32>; }
        data Conservative { borrowed: &i32; }
        data Main {}
        machine Main::run(&mut self) {}
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");

    let outer = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Outer")
        .expect("Outer");
    let outer_fact = checked
        .facts
        .carry
        .for_data(outer.symbol)
        .expect("carry fact");
    assert_eq!(
        outer_fact.declared,
        Some(omega_core::semantics::CarryPolicy::PERMISSIVE)
    );
    assert_eq!(
        outer_fact.effective,
        omega_core::semantics::CarryPolicy::PERMISSIVE
    );

    let concrete = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Concrete")
        .expect("Concrete");
    assert_eq!(
        checked
            .facts
            .carry
            .for_data(concrete.symbol)
            .expect("carry fact")
            .effective,
        omega_core::semantics::CarryPolicy::PERMISSIVE
    );

    let conservative = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "Conservative")
        .expect("Conservative");
    let conservative_fact = checked
        .facts
        .carry
        .for_data(conservative.symbol)
        .expect("carry fact");
    assert_eq!(conservative_fact.declared, None);
    assert_eq!(
        conservative_fact.effective,
        omega_core::semantics::CarryPolicy::STRICT
    );
}
