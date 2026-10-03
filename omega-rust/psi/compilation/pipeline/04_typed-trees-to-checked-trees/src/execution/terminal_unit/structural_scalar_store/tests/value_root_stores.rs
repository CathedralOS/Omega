//! Store admission by value and destination root: a structural store mints
//! when its value operand is a bound local, a call result, an indexed
//! element read, or a constructed value, and when its destination is a
//! field path that ends in an element selection on a fixed-array member.
//! A slice-rooted local destination mints through the borrowed-window
//! element-write vocabulary — an element view names a window into the
//! receiver collection's storage, so the store mints the move-out/store
//! pair against the view's referent.
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

/// `slots[at] = node` through `&mut [Node]` mints the window pair: the
/// move-out retires the displaced element and the store books the
/// parameter-sourced value against the receiver's storage.
#[test]
fn slice_rooted_destination_admits_parameter_value() {
    admits(
        r#"
        data Node { tag: u64; }
        data Bag { nodes: [Node; 4]; }
        machine Bag::store(&mut self, bag: &mut Bag, at: u64, new_node: Node)
        requires
            at < bag.nodes.len
        {
            let slots: &mut [Node] = bag.nodes.as_mut_slice();
            slots[at] = new_node;
        }
    "#,
        "store",
    );
}

#[test]
fn nominal_owned_local_scalar_store_is_independent_of_establishment_source() {
    for initializer in ["Token { ready: false }", "make()"] {
        let checked = checked_program(&format!(
            r#"
            data Token {{ ready: bool; }}
            machine Token::drop(owner: &mut Token) requires owner.ready {{}}
            machine make() -> Token {{ Token {{ ready: false }} }}
            machine repair() -> Token {{
                let selected: Token = {initializer};
                selected.ready = true;
                selected
            }}
        "#
        ));
        let program = &checked.typed;
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "repair")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        assert!(program.state_parameters(state).is_empty());
        let parameters = Vec::new();
        let stores = super::build_structural_scalar_field_store_sequence(
            program,
            &checked.facts,
            machine,
            state,
            &parameters,
            &[],
            0,
            None,
        )
        .expect("owned nominal local must support its ordinary scalar field store");
        let [checked_trees::CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store)] =
            stores.as_slice()
        else {
            panic!("one authored field store, with no cleanup disguised as a store");
        };
        assert_eq!(store.field_identity, "ready");
        assert!(matches!(
            store.destination,
            checked_trees::CheckedStoreRoot::Local { .. }
        ));
    }
}

#[test]
fn nominal_shared_local_scalar_store_still_requires_exclusive_authority() {
    let checked = checked_program(
        r#"
        data Token { ready: bool; }
        machine Token::drop(owner: &mut Token) requires owner.ready {}
        machine observe(owner: &Token) {
            let selected: &Token = owner;
            selected.ready = true;
        }
    "#,
    );
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "observe")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let mut shapes = super::ShapeCollector::new(program);
    let (structural, scalar) =
        crate::execution::terminal_unit::calls::free_structural_scalar_signature(
            program,
            &mut shapes,
            state,
            &[],
        )
        .expect("the shared ingress itself is an ordinary valid signature");
    assert!(
        super::build_structural_scalar_field_store_sequence(
            program,
            &checked.facts,
            machine,
            state,
            &structural,
            &scalar,
            0,
            None,
        )
        .is_none(),
        "owning the descriptor must not grant mutable referent access"
    );
}
