use super::{
    CheckedScalarComputationKind, CheckedScalarExpressionRole, ExpressionNode, StatementNode,
};
use crate::values::scalar::computations::tests::checked_source;
use checked_trees::{CheckedBooleanExpression, CheckedScalarExpression};

#[test]
fn local_record_case_observation_retains_its_projected_place() {
    let checked = checked_source(
        "data Item [copy] { case Empty; case Ready; }
         data Reward { item: Item; sibling: u64; }
         machine inspect() -> bool {
             let mut reward: Reward;
             reward.item = Item::Ready;
             reward.item == Item::Ready
         }",
        false,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "inspect")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let plans = &checked.facts.values.scalar_computations;
    let root = plans
        .root_at(state.symbol, 2, CheckedScalarExpressionRole::Return)
        .expect("a case stored inside a local record has an executable observation");
    let CheckedScalarComputationKind::Apply { operands, .. } = &plans.nodes.get(root.root).kind
    else {
        panic!("ordinary enum equality retains both operands");
    };
    let observations = plans.operands.span(*operands).unwrap();
    assert_eq!(observations.len(), 4);
    let CheckedScalarComputationKind::CaseMembership { subject, .. } =
        &plans.nodes.get(observations[0]).kind
    else {
        panic!("retain a case observation, not a folded construction");
    };
    let checked_trees::CheckedScalarComputationStructuralArgument::Place(subject) = subject else {
        panic!("the local is observed in place");
    };
    assert_eq!(subject.path.len(), 1);
    assert_eq!(
        subject.access,
        checked_trees::CheckedStructuralAccess::SharedBorrow
    );
    assert!(matches!(
        &plans.nodes.get(observations[2]).kind,
        CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(value))
            if **value == CheckedBooleanExpression::Constant(false)
    ));
}

#[test]
fn enum_equality_observes_complete_left_then_right_case_rosters() {
    for operator in ["==", "!="] {
        let checked = checked_source(
            &format!(
                "data Choice [copy] {{ case Empty; case First; case Second; }}
             machine same(left: Choice, right: Choice) -> bool {{ left {operator} right }}"
            ),
            false,
        );
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "same")
            .unwrap();
        let state = &checked.machine_states(machine)[0];
        let plans = &checked.facts.values.scalar_computations;
        let root = plans
            .root_at(state.symbol, 0, CheckedScalarExpressionRole::Return)
            .expect("executable enum equality");
        let CheckedScalarComputationKind::Apply {
            source_expression,
            expression,
            operands,
        } = &plans.nodes.get(root.root).kind
        else {
            panic!("equality must sequence observations before combining them");
        };
        assert_eq!(
            matches!(expression, checked_trees::CheckedScalarExpression::Boolean(value) if matches!(value.as_ref(), checked_trees::CheckedBooleanExpression::Not(_))),
            operator == "!="
        );
        let owner = checked
            .data_definitions()
            .iter()
            .find(|owner| owner.name.as_str() == "Choice")
            .unwrap();
        let cases: Vec<_> = checked
            .data_members(owner)
            .iter()
            .filter_map(|member| match member {
                checked_trees::data::DataMember::Variant(variant) => Some(variant.symbol),
                _ => None,
            })
            .collect();
        let observations = plans.operands.span(*operands).unwrap();
        assert_eq!(observations.len(), cases.len() * 2);
        for (ordinal, observation) in observations.iter().enumerate() {
            let CheckedScalarComputationKind::CaseMembership {
                source_expression: source,
                subject: checked_trees::CheckedScalarComputationStructuralArgument::Place(subject),
                case,
            } = &plans.nodes.get(*observation).kind
            else {
                panic!("one tag observation per declared case");
            };
            assert_eq!(source, source_expression);
            assert_eq!(*case, cases[ordinal % cases.len()]);
            assert_eq!(
                subject.source,
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol: checked.state_parameters(state)[ordinal / cases.len()].symbol
                }
            );
            assert!(subject.path.is_empty());
        }
    }
}

#[test]
fn enum_equality_retains_indexed_subject_and_formal_identity_after_receiver() {
    let checked = checked_source(
        "data Choice [copy] { case Empty; case First; }
         data Room [copy] { cell: Choice; }
         data Lookup {}
         machine Lookup::same(&mut self, rooms: &[Room], cell: Choice, index: u64) -> bool
         requires index < rooms.len
         { rooms[index].cell == cell }",
        false,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Lookup::same")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let parameters = checked.state_parameters(state);
    let plans = &checked.facts.values.scalar_computations;
    let root = plans
        .root_at(state.symbol, 0, CheckedScalarExpressionRole::Return)
        .expect("indexed enum comparison");
    let CheckedScalarComputationKind::Apply { operands, .. } = &plans.nodes.get(root.root).kind
    else {
        panic!("comparison application");
    };
    let observations = plans.operands.span(*operands).unwrap();
    assert_eq!(observations.len(), 4);
    for (ordinal, observation) in observations.iter().enumerate() {
        let CheckedScalarComputationKind::CaseMembership {
            subject: checked_trees::CheckedScalarComputationStructuralArgument::Place(subject),
            ..
        } = &plans.nodes.get(*observation).kind
        else {
            panic!("tag observation");
        };
        assert_eq!(
            subject.source,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                symbol: parameters[if ordinal < 2 { 1 } else { 2 }].symbol,
            }
        );
        if ordinal < 2 {
            assert!(matches!(
                subject.path.as_slice(),
                [
                    checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                        checked_trees::CheckedRuntimeIndex::Parameter { position: 0 }
                    ),
                    checked_trees::CheckedUnitStructuralPathSegment::Field(_)
                ]
            ));
        } else {
            assert!(subject.path.is_empty());
        }
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
