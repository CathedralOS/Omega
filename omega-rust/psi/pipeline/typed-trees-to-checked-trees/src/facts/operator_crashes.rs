//! Selected operator invocations have their own occurrence and operand custody.
//! Their source crash routes never acquire fabricated ordinary-call coordinates.

use checked_trees::{
    CheckedCrashOperatorSite, CheckedOperatorFacts, CheckedOperatorOccurrence,
    CheckedOperatorResolutionStatus, CheckedValueOrigin, CrashPredicateExpression,
    CrashPredicateIdentity, CrashRouteBucket, CrashRouteGuard, FlowFacts,
};
use diagnostics::Diagnostic;
use facts::FactPlan;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, MatchPattern};
use typed_trees::signature::SignatureContractKind;

pub(crate) fn build(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    flow: &FlowFacts,
    semantic: &FactPlan,
) -> Result<Vec<(SymbolHandle, CheckedCrashOperatorSite)>, Vec<Diagnostic>> {
    let mut sites = Vec::new();
    let mut diagnostics = Vec::new();
    let mut content_conservation = None;
    for (operator_use_handle, operator_use) in operators.uses.iter() {
        let Some(selected) = operators.selected_candidate(operator_use) else {
            continue;
        };
        if !program
            .signature_contracts
            .span_or_empty(selected.contracts)
            .iter()
            .any(|contract| matches!(contract.kind, SignatureContractKind::Crashes { .. }))
        {
            continue;
        }
        if unreachable_match_arm(program, operator_use) {
            continue;
        }
        let invalid = || {
            Diagnostic::error(
                "selected operator crash invocation requires exact selected meaning and ordered operand capture",
            )
        };
        let CheckedValueOrigin::StateStatement {
            machine_symbol,
            state_symbol,
            statement_index,
            ..
        } = operator_use.origin
        else {
            diagnostics.push(invalid());
            continue;
        };
        let Some(operator) = program
            .operators()
            .iter()
            .chain(
                program
                    .domain_definitions()
                    .iter()
                    .flat_map(|domain| program.domain_operators(domain).iter()),
            )
            .find(|operator| operator.symbol == selected.operator_symbol)
        else {
            diagnostics.push(invalid());
            continue;
        };
        let Some(operands) = operator_use.operands(program) else {
            diagnostics.push(invalid());
            continue;
        };
        let parameters = program.operator_parameters(operator);
        let mut captures = flow
            .control
            .operator_invocations
            .iter()
            .filter(|(_, invocation)| invocation.operator_use == operator_use_handle);
        let (Some((invocation_handle, invocation)), None) = (captures.next(), captures.next())
        else {
            diagnostics.push(invalid());
            continue;
        };
        if operator_use.status != CheckedOperatorResolutionStatus::Resolved
            || selected.operator_symbol != operator_use.selected_operator_symbol
            || parameters.len() != operands.len()
            || !flow
                .control
                .operator_operands
                .span_or_empty(invocation.operands)
                .iter()
                .map(|operand| operand.expression)
                .eq(operands.iter().copied())
        {
            diagnostics.push(invalid());
            continue;
        }
        let content_conservation = content_conservation
            .get_or_insert_with(|| validation::build_content_conservation_plans(program));
        let published =
            super::derive_authored_operator_crash_buckets(program, operator, content_conservation);
        let parameter_names = parameters
            .iter()
            .map(|parameter| parameter.name.as_str().to_owned())
            .collect::<Vec<_>>();
        let substitution = operands
            .iter()
            .map(|operand| {
                entry_operand(
                    program,
                    machine_symbol,
                    state_symbol,
                    statement_index,
                    *operand,
                )
            })
            .collect::<Vec<_>>();
        let mut surviving = Vec::new();
        for bucket in &published {
            let mut guards = Vec::new();
            for guard in bucket.alternative_guards() {
                let CrashRouteGuard::Predicate(predicate) = guard else {
                    guards.push(CrashRouteGuard::Truth);
                    continue;
                };
                let expression = program
                    .signature_contracts
                    .span_or_empty(selected.contracts)
                    .iter()
                    .filter(|contract| {
                        matches!(contract.kind, SignatureContractKind::Crashes { .. })
                    })
                    .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
                    .find_map(|fact| {
                        let ProofFact::Expression(expression) = fact else {
                            return None;
                        };
                        let candidate = super::crash_calls::crash_predicate_from_expression(
                            program,
                            *expression,
                            &parameter_names,
                            Some(content_conservation),
                        );
                        (CrashPredicateIdentity::from_expression(candidate) == *predicate)
                            .then_some(*expression)
                    });
                if expression.is_some_and(|expression| {
                    crate::checks::operator_route_is_false(
                        program,
                        flow,
                        semantic,
                        operator_use_handle,
                        parameters,
                        &operands,
                        expression,
                    )
                }) {
                    continue;
                }
                // An operand snapshot is not automatically a machine-entry
                // value. Unknown storage/version provenance widens the route;
                // it must never leave a callee Parameter unsubstituted in the
                // caller namespace or relabel a current read as an entry input.
                let Some(identity) = predicate
                    .expression()
                    .and_then(|identity| substitute_entry(identity, &substitution))
                else {
                    guards.push(CrashRouteGuard::Truth);
                    continue;
                };
                match identity.boolean_value() {
                    Some(false) => {}
                    Some(true) => guards.push(CrashRouteGuard::Truth),
                    None => guards.push(CrashRouteGuard::Predicate(
                        CrashPredicateIdentity::from_expression(identity),
                    )),
                }
            }
            if let Some(bucket) = CrashRouteBucket::new(bucket.cause(), guards) {
                surviving.push(bucket);
            }
        }
        sites.push((
            machine_symbol,
            CheckedCrashOperatorSite {
                operator_use: operator_use_handle,
                invocation: invocation_handle,
                selected_operator: selected.operator_symbol,
                published,
                surviving,
            },
        ));
    }
    if diagnostics.is_empty() {
        Ok(sites)
    } else {
        Err(diagnostics)
    }
}

