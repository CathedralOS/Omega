use super::*;
use checked_trees::CheckedScalarExpressionRole;

fn sequence_source(kind: ResultKind, computed: bool) -> String {
    let declaration = match kind {
        ResultKind::Scalar => "machine Sink::produce(first: u32, second: u32) -> u32 { first }",
        ResultKind::BoundaryScalar => {
            "boundary machine Sink::produce(first: u32, second: u32) -> u32 ensures true;"
        }
        ResultKind::BoundaryStructural => unreachable!(),
    };
    let first = if computed { "numeric(seed)" } else { "seed" };
    let second = if computed {
        "numeric(copied)"
    } else {
        "copied"
    };
    format!(
        "pub data Sink {{}} data Root {{}}
         machine numeric(input: u32) -> u32 {{ input }}
         machine Sink::observe(input: u32) {{}}
         boundary machine Sink::record(input: u32) ensures true;
         {declaration}
         machine Root::enter(input: u32) {{
             let seed: u32 = input;
             Sink::observe(seed);
             let first: u32 = Sink::produce({first}, seed);
             Sink::record(first);
             let copied: u32 = first;
             let second: u32 = Sink::produce({second}, first);
             Sink::observe(second);
         }}"
    )
}

#[test]
fn later_scalar_results_keep_statement_coordinates_and_pre_destination_namespaces() {
    for kind in [ResultKind::Scalar, ResultKind::BoundaryScalar] {
        for computed in [false, true] {
            let source = sequence_source(kind, computed);
            let checked = lower_typed_trees(typed_trees(&source))
                .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
            let machine = caller(&checked);
            let state = &checked.machine_states(machine)[0];
            let statements = checked.statement_table.statements(state.statement_nodes);
            assert_eq!(statements.len(), 7, "no authored temporary statements");
            let plan = checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine.symbol)
                .expect("ordered scalar-local Unit body");
            let bindings = plan
                .operations
                .iter()
                .filter_map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, .. }
                    | CheckedUnitEffectOperationPlan::ScalarCall { result, .. }
                    | CheckedUnitEffectOperationPlan::BoundaryScalarCall { result, .. } => {
                        Some((result.statement_index, result.binding_ordinal))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(bindings, [(0, 0), (2, 1), (4, 2), (5, 3)]);
            let mut namespace = checked
                .state_parameters(state)
                .iter()
                .map(|parameter| parameter.symbol)
                .collect::<Vec<_>>();
            let pure = &checked.facts.values.scalar_expressions;
            let computations = &checked.facts.values.scalar_computations;
            for (statement_index, statement) in statements.iter().enumerate() {
                let StatementNode::LocalData(local) = statement else {
                    continue;
                };
                if [2, 5].contains(&statement_index) {
                    let ExpressionNode::Call(call) =
                        checked.expression_table.expression(local.initial_value)
                    else {
                        panic!("bare result call")
                    };
                    let authored = checked.expression_table.expression_handles(call.arguments);
                    let operation = plan
                        .operations
                        .iter()
                        .find(|operation| match operation {
                            CheckedUnitEffectOperationPlan::ScalarCall { coordinate, .. }
                            | CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                                coordinate,
                                ..
                            } => coordinate.statement_index as usize == statement_index,
                            _ => false,
                        })
                        .unwrap();
                    let arguments = match (operation, kind) {
                        (
                            CheckedUnitEffectOperationPlan::ScalarCall {
                                scalar_arguments, ..
                            },
                            ResultKind::Scalar,
                        )
                        | (
                            CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                                scalar_arguments,
                                ..
                            },
                            ResultKind::BoundaryScalar,
                        ) => scalar_arguments,
                        _ => panic!("result carrier follows its exact target"),
                    };
                    let roots = computations
                        .roots
                        .iter()
                        .map(|(_, root)| root)
                        .filter(|root| {
                            root.state == state.symbol
                                && root.statement_ordinal as usize == statement_index
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(roots.len(), usize::from(computed), "operand roots only");
                    if computed {
                        assert_eq!(roots[0].role, role(kind, 0));
                        assert_eq!(roots[0].machine, machine.symbol);
                        assert_eq!(
                            arguments[0],
                            CheckedCallScalarArgument::Computation(roots[0].root)
                        );
                        let node = computations.nodes.get(roots[0].root);
                        assert_eq!(node.authored_root, authored[0]);
                        let CheckedScalarComputationKind::Call {
                            source_call,
                            call_ordinal,
                            ..
                        } = node.kind
                        else {
                            panic!("one nested numeric invocation")
                        };
                        assert_eq!(call_ordinal, 1);
                        let occurrence = checked.facts.flow.control.calls.get(source_call);
                        assert_eq!(occurrence.statement_index, statement_index);
                        assert_eq!(occurrence.authored_expression, authored[0]);
                        assert_ne!(occurrence.authored_expression, local.initial_value);
                    } else {
                        assert!(matches!(arguments[0], CheckedCallScalarArgument::Pure(_)));
                    }
                    for ordinal in if computed { 1..2 } else { 0..2 } {
                        assert!(matches!(
                            arguments[ordinal],
                            CheckedCallScalarArgument::Pure(_)
                        ));
                        let (binding, _) = pure
                            .bound_expression_at(
                                state.symbol,
                                statement_index as u32,
                                role(kind, ordinal as u32),
                            )
                            .unwrap();
                        assert_eq!(binding.expression, authored[ordinal]);
                        assert_eq!(
                            pure.binding_symbols.span_or_empty(binding.symbols),
                            namespace
                        );
                        assert!(
                            !namespace.contains(&local.symbol),
                            "destination is not yet live"
                        );
                    }
                    assert!(!roots.iter().any(|root| matches!(
                        root.role,
                        CheckedScalarExpressionRole::LocalInitializer { .. }
                    )));
                } else {
                    let ordinal = if statement_index == 0 { 0 } else { 2 };
                    let (binding, _) = pure
                        .bound_expression_at(
                            state.symbol,
                            statement_index as u32,
                            CheckedScalarExpressionRole::LocalInitializer {
                                binding_ordinal: ordinal,
                            },
                        )
                        .unwrap();
                    assert_eq!(binding.destination, local.symbol);
                    assert_eq!(
                        pure.binding_symbols.span_or_empty(binding.symbols),
                        namespace
                    );
                }
                namespace.push(local.symbol);
            }
        }
    }
}

