use super::{
    CheckedUnitEffectOperationPlan, ShapeCollector, build_structural_scalar_field_store_sequence,
    checked_program, machine_binders,
};
use crate::execution::terminal_unit::calls::{
    free_structural_scalar_signature, structural_scalar_signature,
};

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
    let (structural, scalar) = if program.attached_data_definition(machine).is_some() {
        let (_, structural, scalar) = structural_scalar_signature(
            program,
            &mut shapes,
            machine,
            state,
            &machine_binders(program, machine),
            true,
        )
        .unwrap();
        (structural, scalar)
    } else {
        free_structural_scalar_signature(
            program,
            &mut shapes,
            state,
            &machine_binders(program, machine),
        )
        .unwrap()
    };
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

/// An element that reads storage could observe an earlier element's store,
/// so a literal built from the destination's own elements keeps declining.
#[test]
fn an_array_literal_reading_storage_is_not_split_into_element_stores() {
    let stores = stores(
        r#"
        data Board { cells: [u16; 3]; }
        machine Board::rotate(board: &mut Board) {
            board.cells = [board.cells[1], board.cells[2], board.cells[0]];
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

#[test]
fn local_record_nested_array_literal_keeps_owned_root_and_row_major_paths() {
    let source = r#"
        data Matrices { cells: [[i32 in Wrapping; 3]; 2]; }
        machine initialize() {
            let mut matrices: Matrices;
            matrices.cells = [[1, 2, 3], [4, 5, 6]];
        }
    "#;
    let checked = checked_program(source);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "initialize")
        .unwrap();
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .omission_for_machine(machine.symbol)
            .is_none(),
        "ordinary statement sequencing must retain local aggregate assignments"
    );
    let planned = stores(source, "initialize").expect("owned local array stores");
    assert_eq!(planned.len(), 6);
    let mut root = None;
    for (ordinal, store) in planned.iter().enumerate() {
        let CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index,
            destination,
            path,
            ..
        } = store
        else {
            panic!("literal elements use ordinary primitive stores")
        };
        assert_eq!(*statement_index, 1);
        let checked_trees::CheckedStoreRoot::Local { symbol } = destination else {
            panic!("a local cannot fabricate a parameter destination")
        };
        assert_eq!(*root.get_or_insert(*symbol), *symbol);
        let [
            checked_trees::CheckedUnitStructuralPathSegment::Field(_),
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(row),
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(column),
        ] = path.as_slice()
        else {
            panic!("one field and both literal indices survive")
        };
        assert_eq!(
            (*row, *column),
            ((ordinal / 3) as u64, (ordinal % 3) as u64)
        );
    }
}

#[test]
fn local_array_literal_reading_storage_is_not_split_into_element_stores() {
    let planned = stores(
        r#"
        data Board { cells: [u16; 3]; }
        machine rotate() {
            let mut board: Board;
            board.cells = [board.cells[1], board.cells[2], board.cells[0]];
        }
    "#,
        "rotate",
    );
    assert!(
        planned.is_none_or(|stores| !stores.iter().any(|store| matches!(
            store,
            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
        ))),
        "local storage reads must preserve aggregate evaluation before stores"
    );
}

#[test]
fn array_literal_store_rejects_shared_storage_and_wrong_extents() {
    for source in [
        r#"
            data Board { cells: [u16; 3]; }
            machine replace(board: &Board) { board.cells = [1, 2, 3]; }
        "#,
        r#"
            data Board { cells: [u16; 3]; }
            machine replace() {
                let mut board: Board;
                board.cells = [1, 2];
            }
        "#,
    ] {
        let result = crate::tests::front_end::typed_program_result(source)
            .and_then(|typed| crate::lower_typed_trees(typed, &crate::CheckingRequest::settled()));
        assert!(
            result.is_err(),
            "authority and declared extent remain mandatory"
        );
    }
}
