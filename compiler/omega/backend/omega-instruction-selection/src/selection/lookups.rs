use crate::InstructionSelectionInput;
use omega_control_flow::{Operation, StateKey, StateParameterFlow};
use omega_platform_interface::HostCall;
use omega_state_calls::StateCall;
use omega_state_storage::StateMutation;
use psi_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use psi_checked_trees::name::Identifier;
use psi_numerics::arithmetic::{ArithmeticDomain, ArithmeticPolicyAdapter};
use psi_numerics::float_semantics::FloatFormat;
use psi_symbols::SymbolHandle;

fn state_key_matches_statement_source(expected: StateKey, actual: StateKey) -> bool {
    expected == actual || (expected.machine == actual.machine && expected.state == actual.state)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CarriedFloatPolicyDomain {
    Missing,
    Resolved(ArithmeticDomain),
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CarriedFloatProviderPlan {
    Missing,
    Resolved(u64),
    Invalid,
}

/// Resolve the exact selected ProviderPlan identity carried for one checked
/// float operator. Missing or contradictory identity is a hard lowering
/// failure at the consumer.
pub(super) fn carried_float_provider_plan(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> CarriedFloatProviderPlan {
    let canonical = canonical_checked_operator_expression(
        input.program,
        source_key,
        statement_index,
        expressions,
        expression,
    );
    match canonical {
        CanonicalOperatorExpression::Resolved { expression, origin } => {
            let Some((source_key, statement_index)) = operator_origin_key(origin) else {
                return CarriedFloatProviderPlan::Invalid;
            };
            let carried = carried_float_provider_plan_in_control_flow(
                input.control_flow,
                source_key,
                statement_index,
                expression,
            );
            let checked = checked_float_provider_plan_for_statement(
                input.program,
                source_key,
                statement_index,
                expression,
            );
            return reconcile_float_provider_plan_evidence(checked, carried);
        }
        CanonicalOperatorExpression::Missing => return CarriedFloatProviderPlan::Missing,
        CanonicalOperatorExpression::Invalid => return CarriedFloatProviderPlan::Invalid,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CanonicalOperatorExpression {
    Missing,
    Resolved {
        expression: ExpressionHandle,
        origin: psi_checked_trees::CheckedValueOrigin,
    },
    Invalid,
}

fn operator_origin_key(origin: psi_checked_trees::CheckedValueOrigin) -> Option<(StateKey, usize)> {
    let psi_checked_trees::CheckedValueOrigin::StateStatement {
        machine_symbol,
        state_symbol,
        statement_index,
        ..
    } = origin
    else {
        return None;
    };
    Some((
        StateKey {
            machine: machine_symbol,
            state: state_symbol,
            segment_index: 0,
        },
        statement_index,
    ))
}

/// Recover the canonical checked-tree identity of an operator after downstream
/// planners have copied its expression into a private table. `copy_from`
/// preserves the authored source span on every node, including the operator
/// token span on binary expressions, so the span plus the matched fact's
/// statement origin is the stable identity across those tables. The supplied
/// `source_key` is the runtime operand-resolution context and may differ from
/// that origin after an inlined callee parameter is substituted with a caller
/// expression. Exact handle/span identity therefore recovers the origin before
/// applying any state filter. When a realization necessarily rebuilds a
/// normalized operator without that identity, the stricter same-state
/// evidence-equivalence bridge below is the only fallback.
fn canonical_checked_operator_expression(
    program: &psi_checked_trees::CheckedTrees,
    source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> CanonicalOperatorExpression {
    if !expression.is_valid() {
        return CanonicalOperatorExpression::Missing;
    }
    let local_span = expressions.source_span(expression);
    let canonical_table = &program.expression_table;
    let table_is_canonical = std::ptr::eq(expressions, canonical_table);
    let mut resolved = None;

    for (candidate_expression, origin, identity, policy_adapter) in program
        .facts
        .operators
        .uses
        .iter()
        .map(|(_, operator_use)| {
            (
                operator_use.expression,
                operator_use.origin,
                operator_use.provider_plan_identity,
                operator_use.policy_adapter,
            )
        })
        .chain(
            program
                .facts
                .operators
                .named_uses
                .iter()
                .map(|(_, operator_use)| {
                    (
                        operator_use.expression,
                        operator_use.origin,
                        operator_use.provider_plan_identity,
                        operator_use.policy_adapter,
                    )
                }),
        )
    {
        let psi_checked_trees::CheckedValueOrigin::StateStatement {
            machine_symbol: _,
            state_symbol: _,
            statement_index: _,
            ..
        } = origin
        else {
            continue;
        };
        let same_expression = if table_is_canonical {
            candidate_expression == expression
        } else {
            local_span != Default::default()
                && canonical_table.source_span(candidate_expression) == local_span
        };
        if !same_expression {
            continue;
        }
        // One authored token can own multiple normalized nodes (`abs` owns
        // both its generated `max` and subtraction). Span identity narrows the
        // occurrence; the normalized node shape narrows the operation within
        // that occurrence. This is deliberately not a shape-only search.
        if !same_normalized_operator_shape(
            expressions,
            expression,
            canonical_table,
            candidate_expression,
        ) {
            continue;
        }
        if identity == 0 {
            return CanonicalOperatorExpression::Invalid;
        }
        match resolved {
            Some((_, existing_origin, existing_identity, existing_policy))
                if existing_origin != origin
                    || existing_identity != identity
                    || existing_policy != policy_adapter =>
            {
                return CanonicalOperatorExpression::Invalid;
            }
            None => {
                resolved = Some((candidate_expression, origin, identity, policy_adapter));
            }
            _ => {}
        }
    }

    if let Some((expression, origin, _, _)) = resolved {
        return CanonicalOperatorExpression::Resolved { expression, origin };
    }

    // Float realization and local substitution can rebuild one already-checked
    // operator into a private expression table without carrying its authored
    // span. Bridge that synthetic node only when every fact in the exact
    // runtime statement with the same normalized operation shape agrees on
    // one identity, policy, and source occurrence. The statement identity is
    // already part of every carried value lookup and separates zero-span
    // compiler-normalized float builtins in one state.
    let mut shape_match = None;
    let mut shape_origin = None;
    let mut shape_identity = None;
    let mut shape_policy = None;
    for (candidate_expression, origin, identity, policy_adapter) in program
        .facts
        .operators
        .uses
        .iter()
        .map(|(_, operator_use)| {
            (
                operator_use.expression,
                operator_use.origin,
                operator_use.provider_plan_identity,
                operator_use.policy_adapter,
            )
        })
        .chain(
            program
                .facts
                .operators
                .named_uses
                .iter()
                .map(|(_, operator_use)| {
                    (
                        operator_use.expression,
                        operator_use.origin,
                        operator_use.provider_plan_identity,
                        operator_use.policy_adapter,
                    )
                }),
        )
    {
        let psi_checked_trees::CheckedValueOrigin::StateStatement {
            machine_symbol,
            state_symbol,
            statement_index: candidate_statement,
            ..
        } = origin
        else {
            continue;
        };
        if machine_symbol != source_key.machine
            || state_symbol != source_key.state
            || candidate_statement != statement_index
        {
            continue;
        }
        if !same_normalized_operator_shape(
            expressions,
            expression,
            canonical_table,
            candidate_expression,
        ) {
            continue;
        }
        if identity == 0 {
            return CanonicalOperatorExpression::Invalid;
        }
        match shape_identity {
            Some(existing) if existing != identity => {
                return CanonicalOperatorExpression::Invalid;
            }
            None => shape_identity = Some(identity),
            _ => {}
        }
        match shape_policy {
            Some(existing) if existing != policy_adapter => {
                return CanonicalOperatorExpression::Invalid;
            }
            None => shape_policy = Some(policy_adapter),
            _ => {}
        }
        match shape_origin {
            Some(existing) if existing != origin => {
                return CanonicalOperatorExpression::Invalid;
            }
            None => shape_origin = Some(origin),
            _ => {}
        }
        shape_match.get_or_insert(candidate_expression);
    }
    shape_match.map_or(CanonicalOperatorExpression::Missing, |expression| {
        CanonicalOperatorExpression::Resolved {
            expression,
            origin: shape_origin.expect("a shape match retains its checked origin"),
        }
    })
}

fn same_normalized_operator_shape(
    local_table: &ExpressionTable,
    local_expression: ExpressionHandle,
    checked_table: &ExpressionTable,
    checked_expression: ExpressionHandle,
) -> bool {
    match (
        local_table.expression(local_expression),
        checked_table.expression(checked_expression),
    ) {
        (ExpressionNode::Binary(local), ExpressionNode::Binary(checked)) => {
            local.operator == checked.operator
        }
        (ExpressionNode::Call(local), ExpressionNode::Call(checked)) => {
            !local.receiver.is_valid()
                && !checked.receiver.is_valid()
                && local.target_symbol.is_valid()
                && local.target_symbol == checked.target_symbol
                && local.arguments.count() == checked.arguments.count()
        }
        (ExpressionNode::Cast(local), ExpressionNode::Cast(checked)) => {
            local.target_type == checked.target_type
                && local.domain == checked.domain
                && local.semantic_domain_symbol == checked.semantic_domain_symbol
                && local.form == checked.form
        }
        _ => false,
    }
}

fn checked_float_provider_plan_for_statement(
    program: &psi_checked_trees::CheckedTrees,
    source_key: StateKey,
    statement_index: usize,
    expression: ExpressionHandle,
) -> CarriedFloatProviderPlan {
    let mut resolved = None;
    for (candidate_expression, origin, identity) in program
        .facts
        .operators
        .uses
        .iter()
        .map(|(_, operator_use)| {
            (
                operator_use.expression,
                operator_use.origin,
                operator_use.provider_plan_identity,
            )
        })
        .chain(
            program
                .facts
                .operators
                .named_uses
                .iter()
                .map(|(_, operator_use)| {
                    (
                        operator_use.expression,
                        operator_use.origin,
                        operator_use.provider_plan_identity,
                    )
                }),
        )
    {
        let psi_checked_trees::CheckedValueOrigin::StateStatement {
            machine_symbol,
            state_symbol,
            statement_index: candidate_statement,
            ..
        } = origin
        else {
            continue;
        };
        if candidate_expression != expression
            || machine_symbol != source_key.machine
            || state_symbol != source_key.state
            || candidate_statement != statement_index
            || identity == 0
        {
            continue;
        }
        match resolved {
            Some(existing) if existing != identity => return CarriedFloatProviderPlan::Invalid,
            None => resolved = Some(identity),
            _ => {}
        }
    }
    resolved.map_or(
        CarriedFloatProviderPlan::Missing,
        CarriedFloatProviderPlan::Resolved,
    )
}

fn reconcile_float_provider_plan_evidence(
    checked: CarriedFloatProviderPlan,
    carried: CarriedFloatProviderPlan,
) -> CarriedFloatProviderPlan {
    match (checked, carried) {
        (CarriedFloatProviderPlan::Invalid, _) | (_, CarriedFloatProviderPlan::Invalid) => {
            CarriedFloatProviderPlan::Invalid
        }
        (
            CarriedFloatProviderPlan::Resolved(expected),
            CarriedFloatProviderPlan::Resolved(actual),
        ) if expected == actual => CarriedFloatProviderPlan::Resolved(actual),
        (CarriedFloatProviderPlan::Resolved(_), _) => CarriedFloatProviderPlan::Invalid,
        (CarriedFloatProviderPlan::Missing, carried) => carried,
    }
}

fn carried_float_provider_plan_in_control_flow(
    control_flow: &omega_control_flow::ControlFlowPlan,
    source_key: StateKey,
    statement_index: usize,
    expression: ExpressionHandle,
) -> CarriedFloatProviderPlan {
    let Some(state) = control_flow.state_by_key(source_key).or_else(|| {
        control_flow
            .states
            .iter()
            .find(|(_, state)| state_key_matches_statement_source(state.key, source_key))
            .map(|(_, state)| state)
    }) else {
        return CarriedFloatProviderPlan::Missing;
    };
    let mut resolved = None;
    for value in control_flow
        .semantics
        .values
        .values
        .span_or_empty(state.values.values)
    {
        let omega_control_flow::StateValueOrigin::Statement {
            statement_index: value_statement_index,
            ..
        } = value.origin;
        if value_statement_index != statement_index || value.expression != expression {
            continue;
        }
        let Some(identity) = value.operator_provider_plan_identity else {
            continue;
        };
        if identity == 0 {
            return CarriedFloatProviderPlan::Invalid;
        }
        match resolved {
            Some(existing) if existing != identity => return CarriedFloatProviderPlan::Invalid,
            None => resolved = Some(identity),
            _ => {}
        }
    }
    resolved.map_or(
        CarriedFloatProviderPlan::Missing,
        CarriedFloatProviderPlan::Resolved,
    )
}

#[cfg(test)]
mod float_provider_plan_tests {
    use super::*;
    use omega_control_flow::{
        ControlFlowPlan, StateFlow, StateValueFact, StateValueOrigin, StateValueStatementRole,
    };
    use psi_checked_trees::{CheckedOperatorUseFact, CheckedValueStatementRole};
    use psi_source::{SourceId, SourceSpan, Span};
    use psi_symbols::SymbolHandle;

    fn carried(identities: &[Option<u64>]) -> CarriedFloatProviderPlan {
        let source_key = StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(2),
            segment_index: 3,
        };
        let expression = ExpressionHandle::from_arena_index(4);
        let mut control_flow = ControlFlowPlan::default();
        let mut state = StateFlow {
            key: source_key,
            ..StateFlow::default()
        };
        for identity in identities {
            control_flow.semantics.values.values.append_to_span(
                &mut state.values.values,
                StateValueFact {
                    machine_symbol: source_key.machine,
                    state_symbol: source_key.state,
                    expression,
                    origin: StateValueOrigin::Statement {
                        statement_index: 5,
                        role: StateValueStatementRole::AssignmentValue,
                    },
                    arithmetic_policy_adapter: None,
                    operator_provider_plan_identity: *identity,
                },
            );
        }
        control_flow.states.insert(state);
        carried_float_provider_plan_in_control_flow(&control_flow, source_key, 5, expression)
    }

    #[test]
    fn carried_float_provider_plan_resolves_exact_nonzero_identity() {
        assert_eq!(
            carried(&[Some(0x1234_5678_9abc_def0)]),
            CarriedFloatProviderPlan::Resolved(0x1234_5678_9abc_def0)
        );
    }

    #[test]
    fn carried_float_provider_plan_rejects_zero_and_contradictions() {
        assert_eq!(carried(&[Some(0)]), CarriedFloatProviderPlan::Invalid);
        assert_eq!(
            carried(&[Some(11), Some(12)]),
            CarriedFloatProviderPlan::Invalid
        );
        assert_eq!(carried(&[None]), CarriedFloatProviderPlan::Missing);
    }

    #[test]
    fn checked_migrated_provider_evidence_cannot_disappear_or_change_in_lowering() {
        let checked = CarriedFloatProviderPlan::Resolved(11);
        assert_eq!(
            reconcile_float_provider_plan_evidence(checked, CarriedFloatProviderPlan::Missing),
            CarriedFloatProviderPlan::Invalid
        );
        assert_eq!(
            reconcile_float_provider_plan_evidence(checked, CarriedFloatProviderPlan::Resolved(12)),
            CarriedFloatProviderPlan::Invalid
        );
        assert_eq!(
            reconcile_float_provider_plan_evidence(checked, CarriedFloatProviderPlan::Resolved(11)),
            CarriedFloatProviderPlan::Resolved(11)
        );
    }

    #[test]
    fn exact_operator_span_recovers_callee_origin_in_caller_operand_context() {
        let callee_key = StateKey {
            machine: SymbolHandle::from_arena_index(31),
            state: SymbolHandle::from_arena_index(32),
            segment_index: 0,
        };
        let caller_key = StateKey {
            machine: SymbolHandle::from_arena_index(41),
            state: SymbolHandle::from_arena_index(42),
            segment_index: 0,
        };
        let origin = psi_checked_trees::CheckedValueOrigin::StateStatement {
            machine_symbol: callee_key.machine,
            state_symbol: callee_key.state,
            statement_index: 3,
            role: CheckedValueStatementRole::LocalInitializer,
        };

        let mut program = psi_checked_trees::CheckedTrees::default();
        let left = program
            .typed
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let right = program
            .typed
            .expression_table
            .insert(ExpressionNode::Boolean(false));
        let expression = program
            .typed
            .expression_table
            .insert(ExpressionNode::Binary(
                psi_checked_trees::expression::TableBinaryExpression {
                    left,
                    operator: psi_checked_trees::expression::BinaryOperator::And,
                    right,
                },
            ));
        program.typed.expression_table.set_source_span(
            expression,
            SourceSpan::new(SourceId(7), Span::new(100, 110)),
        );
        program.facts.operators.uses.insert(CheckedOperatorUseFact {
            expression,
            origin,
            provider_plan_identity: 17,
            ..CheckedOperatorUseFact::default()
        });

        let mut realized = ExpressionTable::new();
        let realized_expression = realized.copy_from(&program.expression_table, expression);
        assert_eq!(
            canonical_checked_operator_expression(
                &program,
                caller_key,
                9,
                &realized,
                realized_expression,
            ),
            CanonicalOperatorExpression::Resolved { expression, origin }
        );
    }

    fn checked_binary_with_span(
        program: &mut psi_checked_trees::CheckedTrees,
        operator: psi_checked_trees::expression::BinaryOperator,
        span: SourceSpan,
    ) -> ExpressionHandle {
        let left = program
            .typed
            .expression_table
            .insert(ExpressionNode::Boolean(true));
        let right = program
            .typed
            .expression_table
            .insert(ExpressionNode::Boolean(false));
        let expression = program
            .typed
            .expression_table
            .insert(ExpressionNode::Binary(
                psi_checked_trees::expression::TableBinaryExpression {
                    left,
                    operator,
                    right,
                },
            ));
        program
            .typed
            .expression_table
            .set_source_span(expression, span);
        expression
    }

    #[test]
    fn exact_operator_span_accepts_equivalent_duplicates_and_ignores_other_shapes() {
        let source_key = StateKey {
            machine: SymbolHandle::from_arena_index(51),
            state: SymbolHandle::from_arena_index(52),
            segment_index: 0,
        };
        let origin = psi_checked_trees::CheckedValueOrigin::StateStatement {
            machine_symbol: source_key.machine,
            state_symbol: source_key.state,
            statement_index: 6,
            role: CheckedValueStatementRole::AssignmentValue,
        };
        let span = SourceSpan::new(SourceId(8), Span::new(200, 203));
        let mut program = psi_checked_trees::CheckedTrees::default();
        let first = checked_binary_with_span(
            &mut program,
            psi_checked_trees::expression::BinaryOperator::Add,
            span,
        );
        let duplicate = checked_binary_with_span(
            &mut program,
            psi_checked_trees::expression::BinaryOperator::Add,
            span,
        );
        let other_shape = checked_binary_with_span(
            &mut program,
            psi_checked_trees::expression::BinaryOperator::Subtract,
            span,
        );
        for expression in [other_shape, first, duplicate] {
            program.facts.operators.uses.insert(CheckedOperatorUseFact {
                expression,
                origin,
                provider_plan_identity: 23,
                ..CheckedOperatorUseFact::default()
            });
        }

        let mut realized = ExpressionTable::new();
        let realized_expression = realized.copy_from(&program.expression_table, first);
        assert_eq!(
            canonical_checked_operator_expression(
                &program,
                source_key,
                6,
                &realized,
                realized_expression,
            ),
            CanonicalOperatorExpression::Resolved {
                expression: first,
                origin,
            }
        );
    }

    #[test]
    fn exact_operator_span_rejects_duplicate_identity_policy_or_origin_contradictions() {
        fn resolve_duplicates(
            second_identity: u64,
            second_policy: psi_checked_trees::CheckedArithmeticPolicyAdapter,
            second_origin: psi_checked_trees::CheckedValueOrigin,
        ) -> CanonicalOperatorExpression {
            let source_key = StateKey {
                machine: SymbolHandle::from_arena_index(61),
                state: SymbolHandle::from_arena_index(62),
                segment_index: 0,
            };
            let origin = psi_checked_trees::CheckedValueOrigin::StateStatement {
                machine_symbol: source_key.machine,
                state_symbol: source_key.state,
                statement_index: 7,
                role: CheckedValueStatementRole::AssignmentValue,
            };
            let span = SourceSpan::new(SourceId(9), Span::new(300, 303));
            let mut program = psi_checked_trees::CheckedTrees::default();
            let first = checked_binary_with_span(
                &mut program,
                psi_checked_trees::expression::BinaryOperator::Subtract,
                span,
            );
            let duplicate = checked_binary_with_span(
                &mut program,
                psi_checked_trees::expression::BinaryOperator::Subtract,
                span,
            );
            program.facts.operators.uses.insert(CheckedOperatorUseFact {
                expression: first,
                origin,
                provider_plan_identity: 29,
                ..CheckedOperatorUseFact::default()
            });
            program.facts.operators.uses.insert(CheckedOperatorUseFact {
                expression: duplicate,
                origin: second_origin,
                provider_plan_identity: second_identity,
                policy_adapter: second_policy,
                ..CheckedOperatorUseFact::default()
            });
            let mut realized = ExpressionTable::new();
            let realized_expression = realized.copy_from(&program.expression_table, first);
            canonical_checked_operator_expression(
                &program,
                source_key,
                7,
                &realized,
                realized_expression,
            )
        }

        let origin = psi_checked_trees::CheckedValueOrigin::StateStatement {
            machine_symbol: SymbolHandle::from_arena_index(61),
            state_symbol: SymbolHandle::from_arena_index(62),
            statement_index: 7,
            role: CheckedValueStatementRole::AssignmentValue,
        };
        assert_eq!(
            resolve_duplicates(
                30,
                psi_checked_trees::CheckedArithmeticPolicyAdapter::None,
                origin,
            ),
            CanonicalOperatorExpression::Invalid,
        );
        assert_eq!(
            resolve_duplicates(
                29,
                psi_checked_trees::CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite {
                    format: psi_numerics::float_semantics::FloatFormat::BINARY64,
                },
                origin,
            ),
            CanonicalOperatorExpression::Invalid,
        );
        assert_eq!(
            resolve_duplicates(
                29,
                psi_checked_trees::CheckedArithmeticPolicyAdapter::None,
                psi_checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol: SymbolHandle::from_arena_index(61),
                    state_symbol: SymbolHandle::from_arena_index(62),
                    statement_index: 8,
                    role: CheckedValueStatementRole::AssignmentValue,
                },
            ),
            CanonicalOperatorExpression::Invalid,
        );
    }
}

/// Resolve one float operation's already-checked result adapter from the
/// control-flow value spine. This deliberately does not inspect operand types:
/// missing evidence, format mismatch, or contradictory carried facts fail
/// closed at the consumer.
pub(super) fn carried_float_policy_domain(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
    byte_width: usize,
) -> CarriedFloatPolicyDomain {
    let canonical = canonical_checked_operator_expression(
        input.program,
        source_key,
        statement_index,
        expressions,
        expression,
    );
    match canonical {
        CanonicalOperatorExpression::Resolved { expression, origin } => {
            let Some((source_key, statement_index)) = operator_origin_key(origin) else {
                return CarriedFloatPolicyDomain::Invalid;
            };
            return carried_float_policy_domain_for_canonical(
                input,
                source_key,
                statement_index,
                expression,
                byte_width,
            );
        }
        CanonicalOperatorExpression::Missing => return CarriedFloatPolicyDomain::Missing,
        CanonicalOperatorExpression::Invalid => return CarriedFloatPolicyDomain::Invalid,
    }
}

fn carried_float_policy_domain_for_canonical(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    expression: ExpressionHandle,
    byte_width: usize,
) -> CarriedFloatPolicyDomain {
    let Some(state) = input.control_flow.state_by_key(source_key).or_else(|| {
        input
            .control_flow
            .states
            .iter()
            .find(|(_, state)| state_key_matches_statement_source(state.key, source_key))
            .map(|(_, state)| state)
    }) else {
        return CarriedFloatPolicyDomain::Missing;
    };
    let expected_format = match byte_width {
        4 => Some(FloatFormat::BINARY32),
        8 => Some(FloatFormat::BINARY64),
        _ => None,
    };
    let mut resolved = None;

    for value in input
        .control_flow
        .semantics
        .values
        .values
        .span_or_empty(state.values.values)
    {
        let omega_control_flow::StateValueOrigin::Statement {
            statement_index: value_statement_index,
            ..
        } = value.origin;
        if value_statement_index != statement_index || value.expression != expression {
            continue;
        }
        let Some(adapter) = value.arithmetic_policy_adapter else {
            continue;
        };
        let domain = match adapter {
            ArithmeticPolicyAdapter::None => ArithmeticDomain::Exact,
            ArithmeticPolicyAdapter::FloatSaturatingOverflowOnly { format } => {
                if Some(format) != expected_format {
                    return CarriedFloatPolicyDomain::Invalid;
                }
                ArithmeticDomain::Saturating
            }
            ArithmeticPolicyAdapter::FloatTrappingNonFinite { format } => {
                if Some(format) != expected_format {
                    return CarriedFloatPolicyDomain::Invalid;
                }
                ArithmeticDomain::Trapping
            }
        };
        match resolved {
            Some(existing) if existing != domain => {
                return CarriedFloatPolicyDomain::Invalid;
            }
            None => resolved = Some(domain),
            _ => {}
        }
    }

    resolved.map_or(
        CarriedFloatPolicyDomain::Missing,
        CarriedFloatPolicyDomain::Resolved,
    )
}

pub(super) fn host_call_for_statement<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan HostCall> {
    input
        .host_calls
        .calls
        .iter()
        .find(|(_, host_call)| {
            state_key_matches_statement_source(host_call.source_key, source_key)
                && host_call.statement_index == statement_index
        })
        .map(|(_, host_call)| host_call)
}

pub(super) fn state_call_for_statement<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan StateCall> {
    input
        .state_calls
        .statement_call(source_key, statement_index)
}

pub(super) fn state_assignment_value_call<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan StateCall> {
    input
        .state_calls
        .assignment_value_call(source_key, statement_index)
}

pub(super) fn state_assignment_value_call_by_ordinal<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<&'plan StateCall> {
    input
        .state_calls
        .assignment_value_call_by_ordinal(source_key, statement_index, call_ordinal)
}

pub(super) fn state_call_argument_call_by_ordinal<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<&'plan StateCall> {
    input
        .state_calls
        .call_argument_call_by_ordinal(source_key, statement_index, call_ordinal)
}

pub(super) fn state_transition_guard_call<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan StateCall> {
    input
        .state_calls
        .transition_guard_call(source_key, statement_index)
}

pub(super) fn state_transition_argument_call<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan StateCall> {
    input
        .state_calls
        .transition_argument_call(source_key, statement_index)
}

pub(super) fn state_transition_argument_call_by_ordinal<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<&'plan StateCall> {
    input
        .state_calls
        .transition_argument_call_by_ordinal(source_key, statement_index, call_ordinal)
}

pub(super) fn state_parameters<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    state_key: StateKey,
) -> &'plan [StateParameterFlow] {
    input
        .control_flow
        .state_by_key(state_key)
        .map(|state| input.control_flow.state_parameters(state))
        .unwrap_or(&[])
}

