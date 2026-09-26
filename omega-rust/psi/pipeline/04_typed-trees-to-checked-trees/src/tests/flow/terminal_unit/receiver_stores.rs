//! Receiver identity and write-frame checks for projected scalar stores.
use super::CheckedUnitEffectOperationPlan;
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;

#[test]
fn indexed_policy_array_element_store_keeps_its_canonical_path() {
    let checked = checked(
        r#"
        data Root { values: [i32 in Wrapping; 4]; }
        machine Root::enter(&mut self) { self.values[0] = 5; }
        machine fill(values: &mut [i32 in Wrapping; 4]) { values[1] = 7; }
        machine seed(values: &mut [u16; 2]) { values[0] = 3; }
        "#,
    );
    for (name, statement_index, expected_path) in [
        (
            "seed",
            0,
            vec![checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                0,
            )],
        ),
        (
            "Root::enter",
            0,
            vec![
                checked_trees::CheckedUnitStructuralPathSegment::Field("values".into()),
                checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(0),
            ],
        ),
        (
            "fill",
            0,
            vec![checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                1,
            )],
        ),
    ] {
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, name))
            .unwrap_or_else(|| panic!("{name} retains its indexed primitive store"));
        let [
            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                statement_index: store_statement_index,
                destination:
                    checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index: 0 },
                path,
                ..
            },
            CheckedUnitEffectOperationPlan::Complete { .. },
        ] = plan.operations.as_slice()
        else {
            panic!("{name} retains one indexed primitive store and return")
        };
        assert_eq!(*store_statement_index, statement_index);
        assert_eq!(path.as_slice(), expected_path.as_slice());
    }
}
