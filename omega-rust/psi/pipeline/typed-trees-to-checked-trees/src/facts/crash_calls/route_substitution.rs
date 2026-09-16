//! Call argument substitution into published crash routes.

use crate::facts::crash_calls::crash_predicate_from_expression;
use crate::facts::crash_calls::private_summaries::crash_route_expressions_by_identity;
use crate::facts::crash_calls::summary_predicates::{
    CallArgumentSubstitution, SummaryCrashBucket, SummaryCrashPredicate, SummaryCrashRouteGuard,
    concrete_guard_scalar_value, normalize_summary_buckets, normalize_summary_guards,
    scalar_guard_is_integer_comparison, summary_boolean_value,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

pub(crate) enum SelectedTargetCrashRoutes<'a> {
    Published {
        buckets: &'a [checked_trees::CrashRouteBucket],
        contracts: &'a [typed_trees::signature::SignatureContract],
    },
    Private(&'a [SummaryCrashBucket]),
    Empty,
}

pub(crate) fn call_argument_substitution(
    program: &TypedTrees,
    operators: &checked_trees::CheckedOperatorFacts,
    target_parameters: &[typed_trees::signature::StateParameter],
    arguments: &[typed_trees::expression::ExpressionHandle],
    caller_state: SymbolHandle,
    before_statement: usize,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> CallArgumentSubstitution {
    // Both direct published routes and private/transitive summaries cross the
    // same namespace boundary. Source spelling and statement position cannot
    // prove that a current actual still denotes the caller's entry value.
    // Retain exact immutable entry inputs/literals; unsupported current-value
    // provenance leaves a missing substitution, which widens the cause below.
    let owner = program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == caller_state)
            .map(|state| (machine, state))
    });
    let mut argument_index = 0usize;
    let mut identity = Vec::with_capacity(target_parameters.len());
    let mut scalar = Vec::with_capacity(target_parameters.len());
    for parameter in target_parameters {
        if parameter.is_self {
            // Receiver-entry identity needs retained referent custody. A name
            // alone must not impersonate a caller entry value.
            identity.push(None);
            continue;
        }
        let argument = arguments.get(argument_index).copied();
        argument_index = argument_index.saturating_add(1);
        let entry_identity = argument.and_then(|argument| {
            let (machine, state) = owner?;
            crate::facts::crash_entry_values::entry_operand(
                program,
                machine.symbol,
                state.symbol,
                before_statement,
                argument,
            )
        });
        // Scalar annotations live in the dense primitive namespace: only
        // primitive-typed formals occupy a slot, regardless of aggregate
        // parameters or receivers before them. Checked scalar evidence is
        // retained independently of entry custody so arithmetic actuals and
        // unprovable origins can still discharge once they become concrete.
        if let Some(expected) = program.primitive_type_reference(parameter.type_reference) {
            scalar.push(argument.and_then(|argument| {
                crate::values::lower_state_scalar_expression(
                    program,
                    operators,
                    owner?.1,
                    before_statement,
                    argument,
                    expected,
                    exact_integer_casts,
                )
            }));
        }
        identity.push(entry_identity);
    }
    CallArgumentSubstitution { identity, scalar }
}