pub(super) fn state_operations<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    state_key: StateKey,
) -> Option<&'plan [Operation]> {
    input
        .control_flow
        .state_by_key(state_key)
        .and_then(|state| input.control_flow.operations.span(state.operations))
}

/// The machine a method call's receiver statically dispatches to, when the
/// receiver's concrete data type is derivable from the receiver EXPRESSION alone:
/// a reference parameter (`s.code()` -- the param symbol's declared type) or a
/// field of an attached data record (`self.c.code()`, including an alias- or
/// binding-resolved `(mut self.c).code()` -- the contained object's type).
/// `None` when the type cannot be derived (e.g. a still-`dyn Trait` receiver), so
/// callers keep their unfiltered candidate set.
///
/// This is the discriminator that keeps SAME-NAMED methods on DIFFERENT data
/// types apart: leaf-expansion candidates are matched by target state NAME
/// (`code`), which both `Circle::code` and `Square::code` answer to -- without
/// the receiver's type the lexically-first impl would win at every call site.
pub(super) fn static_receiver_machine_for_call(
    input: &InstructionSelectionInput<'_>,
    receiver: Option<&Expression>,
    target_symbol: SymbolHandle,
    target_name: &str,
) -> Option<SymbolHandle> {
    let type_name = static_receiver_type_name(input, receiver?)?;
    attached_machine_with_target_state(input, &type_name, target_symbol, target_name)
}

