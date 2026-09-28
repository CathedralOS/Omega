use super::checked_arms;
use checked_trees::CheckedScalarExpressionRole;
use checked_trees_to_lowered_psi::TerminalMachineSelection;

fn assert_initializer_roots(
    checked: &checked_trees::CheckedTrees,
    names: &[&str],
    state_count: usize,
) {
    assert_selected_initializer_roots(checked, names, names, state_count);
}

fn assert_selected_initializer_roots(
    checked: &checked_trees::CheckedTrees,
    names: &[&str],
    computed_names: &[&str],
    state_count: usize,
) {
    use typed_trees::statement::StatementNode;

    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let states = checked.typed.machine_states(machine);
    assert_eq!(states.len(), state_count, "no manufactured source state");
    let mut locals = Vec::new();
    for state in states {
        for (statement_ordinal, statement) in checked
            .typed
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
        {
            if let StatementNode::LocalData(local) = statement {
                locals.push(local.name.as_str());
                assert!(
                    local.initial_value.is_valid(),
                    "authored initializer retained"
                );
                if !computed_names.contains(&local.name.as_str()) {
                    continue;
                }
                let roots: Vec<_> = checked
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .iter()
                    .filter(|(_, root)| {
                        root.machine == machine.symbol
                            && root.state == state.symbol
                            && root.statement_ordinal == statement_ordinal as u32
                            && matches!(
                                root.role,
                                CheckedScalarExpressionRole::LocalInitializer { .. }
                                    | CheckedScalarExpressionRole::StorageInitializer
                            )
                    })
                    .collect();
                assert_eq!(roots.len(), 1, "one root for {}", local.name.as_str());
                let root = roots[0].1;
                assert_eq!(
                    root.role == CheckedScalarExpressionRole::StorageInitializer,
                    local.is_mutable,
                    "initializer destination agrees with source storage"
                );
                assert_eq!(
                    checked
                        .facts
                        .values
                        .scalar_computations
                        .nodes
                        .get(root.root)
                        .authored_root,
                    local.initial_value,
                    "initializer root belongs to the authored LocalData"
                );
            }
        }
    }
    assert_eq!(locals, names, "no hoisted source temporaries");
}