pub(crate) fn substitute_checked_boolean_expression(
    expression: &checked_trees::CheckedBooleanExpression,
    arguments: &[Option<checked_trees::CheckedScalarExpression>],
) -> Option<checked_trees::CheckedBooleanExpression> {
    use checked_trees::{CheckedBooleanExpression, CheckedScalarExpression};

    Some(match expression {
        CheckedBooleanExpression::Constant(value) => CheckedBooleanExpression::Constant(*value),
        CheckedBooleanExpression::Parameter { position } => {
            let CheckedScalarExpression::Boolean(expression) =
                arguments.get(*position)?.as_ref()?.clone()
            else {
                return None;
            };
            *expression
        }
        // A callee contract is parameter-relative. A local can appear only
        // after composing a private body summary; it cannot be rebound by the
        // outer call and therefore deliberately loses portable structure.
        CheckedBooleanExpression::Local { .. } | CheckedBooleanExpression::StorageRead { .. } => {
            return None;
        }
        CheckedBooleanExpression::StructuralParameterField { .. }
        | CheckedBooleanExpression::IeeeFloatComparison { .. }
        | CheckedBooleanExpression::ByteSequenceEqual { .. }
        | CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | CheckedBooleanExpression::StructuralCaseMembership { .. } => return None,
        CheckedBooleanExpression::Not(operand) => CheckedBooleanExpression::Not(Box::new(
            substitute_checked_boolean_expression(operand, arguments)?,
        )),
        CheckedBooleanExpression::Equal { left, right } => CheckedBooleanExpression::Equal {
            left: Box::new(substitute_checked_boolean_expression(left, arguments)?),
            right: Box::new(substitute_checked_boolean_expression(right, arguments)?),
        },
        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            CheckedBooleanExpression::IntegerComparison {
                kind: *kind,
                left: Box::new(substitute_checked_scalar_expression(left, arguments)?),
                right: Box::new(substitute_checked_scalar_expression(right, arguments)?),
            }
        }
        CheckedBooleanExpression::And { left, right } => CheckedBooleanExpression::And {
            left: Box::new(substitute_checked_boolean_expression(left, arguments)?),
            right: Box::new(substitute_checked_boolean_expression(right, arguments)?),
        },
        CheckedBooleanExpression::Or { left, right } => CheckedBooleanExpression::Or {
            left: Box::new(substitute_checked_boolean_expression(left, arguments)?),
            right: Box::new(substitute_checked_boolean_expression(right, arguments)?),
        },
    })
}

