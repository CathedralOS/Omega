//! Replay an intrinsic enum equality as one complete observation transaction.
//! Its generated memberships are not independently authored Boolean values:
//! only the exact ordered roster and its equivalence template denote equality.
use super::{authored_state, case_sources};
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::data::{DataDefinition, DataMember, DataShapeKind};
use checked_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use checked_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use checked_trees::{
    CheckedBooleanExpression as Boolean, CheckedScalarComputationHandle,
    CheckedScalarComputationKind as Computation, CheckedScalarComputationStructuralArgument,
    CheckedScalarExpression, CheckedTrees, CheckedUnitStructuralArgumentSourcePlan,
};

/// `false` leaves an ordinary application to its existing receiver. An enum
/// application either rejoins every operand and the template or fails closed.
pub(crate) fn validate_application(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    root: CheckedScalarComputationHandle,
) -> Result<bool, LoweringError> {
    let plans = &checked.facts.values.scalar_computations;
    if !plans.nodes.is_valid(root) {
        return unsupported("enum equality has a stale computation");
    }
    let node = plans.nodes.get(root);
    let Computation::Apply {
        source_expression,
        expression,
        operands,
    } = &node.kind
    else {
        return Ok(false);
    };
    if !checked
        .expression_table
        .expression_is_valid(*source_expression)
    {
        return unsupported("enum equality has a stale source occurrence");
    }
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(*source_expression)
    else {
        return Ok(false);
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) {
        return Ok(false);
    }
    let (machine, source_state) = authored_state(checked, state)?;
    let source_type = |source| {
        validation::declared_place_type_raw(&checked.typed, machine, Some(source_state), source)
            .or_else(|| {
                validation::scalar_case_constructor(&checked.typed, source)
                    .map(|constructor| constructor.type_reference)
            })
    };
    let Some(owner) = source_type(binary.left).and_then(|reference| enum_owner(checked, reference))
    else {
        return Ok(false);
    };
    let Some(right_owner) =
        source_type(binary.right).and_then(|reference| enum_owner(checked, reference))
    else {
        return unsupported("enum equality changed its right nominal carrier");
    };
    if owner.symbol != right_owner.symbol
        || node.primitive_type != PrimitiveType::Bool
        || !selected_builtin(checked, *source_expression, binary.operator)
        || !validation::has_builtin_binary_expression_meaning(
            &checked.typed,
            machine,
            Some(source_state),
            *source_expression,
        )
    {
        return unsupported("enum equality changed its carrier or selected meaning");
    }
    let cases = checked.data_members(owner);
    if cases.is_empty()
        || cases.iter().any(|member| {
            !matches!(member, DataMember::Variant(variant)
            if variant.symbol.is_valid() && checked.data_payload_fields(variant).is_empty())
        })
    {
        return unsupported("enum equality requires a complete payloadless case roster");
    }
    let operands = plans
        .operands
        .span(*operands)
        .ok_or(LoweringError::Unsupported(
            "enum equality has a stale operand roster",
        ))?;
    if cases.len().checked_mul(2) != Some(operands.len()) {
        return unsupported("enum equality omitted or duplicated an observation");
    }
    let mut observed_type = None;
    for (position, operand) in operands.iter().enumerate() {
        if !plans.nodes.is_valid(*operand) || operands[..position].contains(operand) {
            return unsupported("enum equality has a stale or reused observation");
        }
        let observed = plans.nodes.get(*operand);
        let DataMember::Variant(expected) = &cases[position % cases.len()] else {
            return unsupported("enum equality lost its case declaration");
        };
        if observed.primitive_type != PrimitiveType::Bool {
            return unsupported("enum equality has a non-Boolean observation");
        }
        let source = if position < cases.len() {
            binary.left
        } else {
            binary.right
        };
        let reference = match &observed.kind {
            Computation::CaseMembership {
                source_expression: occurrence,
                subject: CheckedScalarComputationStructuralArgument::Place(subject),
                case,
            } if matches!(
                subject.source,
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
            ) =>
            {
                if occurrence != source_expression || *case != expected.symbol {
                    return unsupported(
                        "enum equality changed its observation occurrence or order",
                    );
                }
                if !validation::place_has_builtin_coordinates(
                    &checked.typed,
                    machine,
                    Some(source_state),
                    source,
                ) {
                    return unsupported("enum equality requires an established builtin place");
                }
                case_sources::named_place(checked, machine, source_state, source, subject)?
            }
            Computation::Value(CheckedScalarExpression::Boolean(value)) => {
                let constructor = validation::scalar_case_constructor(&checked.typed, source)
                    .ok_or(LoweringError::Unsupported(
                        "enum equality replaced a runtime observation with a constant",
                    ))?;
                if !constructor.fields.is_empty()
                    || **value != Boolean::Constant(constructor.case == expected.symbol)
                {
                    return unsupported("enum equality changed its constant constructor tag");
                }
                constructor.type_reference
            }
            _ => return unsupported("enum equality substituted its operand source"),
        };
        if enum_owner(checked, reference).is_none_or(|observed| observed.symbol != owner.symbol) {
            return unsupported("enum equality substituted its observed nominal type");
        }
        let identity = checked.normalized_type_identity(reference);
        if observed_type
            .as_ref()
            .is_some_and(|expected| expected != &identity)
        {
            return unsupported("enum equality changed its exact operand type");
        }
        observed_type = Some(identity);
    }
    let CheckedScalarExpression::Boolean(template) = expression else {
        return unsupported("enum equality has a non-Boolean template");
    };
    if !exact_template(
        template,
        cases.len(),
        binary.operator == BinaryOperator::NotEqual,
    ) {
        return unsupported("enum equality changed its complete equivalence template");
    }
    Ok(true)
}

