//! Constructor subjects remain real established owners through observation.

use super::*;
use checked_trees::CheckedScalarComputationKind;
use checked_trees::expression::ExpressionNode;

const SOURCE: &str = r#"
    data Choice { case Empty; case Some(first: bool, second: bool); }
    machine first(value: bool) -> bool { value }
    machine second(value: bool) -> bool { !value }
    machine choose(value: bool) -> bool {
        Choice::Some { second: second(value), first: first(value) } in Choice::Some
    }
"#;

#[test]
fn copy_case_return_requires_its_producer_to_dominate_the_return() {
    let checked = checked_source(
        "data Choice [copy] { case Empty; case Some(value: u32); }
         machine choose(selected: bool) -> Choice {
             match selected { true -> Choice::Some { value: 37 }, false -> Choice::Empty }
         }",
    );
    let artifact = produce_terminal_artifact(&checked, "choose").unwrap();
    let mut module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let machine = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let branch_local = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| operation.result.structural().map(|result| result.place))
        .unwrap();
    let returned = machine
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            Terminator::ReturnStructural { source, .. } => Some(source),
            _ => None,
        })
        .expect("joined result return");
    assert_ne!(*returned, branch_local);
    *returned = branch_local;
    assert!(
        terminal_verifier::verify_module(
            &module,
            &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err(),
        "a copy-case constructor in one arm cannot supply the other arm's return"
    );
}

#[test]
fn copy_local_case_membership_repeats_direct_and_selected_observations_without_cleanup() {
    let checked = checked_source(
        r#"
        data Choice [copy] { case Empty; case Some(value: u32); }
        machine direct() -> bool {
            let choice: Choice = Choice::Some { value: 37 };
            let seen: bool = choice in Choice::Some;
            seen == (choice in Choice::Some)
        }
        machine selected(value: bool) -> bool {
            let choice: Choice = match value {
                true -> Choice::Some { value: 38 },
                false -> Choice::Empty
            };
            let seen: bool = choice in Choice::Some;
            (seen == value) && (seen == (choice in Choice::Some))
        }
        "#,
    );
    let profile = proof_admission::AdmissionProfile::default();
    for name in ["direct", "selected"] {
        let artifact = produce_terminal_artifact(&checked, name)
            .expect("copy case locals retain no affine disposal debt");
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let root = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let mut constructions = 0;
        let observations = root
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| {
                if matches!(operation.kind, OperationKind::EstablishScalarCase { .. }) {
                    let OperationResult::Structural(result) = &operation.result else {
                        panic!("case establishment has a structural result");
                    };
                    assert_eq!(result.multiplicity, StructuralMultiplicity::Unrestricted);
                    constructions += 1;
                }
                match operation.kind {
                    OperationKind::StructuralCaseMembership { source, .. } => Some(source),
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(constructions, if name == "direct" { 1 } else { 2 });
        assert_eq!(observations.len(), 2);
        assert_eq!(observations[0], observations[1]);
        if name == "selected" {
            let branch_local = root
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|operation| match &operation.result {
                    OperationResult::Structural(result) => Some(result.place),
                    _ => None,
                })
                .expect("arm-local construction");
            let mut changed = module.clone();
            let root = changed
                .machines
                .iter_mut()
                .find(|machine| machine.id == changed.entry)
                .unwrap();
            let membership = root
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::StructuralCaseMembership { .. }
                    )
                })
                .unwrap();
            let OperationKind::StructuralCaseMembership { source, .. } = &mut membership.kind
            else {
                panic!("membership");
            };
            *source = branch_local;
            assert!(
                terminal_verifier::verify_module(
                    &changed,
                    &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
                    &profile,
                )
                .is_err(),
                "copyability does not make an arm-local producer dominate the join"
            );
        }
        for block in &root.blocks {
            match &block.terminator {
                Terminator::Return {
                    cleanup_actions, ..
                } => assert!(cleanup_actions.is_empty()),
                Terminator::Jump {
                    trivial_affine_discards,
                    residual_affine_discards,
                    ..
                } => {
                    assert!(trivial_affine_discards.is_empty());
                    assert!(residual_affine_discards.is_empty());
                }
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    assert!(when_true.trivial_affine_discards.is_empty());
                    assert!(when_false.trivial_affine_discards.is_empty());
                }
                _ => {}
            }
        }
        for selected in [false, true] {
            let arguments = if name == "direct" {
                Vec::new()
            } else {
                vec![terminal_interpreter::TerminalScalarValue::Boolean(selected)]
            };
            assert_eq!(
                terminal_interpreter::interpret_terminal_artifact(
                    artifact.semantic_bytes(),
                    artifact.proof_bytes(),
                    &profile,
                    &arguments,
                )
                .unwrap(),
                terminal_interpreter::TerminalExecutionResult::Scalar(
                    terminal_interpreter::TerminalScalarValue::Boolean(true)
                )
            );
        }
    }
}