/// Table-expression twin of [`static_receiver_machine_for_call`].
pub(super) fn static_receiver_machine_for_table_call(
    input: &InstructionSelectionInput<'_>,
    expressions: &ExpressionTable,
    receiver: ExpressionHandle,
    target_symbol: SymbolHandle,
    target_name: &str,
) -> Option<SymbolHandle> {
    if !receiver.is_valid() {
        return None;
    }
    let type_name = static_table_receiver_type_name(input, expressions, receiver)?;
    attached_machine_with_target_state(input, &type_name, target_symbol, target_name)
}

/// The static data type NAME a receiver expression resolves to, keyed on the
/// SYMBOL it names (program-unique, so no source-state context is needed): a
/// state parameter's declared type, or a contained object's type (an
/// attached-data field whose type has an attached machine).
fn static_receiver_type_name(
    input: &InstructionSelectionInput<'_>,
    receiver: &Expression,
) -> Option<Identifier> {
    match receiver {
        Expression::Mutable(inner) => static_receiver_type_name(input, inner),
        Expression::Member(member) => receiver_symbol_type_name(input, member.member_symbol),
        Expression::Name(path) => {
            let symbol = if path.symbol().is_valid() {
                path.symbol()
            } else {
                path.head_symbol()
            };
            receiver_symbol_type_name(input, symbol)
        }
        _ => None,
    }
}

