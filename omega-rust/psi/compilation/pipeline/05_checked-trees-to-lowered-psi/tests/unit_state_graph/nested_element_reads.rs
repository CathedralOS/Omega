//! A runtime selector beneath another runtime selector (`root.a[i][k]`) in a
//! conditional successor argument. Each selector keeps its own `RuntimeIndex`
//! segment and obligation on one `PrimitiveScalarRead`, and the retained
//! selectors rejoin the authored indexing chain position for position: a
//! reordered or dropped selector does not lower.

use checked_trees::CheckedScalarExpression;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_psi::{OperationKind, StructuralPathSegment};
use terminal_verifier::validate_module;

const SOURCE: &str = r#"
    data Root { a: [[i32 in Wrapping; 3]; 3]; out: i32 in Wrapping; }
    machine Root::enter(root: &mut Root) {
        transition { _ -> dot(1, 0, 0, root) }
        state dot(i: u64, k: u64, acc: i32 in Wrapping, root: &mut Root)
        requires i <= 2
        requires k <= 2 {
            let next_k: u64 = k + 1;
            transition next_k <= 2 {
                true -> dot(i, next_k, acc + root.a[i][k], root)
                false -> done(acc + root.a[i][k], root)
            }
        }
        state done(acc: i32 in Wrapping, root: &mut Root) { root.out = acc; }
    }
"#;

const ARRAY_PARAMETER_SOURCE: &str = r#"
    data Root {}
    machine Root::enter(grid: &mut [[i32 in Wrapping; 4]; 3],
                        other: &mut [[i32 in Wrapping; 4]; 3],
                        row: u64, column: u64) -> i32 in Wrapping
    requires row <= 2
    requires column <= 3 {
        transition row == 0 {
            true -> finish(grid[row][column])
            false -> finish(grid[row][column])
        }
        state finish(value: i32 in Wrapping) -> i32 in Wrapping { value }
    }
"#;

const MEMBER_PARAMETER_SOURCE: &str = r#"
    data Root { out: i32 in Wrapping; }
    data Row [copy] { values: [i32 in Wrapping; 4]; }
    machine Root::enter(context: &mut Root, rows: &mut [Row; 3], other: &mut [Row; 3],
                        row: u64, column: u64) -> i32 in Wrapping
    requires row <= 2
    requires column <= 3 {
        transition row == 0 {
            true -> finish(rows[row].values[column])
            false -> finish(rows[row].values[column])
        }
        state finish(value: i32 in Wrapping) -> i32 in Wrapping { value }
    }
"#;

#[test]
fn member_selector_field_types_follow_the_root_not_declaration_order() {
    let selected = "data Selector [copy] { column: u64; }";
    let unrelated = "data Unrelated [copy] { column: u8; }";
    let body = r#"
        data Root {}
        data Row [copy] { values: [i32 in Wrapping; 4]; }
        machine Root::enter(selector: Selector, rows: &[Row; 3], row: u64) -> i32 in Wrapping
        requires row <= 2
        requires selector.column <= 3 {
            transition row == 0 {
                true -> finish(rows[row].values[selector.column])
                false -> finish(rows[row].values[selector.column])
            }
            state finish(value: i32 in Wrapping) -> i32 in Wrapping { value }
        }
    "#;
    for declarations in [
        format!("{unrelated}\n{selected}"),
        format!("{selected}\n{unrelated}"),
    ] {
        let source = format!("{declarations}\n{body}");
        let lowered = checked_trees_to_lowered_psi::lower_machine(
            &crate::front_end::checked_program(&source),
            TerminalMachineSelection::Name("Root::enter"),
        )
        .expect("a same-named unrelated field cannot change selector custody");
        validate_module(&lowered.semantic_module)
            .expect("the selected declaration supplies the integer type and bound");
    }
}