#[test]
fn selected_local_case_membership_observes_the_joined_owner_and_rejects_foreign_payloads() {
    let checked = checked_source(
        r#"
        data Choice { case Empty; case Some(value: bool); }
        data Other { case Empty; case Some(value: bool); }
        machine identity(value: bool) -> bool { value }
        machine choose(selected: bool, value: bool) -> bool {
            let choice: Choice = match selected {
                true -> Choice::Some { value: identity(value) },
                false -> Choice::Empty
            };
            choice in Choice::Some
        }
    "#,
    );
    let artifact =
        produce_terminal_artifact(&checked, "choose").expect("selected local membership");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile).unwrap();
    let root = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let observed = root
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::StructuralCaseMembership { source, .. } => Some(source),
            _ => None,
        })
        .unwrap();
    assert!(
        root.blocks.iter().any(|block| block
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == observed)),
        "membership observes the selected result join, not either constructor"
    );
    for selected in [false, true] {
        assert_eq!(
            terminal_interpreter::interpret_terminal_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &profile,
                &[
                    terminal_interpreter::TerminalScalarValue::Boolean(selected),
                    terminal_interpreter::TerminalScalarValue::Boolean(false)
                ],
            )
            .unwrap(),
            terminal_interpreter::TerminalExecutionResult::Scalar(
                terminal_interpreter::TerminalScalarValue::Boolean(selected)
            )
        );
    }
    let foreign = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Other")
        .unwrap();
    let foreign_case = checked
        .data_members(foreign)
        .iter()
        .find_map(|member| match member {
            checked_trees::data::DataMember::Variant(case)
                if !checked.data_payload_fields(case).is_empty() =>
            {
                Some(case)
            }
            _ => None,
        })
        .unwrap();
    let (handle, fields) = checked
        .facts
        .values
        .structural_values
        .nodes
        .iter()
        .find_map(|(handle, node)| match &node.kind {
            checked_trees::CheckedStructuralValueKind::Case(construction)
                if !construction.fields.is_empty() =>
            {
                Some((handle, construction.fields))
            }
            _ => None,
        })
        .unwrap();
    for replace_case in [false, true] {
        let mut changed = checked.clone();
        if replace_case {
            let checked_trees::CheckedStructuralValueKind::Case(construction) = &mut changed
                .facts
                .values
                .structural_values
                .nodes
                .get_mut(handle)
                .kind
            else {
                panic!("case");
            };
            construction.case = foreign_case.symbol;
        } else {
            changed
                .facts
                .values
                .scalar_computations
                .case_fields
                .span_mut(fields)
                .unwrap()[0]
                .symbol = checked.data_payload_fields(foreign_case)[0].symbol;
        }
        assert!(
            lower_machine(&changed, "choose").is_err(),
            "same-shaped foreign payload or case cannot authorize a selected leaf"
        );
    }
}

#[test]
fn local_case_membership_rejoins_establishment_and_exit_provenance() {
    let checked = checked_source(
        "data Choice { case Empty; }
         machine choose() -> bool {
             let choice: Choice = Choice::Empty;
             choice in Choice::Empty
         }",
    );
    lower_machine(&checked, "choose").expect("exact local lifetime");
    let events = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter_map(|(handle, event)| {
            matches!(event.root, facts::PlaceRoot::Symbol(_)).then_some(handle)
        })
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 2, "one establishment and one exit drop");
    for handle in events {
        for omit in [false, true] {
            let mut changed = checked.clone();
            let event = changed.facts.flow.ownership.permissions.get_mut(handle);
            if omit {
                event.machine_symbol = symbols::SymbolHandle::invalid();
            } else {
                event.provenance = language_semantics::PermissionProvenance::Unknown;
            }
            assert!(
                lower_machine(&changed, "choose").is_err(),
                "local lifetime cannot omit a permission or replace its establishment provenance"
            );
        }
    }
}