fn enum_owner(checked: &CheckedTrees, reference: TypeReferenceHandle) -> Option<&DataDefinition> {
    let reference = validation::unwrapped_type_reference(checked, reference)?;
    let owner = if let Some(instance) = checked.closed_data_definition(reference) {
        instance
    } else {
        let TypeReferenceNode::Named { symbol, .. } =
            checked.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        let mut declarations = checked
            .data_definitions()
            .iter()
            .filter(|data| data.symbol == *symbol);
        let owner = declarations.next()?;
        if !symbol.is_valid() || declarations.next().is_some() || !owner.type_parameters.is_empty()
        {
            return None;
        }
        owner
    };
    (DataDefinition::shape_kind_from_members(checked.data_members(owner)) == DataShapeKind::Enum)
        .then_some(owner)
}

fn selected_builtin(
    checked: &CheckedTrees,
    source: ExpressionHandle,
    operator: BinaryOperator,
) -> bool {
    let mut selected = checked
        .facts
        .operators
        .uses
        .iter()
        .map(|(_, row)| row)
        .filter(|row| row.expression == source);
    let Some(row) = selected.next() else {
        return true;
    };
    let spelling = if operator == BinaryOperator::Equal {
        language_core::OperatorSpelling::Equal
    } else {
        language_core::OperatorSpelling::NotEqual
    };
    selected.next().is_none()
        && row.spelling == spelling
        && row.status == checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
        && !row.selected_operator_symbol.is_valid()
        && checked
            .facts
            .operators
            .candidates
            .span(row.candidates)
            .is_some_and(|candidates| {
                candidates.len() == row.candidate_count
                    && candidates
                        .iter()
                        .all(|candidate| candidate.is_domain_owned())
            })
}