fn unreachable_match_arm(
    program: &TypedTrees,
    operator_use: &checked_trees::CheckedOperatorUseFact,
) -> bool {
    let CheckedOperatorOccurrence::MatchEquality { source_arm } = operator_use.occurrence else {
        return false;
    };
    let ExpressionNode::Match(dispatch) =
        program.expression_table.expression(operator_use.expression)
    else {
        return false;
    };
    // Compare complete generational handles, not merely an arena-index offset.
    for ordinal in 0..program.expression_table.match_arms(dispatch.arms).len() {
        let Some(index) = u32::try_from(ordinal)
            .ok()
            .and_then(|ordinal| dispatch.arms.start().arena_index().checked_add(ordinal))
        else {
            return false;
        };
        if arena::Handle::from_parts(index, dispatch.arms.start().generation()) == source_arm {
            return program
                .expression_table
                .match_arms(dispatch.arms)
                .iter()
                .take(ordinal)
                .any(|arm| matches!(arm.pattern, MatchPattern::Wildcard));
        }
    }
    false
}

pub(super) fn entry_operand(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    before_statement: usize,
    expression: ExpressionHandle,
) -> Option<CrashPredicateExpression> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Boolean(value) => Some(CrashPredicateExpression::Boolean(*value)),
        ExpressionNode::Integer(value) => {
            Some(CrashPredicateExpression::Integer(value.text().to_owned()))
        }
        ExpressionNode::Unary(unary)
            if unary.operator == typed_trees::expression::UnaryOperator::LogicalNot =>
        {
            Some(CrashPredicateExpression::Unary {
                operator: unary.operator as u8,
                operand: Box::new(entry_operand(
                    program,
                    machine_symbol,
                    state_symbol,
                    before_statement,
                    unary.operand,
                )?),
            })
        }
        ExpressionNode::Name(path) => {
            if program
                .expression_table
                .name_path_members(path.members)
                .len()
                != 1
            {
                return None;
            }
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == machine_symbol)?;
            let state = program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == state_symbol)?;
            let preceding = program
                .statement_table
                .statements(state.statement_nodes)
                .get(..before_statement)?;
            for (ordinal, statement) in preceding.iter().enumerate() {
                if let typed_trees::statement::StatementNode::LocalData(local) = statement
                    && local.symbol == path.symbol
                {
                    if local.is_mutable
                        || program
                            .primitive_type_reference(local.type_reference)
                            .is_none()
                    {
                        return None;
                    }
                    // This transports a fixed scalar value, not a current read
                    // of its initializer. Every dependency must independently
                    // be immutable and entry-relative; mutable initializers
                    // are rejected even if their storage now has useful facts.
                    // Decreasing the prefix also prevents recursive aliases.
                    return entry_operand(
                        program,
                        machine_symbol,
                        state_symbol,
                        ordinal,
                        local.initial_value,
                    );
                }
            }
            let entry_index = crate::checks::termination::named_transition_target_state_index(
                program,
                machine,
                machine.symbol,
            )?;
            let entry = program.machine_states(machine).get(entry_index)?;
            if state_symbol != entry.symbol
                || entry_has_incoming_transition(program, machine, entry_index)
            {
                return None;
            }
            let (ordinal, _) =
                program
                    .state_parameters(entry)
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| {
                        parameter.symbol == path.symbol
                            && !parameter.is_mutable
                            && !parameter.is_self
                            && program
                                .primitive_type_reference(parameter.type_reference)
                                .is_some()
                    })?;
            Some(CrashPredicateExpression::Parameter(
                u32::try_from(ordinal).ok()?,
            ))
        }
        _ => None,
    }
}