fn static_table_receiver_type_name(
    input: &InstructionSelectionInput<'_>,
    expressions: &ExpressionTable,
    receiver: ExpressionHandle,
) -> Option<Identifier> {
    match expressions.expression(receiver) {
        ExpressionNode::Mutable(inner) => {
            static_table_receiver_type_name(input, expressions, *inner)
        }
        ExpressionNode::Member(member) => receiver_symbol_type_name(input, member.member_symbol),
        ExpressionNode::Name(path) => {
            let symbol = if path.symbol.is_valid() {
                path.symbol
            } else {
                path.head_symbol
            };
            receiver_symbol_type_name(input, symbol)
        }
        _ => None,
    }
}

fn receiver_symbol_type_name(
    input: &InstructionSelectionInput<'_>,
    symbol: SymbolHandle,
) -> Option<Identifier> {
    if !symbol.is_valid() {
        return None;
    }
    let parameter_type = input.control_flow.states.iter().find_map(|(_, state)| {
        input
            .control_flow
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == symbol)
            .map(|parameter| parameter.type_name.clone())
    });
    parameter_type.or_else(|| {
        input.control_flow.machines.iter().find_map(|(_, machine)| {
            input
                .control_flow
                .machine_contains(machine)
                .iter()
                .find(|contained| contained.symbol == symbol)
                .map(|contained| contained.type_name.clone())
        })
    })
}

