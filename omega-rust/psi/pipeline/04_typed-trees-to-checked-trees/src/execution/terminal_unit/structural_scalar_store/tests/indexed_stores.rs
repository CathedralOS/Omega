use super::{
    CheckedUnitEffectOperationPlan, ShapeCollector, build_structural_scalar_field_store_sequence,
    checked_program, machine_binders,
};
use crate::execution::terminal_unit::calls::structural_scalar_signature;
use checked_trees::types::PrimitiveType;

/// An index read off a state-carried parameter stores through the primitive
/// store whose path ends at the selected element: `self.cells[position]` is
/// `[cells, RuntimeIndex(AssignmentIndex)]`, never the selector as an operand.
#[test]
fn state_carried_declared_range_index_stores_through_receiver_array_field() {
    let source = r#"
        data Ring { cells: [u16; 4]; }
        machine Ring::poke(&mut self, value: u16) {
            transition { _ -> store(0, value) }
            state store(&mut self, position: u64 [0..4], value: u16) {
                self.cells[position] = value;
            }
        }
    "#;
    let checked = checked_program(source);
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Ring::poke")
        .unwrap();
    let state = &program.machine_states(machine)[1];
    let mut shapes = ShapeCollector::new(program);
    let (_, structural, scalar) = structural_scalar_signature(
        program,
        &mut shapes,
        machine,
        state,
        &machine_binders(program, machine),
        true,
    )
    .expect("attached ranged state signature");
    let stores = build_structural_scalar_field_store_sequence(
        program,
        &checked.facts,
        machine,
        state,
        &structural,
        &scalar,
        0,
        None,
    )
    .expect("declared-range state index produces a primitive element store");
    let [
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            destination,
            path,
            value,
            ..
        },
    ] = stores.as_slice()
    else {
        panic!("one primitive store");
    };
    assert!(matches!(
        destination,
        checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index: 0 }
    ));
    assert!(matches!(
        path.as_slice(),
        [
            checked_trees::CheckedUnitStructuralPathSegment::Field(identity),
            checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth: 0 }
            ),
        ] if identity == "cells"
    ));
    assert!(matches!(
        value,
        checked_trees::CheckedCallScalarArgument::Pure(
            checked_trees::CheckedScalarExpression::Parameter {
                position: 1,
                primitive_type: PrimitiveType::U16,
            }
        )
    ));
}

/// The planner proves no bound for a runtime element: a plain `u64`
/// selector and declared ranges whose maximum does not fit the array (the
/// checker accepts them here through the literal argument's meet) still
/// retain the element, and Terminal re-proves `index < extent` from the
/// facts that reach the store or rejects it.
#[test]
fn state_carried_index_retains_its_runtime_element_whatever_its_declared_bound() {
    for position in ["u64", "u64 [0..=4]", "u64 [0..5]"] {
        let source = format!(
            r#"
            data Ring {{ cells: [u16; 4]; }}
            machine Ring::poke(&mut self, value: u16)
            {{
                transition {{ _ -> store(0, value) }}
                state store(&mut self, position: {position}, value: u16) {{
                    self.cells[position] = value;
                }}
            }}
        "#
        );
        let checked = checked_program(&source);
        let program = &checked.typed;
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Ring::poke")
            .unwrap();
        let state = &program.machine_states(machine)[1];
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
        let stores = build_structural_scalar_field_store_sequence(
            program,
            &checked.facts,
            machine,
            state,
            &structural,
            &scalar,
            0,
            None,
        )
        .unwrap_or_else(|| panic!("position: {position} retains its runtime element"));
        assert!(
            matches!(
                stores.as_slice(),
                [CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { path, .. }]
                    if matches!(path.last(), Some(checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(_)))
            ),
            "position: {position} stores through its runtime element: {stores:#?}"
        );
    }
}

/// The ordered stores of `Grid::update`'s entry state.
fn grid_stores(source: &str) -> Vec<CheckedUnitEffectOperationPlan> {
    let checked = checked_program(source);
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Grid::update")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let mut shapes = ShapeCollector::new(program);
    let (_, structural, scalar) = structural_scalar_signature(
        program,
        &mut shapes,
        machine,
        state,
        &machine_binders(program, machine),
        true,
    )
    .expect("attached grid signature");
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
    .expect("the grid stores compose")
}

fn runtime(depth: u32) -> checked_trees::CheckedUnitStructuralPathSegment {
    checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
        checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth },
    )
}

/// Every selector of the target is its own runtime element, numbered by its
/// depth from the target: `self.cells[i][j]` selects `j` at depth 0 and `i`
/// at depth 1, and its path lists them in path order.
#[test]
fn nested_selectors_are_runtime_elements_by_depth() {
    let stores = grid_stores(
        r#"
        data Grid { cells: [[u16; 4]; 3]; }
        machine Grid::update(&mut self, i: u64 [0..=2], j: u64 [0..=3], value: u16) {
            self.cells[i][j] = value;
        }
    "#,
    );
    let [CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { path, .. }] = stores.as_slice()
    else {
        panic!("one element store: {stores:#?}");
    };
    assert_eq!(
        path.as_slice(),
        [
            checked_trees::CheckedUnitStructuralPathSegment::Field("cells".into()),
            runtime(1),
            runtime(0),
        ]
    );
}

/// A field below a runtime element is the scalar field store's leaf: its
/// carrier composes fields and literal or runtime elements in any order.
#[test]
fn field_stores_carry_runtime_and_literal_elements_in_any_order() {
    let stores = grid_stores(
        r#"
        data Point { x: u16; y: u16; }
        data Entity { pos: Point; hp: u16; }
        data Grid { ents: [Entity; 3]; }
        machine Grid::update(&mut self, i: u64 [0..=2], value: u16) {
            self.ents[i].hp = value;
            self.ents[0].pos.x = value;
            self.ents[i].pos.y = value;
        }
    "#,
    );
    let carriers = stores
        .iter()
        .map(|store| match store {
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => {
                (store.carrier_path.clone(), store.field_identity.clone())
            }
            other => panic!("field stores only: {other:#?}"),
        })
        .collect::<Vec<_>>();
    let field =
        |identity: &str| checked_trees::CheckedUnitStructuralPathSegment::Field(identity.into());
    assert_eq!(
        carriers,
        [
            (vec![field("ents"), runtime(0)], "hp".to_owned()),
            (
                vec![
                    field("ents"),
                    checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(0),
                    field("pos"),
                ],
                "x".to_owned()
            ),
            (
                vec![field("ents"), runtime(0), field("pos")],
                "y".to_owned()
            ),
        ]
    );
}