fn substitute_checked_scalar_expression(
    expression: &checked_trees::CheckedScalarExpression,
    arguments: &[Option<checked_trees::CheckedScalarExpression>],
) -> Option<checked_trees::CheckedScalarExpression> {
    use checked_trees::CheckedScalarExpression;

    Some(match expression {
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => {
            let substituted = arguments.get(*position)?.as_ref()?.clone();
            (crate::values::scalar_expression_type(&substituted) == Some(*primitive_type))
                .then_some(substituted)?
        }
        CheckedScalarExpression::Local { .. }
        | CheckedScalarExpression::StorageRead { .. }
        | CheckedScalarExpression::StructuralParameterField { .. }
        | CheckedScalarExpression::StructuralParameterByteLength { .. }
        | CheckedScalarExpression::IntegerTrappingCast { .. }
        | CheckedScalarExpression::IntegerWrappingCast { .. }
        | CheckedScalarExpression::StructuralParameterIndexedRead { .. } => return None,
        CheckedScalarExpression::IntegerLiteral { literal } => {
            CheckedScalarExpression::IntegerLiteral {
                literal: literal.clone(),
            }
        }
        CheckedScalarExpression::IeeeFloatLiteral { value } => {
            CheckedScalarExpression::IeeeFloatLiteral { value: *value }
        }
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
        } => CheckedScalarExpression::IntegerBinary {
            kind: *kind,
            primitive_type: *primitive_type,
            left: Box::new(substitute_checked_scalar_expression(left, arguments)?),
            right: Box::new(substitute_checked_scalar_expression(right, arguments)?),
        },
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } => CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type: *primitive_type,
            operand: Box::new(substitute_checked_scalar_expression(operand, arguments)?),
        },
        CheckedScalarExpression::IntegerWiden {
            primitive_type,
            operand,
        } => CheckedScalarExpression::IntegerWiden {
            primitive_type: *primitive_type,
            operand: Box::new(substitute_checked_scalar_expression(operand, arguments)?),
        },
        CheckedScalarExpression::IntegerExactCast {
            primitive_type,
            operand,
            range,
        } => CheckedScalarExpression::IntegerExactCast {
            primitive_type: *primitive_type,
            operand: Box::new(substitute_checked_scalar_expression(operand, arguments)?),
            range: range.clone(),
        },
        CheckedScalarExpression::Boolean(expression) => CheckedScalarExpression::Boolean(Box::new(
            substitute_checked_boolean_expression(expression, arguments)?,
        )),
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn refine_published_crash_routes(
    program: &TypedTrees,
    operators: &checked_trees::CheckedOperatorFacts,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
    state_flow: &checked_trees::FlowStateFact,
    call_flow: &checked_trees::FlowCallFact,
    call_site: &crate::semantic_calls::CallSite<'_>,
    target_state_symbol: SymbolHandle,
    target_parameters: &[typed_trees::signature::StateParameter],
    target_parameter_names: &[String],
    buckets: &[checked_trees::CrashRouteBucket],
    contracts: &[typed_trees::signature::SignatureContract],
    content_conservation: &[validation::ContentConservationSourcePlan],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Vec<SummaryCrashBucket> {
    let route_expressions = crash_route_expressions_by_identity(
        program,
        contracts,
        target_parameter_names,
        content_conservation,
    );
    let arguments = crate::semantic_calls::call_site_argument_expressions(program, call_site);
    let substitution = call_argument_substitution(
        program,
        operators,
        target_parameters,
        arguments,
        state_flow.state_symbol,
        call_flow.statement_index,
        exact_integer_casts,
    );
    let mut surviving = Vec::new();
    for bucket in buckets {
        let mut guards = Vec::new();
        for guard in bucket.alternative_guards() {
            match guard {
                checked_trees::CrashRouteGuard::Truth => {
                    guards.push(SummaryCrashRouteGuard::Truth);
                }
                checked_trees::CrashRouteGuard::Predicate(identity) => {
                    let expression = *route_expressions.get(identity).expect(
                        "a canonical published crash route retains its typed producer expression",
                    );
                    // The token-only predicate identity does not select operator
                    // meaning. Retain authority only from this exact local owner.
                    let builtin_meaning = program
                        .machines()
                        .iter()
                        .find_map(|machine| {
                            program
                                .machine_states(machine)
                                .iter()
                                .find(|state| state.symbol == target_state_symbol)
                                .map(|state| {
                                    validation::has_builtin_bound_expression_meaning(
                                        program,
                                        machine,
                                        Some(state),
                                        expression,
                                    )
                                })
                        })
                        .unwrap_or(false);
                    let predicate = crash_predicate_from_expression(
                        program,
                        expression,
                        target_parameter_names,
                        Some(content_conservation),
                    );
                    // The direct evaluator follows actuals and local initializers.
                    // Only referenced formals with retained immutable entry origins
                    // may enter it; callee meaning cannot authorize caller operators.
                    let entry_predicate = crate::facts::crash_entry_values::substitute_entry(
                        &predicate,
                        &substitution.identity,
                    );
                    let concrete_value = (builtin_meaning && entry_predicate.is_some())
                        .then(|| {
                            crate::checks::contracts::call_site_boolean_contract_expression_value(
                                program,
                                state_flow,
                                call_flow,
                                call_site,
                                target_state_symbol,
                                target_parameters,
                                expression,
                                call_frames,
                            )
                        })
                        .flatten();
                    match concrete_value {
                        Some(false) => {}
                        Some(true) => guards.push(SummaryCrashRouteGuard::Truth),
                        None => {
                            let Some(predicate) = entry_predicate else {
                                guards.push(SummaryCrashRouteGuard::Truth);
                                continue;
                            };
                            let scalar = identity.scalar_expression().and_then(|scalar| {
                                substitute_checked_boolean_expression(scalar, &substitution.scalar)
                            });
                            let folded = if builtin_meaning {
                                summary_boolean_value(&predicate)
                            } else {
                                predicate.boolean_value()
                            };
                            // Domain-free folding stands down on arithmetic
                            // operands: a checked integer-comparison
                            // annotation decides them under their selected
                            // domains once every actual is concrete.
                            let value = folded.or_else(|| {
                                (builtin_meaning
                                    && scalar
                                        .as_ref()
                                        .is_some_and(scalar_guard_is_integer_comparison))
                                .then(|| scalar.as_ref().and_then(concrete_guard_scalar_value))
                                .flatten()
                            });
                            match value {
                                Some(false) => {}
                                Some(true) => guards.push(SummaryCrashRouteGuard::Truth),
                                None => guards.push(SummaryCrashRouteGuard::Predicate(
                                    SummaryCrashPredicate {
                                        identity: predicate,
                                        builtin_meaning,
                                        scalar,
                                    },
                                )),
                            }
                        }
                    }
                }
            }
        }
        normalize_summary_guards(&mut guards);
        if !guards.is_empty() {
            surviving.push(SummaryCrashBucket {
                cause: bucket.cause(),
                alternative_guards: guards,
            });
        }
    }
    normalize_summary_buckets(surviving)
}
