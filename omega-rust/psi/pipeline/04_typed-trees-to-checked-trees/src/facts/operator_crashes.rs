//! Selected operator invocations have their own occurrence and operand custody.
//! Their source crash routes never acquire fabricated ordinary-call coordinates.

use super::crash_entry_values::{
    entry_operand_projected, formal_member_projection, substitute_entry_projected,
};

use checked_trees::{
    CheckedCrashOperatorSite, CheckedOperatorFacts, CheckedOperatorOccurrence,
    CheckedOperatorResolutionStatus, CheckedValueOrigin, CrashPredicateIdentity, CrashRouteBucket,
    CrashRouteGuard, FlowFacts,
};
use diagnostics::Diagnostic;
use facts::FactPlan;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, MatchPattern};
use typed_trees::signature::SignatureContractKind;
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceNode;

mod named_routes;

#[cfg(test)]
mod tests;

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
        let (published, surviving) = retained_operator_crash_routes(
            program,
            operators,
            flow,
            semantic,
            operator_use_handle,
            arena::Handle::invalid(),
            program
                .signature_contracts
                .span_or_empty(selected.contracts),
            operator,
            parameters,
            &operands,
            machine_symbol,
            state_symbol,
            statement_index,
            content_conservation,
        );
        sites.push((
            machine_symbol,
            CheckedCrashOperatorSite {
                operator_use: operator_use_handle,
                invocation: invocation_handle,
                named_use: arena::Handle::invalid(),
                selected_operator: selected.operator_symbol,
                published,
                surviving,
            },
        ));
    }
    // Named `Namespace::requirement(...)` calls carry the same selected crash
    // obligations as spelled uses. They have no `uses` row, so their identity
    // is the `named_uses` handle; the flow pass records their operand-time
    // capture under that same handle, and route discharge prefers it over the
    // containing statement's entry contexts (see named_routes).
    for (named_use_handle, named_use) in operators.named_uses.iter() {
        let Some(operator) = typed_trees::operator::declaration_by_symbol(
            program,
            named_use.selected_operator_symbol,
        ) else {
            continue;
        };
        let contracts = program
            .signature_contracts
            .span_or_empty(operator.contracts);
        if !contracts
            .iter()
            .any(|contract| matches!(contract.kind, SignatureContractKind::Crashes { .. }))
        {
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
        } = named_use.origin
        else {
            diagnostics.push(invalid());
            continue;
        };
        let ExpressionNode::Call(call) = program.expression_table.expression(named_use.expression)
        else {
            diagnostics.push(invalid());
            continue;
        };
        let parameters = program.operator_parameters(operator);
        let Some(operands) = named_call_operands(program, call, parameters) else {
            diagnostics.push(invalid());
            continue;
        };
        let content_conservation = content_conservation
            .get_or_insert_with(|| validation::build_content_conservation_plans(program));
        let (published, surviving) = retained_operator_crash_routes(
            program,
            operators,
            flow,
            semantic,
            arena::Handle::invalid(),
            named_use_handle,
            contracts,
            operator,
            parameters,
            &operands,
            machine_symbol,
            state_symbol,
            statement_index,
            content_conservation,
        );
        sites.push((
            machine_symbol,
            CheckedCrashOperatorSite {
                operator_use: arena::Handle::invalid(),
                invocation: arena::Handle::invalid(),
                named_use: named_use_handle,
                selected_operator: named_use.selected_operator_symbol,
                published,
                surviving,
            },
        ));
    }
    // A `StatementNode::Call` that still resolves to an operator mints no
    // `named_uses` row — only return-typed selections are rewritten into the
    // `LocalData` expression form operand capture sees. A surviving statement
    // call to a crash-contracted operator — a declaration with no result
    // type, or a requirement the rewrite does not select — would pass with
    // its routes unchecked; it has no site to retain, so admit nothing.
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
            {
                let StatementNode::Call(call) = statement else {
                    continue;
                };
                let Some(operator) = crate::flow::resolved_operator_statement_symbol(program, call)
                    .and_then(|symbol| {
                        typed_trees::operator::declaration_by_symbol(program, symbol)
                    })
                else {
                    continue;
                };
                if !program
                    .signature_contracts
                    .span_or_empty(operator.contracts)
                    .iter()
                    .any(|contract| matches!(contract.kind, SignatureContractKind::Crashes { .. }))
                {
                    continue;
                }
                diagnostics.push(Diagnostic::error(format!(
                    "statement call to named operator `{}` carries crash routes no checked site can cover",
                    program
                        .operator_path_members(operator.name)
                        .iter()
                        .map(|member| member.as_str())
                        .collect::<Vec<_>>()
                        .join("::"),
                )));
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(sites)
    } else {
        Err(diagnostics)
    }
}