#[test]
fn computed_initializer_custody_mutations_reject_before_publication() {
    let source = r#"
        machine identity(input: bool) -> bool
        requires true == true
        ensures true == true
        { input }
        machine value(flag: bool) -> bool
        requires true == true
        ensures true == true
        {
            let saved: bool = identity(flag) && true;
            let mut current: bool = identity(!flag) || false;
            saved && !current
        }
    "#;
    let checked = checked_arms(source, false);
    assert_initializer_roots(&checked, &["saved", "current"], 1);
    checked_trees_to_lowered_psi::lower_machine(&checked, TerminalMachineSelection::Name("value"))
        .unwrap();
    let roots: Vec<_> = checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .filter(|(_, root)| {
            matches!(
                root.role,
                CheckedScalarExpressionRole::LocalInitializer { .. }
                    | CheckedScalarExpressionRole::StorageInitializer
            )
        })
        .map(|(handle, root)| (handle, root.clone()))
        .collect();
    assert_eq!(roots.len(), 2);
    for (handle, root) in &roots {
        let opposite = &roots.iter().find(|(other, _)| other != handle).unwrap().1;
        for mutation in 0..9 {
            let mut changed = checked.clone();
            let plans = &mut changed.facts.values.scalar_computations;
            match mutation {
                0 => {
                    plans.roots.append(root.clone());
                }
                1 => plans.roots.get_mut(*handle).root = arena::Handle::invalid(),
                2 => plans.roots.get_mut(*handle).role = CheckedScalarExpressionRole::Return,
                3 => {
                    plans.roots.get_mut(*handle).role =
                        CheckedScalarExpressionRole::LocalInitializer {
                            binding_ordinal: 99,
                        }
                }
                4 => plans.roots.get_mut(*handle).statement_ordinal += 100,
                5 => plans.nodes.get_mut(root.root).authored_root = arena::Handle::invalid(),
                6 => plans.roots.get_mut(*handle).root = opposite.root,
                7 => plans.roots.get_mut(*handle).role = opposite.role,
                8 => {
                    plans.nodes.get_mut(root.root).primitive_type =
                        typed_trees::types::PrimitiveType::U8;
                    let graph = changed
                        .facts
                        .flow
                        .terminal_scalar_graphs
                        .machines
                        .iter_mut()
                        .find(|graph| graph.machine == root.machine)
                        .unwrap();
                    let state = graph
                        .states
                        .iter_mut()
                        .find(|state| state.state == root.state)
                        .unwrap();
                    let binding = state
                        .bindings
                        .iter_mut()
                        .find(|binding| binding.statement_ordinal == root.statement_ordinal)
                        .unwrap();
                    binding.primitive_type = typed_trees::types::PrimitiveType::U8;
                }
                _ => unreachable!(),
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(
                    &changed,
                    TerminalMachineSelection::Name("value")
                )
                .is_err(),
                "mutation={mutation}, role={:?}",
                root.role
            );
        }
    }

    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .unwrap();
    let state = &checked.typed.machine_states(machine)[0];
    let parameter = checked.typed.state_parameters(state)[0].symbol;
    let (storage_handle, storage_root) = roots
        .iter()
        .find(|(_, root)| root.role == CheckedScalarExpressionRole::StorageInitializer)
        .unwrap();
    for symbol in [symbols::SymbolHandle::invalid(), parameter] {
        let mut changed = checked.clone();
        let graph = changed
            .facts
            .flow
            .terminal_scalar_graphs
            .machines
            .iter_mut()
            .find(|graph| graph.machine == storage_root.machine)
            .unwrap();
        let state = graph
            .states
            .iter_mut()
            .find(|state| state.state == storage_root.state)
            .unwrap();
        let binding = state
            .bindings
            .iter_mut()
            .find(|binding| binding.statement_ordinal == storage_root.statement_ordinal)
            .unwrap();
        binding.destination =
            checked_trees::CheckedScalarBindingDestination::StorageInitialize { symbol };
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("value")
            )
            .is_err(),
            "computed storage initializer rejects nonlocal destination {symbol:?}"
        );
    }

    for (handle, root) in &roots {
        let typed_trees::statement::StatementNode::LocalData(local) = &checked
            .typed
            .statement_table
            .statements(state.statement_nodes)[root.statement_ordinal as usize]
        else {
            panic!("authored local");
        };
        let (role, destination) = if handle == storage_handle {
            (
                CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 1 },
                checked_trees::CheckedScalarBindingDestination::Immutable,
            )
        } else {
            (
                CheckedScalarExpressionRole::StorageInitializer,
                checked_trees::CheckedScalarBindingDestination::StorageInitialize {
                    symbol: local.symbol,
                },
            )
        };
        let mut changed = checked.clone();
        changed
            .facts
            .values
            .scalar_computations
            .roots
            .get_mut(*handle)
            .role = role;
        let graph = changed
            .facts
            .flow
            .terminal_scalar_graphs
            .machines
            .iter_mut()
            .find(|graph| graph.machine == root.machine)
            .unwrap();
        let state = graph
            .states
            .iter_mut()
            .find(|state| state.state == root.state)
            .unwrap();
        let binding = state
            .bindings
            .iter_mut()
            .find(|binding| binding.statement_ordinal == root.statement_ordinal)
            .unwrap();
        binding.destination = destination;
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("value")
            )
            .is_err(),
            "coordinated graph and root-role change cannot alter authored mutability {:?}",
            root.role
        );
    }
}

