//! Exact entry equations and comparison polarity refine authenticated operands.
//! A current actual first crosses the shared capture/provenance boundary. Entry
//! requirements substitute closed literal values and retain checked comparison
//! polarity over those original operands, independently of subsequent writes.
//! Unknown origins and unselected arithmetic supply no entry evidence.
use checked_trees::{
    CheckedBooleanExpression as Boolean, CheckedScalarExpression as Scalar,
    CrashPredicateExpression as Expression,
};
use typed_trees::expression::BinaryOperator;

pub(super) fn refine(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    machine: &typed_trees::machine::Machine,
    buckets: &mut Vec<super::summary_predicates::SummaryCrashBucket>,
) {
    let Some(entry) = program.machine_states(machine).first() else {
        return;
    };
    let parameters = program.state_parameters(entry);
    let names = parameters
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect::<Vec<_>>();
    let mut equations = Vec::new();
    let mut excluded = Vec::new();
    for contract in program
        .machine_contracts(machine)
        .iter()
        .chain(program.state_contracts(entry))
        .filter(|contract| contract.kind == typed_trees::signature::SignatureContractKind::Requires)
    {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let Some(expression) = fact.as_expression() else {
                continue;
            };
            let Some(checked) = crate::values::lower_machine_entry_crash_contract_expression(
                program,
                operators,
                machine,
                *expression,
                &[],
            ) else {
                continue;
            };
            let identity = super::proof_crash_predicate_from_expression(
                program,
                *expression,
                parameters,
                &names,
                None,
            );
            collect(&checked, &identity, &mut equations);
            if super::summary_predicates::executable_entry_predicate(&checked) {
                excluded.extend(excluded_comparisons(&checked, &identity, false, 0));
            }
        }
    }
    if equations.is_empty() && excluded.is_empty() {
        return;
    }
    for bucket in buckets.iter_mut() {
        bucket.alternative_guards.retain(|guard| {
            let super::summary_predicates::SummaryCrashRouteGuard::Predicate(predicate) = guard
            else {
                return true;
            };
            if !predicate.builtin_meaning {
                return true;
            }
            let substituted = substitute(&predicate.identity, &equations);
            !excluded.contains(&substituted)
                && super::summary_predicates::summary_boolean_value(&substituted) != Some(false)
        });
    }
    buckets.retain(|bucket| !bucket.alternative_guards.is_empty());
}