fn entry_has_incoming_transition(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    entry_index: usize,
) -> bool {
    use typed_trees::statement::{StatementNode, TransitionTargetNode};

    // Typed statement lowering flattens nested transition forms into states.
    // Inspect ordinary targets and continuation targets, just as the existing
    // termination graph does. A repeated immutable declaration is a fresh
    // arrival value, not necessarily the original invocation-entry value.
    program.machine_states(machine).iter().any(|state| {
        program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .any(|statement| {
                let StatementNode::Transition(transition) = statement else {
                    return false;
                };
                [transition.target, transition.continuation]
                    .into_iter()
                    .filter(|target| target.is_valid())
                    .any(|target| {
                        let symbol = match program.statement_table.transition_target(target) {
                            TransitionTargetNode::Named { path, .. } => path.symbol,
                            TransitionTargetNode::SelfTarget => state.symbol,
                            TransitionTargetNode::Value(_) | TransitionTargetNode::Terminal => {
                                return false;
                            }
                        };
                        crate::checks::termination::named_transition_target_state_index(
                            program, machine, symbol,
                        ) == Some(entry_index)
                    })
            })
    })
}

pub(super) fn substitute_entry(
    expression: &CrashPredicateExpression,
    operands: &[Option<CrashPredicateExpression>],
) -> Option<CrashPredicateExpression> {
    Some(match expression {
        CrashPredicateExpression::Parameter(ordinal) => operands.get(*ordinal as usize)?.clone()?,
        CrashPredicateExpression::Boolean(_) | CrashPredicateExpression::Integer(_) => {
            expression.clone()
        }
        CrashPredicateExpression::Binary {
            operator,
            left,
            right,
        } => CrashPredicateExpression::Binary {
            operator: *operator,
            left: Box::new(substitute_entry(left, operands)?),
            right: Box::new(substitute_entry(right, operands)?),
        },
        CrashPredicateExpression::Unary { operator, operand } => CrashPredicateExpression::Unary {
            operator: *operator,
            operand: Box::new(substitute_entry(operand, operands)?),
        },
        _ => return None,
    })
}
