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
fn indexed_element_destination_admits_constructor_value() {
    admits(
        r#"
        data Fmt [copy] { case Decimal; case Hex; }
        data Arr { items: [Fmt; 4]; }
        machine Arr::set(&mut self) {
            self.items[0] = Fmt::Hex;
        }
    "#,
        "set",
    );
}

#[test]
fn indexed_element_destination_admits_bound_local_value() {
    admits(
        r#"
        data Fmt [copy] { case Decimal; case Hex; }
        data Arr { items: [Fmt; 4]; }
        machine Arr::set(&mut self, fresh: Fmt) {
            let v: Fmt = fresh;
            self.items[1] = v;
        }
    "#,
        "set",
    );
}

#[test]
fn indexed_element_destination_admits_call_result_value() {
    admits(
        r#"
        data Node { tag: u64; }
        data Holder { nodes: [Node; 4]; }
        machine Node::clone(source: &Node) -> Node { Node { tag: source.tag } }
        machine Holder::set(&mut self, field: &Node) {
            self.nodes[1] = Node::clone(field);
        }
    "#,
        "set",
    );
}

#[test]
fn indexed_element_destination_admits_scalar_value() {
    admits(
        r#"
        data Text { bytes: [u8; 16]; len: u64; }
        machine Text::put_byte(&mut self, byte: u8) {
            transition self.len < 16 {
                true -> store(byte)
                false -> done()
            }
            state store(&mut self, byte: u8) {
                self.bytes[self.len] = byte;
            }
            state done(&mut self) {}
        }
    "#,
        "put_byte",
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

#[test]
fn field_destination_admits_call_result_value() {
    admits(
        r#"
        data Node { tag: u64; }
        data Holder { node: Node; }
        machine Node::clone(source: &Node) -> Node { Node { tag: source.tag } }
        machine Holder::set(&mut self, field: &Node) {
            self.node = Node::clone(field);
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

#[test]
fn indexed_element_destination_admits_parameter_value() {
    admits(
        r#"
        data Fmt [copy] { case Decimal; case Hex; }
        data Node { tag: u64; fmt: Fmt; }
        data Holder { nodes: [Node; 4]; }
        machine Holder::fill(&mut self, at: u64, node: Node) requires at < self.nodes.len {
            self.nodes[at] = node;
        }
    "#,
        "fill",
    );
}

#[test]
fn indexed_element_destination_admits_bound_record_value() {
    admits(
        r#"
        data Fmt [copy] { case Decimal; case Hex; }
        data Node { tag: u64; fmt: Fmt; }
        data Holder { nodes: [Node; 4]; }
        machine Holder::fill(&mut self, at: u64) requires at < self.nodes.len {
            let node: Node = Node { tag: 1, fmt: Fmt::Hex };
            self.nodes[at] = node;
        }
    "#,
        "fill",
    );
}

#[test]
fn fixed_element_destination_admits_parameter_value() {
    admits(
        r#"
        data Fmt [copy] { case Decimal; case Hex; }
        data Node { tag: u64; fmt: Fmt; }
        data Holder { nodes: [Node; 4]; }
        machine Holder::fill(&mut self, node: Node) {
            self.nodes[1] = node;
        }
    "#,
        "fill",
    );
}

#[test]
fn fixed_element_destination_admits_bound_local_value() {
    admits(
        r#"
        data Fmt [copy] { case Decimal; case Hex; }
        data Node { tag: u64; fmt: Fmt; }
        data Holder { nodes: [Node; 4]; }
        machine Holder::fill(&mut self) {
            let node: Node = Node { tag: 1, fmt: Fmt::Hex };
            self.nodes[1] = node;
        }
    "#,
        "fill",
    );
}

#[test]
fn field_destination_admits_parameter_value() {
    admits(
        r#"
        data Fmt [copy] { case Decimal; case Hex; }
        data Node { tag: u64; fmt: Fmt; }
        data Holder { inner: Node; }
        machine Holder::fill(&mut self, node: Node) {
            self.inner = node;
        }
    "#,
        "fill",
    );
}

#[test]
fn field_destination_admits_bound_record_value() {
    admits(
        r#"
        data Fmt [copy] { case Decimal; case Hex; }
        data Node { tag: u64; fmt: Fmt; }
        data Holder { inner: Node; }
        machine Holder::fill(&mut self) {
            let node: Node = Node { tag: 1, fmt: Fmt::Hex };
            self.inner = node;
        }
    "#,
        "fill",
    );
}