/// The machine ATTACHED to data type `type_name` that implements the call's
/// target method (matched by state symbol or name), or `None`.
fn attached_machine_with_target_state(
    input: &InstructionSelectionInput<'_>,
    type_name: &Identifier,
    target_symbol: SymbolHandle,
    target_name: &str,
) -> Option<SymbolHandle> {
    let candidates = input
        .control_flow
        .machines
        .iter()
        .filter_map(|(_, machine)| {
            (machine.attached_data.as_ref() == Some(type_name)).then_some(machine)
        })
        .collect::<Vec<_>>();
    if target_symbol.is_valid()
        && let Some(exact) = candidates.iter().find_map(|machine| {
            input
                .control_flow
                .states
                .span(machine.states)?
                .iter()
                .any(|state| state.key.state == target_symbol)
                .then_some(machine.symbol)
        })
    {
        return Some(exact);
    }
    candidates.into_iter().find_map(|machine| {
        input
            .control_flow
            .states
            .span(machine.states)?
            .iter()
            .any(|state| state.name.as_str() == target_name)
            .then_some(machine.symbol)
    })
}

pub(super) fn state_mutation_for_statement<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan StateMutation> {
    input
        .state_storage
        .mutations
        .iter()
        .find(|(_, mutation)| {
            state_key_matches_statement_source(mutation.source_key, source_key)
                && mutation.statement_index == statement_index
        })
        .map(|(_, mutation)| mutation)
}