#[test]
fn local_case_membership_reuses_one_affine_owner_through_repeated_observations() {
    let checked = checked_source(
        r#"
        data Choice { case Empty; case Some(first: bool, second: bool); }
        machine first(value: bool) -> bool { value }
        machine second(value: bool) -> bool { !value }
        machine choose(value: bool) -> bool {
            let choice: Choice = Choice::Some { second: second(value), first: first(value) };
            let seen: bool = choice in Choice::Some;
            seen && (choice in Choice::Some)
        }
    "#,
    );
    let artifact = produce_terminal_artifact(&checked, "choose").expect("ordinary local case");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile).unwrap();
    let root = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let constructions = root
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| {
            if !matches!(operation.kind, OperationKind::EstablishScalarCase { .. }) {
                return None;
            }
            let OperationResult::Structural(result) = &operation.result else {
                panic!("structural construction");
            };
            assert_eq!(result.multiplicity, StructuralMultiplicity::Affine);
            Some(result.place)
        })
        .collect::<Vec<_>>();
    assert_eq!(constructions.len(), 1);
    let observations = root
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            OperationKind::StructuralCaseMembership { source, .. } => Some(source),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(observations, vec![constructions[0]; 2]);
    for value in [false, true] {
        assert_eq!(
            terminal_interpreter::interpret_terminal_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &profile,
                &[terminal_interpreter::TerminalScalarValue::Boolean(value)],
            )
            .unwrap(),
            terminal_interpreter::TerminalExecutionResult::Scalar(
                terminal_interpreter::TerminalScalarValue::Boolean(true),
            )
        );
    }
}

#[test]
fn local_case_membership_rejects_payload_and_same_typed_source_substitution() {
    let checked = checked_source(
        r#"
        data Choice { case Empty; case Some(value: u32); }
        machine choose() -> bool {
            let choice: Choice = Choice::Some { value: 37 };
            let other: Choice = Choice::Some { value: 38 };
            choice in Choice::Some
        }
    "#,
    );
    lower_machine(&checked, "choose").expect("two independent local cases");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| {
            checked
                .machines()
                .iter()
                .any(|machine| machine.symbol == plan.machine && machine.name.as_str() == "choose")
        })
        .unwrap();
    let state = plan.state;
    let fields = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } => {
                let checked_trees::CheckedStructuralValueKind::Case(construction) = &checked
                    .facts
                    .values
                    .structural_values
                    .nodes
                    .get(*value)
                    .kind
                else {
                    panic!("retained local constructor");
                };
                Some(
                    crate::scalar_computations::cases::fields(&checked, construction).unwrap()[0]
                        .value,
                )
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(fields.len(), 2);
    for mutate_source in [false, true] {
        let mut changed = checked.clone();
        let nodes = &checked.facts.values.scalar_computations.nodes;
        if mutate_source {
            let source = nodes.get(fields[0]).authored_root;
            let donor = nodes.get(fields[1]).authored_root;
            *changed.typed.expression_table.expression_mut(source) =
                checked.expression_table.expression(donor).clone();
        } else {
            changed
                .facts
                .values
                .scalar_computations
                .nodes
                .get_mut(fields[0])
                .kind = nodes.get(fields[1]).kind.clone();
        }
        assert!(
            lower_machine(&changed, "choose").is_err(),
            "same-type local payload tampering must reject"
        );
    }
    let (_, source_state) = crate::scalar_source_custody::authored_state(&checked, state).unwrap();
    let other = checked
        .statement_table
        .statements(source_state.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            checked_trees::statement::StatementNode::LocalData(local) => Some(local.symbol),
            _ => None,
        })
        .nth(1)
        .unwrap();
    let mut changed = checked.clone();
    let handle = changed
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .find_map(|(handle, node)| {
            matches!(
                node.kind,
                CheckedScalarComputationKind::CaseMembership {
                    subject: checked_trees::CheckedScalarComputationStructuralArgument::Place(_),
                    ..
                }
            )
            .then_some(handle)
        })
        .unwrap();
    let CheckedScalarComputationKind::CaseMembership {
        subject: checked_trees::CheckedScalarComputationStructuralArgument::Place(argument),
        ..
    } = &mut changed
        .facts
        .values
        .scalar_computations
        .nodes
        .get_mut(handle)
        .kind
    else {
        panic!("local observation");
    };
    argument.source =
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: other };
    assert!(
        lower_machine(&changed, "choose").is_err(),
        "same-typed established local cannot replace the authored subject"
    );
    let drop_handle = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .find_map(|(handle, event)| {
            (event.state_symbol == state
                && event.source == language_semantics::PermissionEventSource::StateExit
                && event.kind == language_semantics::PermissionEventKind::AffineDrop
                && event.root == facts::PlaceRoot::Symbol(other))
            .then_some(handle)
        })
        .expect("local exit disposition");
    let mut changed = checked.clone();
    changed
        .facts
        .flow
        .ownership
        .permissions
        .get_mut(drop_handle)
        .access = language_semantics::PermissionAccess::Shared;
    assert!(
        lower_machine(&changed, "choose").is_err(),
        "local cleanup requires exact owned source disposition"
    );

    // Pure arguments use the same correspondence owner even while this
    // producer currently retains local fields as computation roots.
    let nodes = &checked.facts.values.scalar_computations.nodes;
    let original = nodes.get(fields[0]);
    let CheckedScalarComputationKind::Value(value) = &original.kind else {
        panic!("literal field");
    };
    crate::scalar_source_custody::value_correspondence::validate(
        &checked,
        state,
        0,
        original.authored_root,
        original.primitive_type,
        &checked_trees::CheckedCallScalarArgument::Pure(value.clone()),
    )
    .expect("pure field source correspondence");
    let CheckedScalarComputationKind::Value(donor) = &nodes.get(fields[1]).kind else {
        panic!("donor literal");
    };
    assert!(
        crate::scalar_source_custody::value_correspondence::validate(
            &checked,
            state,
            0,
            original.authored_root,
            original.primitive_type,
            &checked_trees::CheckedCallScalarArgument::Pure(donor.clone()),
        )
        .is_err(),
        "pure field cannot substitute a same-typed literal"
    );
}