/// Only checked fixed-integer comparisons admit ordered complementation.
/// IEEE unordered relations, erased mathematics and selected authored operators
/// never supply these keys. Conjuncts combine; disjuncts retain common keys.
/// The keys refer to invocation-entry identities, not current storage.
fn excluded_comparisons(
    checked: &Boolean,
    identity: &Expression,
    negated: bool,
    depth: usize,
) -> Vec<Expression> {
    use typed_trees::expression::UnaryOperator;
    if depth >= 64 {
        return Vec::new();
    }
    match (checked, identity) {
        (Boolean::Not(inner), Expression::Unary { operator, operand })
            if *operator == UnaryOperator::LogicalNot as u8 =>
        {
            excluded_comparisons(inner, operand, !negated, depth + 1)
        }
        (
            Boolean::And { left, right } | Boolean::Or { left, right },
            Expression::Binary {
                operator,
                left: a,
                right: b,
            },
        ) if (*operator == BinaryOperator::And as u8 && matches!(checked, Boolean::And { .. }))
            || (*operator == BinaryOperator::Or as u8 && matches!(checked, Boolean::Or { .. })) =>
        {
            let mut left = excluded_comparisons(left, a, negated, depth + 1);
            let right = excluded_comparisons(right, b, negated, depth + 1);
            if matches!(checked, Boolean::And { .. }) != negated {
                left.extend(right);
            } else {
                left.retain(|key| right.contains(key));
            }
            left
        }
        (
            Boolean::IntegerComparison { .. },
            Expression::Binary {
                operator,
                left,
                right,
            },
        ) => {
            let complement = [
                (BinaryOperator::Equal, BinaryOperator::NotEqual),
                (BinaryOperator::NotEqual, BinaryOperator::Equal),
                (BinaryOperator::Less, BinaryOperator::GreaterOrEqual),
                (BinaryOperator::LessOrEqual, BinaryOperator::Greater),
                (BinaryOperator::Greater, BinaryOperator::LessOrEqual),
                (BinaryOperator::GreaterOrEqual, BinaryOperator::Less),
            ]
            .into_iter()
            .find_map(|(original, complement)| {
                (*operator == original as u8).then_some(if negated { original } else { complement })
            });
            complement
                .map(|operator| {
                    vec![Expression::Binary {
                        operator: operator as u8,
                        left: left.clone(),
                        right: right.clone(),
                    }]
                })
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

fn collect(
    checked: &Boolean,
    identity: &Expression,
    equations: &mut Vec<(Expression, Expression)>,
) {
    match (checked, identity) {
        (
            Boolean::And { left, right },
            Expression::Binary {
                operator,
                left: a,
                right: b,
            },
        ) if *operator == BinaryOperator::And as u8 => {
            collect(left, a, equations);
            collect(right, b, equations);
        }
        (
            Boolean::IntegerComparison {
                kind: checked_trees::CheckedIntegerComparisonKind::Equal,
                left,
                right,
            },
            Expression::Binary {
                operator,
                left: a,
                right: b,
            },
        ) if *operator == BinaryOperator::Equal as u8 => {
            for (value, subject, literal) in [
                (left.as_ref(), b.as_ref(), a.as_ref()),
                (right.as_ref(), a.as_ref(), b.as_ref()),
            ] {
                if matches!(value, Scalar::IntegerLiteral { .. })
                    && matches!(literal, Expression::Integer(_))
                {
                    equations.push((subject.clone(), literal.clone()));
                }
            }
        }
        _ => {}
    }
}

fn substitute(value: &Expression, equations: &[(Expression, Expression)]) -> Expression {
    if let Some((_, replacement)) = equations.iter().find(|(subject, _)| subject == value) {
        return replacement.clone();
    }
    match value {
        Expression::Binary {
            operator,
            left,
            right,
        } => Expression::Binary {
            operator: *operator,
            left: Box::new(substitute(left, equations)),
            right: Box::new(substitute(right, equations)),
        },
        Expression::Unary { operator, operand } => Expression::Unary {
            operator: *operator,
            operand: Box::new(substitute(operand, equations)),
        },
        Expression::RepresentationView {
            source_type,
            target_type,
            operand,
        } => Expression::RepresentationView {
            source_type: *source_type,
            target_type: *target_type,
            operand: Box::new(substitute(operand, equations)),
        },
        Expression::IntegerWiden {
            source_type,
            target_type,
            domain,
            operand,
        } => Expression::IntegerWiden {
            source_type: *source_type,
            target_type: *target_type,
            domain: *domain,
            operand: Box::new(substitute(operand, equations)),
        },
        // Whole selected observations are exact keys. Do not distribute an
        // equation through a place's root/selector and manufacture a new place.
        _ => value.clone(),
    }
}

/// A completed scalar result can inherit an entry operand from a surviving
/// checked `result == formal` guarantee. The assignment pins the exact call;
/// the guarantee pins normal completion and the captured formal, independently
/// of the callee body or the caller's current source value.
pub(super) fn completed_operand(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    semantic: &facts::FactPlan,
    flow: &checked_trees::FlowFacts,
    caller: &checked_trees::FlowStateFact,
    statement: usize,
    contexts: &[facts::FactContextHandle],
    argument: typed_trees::expression::ExpressionHandle,
) -> Option<Expression> {
    // A direct value already names its producer occurrence. A storage read
    // instead rejoins that occurrence through live capture facts and its write
    // boundary. Both use the same completed-guarantee query below.
    let result = if matches!(
        program.expression_table.expression(argument),
        typed_trees::expression::ExpressionNode::Call(_)
    ) {
        argument
    } else {
        let place = validation::canonical_place_from_expression_in_state(
            program,
            caller.state_symbol,
            statement,
            argument,
        )?;
        let (_, state) = validation::machine_state_by_symbols(
            program,
            caller.machine_symbol,
            caller.state_symbol,
        )?;
        let containing = program.statement_table.statement_at(state, statement)?;
        if crate::facts::crash_entry_values::statement_may_overwrite_place(
            program,
            caller.machine_symbol,
            containing,
            &place,
        ) {
            return None;
        }
        let captured =
            crate::checks::contracts::captured_guarantee_place(program, semantic, contexts, place)?;
        let facts::PlaceRoot::Expression(result) = captured.root else {
            return None;
        };
        if !captured.segments.is_empty() {
            return None;
        }
        result
    };
    let mut agreed = None;
    for fact in contexts.iter().flat_map(|context| {
        semantic
            .context_view(semantic.contexts.get(*context))
            .facts()
    }) {
        let facts::FactPayload::ContractBooleanExpression {
            kind: facts::ContractFactKind::Ensures,
            expression,
            ..
        } = fact.payload
        else {
            continue;
        };
        let facts::ProgramPoint::CallEnsures {
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        } = fact.point
        else {
            continue;
        };
        if fact.origin != facts::FactOrigin::CallEnsures
            || machine_symbol != caller.machine_symbol
            || state_symbol != caller.state_symbol
            || statement_index > statement
        {
            continue;
        }
        let Some(site) = validation::find_call_site(
            program,
            machine_symbol,
            state_symbol,
            statement_index,
            call_ordinal,
        ) else {
            continue;
        };
        if !matches!(site, validation::CallSite::Expression { expression, .. } if expression == result)
        {
            continue;
        }
        let Some(call) = flow
            .control
            .calls
            .span_or_empty(caller.calls)
            .iter()
            .find(|call| {
                call.statement_index == statement_index && call.call_ordinal == call_ordinal
            })
        else {
            continue;
        };
        let Some((machine, state)) = validation::find_machine_head(program, call.target_symbol)
        else {
            continue;
        };
        if !validation::has_builtin_bound_expression_meaning(
            program,
            machine,
            Some(state),
            expression,
        ) {
            continue;
        }
        let parameters = program.state_parameters(state);
        let Some(checked) = crate::values::lower_callable_scalar_contract_predicate(
            program,
            operators,
            machine.symbol,
            parameters,
            state.return_type,
            expression,
            true,
            &mut 4096,
        ) else {
            continue;
        };
        let Boolean::IntegerComparison {
            kind: checked_trees::CheckedIntegerComparisonKind::Equal,
            left,
            right,
        } = checked
        else {
            continue;
        };
        let scalar_parameters = parameters
            .iter()
            .filter(|parameter| crate::values::occupies_scalar_position(program, parameter))
            .collect::<Vec<_>>();
        for (result_term, input) in [
            (left.as_ref(), right.as_ref()),
            (right.as_ref(), left.as_ref()),
        ] {
            let (
                Scalar::Parameter {
                    position: output,
                    primitive_type: output_type,
                },
                Scalar::Parameter {
                    position: input,
                    primitive_type: input_type,
                },
            ) = (result_term, input)
            else {
                continue;
            };
            if *output != scalar_parameters.len() || output_type != input_type {
                continue;
            }
            let Some(parameter) = scalar_parameters.get(*input) else {
                continue;
            };
            let Some(position) = parameters
                .iter()
                .position(|candidate| candidate.symbol == parameter.symbol)
            else {
                continue;
            };
            let Some(actual) =
                validation::call_site_argument_expressions(program, &site).get(position)
            else {
                continue;
            };
            // The actual is evaluated inside its producer statement. A
            // later live guarantee cannot move that capture back across an
            // earlier sibling's write to the argument source.
            let Some((_, producer_state)) =
                validation::machine_state_by_symbols(program, machine_symbol, state_symbol)
            else {
                continue;
            };
            let Some(producer_statement) = program
                .statement_table
                .statement_at(producer_state, statement_index)
            else {
                continue;
            };
            if let Some(actual_place) = validation::canonical_place_from_expression_in_state(
                program,
                state_symbol,
                statement_index,
                *actual,
            ) && crate::facts::crash_entry_values::statement_may_overwrite_place(
                program,
                machine_symbol,
                producer_statement,
                &actual_place,
            ) {
                continue;
            }
            let Some(value) = crate::facts::crash_entry_values::entry_operand(
                program,
                machine_symbol,
                state_symbol,
                statement_index,
                *actual,
            ) else {
                continue;
            };
            if agreed.as_ref().is_some_and(|known| *known != value) {
                return None;
            }
            agreed = Some(value);
        }
    }
    agreed
}