#[test]
fn later_result_eligibility_rejects_mutability_receivers_and_semantic_modifiers() {
    let source = sequence_source(ResultKind::Scalar, true);
    let program = typed_trees(&source);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let StatementNode::LocalData(local) =
        &program.statement_table.statements(state.statement_nodes)[5]
    else {
        panic!("later initializer")
    };
    assert!(validation::unit_result_initializer_call_is_supported(
        &program,
        machine,
        local.initial_value
    ));
    for mutation in 0..5 {
        let mut changed = program.clone();
        if mutation == 0 {
            let StatementNode::LocalData(local) = &mut changed
                .statement_table
                .statements_mut(state.statement_nodes)[5]
            else {
                unreachable!()
            };
            local.is_mutable = true;
        } else {
            let ExpressionNode::Call(call) =
                changed.expression_table.expression_mut(local.initial_value)
            else {
                unreachable!()
            };
            let selected = typed_trees::expression::StaticMachineArgument {
                path: Box::new([]),
                application: None,
                const_literal: None,
                evidence_projection: None,
                symbol: call.target_symbol,
            };
            match mutation {
                1 => call.receiver = local.initial_value,
                2 => call.machine_arguments = Box::new([selected]),
                3 => {
                    call.quotient_operation =
                        Some(typed_trees::expression::QuotientOperationRequest {
                            kind: typed_trees::expression::QuotientOperationKind::Lift,
                            representative_operation: selected,
                            theorem_evidence: Box::new([]),
                        })
                }
                4 => {
                    call.private_layout_operation =
                        Some(typed_trees::expression::PrivateLayoutOperationRequest {
                            selected_slot: selected,
                        })
                }
                _ => unreachable!(),
            }
        }
        assert!(
            !validation::unit_result_initializer_call_is_supported(
                &changed,
                machine,
                local.initial_value
            ),
            "later initializer mutation={mutation}"
        );
    }
}

const BOUNDARY_STRUCTURAL_SEQUENCE: &str =
    "pub data Packet { value: u32; } pub data Sink {} data Root {}
        machine numeric(input: u32) -> u32 { input }
        boundary machine Sink::produce(input: u32, prior: u32) -> Packet ensures true;
        machine Root::enter(input: u32) {
            let prior: u32 = input;
            let result: Packet = Sink::produce(numeric(prior), prior);
        }";