/// The checked statement node at `(source_key, statement_index)`.
fn checked_statement_node<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan psi_checked_trees::statement::StatementNode> {
    let machine = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == source_key.machine)?;
    let state = input
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == source_key.state)?;
    input
        .program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)
}

/// The `(port, value)` argument expressions of an `asm { out .. }` statement (a
/// Call to the unnameable `asm#port_out`). The args come from the (unresolved)
/// state-call record so they resolve against `input.state_calls.expressions` --
/// the same table the state/host-call argument machinery uses -- rather than
/// the raw checked program table (whose handles the operand resolver cannot
/// map to storage places).
pub(super) fn asm_port_write_operands<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<(ExpressionHandle, ExpressionHandle)> {
    // Confirm the statement really is the asm#port_out intrinsic.
    let psi_checked_trees::statement::StatementNode::Call(call) =
        checked_statement_node(input, source_key, statement_index)?
    else {
        return None;
    };
    if call.target.as_str() != "asm#port_out" {
        return None;
    }
    let state_call = state_call_for_statement(input, source_key, statement_index)?;
    let arguments = input.state_calls.arguments.span(state_call.arguments)?;
    Some((arguments.first()?.expression, arguments.get(1)?.expression))
}

/// The `(port_expr, destination_place_expr)` of an `asm { in <dest>, <port> }`
/// statement -- an assignment whose value is the `asm#port_in` call. Both
/// expressions are into `input.state_storage.expressions` (the mutation's
/// table).
pub(super) fn asm_port_read_operands<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<(ExpressionHandle, ExpressionHandle)> {
    let mutation = state_mutation_for_statement(input, source_key, statement_index)?;
    let ExpressionNode::Call(call) = input.state_storage.expressions.expression(mutation.value)
    else {
        return None;
    };
    if call.target.as_str() != "asm#port_in" {
        return None;
    }
    let port = *input
        .state_storage
        .expressions
        .expression_handles(call.arguments)
        .first()?;
    Some((port, mutation.target))
}

