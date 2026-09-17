use checked_interpreter::BuildTimeValue;
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::PrimitiveType;

use super::PendingMembership;
use super::fact_expression::{ConstProofValue, evaluate_domain_fact_expression};
use crate::BuildTimeAdmissionPlan;

pub(super) fn evaluate_membership(
    typed: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    pending: &PendingMembership,
) -> Result<Option<bool>, String> {
    let Some(domain) = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == pending.membership.domain_symbol)
    else {
        return Ok(None);
    };
    let ExpressionNode::Integer(literal) =
        typed.expression_table.expression(pending.membership.value)
    else {
        return Ok(None);
    };
    let value = literal.value_i64().ok_or_else(|| {
        format!(
            "the concrete value `{literal}` does not fit the build-time evaluator's signed integer boundary"
        )
    })?;

    evaluate_domain_facts(typed, admission, domain, value, &mut vec![domain.symbol])
}

/// Membership of one already-evaluated closed integer in a declared domain.
/// Range-endpoint admission proves a callee's domain-qualified input or result
/// through this same fact evaluator, so a domain never enters an endpoint
/// computation by base-type stripping. `None` means the domain's facts are not
/// decidable here (a proposition, an unresolved call, a cycle); callers must
/// treat that as unproven, not as membership. The signed boundary is the
/// evaluator's storage limit, reported rather than truncated.
pub(in crate::const_evaluation) fn evaluate_closed_membership(
    typed: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    domain_symbol: symbols::SymbolHandle,
    value: &numerics::bignum::BigInt,
) -> Result<Option<bool>, String> {
    let Some(domain) = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)
    else {
        return Ok(None);
    };
    let value = value.to_i64().ok_or_else(|| {
        format!(
            "the concrete value `{value}` does not fit the build-time evaluator's signed integer boundary"
        )
    })?;
    evaluate_domain_facts(typed, admission, domain, value, &mut vec![domain.symbol])
}

/// The machine a direct `self` fact call invokes, or `None` when the fact
/// expression is not such a call. Domain facts are lowered outside a machine
/// body, where a free call currently retains its resolved target name but not
/// its state symbol; that exact free-machine name resolves here while
/// ordinary call validation still owns ambiguous or missing targets. Builtins
/// and unresolved or invalid calls stay outside this route.
fn resolve_fact_machine(
    typed: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(&Machine, &State)> {
    let ExpressionNode::Call(call) = typed.expression_table.expression(expression) else {
        return None;
    };
    if !is_direct_self_call(typed, call) {
        return None;
    }
    typed
        .machines()
        .iter()
        .find_map(|machine| {
            typed
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == call.target_symbol)
                .map(|state| (machine, state))
        })
        .or_else(|| {
            typed
                .machines()
                .iter()
                .find(|machine| {
                    machine.attached_data.is_none() && machine.name.as_str() == call.target.as_str()
                })
                .and_then(|machine| {
                    typed
                        .machine_states(machine)
                        .first()
                        .map(|state| (machine, state))
                })
        })
}

/// Whether evaluating one domain's fact tree would invoke a machine whose
/// reachable closure holds a resolved boundary-operator use only exact
/// selected execution can run. Mirrors `evaluate_domain_facts`: expression
/// facts consult their direct self-call target, nested memberships recurse
/// through the same visiting set, and undecidable positions never force a
/// deferral on their own.
pub(super) fn domain_facts_need_operator_selection(
    typed: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    facts: &checked_trees::CheckedOperatorFacts,
    domain_symbol: symbols::SymbolHandle,
    visiting: &mut Vec<symbols::SymbolHandle>,
) -> bool {
    if visiting.contains(&domain_symbol) {
        return false;
    }
    let Some(domain) = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)
    else {
        return false;
    };
    visiting.push(domain_symbol);
    let needed = typed
        .proof_facts
        .span_or_empty(domain.facts)
        .iter()
        .any(|fact| match fact {
            ProofFact::Expression(expression) => resolve_fact_machine(typed, *expression)
                .is_some_and(|(machine, _)| {
                    admission.closure_needs_operator_selection(typed, machine.symbol, facts)
                }),
            ProofFact::Membership(membership) => domain_facts_need_operator_selection(
                typed,
                admission,
                facts,
                membership.domain_symbol,
                visiting,
            ),
            ProofFact::Proposition(_) => false,
        });
    visiting.pop();
    needed
}

