use super::{CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan};
use crate::execution::terminal_unit::ShapeCollector;
use crate::execution::terminal_unit::control::build_checked_machine;
use crate::execution::terminal_unit::structural_scalar_store::build_structural_scalar_field_store_sequence;
use crate::tests::front_end::checked_program;
use checked_trees::{CheckedScalarComputationKind, CheckedStructuralScalarFieldStoreValue};

fn fixture() -> checked_trees::CheckedTrees {
    let source = r#"
        machine narrow(value: u32) -> u8 { (value as u8 in Wrapping) as u8 }
        data Record { value: u8 in Wrapping; }
        machine Record::replace(&mut self, value: u32, previous: u8) {
            self.value = previous;
            self.value = narrow(value) as u8 in Wrapping;
        }
    "#;
    checked_program(source)
}

#[test]
fn selective_crashing_field_rhs_retains_its_ordered_source_plan() {
    use super::super::super::{
        checked_state_contracts_supported, control, state_flow, structural_scalar_signature,
    };
    let source = r#"
        boundary trait Trace { machine observe(value: bool) reaches Trace; }
        data Main { value: bool; }
        machine abort() -> bool crashes Abort { crash Abort; }
        machine Main::main(&mut self, fail: bool) reaches Trace crashes Abort {
            self.value = true;
            Trace::observe(self.value);
            self.value = fail && abort();
            Trace::observe(self.value);
        }
    "#;
    let checked = checked_program(source);
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let mut shapes = ShapeCollector::new(program);
    let (_, structural, scalar) =
        structural_scalar_signature(program, &mut shapes, machine, state, &[], true)
            .expect("retain mutable receiver and scalar input");
    assert!(
        checked_state_contracts_supported(program, machine, state, &structural),
        "retain authored crash contract"
    );
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
    .expect("retain complete write frame and both stores");
    let flow = state_flow(&checked.facts, machine.symbol, state.symbol).unwrap();
    let calls = checked.facts.flow.control.calls.span_or_empty(flow.calls);
    let outer = control::outer_calls(program, &checked.facts, machine.symbol, state, calls)
        .expect("retain assignment call custody separately from statement calls");
    assert_eq!(outer.len(), 2);
    for call in &outer {
        super::super::super::build_call_operation(
            program,
            &checked.facts,
            Some(crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &checked.facts.flow.terminal_structural_scalar_returns,
            }),
            machine,
            state,
            &structural,
            &[],
            &[],
            call,
            false,
            None,
            &[],
        )
        .unwrap_or_else(|| panic!("retain statement call {}", call.statement_index));
    }
    control::statement_sequence::build(
        program,
        &checked.facts,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
            structural_returns: &checked.facts.flow.terminal_structural_scalar_returns,
        },
        &mut shapes,
        machine,
        state,
        &structural,
        &scalar,
        &[],
        &outer,
        &[],
        0,
        None,
        &control::LocalConstructionTrace::default(),
    )
    .expect("retain ordered statement sequence");
    build_checked_machine(
        program,
        &checked.facts,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
            structural_returns: &checked.facts.flow.terminal_structural_scalar_returns,
        },
        &mut shapes,
        machine,
        &[],
        &[],
        None,
    )
    .expect("retain complete ordinary Unit candidate");
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine.symbol)
            .is_some(),
        "retain candidate after closure pruning"
    );
}

