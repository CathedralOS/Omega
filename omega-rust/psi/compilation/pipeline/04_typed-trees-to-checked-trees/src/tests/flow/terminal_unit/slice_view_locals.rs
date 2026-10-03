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

#[test]
fn record_view_initializer_keeps_its_origin_before_a_later_replacement() {
    let checked = checked(
        r#"
        data Sample { bytes: &[u8]; }
        data Context { first: [u8; 4]; second: [u8; 4]; observed: u64; }
        machine replace(sample: &mut Sample, bytes: &[u8]) {
            sample.bytes = bytes;
        }
        machine enter(context: &mut Context) {
            let sample: Sample = Sample { bytes: context.first[0..2] };
            replace(&mut sample, context.second[1..3]);
            context.observed = sample.bytes.len;
        }
    "#,
    );
    let (state, local) = local_symbol(&checked, "enter", "sample");
    let loans = checked
        .facts
        .borrow
        .loans
        .iter()
        .filter(|(_, loan)| loan.owner_symbol == local)
        .map(|(_, loan)| loan)
        .collect::<Vec<_>>();
    assert!(
        checked
            .facts
            .values
            .structural_values
            .roots
            .iter()
            .any(|(_, root)| { root.state == state && root.statement_ordinal == 0 }),
        "the initializer keeps its formation-time source: {loans:#?}",
    );
    // A later replacement cannot stand in for missing formation custody,
    // and two sources at formation cannot be reduced to an arbitrary one.
    for missing in [true, false] {
        let mut borrow = checked.facts.borrow.clone();
        let handle = borrow
            .loans
            .iter()
            .find(|(_, loan)| {
                loan.owner_symbol == local && loan.statement_index == usize::from(!missing)
            })
            .expect("initializer and replacement each have a loan")
            .0;
        borrow.loans.get_mut(handle).statement_index = if missing { 2 } else { 0 };
        let (_, values) = crate::values::build_checked_value_computation_plans(
            &checked.typed,
            &checked.facts.operators,
            &checked.facts.flow,
            &borrow,
            &checked.facts.proof,
            &checked.facts.values.scalar_expressions,
            &[],
        );
        assert!(
            !values
                .roots
                .iter()
                .any(|(_, root)| { root.state == state && root.statement_ordinal == 0 }),
            "missing or ambiguous formation custody must reject"
        );
    }
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

/// Literal bindings use the same ordered namespace as other structural locals,
/// even when mutable scalar storage and its update separate their declarations.
#[test]
fn literal_view_locals_compose_with_primitive_storage_and_scalar_completion() {
    let checked = checked(
        r#"
        machine measure() -> u64 {
            let mut count: u64 = 1;
            let bytes: &[u8] = "abc";
            count = bytes.len;
            let more: &[u8] = "de";
            more.len
        }
    "#,
    );
    let machine = machine_named(&checked, "measure");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .expect("literal locals share ordinary scalar body planning");
    let literals = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            checked_trees::CheckedUnitEffectOperationPlan::EstablishByteSequence {
                result,
                value,
                ..
            } => Some((
                result.statement_index,
                result.binding_ordinal,
                value.literal_bytes()?,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(literals, vec![(1, 0, &b"abc"[..]), (3, 1, &b"de"[..])]);
    assert!(plan.operations.iter().any(|operation| matches!(
        operation,
        checked_trees::CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
    )));
}

#[test]
fn literal_binding_precedes_guarded_structural_result_ordinals() {
    let checked = checked(
        r#"
        data Size [copy] { length: u64; }
        machine choose(flag: bool) -> Size {
            let bytes: &[u8] = "abc";
            transition flag {
                true -> (Size { length: 3 })
                false -> (Size { length: 2 })
            }
        }
    "#,
    );
    let machine = machine_named(&checked, "choose");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .expect("literal prefix and guarded structural completion share one graph");
    let entry = &plan.states[0];
    let literal = entry
        .operations
        .iter()
        .find_map(|operation| match operation {
            checked_trees::CheckedUnitEffectOperationPlan::EstablishByteSequence {
                result, ..
            } => Some(result),
            _ => None,
        })
        .expect("literal prefix binding");
    assert_eq!(literal.binding_ordinal, 0);
    let checked_trees::CheckedComposedUnitControlTerminatorPlan::Guarded { return_values, .. } =
        &entry.terminator
    else {
        panic!("expected guarded structural returns");
    };
    let returned = return_values
        .iter()
        .filter_map(checked_trees::CheckedUnitEffectOperationPlan::structural_result_binding)
        .collect::<Vec<_>>();
    assert!(
        !returned.is_empty(),
        "guarded arms construct structural results"
    );
    assert!(
        returned
            .iter()
            .all(|result| result.binding_ordinal > literal.binding_ordinal),
        "terminator construction must not reuse the preceding literal binding"
    );
}

#[test]
fn literal_view_local_can_be_borrowed_by_a_boundary_before_a_state_edge() {
    let checked = checked(
        r#"
        boundary trait Output { machine write(bytes: &[u8]) reaches Output; }
        data Root {}
        machine Root::enter() reaches Output {
            let bytes: &[u8] = "abc";
            Output::write(bytes);
            transition { _ -> next(bytes) }
            state next(bytes: &[u8]) { Output::write(bytes); }
        }
        "#,
    );
    let machine = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .expect("a borrowed local keeps the same call custody before a state edge");
    assert!(plan.states[0].operations.iter().any(|operation| matches!(
        operation,
        checked_trees::CheckedUnitEffectOperationPlan::Call(call)
            if call.is_boundary()
                && *call.result() == checked_trees::CheckedUnitCallResult::Unit
                && call.structural_arguments.iter().any(|argument|
                    argument.source_structural_result_binding_ordinal() == Some(0)
                        && argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow)
    )));
}

#[test]
fn forwarding_a_nonliteral_view_still_requires_its_source_loan() {
    let checked = checked(
        r#"
        boundary trait Output { machine observe(value: u64) reaches Output; }
        data Helper {}
        machine Helper::measure(bytes: &[u8]) reaches Output { Output::observe(bytes.len); }
        data Root {}
        machine Root::enter(bytes: &[u8]) reaches Output {
            let view: &[u8] = bytes[..];
            Helper::measure(view);
        }
        "#,
    );
    let machine = machine_named(&checked, "enter");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine)
        .expect("the original subslice can be forwarded");
    let borrow_state = checked
        .facts
        .borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.machine_symbol == machine && state.state_symbol == plan.state)
        .expect("caller borrow state");
    let loans = borrow_state.loans;
    assert!(!checked.facts.borrow.loans.span_or_empty(loans).is_empty());
    let mut facts = checked.facts.clone();
    for loan in facts.borrow.loans.span_mut(loans).expect("source loans") {
        loan.owner_symbol = symbols::SymbolHandle::invalid();
    }
    let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
        &checked.typed,
        &facts,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
            structural_returns: &facts.flow.terminal_structural_scalar_returns,
        },
    );
    assert!(
        rebuilt.for_machine(machine).is_none(),
        "a missing loan cannot become literal custody"
    );
}

#[test]
fn view_range_parameter_preserves_shared_and_mutable_access() {
    for (borrow, access) in [
        ("&", CheckedStructuralAccess::SharedBorrow),
        ("&mut ", CheckedStructuralAccess::MutableBorrow),
    ] {
        let checked = checked(&format!(
            "machine measure(values: {borrow}[u64]) -> u64 requires values.len >= 1; {{ let tail: {borrow}[u64] = values[1..]; tail.len }}"
        ));
        let symbol = machine_named(&checked, "measure");
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(symbol)
            .unwrap_or_else(|| {
                panic!(
                    "{borrow} range omitted: {:?}",
                    checked
                        .facts
                        .flow
                        .terminal_unit_effects
                        .omission_for_machine(symbol)
                )
            });
        let range = plan
            .operations
            .iter()
            .find_map(|operation| match operation {
                checked_trees::CheckedUnitEffectOperationPlan::EstablishViewSubslice {
                    source,
                    ..
                } => Some(source),
                _ => None,
            })
            .expect("ordinary view range");
        assert_eq!(range.access, access);
    }
}