fn field_computation(
    checked: &CheckedTrees,
    machine_name: &str,
) -> checked_trees::CheckedScalarComputationHandle {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .unwrap();
    let plans = &checked.facts.values.scalar_computations;
    let root = plans
        .roots
        .iter()
        .find_map(|(_, root)| (root.machine == machine.symbol).then_some(root.root))
        .unwrap();
    let CheckedScalarComputationKind::CaseMembership {
        subject: checked_trees::CheckedScalarComputationStructuralArgument::Case(subject),
        ..
    } = &plans.nodes.get(root).kind
    else {
        panic!("constructor membership root");
    };
    let fields = plans.case_fields.span(subject.fields).unwrap();
    assert_eq!(fields.len(), 1);
    fields[0].value
}

#[test]
fn constructor_membership_rejects_changed_literal_payload_in_either_source_or_plan() {
    let checked = checked_source(
        r#"
        data Choice { case Empty; case Some(value: u32); }
        machine choose() -> bool { Choice::Some { value: 37 } in Choice::Some }
        machine donor() -> bool { Choice::Some { value: 38 } in Choice::Some }
    "#,
    );
    lower_machine(&checked, "choose").expect("original literal construction");
    let field = field_computation(&checked, "choose");
    let donor = field_computation(&checked, "donor");
    let original = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .get(field)
        .clone();
    let replacement = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .get(donor)
        .clone();
    assert!(matches!(
        original.kind,
        CheckedScalarComputationKind::Value(
            checked_trees::CheckedScalarExpression::IntegerLiteral { .. }
        )
    ));
    assert_eq!(original.primitive_type, replacement.primitive_type);

    let mut changed_plan = checked.clone();
    changed_plan
        .facts
        .values
        .scalar_computations
        .nodes
        .get_mut(field)
        .kind = replacement.kind;
    assert!(
        lower_machine(&changed_plan, "choose").is_err(),
        "37 -> 38 cannot change the retained payload while keeping its source, type and case"
    );

    let mut changed_source = checked.clone();
    *changed_source
        .typed
        .expression_table
        .expression_mut(original.authored_root) = checked
        .expression_table
        .expression(replacement.authored_root)
        .clone();
    assert!(
        lower_machine(&changed_source, "choose").is_err(),
        "38 in source cannot keep the old 37 payload merely because membership is unchanged"
    );
}

