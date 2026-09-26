//! A `&[T]` view local lends an owned collection through ordinary statement
//! sequencing: the local carries the borrowed-view shape, the call forwards
//! that view whole, and the callee's own `&[T]` formal carries the same shape.
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;
use checked_trees::{CheckedStructuralAccess, CheckedUnitStructuralArgumentSourcePlan};

/// The view's element carries an arithmetic domain, so the shape also pins
/// that a scalar element is an ordinary view element.
const VIEW_IDENTITY: &str =
    "slice(constrained(named(name(i32)),arithmetic-domain(name(Wrapping))))";

/// A `&[T]` actual on a scalar-computation call loans the established view
/// local whole: the argument names the local's own shared-borrow home rather
/// than fabricating an owning result, the same custody a `&T` local keeps
/// when a computation call loans it onward.
#[test]
fn a_scalar_computation_call_loans_the_view_local_whole() {
    let checked = checked(
        r#"
        data Summer {}

        data Holder {
            values: [i32 in Wrapping; 4];
            summer: Summer;
        }

        machine Summer::sum(&self, s: &[i32 in Wrapping], acc: i32 in Wrapping) -> i32 {
            (acc as i32)
        }

        machine Holder::run(&mut self) -> i32 {
            let view: &[i32 in Wrapping] = self.values.as_slice();
            self.summer.sum(view, 0)
        }
    "#,
    );
    let computations = &checked.facts.values.scalar_computations;
    let calls = computations
        .nodes
        .iter()
        .filter_map(|(_, node)| match &node.kind {
            checked_trees::CheckedScalarComputationKind::Call {
                structural_arguments,
                ..
            } => Some(
                computations
                    .structural_arguments
                    .span_or_empty(*structural_arguments),
            ),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [arguments] = calls.as_slice() else {
        panic!("one scalar computation call: {calls:?}");
    };
    let view_argument = arguments
        .iter()
        .find_map(|argument| match argument {
            checked_trees::CheckedScalarComputationStructuralArgument::Place(plan)
                if plan.type_identity == VIEW_IDENTITY =>
            {
                Some(plan)
            }
            _ => None,
        })
        .expect("the view actual produces a structural place argument");
    let run = machine_named(&checked, "run");
    let machine = crate::lookup::machine_by_symbol(&checked.typed, run)
        .expect("the caller machine is present");
    let state = checked
        .typed
        .machine_states(machine)
        .iter()
        .next()
        .expect("run has an entry state");
    let view_local = checked
        .typed
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == "view" =>
            {
                Some(local.symbol)
            }
            _ => None,
        })
        .expect("the authored view local is present");
    assert_eq!(
        view_argument.source,
        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: view_local }
    );
    assert!(view_argument.path.is_empty());
    assert_eq!(view_argument.access, CheckedStructuralAccess::SharedBorrow);
}

/// The `let` symbol of the local named `name` in `machine`'s entry state.
fn local_symbol(
    checked: &checked_trees::CheckedTrees,
    machine: &str,
    name: &str,
) -> (symbols::SymbolHandle, symbols::SymbolHandle) {
    let machine = crate::lookup::machine_by_symbol(&checked.typed, machine_named(checked, machine))
        .expect("the machine is present");
    let state = checked
        .typed
        .machine_states(machine)
        .iter()
        .next()
        .expect("the machine has an entry state");
    let local = checked
        .typed
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == name =>
            {
                Some(local.symbol)
            }
            _ => None,
        })
        .expect("the authored local is present");
    (state.symbol, local)
}

/// Lengths and element reads of a view local root at the local itself, the
/// same observation vocabulary a whole view parameter uses.
#[test]
fn view_local_lengths_and_reads_root_at_the_local() {
    let checked = checked(
        r#"
        boundary trait Output {
            machine observe(value: u64) reaches Output;
        }

        data Holder {
            values: [u64; 4];
        }

        machine Holder::run(&mut self) reaches Output {
            let view: &[u64] = self.values.as_slice();
            let tail: &[u64] = view[1..];
            Output::observe(tail.len);
            Output::observe(tail[0]);
        }
    "#,
    );
    let (state, tail) = local_symbol(&checked, "run", "tail");
    let observed = |statement| {
        checked
            .facts
            .values
            .scalar_expressions
            .expressions
            .iter()
            .find(|expression| {
                expression.state == state
                    && expression.statement_ordinal == statement
                    && matches!(
                        expression.role,
                        checked_trees::CheckedScalarExpressionRole::BoundaryCallArgument { .. }
                    )
            })
            .map(|expression| expression.expression.clone())
            .expect("the boundary operand has a pure scalar plan")
    };
    let root = checked_trees::CheckedStorageRoot::ViewLocal { symbol: tail };
    assert_eq!(
        observed(2),
        checked_trees::CheckedScalarExpression::StructuralParameterByteLength {
            root,
            path: Vec::new(),
        }
    );
    assert!(matches!(
        observed(3),
        checked_trees::CheckedScalarExpression::StructuralParameterIndexedRead {
            root: read_root,
            ref path,
            primitive_type: typed_trees::types::PrimitiveType::U64,
            ..
        } if read_root == root && path.is_empty()
    ));
}