#[test]
fn computed_nominal_borrows_retain_receiver_access_path_and_occurrence() {
    use checked_trees::{CheckedScalarComputationKind, CheckedScalarComputationStructuralArgument};
    use typed_trees::types::TypeReferenceNode;

    for explicit in [false, true] {
        for multistate in [false, true] {
            let (callee, invocation) = if explicit {
                (
                    "machine replace(counter: &mut Counter) -> i32 in Wrapping { counter.count = 7; 7 }",
                    "replace(&mut self.left)",
                )
            } else {
                (
                    "machine Counter::replace(&mut self) -> i32 in Wrapping { self.count = 7; 7 }",
                    "self.left.replace()",
                )
            };
            let completion = if multistate {
                "transition { _ -> finish(saved) } state finish(&mut self, result: i32 in Wrapping) -> i32 { result as i32 }"
            } else {
                "saved as i32"
            };
            let source = format!(
                r#"
                data Counter {{ count: i32; }}
                data Main {{ left: Counter; right: Counter; }}
                {callee}
                machine Main::value(&mut self) -> i32 {{
                    let saved: i32 in Wrapping = {invocation} * 10;
                    {completion}
                }}
            "#
            );
            let original = checked_arms(&source, false);
            let lowered = checked_trees_to_lowered_psi::lower_machine(
                &original,
                TerminalMachineSelection::Name("Main::value"),
            )
            .unwrap_or_else(|error| panic!("mutable nominal computation operand: {error:?}, explicit={explicit}, multistate={multistate}"));
            let semantic_bytes = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
            let proof_bytes = terminal_codec::encode_proof_section(
                &lowered.semantic_module,
                &lowered.proof_bundle,
            )
            .unwrap();
            let module = terminal_codec::decode_module(&semantic_bytes).unwrap();
            let proof = terminal_codec::decode_proof_bundle(&proof_bytes).unwrap();
            terminal_verifier::verify_module(
                &module,
                &proof,
                &proof_admission::AdmissionProfile::default(),
            )
            .expect("decoded mutable computation verifies independently");
            let plans = &original.facts.values.scalar_computations;
            let calls: Vec<_> = plans.nodes.iter().filter_map(|(handle, node)| {
                let CheckedScalarComputationKind::Call { structural_arguments, .. } = &node.kind else { return None; };
                let arguments = plans.structural_arguments.span(*structural_arguments)?;
                matches!(arguments, [CheckedScalarComputationStructuralArgument::Place(argument)]
                    if argument.access == checked_trees::CheckedStructuralAccess::MutableBorrow)
                    .then_some((handle, *structural_arguments))
            }).collect();
            let [(call_handle, arguments)] = calls.as_slice() else {
                panic!("one mutable computation call");
            };
            let machine = original
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "Main::value")
                .unwrap();
            let state = &original.machine_states(machine)[0];
            let receiver = state.parameters.start();
            for mutation in ["access", "path", "type", "ordinal", "shared backing"] {
                let mut changed = original.clone();
                if mutation == "ordinal" {
                    let CheckedScalarComputationKind::Call { call_ordinal, .. } = &mut changed
                        .facts
                        .values
                        .scalar_computations
                        .nodes
                        .get_mut(*call_handle)
                        .kind
                    else {
                        unreachable!();
                    };
                    *call_ordinal += 1;
                } else if mutation == "shared backing" {
                    let mut reference = changed
                        .type_reference_table
                        .type_reference(original.state_parameters(state)[0].type_reference)
                        .clone();
                    let TypeReferenceNode::Reference { access, .. } = &mut reference else {
                        panic!("mutable source receiver");
                    };
                    *access = language_semantics::ReferenceAccess::Shared;
                    let reference = changed.typed.type_reference_table.insert(reference);
                    changed
                        .typed
                        .state_parameters
                        .get_mut(receiver)
                        .type_reference = reference;
                } else {
                    let CheckedScalarComputationStructuralArgument::Place(argument) = changed
                        .facts
                        .values
                        .scalar_computations
                        .structural_arguments
                        .get_mut(arguments.start())
                    else {
                        unreachable!();
                    };
                    match mutation {
                        "access" => {
                            argument.access = checked_trees::CheckedStructuralAccess::SharedBorrow
                        }
                        "path" => {
                            argument.path =
                                vec![checked_trees::CheckedUnitStructuralPathSegment::Field(
                                    "right".to_owned(),
                                )]
                        }
                        "type" => argument.type_identity.push_str("_substituted"),
                        _ => unreachable!(),
                    }
                }
                assert!(
                    checked_trees_to_lowered_psi::lower_machine(
                        &changed,
                        TerminalMachineSelection::Name("Main::value")
                    )
                    .is_err(),
                    "{mutation}, explicit={explicit}, multistate={multistate}"
                );
            }
        }
    }
}

