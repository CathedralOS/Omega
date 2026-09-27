//! Store admission by value and destination root: a structural store mints
//! when its value operand is a bound local, a call result, an indexed
//! element read, or a constructed value, and when its destination is a
//! field path that ends in an element selection on a fixed-array member.
//! Slice-rooted destinations stay declined — the element write vocabulary
//! for borrowed slice storage is a separate family.
use super::checked_program;

/// The omission stage a machine declines at, or `None` when every
/// statement planned.
fn omission(source: &str, name: &str) -> Option<String> {
    let checked = checked_program(source);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str().ends_with(name))
        .unwrap_or_else(|| panic!("missing machine {name}"));
    checked
        .facts
        .flow
        .terminal_unit_effects
        .omission_for_machine(machine.symbol)
        .map(|row| format!("{row:?}"))
}

fn admits(source: &str, name: &str) {
    assert_eq!(
        omission(source, name),
        None,
        "{name} declined instead of minting"
    );
}

#[test]
fn field_destination_admits_bound_local_value() {
    admits(
        r#"
        data Fmt [copy] { case Decimal; case Hex; }
        data Holder { fmt: Fmt; }
        machine Holder::set(&mut self, fresh: Fmt) {
            let v: Fmt = fresh;
            self.fmt = v;
        }
    "#,
        "set",
    );
}

#[test]
fn field_destination_admits_indexed_element_value() {
    admits(
        r#"
        data Fmt [copy] { case Decimal; case Hex; }
        data Holder { items: [Fmt; 4]; fmt: Fmt; }
        machine Holder::set(&mut self) {
            self.fmt = self.items[1];
        }
    "#,
        "set",
    );
}

/// A borrowed slice root is not a window destination: `nodes[at] = node`
/// on `&mut [Node]` needs the element-write vocabulary the spec reserves
/// (`ElementViewWrite`), not the structural field store.
#[test]
fn slice_rooted_destination_still_declines() {
    let omission = omission(
        r#"
        data Node { tag: u64; }
        machine Node::store(&mut self, nodes: &mut [Node], at: u64 [0..nodes.len], new_node: Node) {
            nodes[at] = new_node;
        }
    "#,
        "store",
    )
    .expect("slice-rooted store still declines");
    // The value arm now admits the owned parameter source, so the slice
    // destination declines one arm later than the scalar lane's own phase.
    assert!(
        omission.contains("call source result type"),
        "unexpected decline phase: {omission}"
    );
}
