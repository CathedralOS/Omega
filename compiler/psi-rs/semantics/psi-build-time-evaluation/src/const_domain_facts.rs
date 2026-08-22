//! Build-time evaluation of machine-backed integer-domain facts used by
//! concrete const-generic instances.
//!
//! Generic instance synthesis runs before symbol resolution. It can discharge
//! closed arithmetic facts there, but a fact such as `is_buffer_size(self);`
//! must wait until its callee has a typed symbol and a normalized build-time
//! contract summary. This pass runs immediately after the
//! other typed const-evaluation pass and replaces a proven concrete membership
//! with the ordinary `true` fact consumed by checking.

use psi_arena::Handle;
use psi_checked_interpreter::BuildTimeValue;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::{ProofFact, ProofMembershipFact};
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use psi_typed_trees::types::PrimitiveType;

use crate::BuildTimeAdmissionPlan;

mod fact_expression;

use fact_expression::{ConstProofValue, evaluate_domain_fact_expression};

struct PendingMembership {
    fact: Handle<ProofFact>,
    data_symbol: psi_symbols::SymbolHandle,
    instance_name: String,
    membership: ProofMembershipFact,
}

/// Evaluate direct `machine(self)` facts for literal memberships copied
/// into synthesized const-generic data definitions.
pub fn evaluate_const_domain_facts(typed: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut pending = Vec::new();
    for data in typed.data_definitions() {
        // The unspecialized template has no angle-bracket spelling and must
        // retain its symbolic fact for ordinary generic validation.
        if !data.name.as_str().contains('<') {
            continue;
        }
        for offset in 0..data.where_facts.count() {
            let fact = Handle::from_parts(
                data.where_facts.start().arena_index() + offset,
                data.where_facts.start().generation(),
            );
            let ProofFact::Membership(membership) = typed.proof_facts.get(fact) else {
                continue;
            };
            if matches!(
                typed.expression_table.expression(membership.value),
                ExpressionNode::Integer(_)
            ) {
                pending.push(PendingMembership {
                    fact,
                    data_symbol: data.symbol,
                    instance_name: data.name.as_str().to_owned(),
                    membership: *membership,
                });
            }
        }
    }

    if pending.is_empty() {
        return Ok(());
    }

    let admission = BuildTimeAdmissionPlan::infer(typed);
    let mut replacements = Vec::new();
    let mut affected_data = Vec::new();
    let mut diagnostics = Vec::new();

    for pending in pending {
        match evaluate_membership(typed, &admission, &pending) {
            Ok(Some(true)) => {
                replacements.push(pending.fact);
                affected_data.push(pending.data_symbol);
            }
            Ok(Some(false)) => diagnostics.push(Diagnostic::error(format!(
                "const fact for generic instance `{}` is false",
                pending.instance_name
            ))),
            Ok(None) => {}
            Err(reason) => diagnostics.push(Diagnostic::error(format!(
                "const domain fact evaluation for generic instance `{}` failed: {reason}",
                pending.instance_name
            ))),
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let proven = typed.expression_table.insert(ExpressionNode::Boolean(true));
    for fact in replacements {
        *typed.proof_facts.get_mut(fact) = ProofFact::Expression(proven);
    }
    let ungated: Vec<_> = typed
        .data_definitions()
        .iter()
        .filter(|data| affected_data.contains(&data.symbol))
        .filter(|data| {
            typed
                .proof_facts
                .span_or_empty(data.where_facts)
                .iter()
                .all(|fact| match fact {
                    ProofFact::Expression(expression) => matches!(
                        typed.expression_table.expression(*expression),
                        ExpressionNode::Boolean(true)
                    ),
                    ProofFact::Membership(_) => false,
                    ProofFact::Proposition(_) => false,
                })
        })
        .map(|data| data.symbol)
        .collect();
    typed
        .data_definitions
        .for_each_mut(|_, data| data.zero_gated &= !ungated.contains(&data.symbol));
    Ok(())
}

fn evaluate_membership(
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

fn evaluate_machine_fact(
    typed: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    expression: ExpressionHandle,
    self_value: i64,
) -> Result<Option<bool>, String> {
    let ExpressionNode::Call(call) = typed.expression_table.expression(expression) else {
        return Ok(None);
    };
    if !is_direct_self_call(typed, call) {
        return Ok(None);
    }

    let target = typed
        .machines()
        .iter()
        .find_map(|machine| {
            typed
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == call.target_symbol)
                .map(|state| (machine, state))
        })
        // Domain facts are lowered outside a machine body, where a free
        // call currently retains its resolved target name but not its state
        // symbol. Resolve that exact free-machine name here; ordinary call
        // validation still owns ambiguous or missing targets.
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
        });
    let Some((machine, state)) = target else {
        // Builtins and unresolved/invalid calls remain the responsibility of
        // normal expression validation.
        return Ok(None);
    };
    let machine_name = machine.name.as_str();

    admission.require_common_floor(typed, machine)?;

    let parameters = typed.state_parameters(state);
    if parameters.len() != 1
        || parameters[0].is_mutable
        || matches!(
            typed
                .type_reference_table
                .type_reference(parameters[0].type_reference),
            psi_typed_trees::types::TypeReferenceNode::Reference { .. }
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

    let fact_holds = match psi_checked_interpreter::evaluate_build_time_machine(
        typed,
        machine_name,
        vec![BuildTimeValue::Int(self_value)],
    )
    .map_err(|reason| format!("evaluation of `{machine_name}` failed: {reason}"))?
    {
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
    domain: &psi_typed_trees::domain::DomainDefinition,
    self_value: i64,
    visiting: &mut Vec<psi_symbols::SymbolHandle>,
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
    domain_symbol: psi_symbols::SymbolHandle,
    self_value: i64,
    visiting: &mut Vec<psi_symbols::SymbolHandle>,
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