#[test]
fn member_between_parameter_selectors_keeps_the_scalar_namespace_and_bounds() {
    for source in [
        MEMBER_PARAMETER_SOURCE.to_owned(),
        MEMBER_PARAMETER_SOURCE.replace("&mut", "&"),
    ] {
        let lowered = checked_trees_to_lowered_psi::lower_machine(
            &crate::front_end::checked_program(&source),
            TerminalMachineSelection::Name("Root::enter"),
        )
        .expect("parameter selectors compose across a record member");
        validate_module(&lowered.semantic_module).expect("each selected array retains its bound");
        let reads = lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match &operation.kind {
                OperationKind::PrimitiveScalarRead { path, .. } => Some(path),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(reads.len(), 2);
        for path in reads {
            let [
                StructuralPathSegment::RuntimeIndex {
                    index: row,
                    obligation: row_bound,
                },
                StructuralPathSegment::Field(field),
                StructuralPathSegment::RuntimeIndex {
                    index: column,
                    obligation: column_bound,
                },
            ] = path.as_slice()
            else {
                panic!("two selectors retain their intervening member: {path:?}")
            };
            assert_eq!(field, "values");
            assert_ne!(row, column);
            assert_ne!(row_bound, column_bound);
        }
    }
}

#[test]
fn bare_borrowed_array_nested_selectors_keep_independent_bounds() {
    for source in [
        ARRAY_PARAMETER_SOURCE.to_owned(),
        ARRAY_PARAMETER_SOURCE.replace("&mut", "&"),
    ] {
        let lowered = checked_trees_to_lowered_psi::lower_machine(
            &crate::front_end::checked_program(&source),
            TerminalMachineSelection::Name("Root::enter"),
        )
        .expect("nested selectors below a bare borrowed array lower");
        validate_module(&lowered.semantic_module).expect("both array bounds validate");
        let reads = lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match &operation.kind {
                OperationKind::PrimitiveScalarRead { path, .. } => Some(path),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(reads.len(), 2);
        let mut obligations = std::collections::BTreeSet::new();
        for path in reads {
            let [
                StructuralPathSegment::RuntimeIndex {
                    index: row,
                    obligation: row_obligation,
                },
                StructuralPathSegment::RuntimeIndex {
                    index: column,
                    obligation: column_obligation,
                },
            ] = path.as_slice()
            else {
                panic!("a bare array retains exactly two selectors: {path:?}");
            };
            assert_ne!(row, column);
            assert!(obligations.insert(*row_obligation));
            assert!(obligations.insert(*column_obligation));
        }
    }
}

#[test]
fn nested_selectors_each_take_their_own_runtime_segment_and_obligation() {
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &crate::front_end::checked_program(SOURCE),
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("nested runtime-indexed successor arguments lower");
    validate_module(&lowered.semantic_module).expect("the nested reads validate");
    let reads = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::PrimitiveScalarRead { path, .. } => Some(path),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(reads.len(), 2, "one nested read per conditional arm");
    let mut obligations = Vec::new();
    for path in reads {
        let [
            StructuralPathSegment::Field(field),
            StructuralPathSegment::RuntimeIndex {
                index: row,
                obligation: row_obligation,
            },
            StructuralPathSegment::RuntimeIndex {
                index: column,
                obligation: column_obligation,
            },
        ] = path.as_slice()
        else {
            panic!("a field then two runtime selectors: {path:?}");
        };
        assert_eq!(field, "a");
        assert_ne!(row, column, "each selector reads its own value");
        obligations.extend([*row_obligation, *column_obligation]);
    }
    let distinct = obligations
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    assert_eq!(
        distinct,
        obligations.len(),
        "every selector owns its obligation"
    );
}

#[derive(Clone, Copy, Debug)]
enum Forgery {
    SwapSelectors,
    DropInnerSelector,
    /// Keeps the selector the authored top indexing names and substitutes the
    /// outer one, which only the complete chain replay observes.
    SubstituteOuterSelector,
    SubstituteRoot,
    SubstituteInnerParameter,
}

/// Apply `forgery` to every nested read inside `expression`; the count of
/// forged reads.
fn forge(expression: &mut CheckedScalarExpression, forgery: Forgery) -> usize {
    match expression {
        CheckedScalarExpression::StructuralParameterIndexedRead {
            root,
            index,
            inner_indexes,
            ..
        } if !inner_indexes.is_empty() => {
            match forgery {
                Forgery::SwapSelectors => std::mem::swap(index.as_mut(), &mut inner_indexes[0]),
                Forgery::DropInnerSelector => inner_indexes.clear(),
                Forgery::SubstituteOuterSelector => *index = Box::new(inner_indexes[0].clone()),
                Forgery::SubstituteRoot => {
                    *root = checked_trees::CheckedStorageRoot::Parameter { index: 1 };
                }
                Forgery::SubstituteInnerParameter => return 0,
            }
            1
        }
        CheckedScalarExpression::StructuralParameterIndexedRead {
            root, element_path, ..
        } if element_path.iter().any(|segment| {
            matches!(
                segment,
                checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(_)
            )
        }) =>
        {
            match forgery {
                Forgery::SubstituteRoot => {
                    *root = checked_trees::CheckedStorageRoot::Parameter { index: 2 }
                }
                Forgery::SubstituteInnerParameter => {
                    for segment in element_path {
                        if let checked_trees::CheckedStructuralPredicatePathSegment::RuntimeIndex(
                            selector,
                        ) = segment
                        {
                            *selector =
                                checked_trees::CheckedRuntimeIndex::Parameter { position: 0 };
                        }
                    }
                }
                _ => return 0,
            }
            1
        }
        CheckedScalarExpression::IntegerBinary { left, right, .. } => {
            forge(left, forgery) + forge(right, forgery)
        }
        _ => 0,
    }
}

#[test]
fn reordered_or_dropped_selectors_do_not_rejoin_the_authored_read() {
    assert_forged_reads_rejected(SOURCE);
}

#[test]
fn bare_array_nested_reads_reject_changed_selectors_and_sibling_roots() {
    assert_forged_reads_rejected(ARRAY_PARAMETER_SOURCE);
    assert_forged_reads_rejected(&ARRAY_PARAMETER_SOURCE.replace("&mut", "&"));
}

fn assert_forged_reads_rejected(source: &str) {
    assert_forged_reads_rejected_for(
        source,
        &[
            Forgery::SwapSelectors,
            Forgery::DropInnerSelector,
            Forgery::SubstituteOuterSelector,
            Forgery::SubstituteRoot,
        ],
    );
}

#[test]
fn member_between_selectors_rejects_substituted_parameters_and_roots() {
    assert_forged_reads_rejected_for(
        MEMBER_PARAMETER_SOURCE,
        &[Forgery::SubstituteInnerParameter, Forgery::SubstituteRoot],
    );
}

fn assert_forged_reads_rejected_for(source: &str, forgeries: &[Forgery]) {
    for &forgery in forgeries {
        let mut checked = crate::front_end::checked_program(source);
        let mut forged = 0;
        for located in &mut checked.facts.values.scalar_expressions.expressions {
            forged += forge(&mut located.expression, forgery);
        }
        let nodes = &mut checked.facts.values.scalar_computations.nodes;
        let handles = nodes.iter().map(|(handle, _)| handle).collect::<Vec<_>>();
        for handle in handles {
            if let checked_trees::CheckedScalarComputationKind::Value(expression) =
                &mut nodes.get_mut(handle).kind
            {
                forged += forge(expression, forgery);
            }
        }
        assert!(forged > 0, "{forgery:?} reached a nested read");
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &checked,
                TerminalMachineSelection::Name("Root::enter"),
            )
            .is_err(),
            "{forgery:?} must not lower"
        );
    }
}