fn exact_template(mut template: &Boolean, count: usize, unequal: bool) -> bool {
    if unequal {
        let Boolean::Not(inner) = template else {
            return false;
        };
        template = inner;
    }
    let pair = |template: &Boolean, position| {
        matches!(template,
        Boolean::Equal { left, right }
            if matches!(left.as_ref(), Boolean::Parameter { position: actual } if *actual == position)
                && matches!(right.as_ref(), Boolean::Parameter { position: actual } if *actual == count + position))
    };
    for position in (1..count).rev() {
        let Boolean::And { left, right } = template else {
            return false;
        };
        if !pair(right, position) {
            return false;
        }
        template = left;
    }
    count != 0 && pair(template, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_enum_equality_checks_projection_and_constructor_identity() {
        let checked = crate::front_end::checked_program(
            "data Choice [copy] { case Empty; case Ready; }
             data Pair { first: Choice; second: Choice; }
             machine inspect() -> bool {
                 let mut pair: Pair;
                 pair.first = Choice::Ready;
                 pair.first == Choice::Ready
             }",
        );
        crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("inspect"))
            .expect("local field and payloadless constructor compare through one observation plan");
        for corrupt_path in [true, false] {
            let mut changed = checked.clone();
            let mut edited = false;
            changed
                .facts
                .values
                .scalar_computations
                .nodes
                .for_each_mut(|_, node| {
                    if edited {
                        return;
                    }
                    if let Computation::CaseMembership { subject, .. } = &mut node.kind {
                        match subject {
                            CheckedScalarComputationStructuralArgument::Place(place)
                                if corrupt_path =>
                            {
                                place.path.clear();
                                edited = true;
                            }
                            _ => {}
                        }
                    }
                    if !corrupt_path
                        && let Computation::Value(CheckedScalarExpression::Boolean(value)) =
                            &mut node.kind
                        && let Boolean::Constant(value) = value.as_mut()
                    {
                        *value = !*value;
                        edited = true;
                    }
                });
            assert!(edited);
            assert!(
                crate::lower_machine(&changed, crate::TerminalMachineSelection::Name("inspect"))
                    .is_err()
            );
        }
    }

    #[test]
    fn enum_equality_parameter_field_initializer_uses_runtime_observations() {
        for operator in ["==", "!="] {
            let checked = crate::front_end::checked_program(&format!(
                "data Choice [copy] {{ case First; case Second; }}
                 data Context {{ left: Choice; right: Choice; }}
                 machine Context::same(context: &mut Context) -> bool {{
                     let equal: bool = context.left {operator} context.right;
                     equal
                 }}"
            ));
            let machine = checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "Context::same")
                .unwrap();
            let state = checked.machine_states(machine)[0].symbol;
            let role =
                checked_trees::CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 };
            assert!(
                checked
                    .facts
                    .values
                    .scalar_expressions
                    .expression_at(state, 0, role)
                    .is_none(),
                "runtime initialization must not retain contract-only structural equality"
            );
            let root = checked
                .facts
                .values
                .scalar_computations
                .root_at(state, 0, role)
                .expect("the first initializer retains its ordered runtime observations")
                .root;
            assert!(validate_application(&checked, state, root).unwrap());
            crate::lower_machine(
                &checked,
                crate::TerminalMachineSelection::Name("Context::same"),
            )
            .expect("borrowed-self field equality initializer lowers and verifies");
        }
    }

    #[test]
    fn enum_equality_slice_observation_keeps_the_exact_index_parameter() {
        let checked = crate::front_end::checked_program(
            "data Choice [copy] { case First; case Second; }
             data Room [copy] { kind: Choice; }
             machine same(rooms: &[Room], index: u64, other: u64, expected: Choice) -> bool
             requires index < rooms.len
             { rooms[index].kind == expected }",
        );
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "same")
            .unwrap();
        let state = checked.machine_states(machine)[0].symbol;
        let plans = &checked.facts.values.scalar_computations;
        let root = plans
            .roots
            .iter()
            .find_map(|(_, row)| (row.machine == machine.symbol).then_some(row.root))
            .unwrap();
        assert!(validate_application(&checked, state, root).unwrap());
        crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("same"))
            .expect("bounded indexed enum equality lowers");
        let Computation::Apply { operands, .. } = plans.nodes.get(root).kind else {
            panic!("indexed equality application");
        };
        let first = plans.operands.span(operands).unwrap()[0];
        let mut changed = checked.clone();
        let Computation::CaseMembership {
            subject: CheckedScalarComputationStructuralArgument::Place(subject),
            ..
        } = &mut changed
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(first)
            .kind
        else {
            panic!("indexed place observation");
        };
        let selector = subject
            .path
            .iter_mut()
            .find_map(|segment| match segment {
                checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                    checked_trees::CheckedRuntimeIndex::Parameter { position },
                ) => Some(position),
                _ => None,
            })
            .expect("exact integer parameter selector");
        assert_eq!(*selector, 0);
        *selector = 1;
        assert!(validate_application(&changed, state, root).is_err());
        assert!(
            crate::lower_machine(&changed, crate::TerminalMachineSelection::Name("same")).is_err()
        );
    }

    #[test]
    fn enum_equality_replays_the_complete_authored_observation_roster() {
        let checked = crate::front_end::checked_program(
            "data Choice [copy] { case First; case Second; }
             data Other [copy] { case First; case Second; }
             machine same(left: Choice, right: Choice) -> bool { left == right }",
        );
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "same")
            .unwrap();
        let state = checked.machine_states(machine)[0].symbol;
        let plans = &checked.facts.values.scalar_computations;
        let (root, statement) = plans
            .roots
            .iter()
            .find_map(|(_, row)| {
                (row.machine == machine.symbol).then_some((row.root, row.statement_ordinal))
            })
            .unwrap();
        let Computation::Apply {
            source_expression,
            operands,
            ..
        } = &plans.nodes.get(root).kind
        else {
            panic!("source equality retains its ordered application");
        };
        let occurrence = *source_expression;
        let operands = plans.operands.span(*operands).unwrap().to_vec();
        assert_eq!(operands.len(), 4);
        assert!(validate_application(&checked, state, root).unwrap());
        crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("same"))
            .expect("complete source equality lowers");
        let foreign = checked
            .data_definitions()
            .iter()
            .find(|data| data.name.as_str() == "Other")
            .and_then(|data| {
                checked
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Variant(variant) => Some(variant.symbol),
                        _ => None,
                    })
            })
            .unwrap();
        for corruption in ["root", "foreign", "duplicate", "missing", "order", "slot"] {
            let mut changed = checked.clone();
            let plans = &mut changed.facts.values.scalar_computations;
            match corruption {
                "root" => {
                    let Computation::CaseMembership { subject, .. } =
                        &plans.nodes.get(operands[2]).kind
                    else {
                        panic!("right subject");
                    };
                    let right = subject.clone();
                    let Computation::CaseMembership { subject, .. } =
                        &mut plans.nodes.get_mut(operands[0]).kind
                    else {
                        panic!("left subject");
                    };
                    *subject = right;
                }
                "foreign" | "duplicate" => {
                    let replacement = if corruption == "foreign" {
                        foreign
                    } else {
                        let Computation::CaseMembership { case, .. } =
                            plans.nodes.get(operands[1]).kind
                        else {
                            panic!("second case");
                        };
                        case
                    };
                    let Computation::CaseMembership { case, .. } =
                        &mut plans.nodes.get_mut(operands[0]).kind
                    else {
                        panic!("first case");
                    };
                    *case = replacement;
                }
                "missing" | "order" => {
                    let mut replaced = operands.clone();
                    if corruption == "missing" {
                        replaced.pop();
                    } else {
                        replaced.swap(0, 1);
                    }
                    let replacement = plans.operands.insert_many(replaced);
                    let Computation::Apply { operands, .. } = &mut plans.nodes.get_mut(root).kind
                    else {
                        panic!("application");
                    };
                    *operands = replacement;
                }
                "slot" => {
                    let Computation::Apply {
                        expression: CheckedScalarExpression::Boolean(template),
                        ..
                    } = &mut plans.nodes.get_mut(root).kind
                    else {
                        panic!("Boolean template");
                    };
                    let Boolean::And { left, .. } = template.as_mut() else {
                        panic!("two pairs");
                    };
                    let Boolean::Equal { right, .. } = left.as_mut() else {
                        panic!("first pair");
                    };
                    **right = Boolean::Parameter { position: 3 };
                }
                _ => unreachable!(),
            }
            assert!(
                validate_application(&changed, state, root).is_err(),
                "{corruption}"
            );
            assert!(
                crate::lower_machine(&changed, crate::TerminalMachineSelection::Name("same"))
                    .is_err(),
                "{corruption}"
            );
        }
        assert!(
            super::super::value_correspondence::validate(
                &checked,
                state,
                statement,
                occurrence,
                PrimitiveType::Bool,
                &checked_trees::CheckedCallScalarArgument::Computation(operands[0]),
            )
            .is_err(),
            "one synthetic membership cannot replace the source equality"
        );
    }

    #[test]
    fn equivalence_template_keeps_all_pairs_and_the_authored_polarity() {
        let pair = |left, right| Boolean::Equal {
            left: Box::new(Boolean::Parameter { position: left }),
            right: Box::new(Boolean::Parameter { position: right }),
        };
        let template = Boolean::And {
            left: Box::new(pair(0, 2)),
            right: Box::new(pair(1, 3)),
        };
        assert!(exact_template(&template, 2, false));
        assert!(!exact_template(&template, 2, true));
        assert!(!exact_template(&template, 3, false));
        assert!(!exact_template(&pair(0, 2), 2, false));
        assert!(!exact_template(
            &Boolean::And {
                left: Box::new(pair(1, 3)),
                right: Box::new(pair(0, 2)),
            },
            2,
            false
        ));
        let negated = Boolean::Not(Box::new(template));
        assert!(exact_template(&negated, 2, true));
        assert!(!exact_template(&negated, 2, false));
    }
}