#[test]
fn later_boundary_structural_results_keep_operand_roots_and_scalar_namespace() {
    for unrestricted in [false, true] {
        let source = if unrestricted {
            BOUNDARY_STRUCTURAL_SEQUENCE.replace("Packet {", "Packet [copy] {")
        } else {
            BOUNDARY_STRUCTURAL_SEQUENCE.to_owned()
        };
        let checked = lower_typed_trees(typed_trees(&source)).expect("later boundary result");
        let machine = caller(&checked);
        let [state] = checked.machine_states(machine) else {
            panic!("one authored state")
        };
        let [
            StatementNode::LocalData(prior),
            StatementNode::LocalData(local),
        ] = checked.statement_table.statements(state.statement_nodes)
        else {
            panic!("no synthetic source statements")
        };
        assert!(validation::unit_result_initializer_call_is_supported(
            &checked.typed,
            machine,
            local.initial_value
        ));
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine.symbol)
            .unwrap();
        let [
            CheckedUnitEffectOperationPlan::EstablishScalarLocal {
                result: prior_result,
                ..
            },
            CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate,
                result,
                scalar_arguments,
                discard_result_on_return,
                ..
            },
            CheckedUnitEffectOperationPlan::ReturnUnit { .. },
        ] = plan.operations.as_slice()
        else {
            panic!("one scalar prefix and one structural result")
        };
        assert_eq!(
            (prior_result.statement_index, prior_result.binding_ordinal),
            (0, 0)
        );
        assert_eq!((result.statement_index, result.binding_ordinal), (1, 0));
        assert_eq!(
            (coordinate.statement_index, coordinate.call_ordinal),
            (1, 0)
        );
        assert_eq!(*discard_result_on_return, !unrestricted);
        let ExpressionNode::Call(call) = checked.expression_table.expression(local.initial_value)
        else {
            unreachable!()
        };
        let authored = checked.expression_table.expression_handles(call.arguments);
        let computations = &checked.facts.values.scalar_computations;
        let roots = computations
            .roots
            .iter()
            .map(|(_, root)| root)
            .filter(|root| root.state == state.symbol && root.statement_ordinal == 1)
            .collect::<Vec<_>>();
        assert_eq!(roots.len(), 1, "no duplicate whole-initializer computation");
        assert_eq!(roots[0].role, role(ResultKind::BoundaryStructural, 0));
        assert_eq!(
            scalar_arguments[0],
            CheckedCallScalarArgument::Computation(roots[0].root)
        );
        let node = computations.nodes.get(roots[0].root);
        assert_eq!(node.authored_root, authored[0]);
        let CheckedScalarComputationKind::Call {
            source_call,
            call_ordinal,
            arguments,
            ..
        } = node.kind
        else {
            panic!("nested numeric invocation")
        };
        assert_eq!(call_ordinal, 1);
        let occurrence = checked.facts.flow.control.calls.get(source_call);
        assert_eq!(occurrence.statement_index, 1);
        assert_eq!(occurrence.authored_expression, authored[0]);
        let [argument] = computations.operands.span_or_empty(arguments) else {
            panic!("one numeric operand")
        };
        assert!(matches!(
            computations.nodes.get(*argument).kind,
            CheckedScalarComputationKind::Value(checked_trees::CheckedScalarExpression::Local {
                position: 1,
                primitive_type: typed_trees::types::PrimitiveType::U32
            })
        ));
        let pure = &checked.facts.values.scalar_expressions;
        let (binding, _) = pure
            .bound_expression_at(state.symbol, 1, role(ResultKind::BoundaryStructural, 1))
            .unwrap();
        assert_eq!(binding.expression, authored[1]);
        assert_eq!(
            pure.binding_symbols.span_or_empty(binding.symbols),
            [checked.state_parameters(state)[0].symbol, prior.symbol]
        );
        assert!(
            !pure
                .binding_symbols
                .span_or_empty(binding.symbols)
                .contains(&local.symbol)
        );
    }
}

