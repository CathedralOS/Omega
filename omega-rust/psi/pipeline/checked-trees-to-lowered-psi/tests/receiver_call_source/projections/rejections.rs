//! Projected receiver transport does not authorize source access or aliasing.

use super::*;

fn rejects_source(source: &str, expected: &str) {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize receiver rejection");
    let syntax = parse_syntax_trees(&tokens).expect("parse receiver rejection");
    let resolved = lower_syntax_trees(&syntax).expect("resolve receiver rejection");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type receiver rejection");
    let Err(diagnostics) = typed_trees_to_checked_trees::lower_typed_trees(typed) else {
        panic!("source access or aliasing must reject before Terminal planning: {source}")
    };
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(expected)),
        "expected {expected}: {diagnostics:?}"
    );
}

#[test]
fn shared_container_cannot_supply_a_projected_mutable_receiver() {
    rejects_source(
        r#"
        data Record { value: u16; }
        data Container { record: Record; }
        machine Record::replace(&mut self) { self.value = 17; }
        machine invoke(container: &Container) { container.record.replace(); }
    "#,
        "requires a mutable receiver, but its source is not writable",
    );
}

#[test]
fn projected_mutable_receiver_cannot_alias_an_explicit_mutable_argument() {
    rejects_source(
        r#"
        data Record { value: u16; }
        data Container { record: Record; }
        machine Record::replace(&mut self, other: &mut Record) { self.value = 17; }
        machine invoke(container: &mut Container) {
            container.record.replace(&mut container.record);
        }
    "#,
        "mutable receiver overlapping another argument",
    );
}

#[test]
fn shared_whole_receiver_cannot_gain_mutable_authority() {
    rejects_source(
        r#"
        data Record { value: u16; }
        machine Record::replace(&mut self) { self.value = 17; }
        machine invoke(record: &Record) { record.replace(); }
    "#,
        "requires a mutable receiver, but its source is not writable",
    );
}
