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

/// A `let` narrowing an established view local binds the same element range
/// a call argument carries, rooted at the local instead of a parameter, and
/// its endpoint is keyed at the statement's own binding site. The narrowed
/// local is then forwarded as any view local is.
#[test]
fn a_view_local_is_narrowed_by_the_argument_range_builder() {
    let checked = checked(
        r#"
        data Holder {
            values: [i32 in Wrapping; 4];
        }

        machine observe(view: &[i32 in Wrapping]) {}

        machine Holder::run(&mut self) {
            let view: &[i32 in Wrapping] = self.values.as_slice();
            let tail: &[i32 in Wrapping] = view[1..];
            observe(tail);
        }
    "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "run"))
        .unwrap_or_else(|| {
            panic!(
                "a narrowed view local keeps its machine in the unit roster: {:?}",
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .omission_for_machine(machine_named(&checked, "run"))
            )
        });
    let [
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { result: view, .. },
        CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, source },
        CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("authored order: view local, narrowed local, call: {plan:?}");
    };
    let (state, view_symbol) = local_symbol(&checked, "run", "view");
    assert_eq!(result.statement_index, 1);
    assert_eq!(result.binding_ordinal, view.binding_ordinal + 1);
    assert_eq!(result.type_identity, VIEW_IDENTITY);
    assert_eq!(
        result.multiplicity,
        language_semantics::Multiplicity::Unrestricted
    );
    let CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
        root,
        start: Some(start),
        end: None,
        ..
    } = &source.source
    else {
        panic!("an element range with a present start and an omitted end: {source:?}");
    };
    assert_eq!(
        *root,
        checked_trees::CheckedStorageRoot::ViewLocal {
            symbol: view_symbol
        }
    );
    assert_eq!(source.access, CheckedStructuralAccess::SharedBorrow);
    let (_, bound) = checked
        .facts
        .values
        .scalar_expressions
        .bound_expression_at(
            state,
            1,
            checked_trees::CheckedScalarExpressionRole::SubsliceStart {
                site: checked_trees::CheckedSubsliceSite::LocalBinding,
            },
        )
        .expect("the start endpoint is source-bound at the binding site");
    assert_eq!(bound, start);
    let [argument] = structural_arguments.as_slice() else {
        panic!("one forwarded view argument: {structural_arguments:?}");
    };
    assert_eq!(
        argument.source,
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        }
    );
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

/// A range over a fixed-array field names no established view: the binding
/// roots the range at the parameter owning the field and records the field
/// projection, so lowering views the whole array before narrowing it.
#[test]
fn a_field_range_local_narrows_the_whole_field_view() {
    let checked = checked(
        r#"
        data Holder {
            values: [i32 in Wrapping; 5];
            hi: u64;
        }

        machine observe(view: &[i32 in Wrapping]) {}

        machine Holder::run(&mut self) {
            self.hi = 4;
            let tail: &[i32 in Wrapping] = self.values[1..self.hi];
            observe(tail);
        }
    "#,
    );
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "run"))
        .expect("the field range local no longer leaves the unit plan roster");
    let Some(CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, source }) =
        plan.operations.iter().find(|operation| {
            matches!(
                operation,
                CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. }
            )
        })
    else {
        panic!("the field range binds a view subslice: {plan:?}");
    };
    assert_eq!(result.statement_index, 1);
    assert_eq!(result.type_identity, VIEW_IDENTITY);
    assert_eq!(source.access, CheckedStructuralAccess::SharedBorrow);
    assert_eq!(
        source.path,
        vec![checked_trees::CheckedUnitStructuralPathSegment::Field(
            "values".to_owned()
        )]
    );
    let CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
        root: checked_trees::CheckedStorageRoot::Parameter { .. },
        start: Some(_),
        end: Some(_),
        ..
    } = &source.source
    else {
        panic!("the range is rooted at the field's owner with both endpoints: {source:?}");
    };
}