fn evaluate_machine_fact(
    typed: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    expression: ExpressionHandle,
    self_value: i64,
) -> Result<Option<bool>, String> {
    let Some((machine, state)) = resolve_fact_machine(typed, expression) else {
        return Ok(None);
    };
    let machine_name = machine.name.as_str();

    let parameters = typed.state_parameters(state);
    if parameters.len() != 1
        || parameters[0].is_mutable
        || matches!(
            typed
                .type_reference_table
                .type_reference(parameters[0].type_reference),
            typed_trees::types::TypeReferenceNode::Reference { .. }
        )
    {
        return Err(format!(
            "machine `{machine_name}` must take exactly one by-value, non-mutable integer parameter"
        ));
    }
    if !typed
        .primitive_type_reference(parameters[0].type_reference)
        .is_some_and(|primitive| primitive.accepts_integer_literal())
    {
        return Err(format!(
            "machine `{machine_name}` must take exactly one by-value integer parameter"
        ));
    }
    if typed.primitive_type_reference(state.return_type) != Some(PrimitiveType::Bool) {
        return Err(format!(
            "machine `{machine_name}` must return `bool` when used as a domain fact"
        ));
    }

    let fact_holds = match admission.evaluate_const_evaluable_machine_for_invocation(
        typed,
        machine_name,
        vec![BuildTimeValue::Int(self_value)],
        crate::BuildTimeInvocationCustody::Source(typed.expression_table.source_span(expression)),
    )? {
        BuildTimeValue::Bool(holds) => holds,
        other => {
            return Err(format!(
                "machine `{machine_name}` returned `{other:?}` instead of `bool`"
            ));
        }
    };
    Ok(Some(fact_holds))
}

fn evaluate_domain_facts(
    typed: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    domain: &typed_trees::domain::DomainDefinition,
    self_value: i64,
    visiting: &mut Vec<symbols::SymbolHandle>,
) -> Result<Option<bool>, String> {
    for fact in typed.proof_facts.span_or_empty(domain.facts) {
        match fact {
            ProofFact::Expression(expression) => {
                let value = match evaluate_machine_fact(typed, admission, *expression, self_value)?
                {
                    Some(value) => Some(ConstProofValue::Boolean(value)),
                    None => evaluate_domain_fact_expression(typed, *expression, self_value)?,
                };
                match value {
                    Some(ConstProofValue::Boolean(true)) => {}
                    Some(ConstProofValue::Boolean(false)) => return Ok(Some(false)),
                    Some(ConstProofValue::Integer(_)) => {
                        return Err(format!(
                            "domain `{}` has a non-boolean proof fact",
                            domain.name
                        ));
                    }
                    None => return Ok(None),
                }
            }
            ProofFact::Membership(membership) => {
                let nested_value =
                    match evaluate_domain_fact_expression(typed, membership.value, self_value)? {
                        Some(ConstProofValue::Integer(value)) => value,
                        Some(ConstProofValue::Boolean(_)) => {
                            return Err(format!(
                                "domain `{}` has a boolean membership operand",
                                domain.name
                            ));
                        }
                        None => return Ok(None),
                    };
                match evaluate_nested_domain_membership(
                    typed,
                    admission,
                    membership.domain_symbol,
                    nested_value,
                    visiting,
                )? {
                    Some(true) => {}
                    Some(false) => return Ok(Some(false)),
                    None => return Ok(None),
                }
            }
            ProofFact::Proposition(_) => return Ok(None),
        }
    }
    Ok(Some(true))
}

fn evaluate_nested_domain_membership(
    typed: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    domain_symbol: symbols::SymbolHandle,
    self_value: i64,
    visiting: &mut Vec<symbols::SymbolHandle>,
) -> Result<Option<bool>, String> {
    if visiting.contains(&domain_symbol) {
        return Ok(None);
    }
    let Some(domain) = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)
    else {
        return Ok(None);
    };

    visiting.push(domain_symbol);
    let result = evaluate_domain_facts(typed, admission, domain, self_value, visiting);
    visiting.pop();
    result
}

fn is_direct_self_call(typed: &TypedTrees, call: &TableCallExpression) -> bool {
    if call.receiver.is_valid() || !call.machine_arguments.is_empty() {
        return false;
    }
    let [argument] = typed.expression_table.expression_handles(call.arguments) else {
        return false;
    };
    let ExpressionNode::Name(path) = typed.expression_table.expression(*argument) else {
        return false;
    };
    let [name] = typed.expression_table.name_path_members(path.members) else {
        return false;
    };
    name.as_str() == "self"
}