#[test]
fn field_call_assignment_retains_original_root_and_scalar_parameter_namespace() {
    let checked = fixture();
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Record::replace")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let plan = build_checked_machine(
        program,
        &checked.facts,
        crate::execution::terminal_unit::ScalarCalleePlans {
            boundary_returns: &checked.facts.flow.terminal_boundary_scalar_returns,
            structural_returns: &checked.facts.flow.terminal_structural_scalar_returns,
        },
        &mut ShapeCollector::new(program),
        machine,
        &[],
        &[],
        None,
    )
    .expect("call RHS belongs to the field store, not a dropped Unit call");
    let [
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(pure),
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(computed),
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = plan.operations.as_slice()
    else {
        panic!("ordered stores and return");
    };
    assert!(pure.value.as_pure().is_some());
    let CheckedStructuralScalarFieldStoreValue::Computation(handle) = computed.value else {
        panic!("exact computed RHS");
    };
    let plans = &checked.facts.values.scalar_computations;
    let root = plans
        .root_at(
            state.symbol,
            1,
            CheckedScalarExpressionRole::AssignmentValue,
        )
        .unwrap();
    assert_eq!(root.root, handle);
    assert_eq!(root.machine, machine.symbol);
    let calls = plans
        .nodes
        .iter()
        .filter_map(|(_, node)| match &node.kind {
            CheckedScalarComputationKind::Call { arguments, .. } => Some(arguments),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    let [argument] = plans.operands.span(*calls[0]).unwrap() else {
        panic!("one authored argument");
    };
    assert!(matches!(
        plans.nodes.get(*argument).kind,
        CheckedScalarComputationKind::Value(checked_trees::CheckedScalarExpression::Parameter {
            position: 0,
            primitive_type: typed_trees::types::PrimitiveType::U32,
        })
    ));
}

/// A Trapping integer binary keeps its deliberate no-fact boundary: the
/// checked scalar stage records neither a pure `AssignmentValue` expression
/// nor a computation root for `self.x + 1`, so the field-store sequence
/// declines at its source guard -- the destination, field identity, and
/// arithmetic-policy carrier are all admitted. The missing Trapping operation
/// family is owned by ARITHMETIC-POLICY-REALIZATION (Terminal Psi has no
/// Trapping operation to realize) with the check-stage refusal pinned by
/// `flow/transfers` under NOMINAL-FIELD-FLOW; this lane owns only the proof
/// that the store declines exactly there. Pins the
/// `arithmetic/runtime_trapping_overflow_traps` boundary.
#[test]
fn trapping_binary_assignment_declines_at_the_missing_scalar_source() {
    use super::super::super::structural_scalar_signature;
    use crate::execution::terminal_unit::LocalConstructionTrace;
    let source = r#"
        data Main { x: i32 in Trapping; }
        machine Main::bump(&mut self) {
            self.x = 2147483647;
            self.x = self.x + 1;
        }
    "#;
    let checked = checked_program(source);
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::bump")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    // The literal store's source binds an ordinary pure expression, so the
    // receiver destination, the exact field, and the `in Trapping` carrier
    // all pass admission; only the computed RHS below is unowned.
    assert!(
        checked
            .facts
            .values
            .scalar_expressions
            .expression_at(
                state.symbol,
                0,
                CheckedScalarExpressionRole::AssignmentValue
            )
            .is_some(),
        "literal assignment retains its bound source"
    );
    assert!(
        checked
            .facts
            .values
            .scalar_expressions
            .expression_at(
                state.symbol,
                1,
                CheckedScalarExpressionRole::AssignmentValue
            )
            .is_none()
            && checked
                .facts
                .values
                .scalar_computations
                .root_at(
                    state.symbol,
                    1,
                    CheckedScalarExpressionRole::AssignmentValue
                )
                .is_none(),
        "Trapping `self.x + 1` records no checked scalar value fact"
    );
    let mut shapes = ShapeCollector::new(program);
    let (_, structural, scalar) =
        structural_scalar_signature(program, &mut shapes, machine, state, &[], true)
            .expect("mutable receiver signature");
    let trace = LocalConstructionTrace::default();
    assert!(
        super::super::build_structural_scalar_field_store_sequence_traced(
            program,
            &checked.facts,
            machine,
            state,
            &structural,
            &scalar,
            0,
            None,
            &trace,
        )
        .is_none()
    );
    assert_eq!(
        trace.stage(),
        checked_trees::CheckedUnitPlanOmissionStage::LocalConstruction {
            phase: "structural field store: record literal field",
            state_index: None,
            statement_index: None,
        }
    );
}

#[test]
fn field_call_assignment_rejects_missing_stale_and_substituted_root_custody() {
    let checked = fixture();
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Record::replace")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let (root_handle, root) = checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .find(|(_, root)| root.state == state.symbol && root.statement_ordinal == 1)
        .unwrap();
    for mutation in 0..7 {
        let mut facts = checked.facts.clone();
        let plans = &mut facts.values.scalar_computations;
        match mutation {
            0 => plans.roots.get_mut(root_handle).machine = symbols::SymbolHandle::invalid(),
            1 => plans.roots.get_mut(root_handle).root = arena::Handle::invalid(),
            2 => plans.roots.get_mut(root_handle).statement_ordinal = 0,
            3 => plans.roots.get_mut(root_handle).role = CheckedScalarExpressionRole::Guard,
            4 => plans.nodes.get_mut(root.root).authored_root = arena::Handle::invalid(),
            5 => {
                plans.nodes.get_mut(root.root).primitive_type =
                    typed_trees::types::PrimitiveType::U32
            }
            6 => {
                plans.roots.append(root.clone());
            }
            _ => unreachable!(),
        }
        assert!(
            build_checked_machine(
                program,
                &facts,
                crate::execution::terminal_unit::ScalarCalleePlans {
                    boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                    structural_returns: &facts.flow.terminal_structural_scalar_returns
                },
                &mut ShapeCollector::new(program),
                machine,
                &[],
                &[],
                None,
            )
            .is_none(),
            "mutation {mutation}"
        );
    }
}
