//! Composed leaves retain operand computations without adding outer effects.

use super::*;
use checked_trees::{CheckedCallScalarArgument, CheckedScalarComputationKind};

fn checked_operands(linear: bool, nested: bool) -> checked_trees::CheckedTrees {
    let helper = "machine inner(value: u8) -> u8 { value }
        machine outer(value: u8) -> u8 { value }
        machine boolean(value: bool) -> bool { value }";
    let source = if linear {
        format!(
            "{helper}
             pub data Receipt [linear] {{ value: u64; }}
             boundary machine Receipt::settle(self, value: u8, flag: bool, last: u8)
                 ensures true;
             data Root {{}}
             machine Root::enter(flag: bool, receipt: Receipt) {{
                 transition flag {{ true -> yes(receipt) _ -> no(receipt) }}
                 state yes(receipt: Receipt) {{
                     receipt.settle(outer(inner(3u8)), boolean(true) || boolean(false), 7u8);
                 }}
                 state no(receipt: Receipt) {{
                     receipt.settle(outer(inner(4u8)), boolean(false) && boolean(true), 8u8);
                 }}
             }}"
        )
    } else {
        let parameters = if nested {
            "flag: bool, other: bool"
        } else {
            "flag: bool"
        };
        let control = if nested {
            "transition flag { true -> dispatch(other) _ -> no() }
             state dispatch(other: bool) {
                 transition other { true -> yes() _ -> no() }
             }"
        } else {
            "transition flag { true -> yes() _ -> no() }"
        };
        format!(
            "{helper}
             boundary trait Host {{ machine send(value: u8, flag: bool, last: u8); }}
             data Root {{}}
             machine Root::enter({parameters}) {{
                 {control}
                 state yes() {{ Host::send(outer(inner(3u8)), boolean(true) || boolean(false), 7u8); }}
                 state no() {{ Host::send(outer(inner(4u8)), boolean(false) && boolean(true), 8u8); }}
             }}"
        )
    };
    checked(&source)
}

#[test]
fn composed_boundary_leaves_keep_one_effect_and_dense_computed_operands() {
    for (linear, nested) in [(false, false), (true, false), (false, true)] {
        let checked = checked_operands(linear, nested);
        let machine = machine_named(&checked, "enter");
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(machine)
            .unwrap_or_else(|| panic!("linear={linear}, nested={nested}: nested operands do not prevent composed control"));
        assert_eq!(plan.states.len(), if nested { 4 } else { 3 });
        let leaves = plan
            .states
            .iter()
            .filter(|state| !state.operations.is_empty())
            .collect::<Vec<_>>();
        assert_eq!(leaves.len(), 2);
        for leaf in leaves {
            let [
                CheckedUnitEffectOperationPlan::BoundaryCall {
                    coordinate,
                    scalar_arguments,
                    structural_arguments,
                    completion_receipts,
                    ..
                },
            ] = leaf.operations.as_slice()
            else {
                panic!("each authored leaf has one outer boundary operation")
            };
            assert_eq!(coordinate.statement_index, 0);
            assert_eq!(coordinate.call_ordinal, 0);
            assert_eq!(scalar_arguments.len(), 3);
            assert!(matches!(
                scalar_arguments[2],
                CheckedCallScalarArgument::Pure(_)
            ));
            assert_eq!(structural_arguments.len(), usize::from(linear));
            assert_eq!(completion_receipts.len(), usize::from(linear));
            if linear {
                assert_eq!(
                    completion_receipts[0].claim_identity,
                    leaf.entry_claims[0].claim_identity
                );
            }
            let computations = &checked.facts.values.scalar_computations;
            for (ordinal, argument) in scalar_arguments[..2].iter().enumerate() {
                let CheckedCallScalarArgument::Computation(root) = argument else {
                    panic!("nested calls require retained operand roots")
                };
                let matching = computations
                    .roots
                    .iter()
                    .filter(|(_, candidate)| {
                        candidate.machine == machine
                            && candidate.state == leaf.state
                            && candidate.statement_ordinal == 0
                            && candidate.role
                                == CheckedScalarExpressionRole::BoundaryCallArgument {
                                    call_ordinal: 0,
                                    argument_ordinal: ordinal as u32,
                                }
                    })
                    .collect::<Vec<_>>();
                assert!(matches!(matching.as_slice(), [(_, candidate)] if candidate.root == *root));
            }
            let flow = checked
                .facts
                .flow
                .control
                .states
                .iter()
                .find_map(|(_, flow)| {
                    (flow.machine_symbol == machine && flow.state_symbol == leaf.state)
                        .then_some(flow)
                })
                .unwrap();
            let calls = checked.facts.flow.control.calls.span(flow.calls).unwrap();
            assert_eq!(calls.len(), 5, "one outer call and four nested occurrences");
            let mut ordinals = calls
                .iter()
                .map(|call| call.call_ordinal)
                .collect::<Vec<_>>();
            ordinals.sort_unstable();
            assert_eq!(ordinals, [0, 1, 2, 3, 4]);
        }
    }
}

