//! A collection call carries the same applied element across binder scopes.
//! Container geometry and the permissions of the borrow are still checked.

use super::*;

const ROW: &str = "data Row<const Key: u64> [copy] requires Key >= 1, value < Key { value: u64; }";

fn lower(
    declarations: &str,
    entry: &str,
) -> terminal_check_tables::Checked<crate::lowering::Elaboration> {
    let source = crate::tests::boundary_calls::typed(&format!("{ROW} {declarations} {entry}"));
    let typed = crate::preparation::prepare_typed_trees(source, &[], &[]).unwrap();
    let entry = crate::tests::boundary_calls::entry(typed.trees());
    crate::lowering::elaborate_generic(typed.trees(), entry)
}

fn accepted_call(declarations: &str, entry: &str) {
    let mut form = lower(declarations, entry).unwrap_or_else(|error| panic!("{entry}: {error:?}"));
    let checked = discharge(&mut form, &mut Default::default()).unwrap();
    assert!(
        checked
            .machines
            .iter()
            .all(|(_, machine)| { machine.verdicts.iter().all(|(_, verdict)| verdict.is_ok()) }),
        "{entry}"
    );
}

#[test]
fn borrowed_views_keep_the_callers_generic_element() {
    let declarations =
        "data Batch<const Count: u64, const Key: u64> requires Count >= 1, Key >= 1 {
            rows: [Row<Key>; Count];
        }
        machine take<const Key: u64>(rows: @[Row<Key>]) requires Key >= 1 {}";
    for entry in [
        "machine entry<const Count: u64, const Key: u64>(batch: @Batch<Count, Key>)
         requires Count >= 1 && Key >= 1 { take::<Key>(@batch.rows[..]); }",
        "machine entry<const Key: u64>(rows: @[Row<Key>; 2])
         requires Key >= 1 { take::<Key>(@rows[..]); }",
        "machine entry<const Key: u64>(rows: @[Row<Key>])
         requires Key >= 1 { take::<Key>(rows); }",
        "machine entry<const Key: u64>(rows: @[Row<Key>])
         requires Key >= 1 && rows.length > 0 { take::<Key>(@rows[1..]); }",
        "machine entry<const Key: u64>(rows: @[Row<Key>]) requires Key >= 1 {
            transition rows.length > 0 { true -> head(rows), _ -> done() }
            machine head(rows: @[Row<Key>]) requires Key >= 1 && rows.length > 0 {
                take::<Key>(@rows[1..]);
            }
            machine done() {}
        }",
    ] {
        accepted_call(declarations, entry);
    }
    let wrong_key = lower(
        declarations,
        "machine entry<const Key: u64>(rows: @[Row<Key>])
         requires Key >= 1 { take::<2>(rows); }",
    )
    .expect_err("a view cannot change its nominal element");
    assert!(wrong_key.detail.contains("argument"), "{wrong_key:?}");
}

#[test]
fn forwarding_a_fixed_array_preserves_its_extent_and_nominal_element() {
    let declarations = "data Plain [copy] { value: u64; }
        machine literal<const Key: u64>(rows: @[Row<Key>; 2]) requires Key >= 1 {}
        machine sized<const Count: u64, const Key: u64>(rows: @[Row<Key>; Count]) requires Key >= 1 {}
        machine plain<const Count: u64>(rows: @[Plain; Count]) {}";
    for entry in [
        "machine entry<const Key: u64>(rows: @[Row<Key>; 2]) requires Key >= 1 { literal::<Key>(rows); }",
        "machine entry<const Count: u64, const Key: u64>(rows: @[Row<Key>; Count])
         requires Key >= 1 { sized::<Count, Key>(rows); }",
        "machine entry<const Key: u64>(rows: @[Row<Key>; 2]) requires Key >= 1 { sized::<2, Key>(rows); }",
        "machine entry<const Count: u64>(rows: @[Plain; Count]) { plain::<Count>(rows); }",
    ] {
        accepted_call(declarations, entry);
    }
    let wrong_length = lower(declarations,
        "machine entry<const Key: u64>(rows: @[Row<Key>; 2]) requires Key >= 1 { sized::<3, Key>(rows); }",
    ).expect_err("a fixed collection cannot change its extent");
    assert!(wrong_length.detail.contains("argument"), "{wrong_length:?}");
}

#[test]
fn a_boundary_array_extent_does_not_index_its_plain_record_element() {
    let program = crate::tests::boundary_calls::typed(
        "data Plain [copy] { value: u64; }
        boundary trait Elements {
            machine last<const Count: u64>(items: @[Plain; Count]) -> Plain
                reaches none requires Count > 0 crashes none;
        }
        Selected satisfies Elements {
            machine last<const Count: u64>(items: @[Plain; Count]) -> Plain
                requires Count > 0 { items[Count - 1] }
        }
        machine entry() {}",
    );
    let typed = crate::preparation::prepare_typed_trees(program, &[], &[]).unwrap();
    let verdicts = crate::tests::boundary_calls::compilation_results_for_subjects(&typed, &[None]);
    assert!(
        verdicts.len() > 1,
        "check the declaration and its implementation"
    );
    assert!(verdicts.iter().all(accepted), "{verdicts:?}");
}

#[test]
fn a_ranked_self_transfer_preserves_the_forwarded_nominal_input() {
    let read = "machine read<const Key: u64>(row: Row<Key>) requires Key >= 1 {}";
    let entry = "machine entry<const Key: u64>(rows: @[Row<Key>])
        terminates by rows -> Slice::Length
        requires Key >= 1 && rows.length > 0 {
            read::<Key>(rows[0]);
            transition rows.length > 1 { true -> entry::<Key>(@rows[1..]), _ -> done() }
            machine done() {}
        }";
    accepted_call(read, entry);
    let changed = entry.replace("entry::<Key>", "entry::<2>");
    let refusal =
        lower(read, &changed).expect_err("a recursive transfer cannot change the element key");
    assert!(
        refusal.detail.contains("exact element") || refusal.detail.contains("argument"),
        "{refusal:?}"
    );
}
