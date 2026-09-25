use super::{
    CheckedUnitEffectOperationPlan, ShapeCollector, build_structural_scalar_field_store_sequence,
    checked_program, machine_binders,
};
use crate::execution::terminal_unit::calls::structural_scalar_signature;
use checked_trees::CheckedUnitStructuralPathSegment as Segment;

fn stores(source: &str, machine_name: &str) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    let checked = checked_program(source);
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
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
    .unwrap();
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
}

fn element_paths(stores: &[CheckedUnitEffectOperationPlan]) -> Vec<Vec<Segment>> {
    stores
        .iter()
        .map(|store| match store {
            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                statement_index: 0,
                path,
                value: checked_trees::CheckedCallScalarArgument::Pure(_),
                ..
            } => path.clone(),
            other => panic!("an element store of the one assignment, not {other:?}"),
        })
        .collect()
}

/// A closed array literal written whole into a primitive-array field is one
/// primitive store per element, at the field's path plus that element's
/// literal index; a parameter element is as closed as a literal one.
#[test]
fn an_array_literal_field_store_writes_each_element() {
    let stores = stores(
        r#"
        data Board { cells: [u16; 3]; }
        machine Board::reset(&mut self, value: u16) { self.cells = [4, value, 9]; }
    "#,
        "Board::reset",
    )
    .expect("the literal decomposes into element stores");
    let field = || Segment::Field("cells".into());
    assert_eq!(
        element_paths(&stores),
        [
            vec![field(), Segment::FixedIndex(0)],
            vec![field(), Segment::FixedIndex(1)],
            vec![field(), Segment::FixedIndex(2)],
        ]
    );
}

/// A nested literal stores its row-major elements at every dimension's
/// literal index.
#[test]
fn a_nested_array_literal_stores_row_major_elements() {
    let stores = stores(
        r#"
        data Grid { cells: [[u8; 2]; 2]; }
        machine Grid::reset(&mut self) { self.cells = [[1, 2], [3, 4]]; }
    "#,
        "Grid::reset",
    )
    .expect("the nested literal decomposes into element stores");
    let path = |row, column| {
        vec![
            Segment::Field("cells".into()),
            Segment::FixedIndex(row),
            Segment::FixedIndex(column),
        ]
    };
    assert_eq!(
        element_paths(&stores),
        [path(0, 0), path(0, 1), path(1, 0), path(1, 1)]
    );
}

/// An element that reads storage could observe an earlier element's store,
/// so a literal built from the destination's own elements keeps declining.
#[test]
fn an_array_literal_reading_storage_is_not_split_into_element_stores() {
    let stores = stores(
        r#"
        data Board { cells: [u16; 3]; }
        machine Board::rotate(&mut self) {
            self.cells = [self.cells[1], self.cells[2], self.cells[0]];
        }
    "#,
        "Board::rotate",
    );
    assert!(
        stores.is_none_or(|stores| !stores.iter().any(|store| matches!(
            store,
            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
        ))),
        "a storage-reading element never becomes an element store"
    );
}

/// An arithmetic-policy shell around the whole array qualifies arithmetic on
/// its elements, not their storage: the literal still lands element by
/// element.
#[test]
fn an_array_literal_lands_under_a_whole_array_policy_shell() {
    let stores = stores(
        r#"
        data Tally { counts: [i32; 3] in Wrapping; }
        machine Tally::reset(&mut self) { self.counts = [0, 0, 0]; }
    "#,
        "Tally::reset",
    )
    .expect("the literal decomposes under the policy shell");
    let field = || Segment::Field("counts".into());
    assert_eq!(
        element_paths(&stores),
        [
            vec![field(), Segment::FixedIndex(0)],
            vec![field(), Segment::FixedIndex(1)],
            vec![field(), Segment::FixedIndex(2)],
        ]
    );
}