#[test]
fn composed_boundary_operands_reject_missing_duplicate_and_stale_custody() {
    let baseline = checked_operands(false, false);
    let machine = machine_named(&baseline, "enter");
    let leaf = baseline
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .unwrap()
        .states[1]
        .state;
    let root_handle = baseline
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .find_map(|(handle, root)| (root.state == leaf).then_some(handle))
        .unwrap();
    let calls = baseline
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .filter_map(|(handle, node)| match node.kind {
            CheckedScalarComputationKind::Call { source_call, .. }
                if baseline
                    .facts
                    .flow
                    .control
                    .calls
                    .get(source_call)
                    .call_ordinal
                    != 0 =>
            {
                Some(handle)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    for mutation in 0..9 {
        let mut changed = baseline.clone();
        let computations = &mut changed.facts.values.scalar_computations;
        let root = computations.roots.get(root_handle).clone();
        match mutation {
            0 => computations.roots = arena::Arena::new(),
            1 => {
                computations.roots.append(root);
            }
            2 => computations.roots.get_mut(root_handle).root = arena::Handle::invalid(),
            3 => computations.roots.get_mut(root_handle).machine = symbols::SymbolHandle::invalid(),
            4 => computations.nodes.get_mut(root.root).authored_root = arena::Handle::invalid(),
            5 => {
                let CheckedScalarComputationKind::Call { source_call, .. } =
                    &mut computations.nodes.get_mut(calls[0]).kind
                else {
                    unreachable!()
                };
                *source_call = arena::Handle::from_parts(
                    source_call.arena_index(),
                    source_call.generation().wrapping_add(1),
                );
            }
            6 => {
                let CheckedScalarComputationKind::Call { source_call, .. } =
                    computations.nodes.get(calls[0]).kind
                else {
                    unreachable!()
                };
                let CheckedScalarComputationKind::Call {
                    source_call: other, ..
                } = &mut computations.nodes.get_mut(calls[1]).kind
                else {
                    unreachable!()
                };
                *other = source_call;
            }
            7 => {
                let CheckedScalarComputationKind::Call { source_call, .. } =
                    computations.nodes.get(calls[0]).kind
                else {
                    unreachable!()
                };
                changed
                    .facts
                    .flow
                    .control
                    .calls
                    .get_mut(source_call)
                    .call_ordinal = 0;
            }
            _ => {
                let CheckedScalarComputationKind::Call { source_call, .. } =
                    computations.nodes.get(calls[0]).kind
                else {
                    unreachable!()
                };
                changed
                    .facts
                    .flow
                    .control
                    .calls
                    .get_mut(source_call)
                    .authored_expression = arena::Handle::invalid();
            }
        }
        let rebuilt =
            crate::flow::build_checked_unit_effect_plans(&changed.typed, &changed.facts, &[], &[]);
        assert!(
            rebuilt.composed_for_machine(machine).is_none(),
            "mutation {mutation} must reject"
        );
    }
}

#[test]
fn closed_sum_leaves_retain_computed_calls_before_reusing_the_payload() {
    let checked = checked(
        r#"
        machine identity(value: i32) -> i32 { value }
        data ByteRead { case Eof; case Byte(value: i32 [0..=255]); }
        boundary trait Console {
            machine read_byte() -> ByteRead reaches Console;
            machine write_byte(value: i32) reaches Console;
            machine exit_process(value: i32) reaches Console;
        }
        data Main { console: Console; }
        machine Main::main(&mut self) reaches Console {
            let result: ByteRead = self.console.read_byte();
            transition result {
                ByteRead::Byte { value } -> byte(value)
                ByteRead::Eof -> eof()
            }
            state byte(&mut self, value: i32 [0..=255]) {
                self.console.write_byte(identity(identity(value)));
                self.console.exit_process(value);
            }
            state eof(&mut self) { self.console.exit_process(70); }
        }
        "#,
    );
    let machine = machine_named(&checked, "main");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .expect("closed-sum payload leaf retains nested call operands");
    let [entry, byte, _] = plan.states.as_slice() else {
        panic!("closed-sum control retains its authored three states")
    };
    assert!(matches!(
        entry.terminator,
        checked_trees::CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
    ));
    assert_eq!(byte.scalar_parameters.len(), 1);
    let [
        CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate: first_coordinate,
            scalar_arguments: first_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::BoundaryCall {
            coordinate: second_coordinate,
            scalar_arguments: second_arguments,
            ..
        },
    ] = byte.operations.as_slice()
    else {
        panic!("two authored boundary statements remain two outer operations")
    };
    assert_eq!(first_coordinate.statement_index, 0);
    assert_eq!(second_coordinate.statement_index, 1);
    assert_eq!(first_coordinate.call_ordinal, 0);
    assert_eq!(second_coordinate.call_ordinal, 0);
    let [CheckedCallScalarArgument::Computation(root)] = first_arguments.as_slice() else {
        panic!("first boundary operand retains its nested computation")
    };
    assert!(matches!(
        second_arguments.as_slice(),
        [CheckedCallScalarArgument::Pure(
            CheckedScalarExpression::Parameter {
                position: 0,
                primitive_type: PrimitiveType::I32
            }
        )]
    ));
    let computations = &checked.facts.values.scalar_computations;
    assert!(computations.roots.iter().any(|(_, candidate)| {
        candidate.machine == machine
            && candidate.state == byte.state
            && candidate.root == *root
            && candidate.statement_ordinal == 0
            && candidate.role
                == CheckedScalarExpressionRole::BoundaryCallArgument {
                    call_ordinal: 0,
                    argument_ordinal: 0,
                }
    }));

    let mut changed = checked.clone();
    let flow = changed
        .facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == machine && state.state_symbol == byte.state)
                .then_some(state.calls)
        })
        .unwrap();
    let outer = changed
        .facts
        .flow
        .control
        .calls
        .iter()
        .find_map(|(handle, call)| {
            (call.statement_index == 1
                && call.call_ordinal == 0
                && changed
                    .facts
                    .flow
                    .control
                    .calls
                    .span(flow)
                    .unwrap()
                    .iter()
                    .any(|candidate| std::ptr::eq(candidate, call)))
            .then_some(handle)
        })
        .unwrap();
    changed
        .facts
        .flow
        .control
        .calls
        .get_mut(outer)
        .statement_index = 0;
    let rebuilt =
        crate::flow::build_checked_unit_effect_plans(&changed.typed, &changed.facts, &[], &[]);
    assert!(
        rebuilt.composed_for_machine(machine).is_none(),
        "duplicate outer statement custody rejects"
    );
}