#[test]
fn constructor_membership_rejects_erased_field_operator_meaning() {
    let checked = checked_source(
        r#"
        data Choice { case Empty; case Some(value: bool); }
        machine choose(value: bool) -> bool { Choice::Some { value: !value } in Choice::Some }
    "#,
    );
    lower_machine(&checked, "choose").expect("original field negation");
    let field = field_computation(&checked, "choose");
    let mut changed = checked.clone();
    match &mut changed
        .facts
        .values
        .scalar_computations
        .nodes
        .get_mut(field)
        .kind
    {
        CheckedScalarComputationKind::Value(checked_trees::CheckedScalarExpression::Boolean(
            value,
        )) => {
            let checked_trees::CheckedBooleanExpression::Not(operand) = value.as_ref() else {
                panic!("retained pure negation");
            };
            *value = operand.clone();
        }
        CheckedScalarComputationKind::Apply { expression, .. } => {
            *expression = checked_trees::CheckedScalarExpression::Parameter {
                position: 0,
                primitive_type: PrimitiveType::Bool,
            };
        }
        unexpected => panic!("unexpected negation computation: {unexpected:?}"),
    }
    assert!(
        lower_machine(&changed, "choose").is_err(),
        "matching reads cannot justify deleting a field's negation"
    );
}

#[test]
fn constructor_membership_stages_fields_before_observation_and_affine_cleanup() {
    let checked = checked_source(SOURCE);
    let lowered = lower_machine(&checked, "choose").expect("dynamic constructor membership");
    let module = &lowered.semantic_module;
    let root = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let mut current = root.entry;
    let mut visited = Vec::new();
    let mut calls = Vec::new();
    let mut established = None;
    let mut observed = false;
    let mut discarded = false;
    loop {
        assert!(!visited.contains(&current), "fixture is acyclic");
        visited.push(current);
        let block = root
            .blocks
            .iter()
            .find(|block| block.id == current)
            .unwrap();
        for operation in &block.operations {
            if let Some(call) = lowered
                .source_call_occurrences
                .iter()
                .find(|call| call.terminal_operation == operation.id)
            {
                assert!(established.is_none(), "fields finish before construction");
                calls.push(call.source_target);
            }
            match &operation.kind {
                OperationKind::EstablishScalarCase { fields, .. } => {
                    assert!(established.is_none(), "constructor executes once");
                    let OperationResult::Structural(result) = &operation.result else {
                        panic!("case has a structural result");
                    };
                    assert_eq!(result.multiplicity, StructuralMultiplicity::Affine);
                    assert_eq!(fields.len(), 2);
                    assert!(
                        fields[0].field < fields[1].field,
                        "published field order is canonical"
                    );
                    established = Some(result.place);
                }
                OperationKind::StructuralCaseMembership { source, .. } => {
                    assert_eq!(Some(*source), established);
                    assert!(!observed);
                    assert!(!discarded);
                    observed = true;
                }
                _ => {}
            }
        }
        match &block.terminator {
            Terminator::Jump {
                target,
                trivial_affine_discards,
                ..
            } => {
                if !trivial_affine_discards.is_empty() {
                    assert!(observed, "cleanup follows the observation");
                    assert!(!discarded, "temporary is discarded once");
                    assert_eq!(trivial_affine_discards.as_slice(), &[established.unwrap()]);
                    discarded = true;
                }
                current = *target;
            }
            Terminator::Return { .. } => break,
            unexpected => panic!("unexpected straight-line fixture edge: {unexpected:?}"),
        }
    }
    let targets = ["second", "first"].map(|name| {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap();
        // Direct scalar emission records the target machine, not its entry state.
        machine.symbol
    });
    assert_eq!(
        calls, targets,
        "source order survives canonical field publication"
    );
    assert!(observed && discarded);
}