/// Published and surviving crash-route buckets for one selected use. A spelled
/// use discharges each guard the captured invocation contexts prove false. A
/// named call passes an invalid `operator_use` and a valid `named_use`: its
/// own operand-time capture row serves the same `InvocationContexts`
/// discharge, falling back to the containing statement's entry contexts —
/// gated on every leaf occurrence resolving to an entry-proven operand — only
/// for uses whose evaluation position emitted no capture (see
/// `named_routes`). Anything unproven stays conservative.
#[allow(clippy::too_many_arguments)]
fn retained_operator_crash_routes(
    program: &TypedTrees,
    operators: &CheckedOperatorFacts,
    flow: &FlowFacts,
    semantic: &FactPlan,
    operator_use: arena::Handle<checked_trees::CheckedOperatorUseFact>,
    named_use: arena::Handle<checked_trees::CheckedNamedOperatorUseFact>,
    contracts: &[typed_trees::signature::SignatureContract],
    operator: &typed_trees::operator::OperatorDefinition,
    parameters: &[typed_trees::signature::StateParameter],
    operands: &[ExpressionHandle],
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    content_conservation: &[validation::ContentConservationSourcePlan],
) -> (Vec<CrashRouteBucket>, Vec<CrashRouteBucket>) {
    let published = super::derive_authored_operator_crash_buckets(
        program,
        operator,
        operators,
        content_conservation,
    );
    let parameter_names = parameters
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect::<Vec<_>>();
    // A `Parameter` leaf below a member spine asks only for the operand's
    // provenance at that projection: a mutable carrier written only in a
    // sibling field still supplies the read field's entry operand, so the
    // surviving route names the caller's `rec.count` rather than widening to
    // `Truth`. A bare `Parameter` keeps the whole-operand boundary.
    let mut resolve_entry = |ordinal: u32, members: &[String]| {
        let operand = operands.get(ordinal as usize).copied()?;
        let parameter = parameters.get(ordinal as usize)?;
        let projection = formal_member_projection(program, parameter.type_reference, members);
        entry_operand_projected(
            program,
            machine_symbol,
            state_symbol,
            statement_index,
            operand,
            &projection,
        )
    };
    let mut surviving = Vec::new();
    for bucket in &published {
        let mut guards = Vec::new();
        for guard in bucket.alternative_guards() {
            let CrashRouteGuard::Predicate(predicate) = guard else {
                guards.push(CrashRouteGuard::Truth);
                continue;
            };
            let expression = contracts
                .iter()
                .filter(|contract| matches!(contract.kind, SignatureContractKind::Crashes { .. }))
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
            // A spelled use consults the operand-captured invocation
            // contexts; a named call consults its own capture row when the
            // evaluation emitted one, else proves falsity from the containing
            // statement's entry contexts — sound only while every place the
            // leaf reads is an entry-proven operand.
            if expression.is_some_and(|expression| {
                if operator_use.is_valid() {
                    crate::checks::operator_route_is_false(
                        program,
                        flow,
                        semantic,
                        operator_use,
                        parameters,
                        operands,
                        expression,
                    )
                } else {
                    named_routes::named_route_is_false(
                        program,
                        flow,
                        semantic,
                        named_use,
                        machine_symbol,
                        state_symbol,
                        statement_index,
                        parameters,
                        operands,
                        expression,
                    )
                }
            }) {
                continue;
            }
            // An operand snapshot is not automatically a machine-entry
            // value. Unknown storage/version provenance widens the route;
            // it must never leave a callee Parameter unsubstituted in the
            // caller namespace or relabel a current read as an entry input.
            let Some(identity) = predicate
                .expression()
                .and_then(|identity| substitute_entry_projected(identity, &mut resolve_entry))
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
    (published, surviving)
}

/// A named call's exact operands in operator-parameter order. Named resolution
/// already fixed the arity: an `is_self` parameter binds the value receiver;
/// without `is_self`, a receiver call binds its value receiver to the single
/// leading parameter; a static namespace receiver carries no operand at all.
/// The flow pass reuses this ordering when it captures operand-time evidence
/// for a named use, so captured operand rows align with this list.
pub(crate) fn named_call_operands(
    program: &TypedTrees,
    call: &typed_trees::expression::TableCallExpression,
    parameters: &[typed_trees::signature::StateParameter],
) -> Option<Vec<ExpressionHandle>> {
    let arguments = program.expression_table.expression_handles(call.arguments);
    if parameters.iter().any(|parameter| parameter.is_self) {
        if !call.receiver.is_valid() {
            return None;
        }
        let mut argument = arguments.iter().copied();
        let operands = parameters
            .iter()
            .map(|parameter| {
                if parameter.is_self {
                    Some(call.receiver)
                } else {
                    argument.next()
                }
            })
            .collect::<Option<Vec<_>>>()?;
        return argument.next().is_none().then_some(operands);
    }
    if parameters.len() == arguments.len() + 1 {
        return call.receiver.is_valid().then(|| {
            std::iter::once(call.receiver)
                .chain(arguments.iter().copied())
                .collect()
        });
    }
    (parameters.len() == arguments.len()).then(|| arguments.to_vec())
}

/// Operand labels for leaf instantiation at a named call. A reference formal
/// used as a predicate value reads its referent — the borrow at the call site
/// supplies access, not an extra operator in the predicate — so a `&x`
/// operand names `x`, exactly as call-`requires` instantiation renders it
/// (checks/contracts/labels/calls.rs). Other operands keep their proof
/// spelling.
pub(crate) fn named_call_operand_labels(
    program: &TypedTrees,
    parameters: &[typed_trees::signature::StateParameter],
    operands: &[ExpressionHandle],
) -> Vec<String> {
    operands
        .iter()
        .enumerate()
        .map(|(ordinal, operand)| {
            let operand = match (
                parameters.get(ordinal),
                program.expression_table.expression(*operand),
            ) {
                (Some(parameter), ExpressionNode::Borrow(borrow))
                    if matches!(
                        program
                            .type_reference_table
                            .type_reference(parameter.type_reference),
                        TypeReferenceNode::Reference { .. }
                    ) =>
                {
                    borrow.target
                }
                _ => *operand,
            };
            program.render_proof_expression(
                operand,
                typed_trees::proposition::ProofSubstitutions::None,
            )
        })
        .collect()
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
