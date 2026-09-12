use super::*;

#[test]
fn selected_case_fields_keep_distinct_occurrences_in_the_shared_value_plan() {
    let checked = checked_source(
        "data Choice { case Empty; case Some(value: u32); }
         machine identity(value: u32) -> u32 { value }
         machine inspect(selector: u32) -> bool {
             let choice: Choice = match selector {
                 0 -> Choice::Some { value: identity(17) },
                 1 -> Choice::Some { value: identity(37) },
                 _ -> Choice::Empty
             };
             choice in Choice::Some
         }",
        false,
    );
    let values = &checked.facts.values;
    let fields = values
        .scalar_computations
        .roots
        .iter()
        .filter_map(|(_, root)| {
            let CheckedScalarExpressionRole::StructuralValueField {
                expression,
                field_ordinal,
            } = root.role
            else {
                return None;
            };
            Some((expression, field_ordinal, root.root))
        })
        .collect::<Vec<_>>();
    assert_eq!(fields.len(), 2);
    assert_ne!(fields[0].0, fields[1].0);
    assert_eq!(fields[0].1, 0);
    assert_eq!(fields[1].1, 0);
    assert_ne!(fields[0].2, fields[1].2);
    for (expression, _, computation) in fields {
        let constructor = validation::scalar_case_constructor(&checked.typed, expression).unwrap();
        assert_eq!(
            values
                .scalar_computations
                .nodes
                .get(computation)
                .authored_root,
            constructor.fields[0].1
        );
    }
}

#[test]
fn scalar_match_does_not_leave_structural_operand_roots() {
    let checked = checked_source(
        "machine choose(selector: bool) -> u32 { match selector { true -> 17, false -> 37 } }",
        false,
    );
    assert_eq!(
        checked.facts.values.structural_values.roots.iter().count(),
        0
    );
    assert!(
        !checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .any(|(_, root)| matches!(
                root.role,
                CheckedScalarExpressionRole::StructuralValueField { .. }
                    | CheckedScalarExpressionRole::StructuralValueSubject { .. }
                    | CheckedScalarExpressionRole::StructuralValuePattern { .. }
            ))
    );
}

#[test]
fn local_case_construction_has_one_structural_result_and_observation_identity() {
    for constructor in ["Choice::Empty", "Choice::Some { value: identity(value) }"] {
        let checked = checked_source(
            &format!(
                "data Choice {{ case Empty; case Some(value: u32); }}
             machine identity(value: u32) -> u32 {{ value }}
             machine inspect(value: u32) -> bool {{
                 let choice: Choice = {constructor};
                 choice in Choice::Empty
             }}"
            ),
            false,
        );
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "inspect")
            .unwrap();
        let state = &checked.machine_states(machine)[0];
        let StatementNode::LocalData(local) =
            &checked.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("local");
        };
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter()
            .find(|plan| plan.machine == machine.symbol)
            .expect("ordinary statement sequence retains local case");
        let constructors = plan
            .operations
            .iter()
            .filter_map(|operation| match operation {
                checked_trees::CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    result,
                    value,
                    ..
                } => Some((result, value)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(result, value)] = constructors.as_slice() else {
            panic!("one local constructor");
        };
        let checked_trees::CheckedStructuralValueKind::Case(case) = &checked
            .facts
            .values
            .structural_values
            .nodes
            .get(**value)
            .kind
        else {
            panic!("case construction");
        };
        let fields = checked
            .facts
            .values
            .scalar_computations
            .case_fields
            .span(case.fields)
            .unwrap();
        assert_eq!(result.statement_index, 0);
        assert_eq!(result.binding_ordinal, 0);
        assert_eq!(
            result.multiplicity,
            language_semantics::Multiplicity::Affine
        );
        assert_eq!(fields.len(), usize::from(constructor.contains("Some")));
        assert!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .structural_types
                .iter()
                .any(|shape| shape.identity == result.type_identity)
        );
        let plans = &checked.facts.values.scalar_computations;
        let root = plans
            .root_at(state.symbol, 1, CheckedScalarExpressionRole::Return)
            .expect("local observation");
        let CheckedScalarComputationKind::CaseMembership {
            subject: checked_trees::CheckedScalarComputationStructuralArgument::Place(subject),
            ..
        } = &plans.nodes.get(root.root).kind
        else {
            panic!("observation names storage");
        };
        assert_eq!(
            subject.source,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                symbol: local.symbol
            }
        );
        assert_eq!(
            subject.access,
            checked_trees::CheckedStructuralAccess::SharedBorrow
        );
    }
}

