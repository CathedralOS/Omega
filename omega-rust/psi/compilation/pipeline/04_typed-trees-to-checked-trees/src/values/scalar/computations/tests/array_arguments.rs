use super::checked_source;
use checked_trees::{
    CheckedScalarComputationKind, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedStorageRoot, CheckedStructuralPredicatePathSegment,
};

#[test]
fn nested_array_parameter_reads_keep_each_selector_without_a_field_prefix() {
    for access in ["&", "&mut"] {
        let checked = checked_source(
            &format!(
                "machine read(grid: {access}[[i32; 3]; 2], row: u64, column: u64) -> i32
                requires row < 2 requires column < 3 {{
                    let value: i32 = grid[row][column];
                    value
                }}"
            ),
            false,
        );
        let value = initializer(&checked);
        let CheckedScalarExpression::StructuralParameterIndexedRead {
            root,
            path,
            index,
            inner_indexes,
            element_path,
            ..
        } = value
        else {
            panic!("nested array read must retain its source and selectors: {value:?}");
        };
        assert_eq!(*root, CheckedStorageRoot::Parameter { index: 0 });
        assert!(path.is_empty());
        assert!(matches!(
            index.as_ref(),
            CheckedScalarExpression::Parameter { position: 0, .. }
        ));
        assert!(matches!(
            inner_indexes.as_slice(),
            [CheckedScalarExpression::Parameter { position: 1, .. }]
        ));
        assert!(element_path.is_empty());
    }
}

#[test]
fn nested_array_parameter_reads_keep_a_field_between_selectors() {
    for access in ["&", "&mut"] {
        let checked = checked_source(
            &format!(
                "data Row {{ values: [i32; 3]; }}
                machine read(rows: {access}[Row; 2], row: u64, column: u64) -> i32
                requires row < 2 requires column < 3 {{
                    let value: i32 = rows[row].values[column];
                    value
                }}"
            ),
            false,
        );
        let value = initializer(&checked);
        let CheckedScalarExpression::StructuralParameterIndexedRead {
            root,
            path,
            inner_indexes,
            element_path,
            ..
        } = value
        else {
            panic!("member-between read must retain its structural path: {value:?}");
        };
        assert_eq!(*root, CheckedStorageRoot::Parameter { index: 0 });
        assert!(path.is_empty());
        assert!(inner_indexes.is_empty());
        assert!(matches!(
            element_path.as_slice(),
            [
                CheckedStructuralPredicatePathSegment::Field(_),
                CheckedStructuralPredicatePathSegment::RuntimeIndex(_)
            ]
        ));
    }
}

#[test]
fn nested_array_parameter_reads_still_require_both_bounds() {
    for requirements in ["requires row < 2", "requires column < 3"] {
        let typed = crate::tests::front_end::typed_program(&format!(
            "machine read(grid: &[[i32; 3]; 2], row: u64, column: u64) -> i32
            {requirements} {{ grid[row][column] }}"
        ));
        assert!(crate::lower_typed_trees(typed, &crate::CheckingRequest::settled()).is_err());
    }
}

fn initializer(checked: &checked_trees::CheckedTrees) -> &CheckedScalarExpression {
    let machine = &checked.machines()[0];
    let state = &checked.machine_states(machine)[0];
    let role = CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 };
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine.symbol)
            .is_some()
    );
    if let Some(value) =
        checked
            .facts
            .values
            .scalar_expressions
            .expression_at(state.symbol, 0, role)
    {
        return value;
    }
    let computations = &checked.facts.values.scalar_computations;
    let root = computations
        .root_at(state.symbol, 0, role)
        .expect("retained initializer");
    let CheckedScalarComputationKind::Value(value) = &computations.nodes.get(root.root).kind else {
        panic!("a read without calls remains a value computation");
    };
    value
}
