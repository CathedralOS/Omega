//! A runtime selector beneath another runtime selector (`self.a[i][k]`) in a
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
    machine Root::enter(&mut self) {
        transition { _ -> dot(1, 0, 0) }
        state dot(&mut self, i: u64, k: u64, acc: i32 in Wrapping)
        requires i <= 2
        requires k <= 2 {
            let next_k: u64 = k + 1;
            transition next_k <= 2 {
                true -> dot(i, next_k, acc + self.a[i][k])
                false -> done(acc + self.a[i][k])
            }
        }
        state done(&mut self, acc: i32 in Wrapping) { self.out = acc; }
    }
"#;

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
}

/// Apply `forgery` to every nested read inside `expression`; the count of
/// forged reads.
fn forge(expression: &mut CheckedScalarExpression, forgery: Forgery) -> usize {
    match expression {
        CheckedScalarExpression::StructuralParameterIndexedRead {
            index,
            inner_indexes,
            ..
        } if !inner_indexes.is_empty() => {
            match forgery {
                Forgery::SwapSelectors => std::mem::swap(index.as_mut(), &mut inner_indexes[0]),
                Forgery::DropInnerSelector => inner_indexes.clear(),
                Forgery::SubstituteOuterSelector => *index = Box::new(inner_indexes[0].clone()),
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
    for forgery in [
        Forgery::SwapSelectors,
        Forgery::DropInnerSelector,
        Forgery::SubstituteOuterSelector,
    ] {
        let mut checked = crate::front_end::checked_program(SOURCE);
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