#[test]
fn case_constructors_retain_dynamic_fields_in_authored_order() {
    let checked = checked_source(
        "data Choice { case Empty; case Pair(first: u32, second: u32); }
         machine first(value: u32) -> u32 { value }
         machine second(value: u32) -> u32 { value }
         machine inspect(value: u32) -> bool {
             Choice::Pair { second: second(value), first: first(value) } in Choice::Pair
         }",
        false,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let StatementNode::Expression(expression) =
        checked.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("return");
    };
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(expression) else {
        panic!("membership");
    };
    assert!(
        validation::has_exact_case_membership_meaning(
            &checked.typed,
            machine,
            Some(state),
            expression,
            binary
        ),
        "exact membership: {binary:?}"
    );
    assert!(
        validation::scalar_case_constructor(&checked.typed, binary.left).is_some(),
        "constructor: {:?}; owner reference: {:?}",
        checked.expression_table.expression(binary.left),
        checked.type_reference_table.find_named_type_reference(
            checked
                .data_definitions()
                .iter()
                .find(|owner| owner.name.as_str() == "Choice")
                .unwrap()
                .symbol
        )
    );
    let plans = &checked.facts.values.scalar_computations;
    let root = plans
        .root_at(state.symbol, 0, CheckedScalarExpressionRole::Return)
        .expect("constructed membership is executable");
    let CheckedScalarComputationKind::CaseMembership {
        subject: checked_trees::CheckedScalarComputationStructuralArgument::Case(subject),
        case,
        ..
    } = &plans.nodes.get(root.root).kind
    else {
        panic!("membership keeps its actual constructed operand");
    };
    assert_eq!(subject.case, *case);
    let fields = plans.case_fields.span(subject.fields).unwrap();
    assert_eq!(fields.len(), 2);
    for (field, name) in fields.iter().zip(["second", "first"]) {
        assert_eq!(checked.symbols.name(field.symbol), name);
        let CheckedScalarComputationKind::Call { target_machine, .. } =
            plans.nodes.get(field.value).kind
        else {
            panic!("dynamic constructor field is not erased");
        };
        assert_eq!(checked.symbols.name(target_machine), name);
    }
    let graph = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .machines
        .iter()
        .find(|graph| graph.machine == machine.symbol)
        .expect("ordinary scalar graph");
    assert!(
        graph.states[0].structural_parameters.is_empty(),
        "constructor is not a fabricated parameter"
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .structural_types
            .iter()
            .any(|shape| shape.identity
                == checked
                    .normalized_type_identity(subject.type_reference)
                    .as_str())
    );
}

#[test]
fn constructor_classifier_rejects_a_same_shaped_foreign_owner() {
    let mut checked = checked_source(
        "data Choice { case Some(value: u32); }
         data Other { case Some(value: u32); }
         machine inspect() -> bool { Choice::Some { value: 37 } in Choice::Some }",
        false,
    );
    let expression = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .find_map(|(_, node)| match &node.kind {
            CheckedScalarComputationKind::CaseMembership {
                subject: checked_trees::CheckedScalarComputationStructuralArgument::Case(subject),
                ..
            } => Some(subject.expression),
            _ => None,
        })
        .expect("constructor occurrence");
    assert!(validation::scalar_case_constructor(&checked.typed, expression).is_some());
    let foreign = checked
        .data_definitions()
        .iter()
        .find(|owner| owner.name.as_str() == "Other")
        .unwrap()
        .symbol;
    let ExpressionNode::StructLiteral(literal) =
        checked.typed.expression_table.expression_mut(expression)
    else {
        panic!("case literal");
    };
    literal.type_symbol = foreign;
    assert!(validation::scalar_case_constructor(&checked.typed, expression).is_none());
}
