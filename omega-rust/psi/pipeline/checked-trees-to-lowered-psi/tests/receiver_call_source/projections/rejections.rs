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

fn rejects_projected_write_only_callee(
    caller_borrow: &str,
    body: &str,
    declarations: &str,
    expected: &str,
) {
    for nested in [false, true] {
        for (self_caller, bare_self) in [(false, false), (true, false), (true, true)] {
            for caller_first in [false, true] {
                let (source, _, _) = projected_source(
                    caller_borrow,
                    "write",
                    nested,
                    self_caller,
                    false,
                    caller_first,
                    bare_self,
                );
                let source = source.replace("self.value = 17;", body);
                rejects_source(&format!("{source}\n{declarations}"), expected);
            }
        }
    }
}

#[test]
fn shared_container_cannot_supply_a_projected_write_only_receiver() {
    rejects_projected_write_only_callee(
        "",
        "self.value = 17;",
        "",
        "requires a write-only receiver, but its source is not writable",
    );
}

#[test]
fn attenuated_write_only_callee_cannot_read_after_store() {
    rejects_projected_write_only_callee(
        "mut",
        "self.value = 17; let observed: u16 = self.value;",
        "",
        "reads field `value` from write-only parameter `self`",
    );
}

#[test]
fn attenuated_write_only_callee_cannot_forward_readable_access() {
    for (borrow, expected) in [
        ("", "reads write-only parameter `self`"),
        ("mut", "widens write-only parameter `self` to `&mut`"),
    ] {
        rejects_projected_write_only_callee(
            "mut",
            &format!("self.value = 17; observe(&{borrow} self);"),
            &format!("machine observe(record: &{borrow} Record) {{}}"),
            expected,
        );
    }
}

#[test]
fn attenuated_projected_receiver_cannot_overlap_an_explicit_argument() {
    for nested in [false, true] {
        for (self_caller, bare_self) in [(false, false), (true, false), (true, true)] {
            for caller_first in [false, true] {
                let (source, _, field_names) = projected_source(
                    "mut",
                    "write",
                    nested,
                    self_caller,
                    false,
                    caller_first,
                    bare_self,
                );
                // Independently vary the argument spelling: bare and explicit
                // self must denote the same loan even when mixed in one call.
                let argument_spellings: &[bool] = if self_caller {
                    &[false, true]
                } else {
                    &[false]
                };
                for &bare_argument in argument_spellings {
                    let root = if self_caller { "self" } else { "container" };
                    let argument_receiver = if bare_argument {
                        field_names.join(".")
                    } else {
                        format!("{root}.{}", field_names.join("."))
                    };
                    let source = source
                        .replace("&write self)", "&write self, other: &write u16)")
                        .replace(
                            ".replace();",
                            &format!(".replace(&write {argument_receiver}.value);"),
                        );
                    // Even an empty callee must exclude its overlapping argument.
                    for body in ["", "self.value = 17;"] {
                        rejects_source(
                            &source.replace("self.value = 17;", body),
                            "write-only receiver overlapping another argument in the same call",
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn attenuated_receiver_allows_an_exact_disjoint_attached_field_argument() {
    for receiver in ["inner.record", "self.inner.record"] {
        for argument in ["inner.other.value", "self.inner.other.value"] {
            let source = format!(
                "data Record {{ value: u16; }}
                 data Inner {{ record: Record; other: Record; }}
                 data Container {{ inner: Inner; }}
                 machine Record::replace(&write self, other: &write u16) {{
                     self.value = 17;
                     other = 23;
                 }}
                 machine Container::forward(&mut self) {{
                     {receiver}.replace(&write {argument});
                 }}"
            );
            checked_from_source(&source);
        }
    }
}