/// Destination place of `asm { pushfq <dest> }`, represented as an assignment
/// whose value is the unnameable zero-argument snapshot intrinsic.
pub(super) fn asm_flags_snapshot_destination(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<ExpressionHandle> {
    let mutation = state_mutation_for_statement(input, source_key, statement_index)?;
    let ExpressionNode::Call(call) = input.state_storage.expressions.expression(mutation.value)
    else {
        return None;
    };
    (call.target.as_str() == "asm#pushfq").then_some(mutation.target)
}

/// Source value of `asm { popfq <saved> }`, represented as the sole argument
/// of the unnameable restore intrinsic.
pub(super) fn asm_flags_restore_source(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<ExpressionHandle> {
    let psi_checked_trees::statement::StatementNode::Call(call) =
        checked_statement_node(input, source_key, statement_index)?
    else {
        return None;
    };
    if call.target.as_str() != "asm#popfq" {
        return None;
    }
    let state_call = state_call_for_statement(input, source_key, statement_index)?;
    Some(
        input
            .state_calls
            .arguments
            .span(state_call.arguments)?
            .first()?
            .expression,
    )
}

/// `(index, destination)` for `asm { rdmsr <dest>, <index> }`.
pub(super) fn asm_msr_read_operands(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<(ExpressionHandle, ExpressionHandle)> {
    let mutation = state_mutation_for_statement(input, source_key, statement_index)?;
    let ExpressionNode::Call(call) = input.state_storage.expressions.expression(mutation.value)
    else {
        return None;
    };
    if call.target.as_str() != "asm#rdmsr" {
        return None;
    }
    let index = *input
        .state_storage
        .expressions
        .expression_handles(call.arguments)
        .first()?;
    Some((index, mutation.target))
}

/// `(index, value)` for `asm { wrmsr <index>, <value> }`.
pub(super) fn asm_msr_write_operands(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<(ExpressionHandle, ExpressionHandle)> {
    let psi_checked_trees::statement::StatementNode::Call(call) =
        checked_statement_node(input, source_key, statement_index)?
    else {
        return None;
    };
    if call.target.as_str() != "asm#wrmsr" {
        return None;
    }
    let state_call = state_call_for_statement(input, source_key, statement_index)?;
    let arguments = input.state_calls.arguments.span(state_call.arguments)?;
    Some((arguments.first()?.expression, arguments.get(1)?.expression))
}

pub(super) fn asm_control_register_read_destination(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<(
    psi_language_core::inline_assembly::AsmControlRegister,
    ExpressionHandle,
)> {
    let mutation = state_mutation_for_statement(input, source_key, statement_index)?;
    let ExpressionNode::Call(call) = input.state_storage.expressions.expression(mutation.value)
    else {
        return None;
    };
    let register =
        psi_language_core::inline_assembly::AsmControlRegister::from_read_intrinsic_name(
            call.target.as_str(),
        )?;
    Some((register, mutation.target))
}

pub(super) fn asm_control_register_write_source(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<(
    psi_language_core::inline_assembly::AsmControlRegister,
    ExpressionHandle,
)> {
    let psi_checked_trees::statement::StatementNode::Call(call) =
        checked_statement_node(input, source_key, statement_index)?
    else {
        return None;
    };
    let register =
        psi_language_core::inline_assembly::AsmControlRegister::from_write_intrinsic_name(
            call.target.as_str(),
        )?;
    let state_call = state_call_for_statement(input, source_key, statement_index)?;
    let source = input
        .state_calls
        .arguments
        .span(state_call.arguments)?
        .first()?
        .expression;
    Some((register, source))
}