#[test]
fn selected_constructor_membership_closes_its_affine_frontier_before_the_join() {
    let checked = checked_source(
        r#"
        data Choice { case Empty; case Some(value: bool); }
        machine identity(value: bool) -> bool { value }
        machine choose(gate: bool) -> bool {
            gate && (Choice::Some { value: identity(gate) } in Choice::Some)
        }
    "#,
    );
    let artifact =
        produce_terminal_artifact(&checked, "choose").expect("selected case construction");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile).unwrap();
    for gate in [false, true] {
        assert_eq!(
            terminal_interpreter::interpret_terminal_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &profile,
                &[terminal_interpreter::TerminalScalarValue::Boolean(gate)],
            )
            .unwrap(),
            terminal_interpreter::TerminalExecutionResult::Scalar(
                terminal_interpreter::TerminalScalarValue::Boolean(gate),
            ),
        );
    }
    let mut missing_cleanup = module.clone();
    let root = missing_cleanup
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let cleanup = root
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator {
            Terminator::Jump {
                trivial_affine_discards,
                ..
            } if !trivial_affine_discards.is_empty() => Some(trivial_affine_discards),
            _ => None,
        })
        .expect("selected temporary has real edge cleanup");
    cleanup.clear();
    assert!(
        terminal_verifier::verify_module(&missing_cleanup, &proof, &profile).is_err(),
        "deleting cleanup cannot leave an affine owner at the join"
    );
}

#[test]
fn constructor_membership_rejoins_field_roster_roots_types_and_authored_source() {
    let checked = checked_source(SOURCE);
    lower_machine(&checked, "choose").expect("original source custody");
    let (handle, subject) = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .find_map(|(handle, node)| match &node.kind {
            CheckedScalarComputationKind::CaseMembership {
                subject: checked_trees::CheckedScalarComputationStructuralArgument::Case(subject),
                ..
            } => Some((handle, subject.clone())),
            _ => None,
        })
        .unwrap();
    let original = checked
        .facts
        .values
        .scalar_computations
        .case_fields
        .span(subject.fields)
        .unwrap()
        .to_vec();
    for mutation in 0..5 {
        let mut changed = checked.clone();
        match mutation {
            0 => {
                let fields = changed
                    .facts
                    .values
                    .scalar_computations
                    .case_fields
                    .span_mut(subject.fields)
                    .unwrap();
                fields[0].value = original[1].value;
                fields[1].value = original[0].value;
            }
            1 => {
                changed
                    .facts
                    .values
                    .scalar_computations
                    .case_fields
                    .span_mut(subject.fields)
                    .unwrap()[0]
                    .symbol = original[1].symbol;
            }
            2 => {
                changed
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get_mut(original[0].value)
                    .primitive_type = PrimitiveType::U32;
            }
            3 => {
                let expression = changed
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get(original[0].value)
                    .authored_root;
                *changed.typed.expression_table.expression_mut(expression) =
                    ExpressionNode::Boolean(false);
            }
            _ => {
                let CheckedScalarComputationKind::CaseMembership {
                    subject:
                        checked_trees::CheckedScalarComputationStructuralArgument::Case(subject),
                    ..
                } = &mut changed
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get_mut(handle)
                    .kind
                else {
                    panic!("retained membership");
                };
                subject.fields = arena::HandleSpan::from_parts(subject.fields.start(), 1);
            }
        }
        assert!(
            lower_machine(&changed, "choose").is_err(),
            "source/plan mutation {mutation} must reject"
        );
    }
}

#[test]
fn constructor_membership_rejects_a_same_spelled_foreign_observation_case() {
    let checked = checked_source(&format!(
        "{SOURCE}
        data Other {{ case Empty; case Some(first: bool, second: bool); }}
    "
    ));
    lower_machine(&checked, "choose").expect("original nominal owner");
    let owner = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Other")
        .unwrap();
    let foreign = checked
        .data_members(owner)
        .iter()
        .find_map(|member| match member {
            checked_trees::data::DataMember::Variant(case) if case.name.as_str() == "Some" => {
                Some(case.symbol)
            }
            _ => None,
        })
        .unwrap();
    let mut changed = checked.clone();
    let handle = changed
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .find_map(|(handle, node)| {
            matches!(
                node.kind,
                CheckedScalarComputationKind::CaseMembership { .. }
            )
            .then_some(handle)
        })
        .unwrap();
    let CheckedScalarComputationKind::CaseMembership { case, .. } = &mut changed
        .facts
        .values
        .scalar_computations
        .nodes
        .get_mut(handle)
        .kind
    else {
        panic!("retained membership");
    };
    *case = foreign;
    assert!(lower_machine(&changed, "choose").is_err());
}
