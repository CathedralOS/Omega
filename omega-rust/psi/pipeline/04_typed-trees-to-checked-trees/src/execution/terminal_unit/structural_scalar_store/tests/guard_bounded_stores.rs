use super::{
    ShapeCollector, build_structural_scalar_field_store_sequence, checked_program, machine_binders,
};
use crate::execution::terminal_unit::calls::structural_scalar_signature;

fn stores_for_store_state(checked: &checked_trees::CheckedTrees) -> bool {
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Ring::poke")
        .unwrap();
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == "store")
        .unwrap();
    let mut shapes = ShapeCollector::new(program);
    let (_, structural, scalar) = structural_scalar_signature(
        program,
        &mut shapes,
        machine,
        state,
        &machine_binders(program, machine),
        true,
    )
    .expect("attached state signature");
    build_structural_scalar_field_store_sequence(
        program,
        &checked.facts,
        machine,
        state,
        &structural,
        &scalar,
        0,
        None,
    )
    .is_some()
}

/// The indexed-access checker proves bounds through its whole-machine meet --
/// propagated declared ranges, literal arguments, and interval arithmetic.
/// The store planner no longer replays a narrower syntactic proof of its
/// own: a selector bounded only through those channels still retains its
/// runtime element, and Terminal decides from the facts that reach the store.
#[test]
fn guard_bounded_index_retains_bounds_the_edge_does_not_carry() {
    // A declared range on the CALLER's parameter flows through the meet
    // but appears in no guard conjunct on the edge into `store`.
    let declared_on_caller = r#"
        data Ring { cells: [u16; 4]; }
        machine Ring::poke(&mut self, value: u16) {
            transition { _ -> fill(0, value) }
            state fill(&mut self, position: u64 [0..4], value: u16) {
                transition { _ -> store(position, value) }
            }
            state store(&mut self, position: u64, value: u16) {
                self.cells[position] = value;
            }
        }
    "#;
    assert!(
        stores_for_store_state(&checked_program(declared_on_caller)),
        "caller-declared bound is a meet fact the planner no longer re-proves",
    );

    // `position + 1` is bounded by interval arithmetic on `position < 3`;
    // the edge conjunct does not spell the argument expression itself.
    let arithmetic_argument = r#"
        data Ring { cells: [u16; 4]; }
        machine Ring::poke(&mut self, value: u16) {
            transition { _ -> fill(0, value) }
            state fill(&mut self, position: u64, value: u16) {
                transition position < 3 {
                    true -> store(position + 1, value)
                }
                transition { _ -> done() }
            }
            state store(&mut self, position: u64, value: u16) {
                self.cells[position] = value;
            }
            state done(&mut self) {}
        }
    "#;
    assert!(
        stores_for_store_state(&checked_program(arithmetic_argument)),
        "an argument bounded by interval arithmetic keeps its runtime element",
    );

    // A constant argument is trivially in-extent but carries no conjunct.
    let literal_argument = r#"
        data Ring { cells: [u16; 4]; }
        machine Ring::poke(&mut self, value: u16) {
            transition { _ -> store(0, value) }
            state store(&mut self, position: u64, value: u16) {
                self.cells[position] = value;
            }
        }
    "#;
    assert!(
        stores_for_store_state(&checked_program(literal_argument)),
        "a literal argument's selector keeps its runtime element",
    );
}