#[test]
fn later_boundary_structural_eligibility_keeps_ownership_and_target_fences() {
    use typed_trees::types::{DomainConstraint, TypeConstraintNode, TypeReferenceNode};
    let program = typed_trees(BOUNDARY_STRUCTURAL_SEQUENCE);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let StatementNode::LocalData(local) =
        &program.statement_table.statements(state.statement_nodes)[1]
    else {
        panic!("later structural initializer")
    };
    for mutation in 0..4 {
        let mut changed = program.clone();
        let reference = match mutation {
            1 => changed
                .type_reference_table
                .insert(TypeReferenceNode::Reference {
                    referee: local.type_reference,
                    access: language_semantics::ReferenceAccess::Shared,
                    lifetime: None,
                }),
            2 => {
                let constraints =
                    changed
                        .type_reference_table
                        .insert_constraints([TypeConstraintNode::Domain(DomainConstraint {
                            name: typed_trees::name::Identifier::generated("Unestablished"),
                            ..DomainConstraint::default()
                        })]);
                changed
                    .type_reference_table
                    .insert(TypeReferenceNode::Constrained {
                        base_type: local.type_reference,
                        constraints,
                    })
            }
            _ => local.type_reference,
        };
        let StatementNode::LocalData(changed_local) = &mut changed
            .statement_table
            .statements_mut(state.statement_nodes)[1]
        else {
            unreachable!()
        };
        changed_local.type_reference = reference;
        if mutation == 0 {
            changed_local.is_mutable = true;
        } else if mutation == 3 {
            changed
                .machines_mut()
                .iter_mut()
                .find(|target| target.name.as_str() == "Sink::produce")
                .unwrap()
                .supply_mode = language_semantics::MachineSupplyMode::Requirement;
        }
        assert!(
            !validation::unit_result_initializer_call_is_supported(
                &changed,
                machine,
                local.initial_value
            ),
            "unsupported later structural result mutation {mutation}"
        );
    }
    let linear =
        typed_trees(&BOUNDARY_STRUCTURAL_SEQUENCE.replace("Packet {", "Packet [linear] {"));
    let machine = linear
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .unwrap();
    let [state] = linear.machine_states(machine) else {
        unreachable!()
    };
    let StatementNode::LocalData(local) =
        &linear.statement_table.statements(state.statement_nodes)[1]
    else {
        unreachable!()
    };
    assert!(!validation::unit_result_initializer_call_is_supported(
        &linear,
        machine,
        local.initial_value
    ));
}

#[test]
fn boundary_structural_results_transfer_once_through_existing_affine_consumers() {
    for forward in [false, true] {
        let completion = if forward {
            "let moved: Packet = forward(result); Root::consume(moved);"
        } else {
            "Root::consume(result);"
        };
        let source = format!(
            "{} machine Root::consume(packet: Packet) {{}}
             machine forward(packet: Packet) -> Packet {{ packet }}",
            BOUNDARY_STRUCTURAL_SEQUENCE.replace(
                "Sink::produce(numeric(prior), prior);",
                &format!("Sink::produce(numeric(prior), prior); {completion}")
            )
        );
        let checked = lower_typed_trees(typed_trees(&source)).expect("affine result moves");
        let machine = caller(&checked);
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(machine.symbol)
            .unwrap();
        let results = plan
            .operations
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    result,
                    discard_result_on_return,
                    ..
                }
                | CheckedUnitEffectOperationPlan::StructuralCall {
                    result,
                    discard_result_on_return,
                    ..
                } => Some((result.binding_ordinal, *discard_result_on_return)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            results,
            if forward {
                vec![(0, false), (1, false)]
            } else {
                vec![(0, false)]
            }
        );
        let sources = plan
            .operations
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::StructuralCall {
                    structural_arguments,
                    ..
                }
                | CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } => {
                    let [argument] = structural_arguments.as_slice() else {
                        return None;
                    };
                    argument.source_structural_result_binding_ordinal()
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(sources, if forward { vec![0, 1] } else { vec![0] });
        let repeated = source
            .replace(
                "Root::consume(result);",
                "Root::consume(result); Root::consume(result);",
            )
            .replace(
                "Root::consume(moved);",
                "Root::consume(moved); Root::consume(moved);",
            );
        assert!(
            lower_typed_trees(typed_trees(&repeated)).is_err(),
            "second owned move must reject"
        );
    }
}
