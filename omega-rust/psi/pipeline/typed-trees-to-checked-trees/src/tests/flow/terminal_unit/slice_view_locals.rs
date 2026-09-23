//! A `&[T]` view local lends an owned collection through ordinary statement
//! sequencing: the local carries the borrowed-view shape, the call forwards
//! that view whole, and the callee's own `&[T]` formal carries the same shape.
use super::{CheckedUnitEffectOperationPlan, CheckedUnitStructuralTypeShape};
use crate::tests::flow::terminal_unit::checked;
use crate::tests::flow::terminal_unit::machine_named;
use checked_trees::{CheckedStructuralAccess, CheckedUnitStructuralArgumentSourcePlan};

const SOURCE: &str = r#"
    data Holder {
        values: [i32 in Wrapping; 4];
    }

    machine observe(view: &[i32 in Wrapping]) {}

    machine Holder::run(&mut self) {
        self.values[0] = 1;
        let view: &[i32 in Wrapping] = self.values.as_slice();
        observe(view);
    }
"#;

/// The view's element carries an arithmetic domain, so the shape also pins
/// that a scalar element is an ordinary view element.
const VIEW_IDENTITY: &str =
    "slice(constrained(named(name(i32)),arithmetic-domain(name(Wrapping))))";

#[test]
fn view_local_carries_a_runtime_length_shape_over_its_scalar_element() {
    let checked = checked(SOURCE);
    let view = checked
        .facts
        .flow
        .terminal_unit_effects
        .structural_types
        .iter()
        .find(|plan| plan.identity == VIEW_IDENTITY)
        .expect("the borrowed view has its own checked structural type");
    let CheckedUnitStructuralTypeShape::BorrowedSliceView {
        element_type_identity,
    } = &view.shape
    else {
        panic!("a borrowed view is not an owning indexed aggregate: {view:?}");
    };
    // No length belongs to the shape: a view's extent is its own stored
    // runtime length, not part of the type it borrows through.
    assert_eq!(element_type_identity, "named(name(i32))");
    let element = checked
        .facts
        .flow
        .terminal_unit_effects
        .structural_types
        .iter()
        .find(|plan| plan.identity == *element_type_identity)
        .expect("the view's element keeps its own retained shape");
    assert!(matches!(
        element.shape,
        CheckedUnitStructuralTypeShape::PrimitiveScalar(typed_trees::types::PrimitiveType::I32)
    ));
}

#[test]
fn ordinary_statement_sequencing_establishes_and_forwards_the_view_local() {
    let checked = checked(SOURCE);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "run"))
        .expect("the view local no longer leaves the unit plan roster");
    let [
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. },
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, calls, .. },
        CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("authored statement order: store, view local, call: {plan:?}");
    };
    assert_eq!(result.type_identity, VIEW_IDENTITY);
    assert_eq!(result.statement_index, 1);
    // Lending an existing collection establishes no call of its own.
    assert!(calls.is_empty());
    let [argument] = structural_arguments.as_slice() else {
        panic!("one forwarded view argument: {structural_arguments:?}");
    };
    assert_eq!(
        argument.source,
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        }
    );
    assert!(argument.path.is_empty());
    assert_eq!(argument.type_identity, VIEW_IDENTITY);
    assert_eq!(argument.access, CheckedStructuralAccess::SharedBorrow);
}

#[test]
fn a_callee_slice_view_formal_carries_the_same_shape() {
    let checked = checked(SOURCE);
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "observe"))
        .expect("a `&[T]` formal is an ordinary structural parameter");
    let [parameter] = plan.structural_parameters.as_slice() else {
        panic!("one view parameter: {plan:?}");
    };
    assert_eq!(parameter.type_identity, VIEW_IDENTITY);
    assert_eq!(parameter.access, CheckedStructuralAccess::SharedBorrow);
}

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

/// The lent place must hold the very elements the view carries. A view whose
/// element type is not the lent collection's element has no admitted value,
/// so its local leaves the roster rather than borrowing a mismatched extent.
#[test]
fn a_view_of_another_element_type_is_not_admitted() {
    let checked = checked(
        r#"
        data Holder {
            values: [i32 in Wrapping; 4];
        }

        machine observe(view: &[u64]) {}

        machine Holder::run(&mut self) {
            self.values[0] = 1;
            let view: &[u64] = self.values.as_slice();
            observe(view);
        }
    "#,
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine_named(&checked, "run"))
            .is_none()
    );
}