#[test]
fn pure_computed_callee_retains_exact_unread_receiver_access() {
    use checked_trees::CheckedStructuralAccess;
    use typed_trees::types::TypeReferenceNode;

    for transition in [false, true] {
        let body = if transition {
            "transition { _ -> (n * 2) }"
        } else {
            "n * 2"
        };
        let source = format!(
            r#"
            data Calc {{}}
            data Main {{ calc: Calc; }}
            machine Calc::double_val(&mut self, n: i32 in Wrapping) -> i32 in Wrapping {{ {body} }}
            machine Main::value(&mut self) -> i32 {{
                let result: i32 in Wrapping = self.calc.double_val(6) * 3;
                result as i32
            }}
        "#
        );
        let checked = checked_arms(&source, false);
        let callee = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Calc::double_val")
            .unwrap();
        let callee_symbol = callee.symbol;
        let state = &checked.machine_states(callee)[0];
        let receiver_handle = state.parameters.start();
        let receiver_type = checked.state_parameters(state)[0].type_reference;
        let graph_index = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .machines
            .iter()
            .position(|graph| graph.machine == callee_symbol)
            .expect("pure callee has its ordinary scalar graph");
        let graph = &checked.facts.flow.terminal_scalar_graphs.machines[graph_index];
        assert_eq!(graph.states[0].structural_parameters.len(), 1);
        assert_eq!(graph.states[0].scalar_parameters.len(), 1);
        assert_eq!(
            graph.states[0].structural_parameters[0].access,
            CheckedStructuralAccess::MutableBorrow
        );
        for name in ["Calc::double_val", "Main::value"] {
            let lowered = checked_trees_to_lowered_psi::lower_machine(
                &checked,
                TerminalMachineSelection::Name(name),
            )
            .unwrap_or_else(|error| panic!("{name}, transition={transition}: {error:?}"));
            terminal_verifier::verify_module(
                &lowered.semantic_module,
                &lowered.proof_bundle,
                &proof_admission::AdmissionProfile::default(),
            )
            .expect("pure mutable receiver and caller independently verify");
        }
        for mutation in ["retained shared", "source shared"] {
            let mut changed = checked.clone();
            if mutation == "retained shared" {
                changed.facts.flow.terminal_scalar_graphs.machines[graph_index].states[0]
                    .structural_parameters[0]
                    .access = CheckedStructuralAccess::SharedBorrow;
            } else {
                let mut reference = changed
                    .type_reference_table
                    .type_reference(receiver_type)
                    .clone();
                let TypeReferenceNode::Reference { access, .. } = &mut reference else {
                    panic!("authored mutable receiver reference");
                };
                *access = language_semantics::ReferenceAccess::Shared;
                let reference = changed.typed.type_reference_table.insert(reference);
                changed
                    .typed
                    .state_parameters
                    .get_mut(receiver_handle)
                    .type_reference = reference;
            }
            let error = checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Main::value"),
            )
            .expect_err("retained receiver cannot disagree with its source");
            if mutation == "retained shared" {
                assert!(
                    matches!(
                        error,
                        checked_trees_to_lowered_psi::LoweringError::Unsupported(
                            "scalar parameter storage disagrees with its authored signature"
                        )
                    ),
                    "transition={transition}, mutation={mutation}: {error:?}"
                );
            }
        }
    }
}
