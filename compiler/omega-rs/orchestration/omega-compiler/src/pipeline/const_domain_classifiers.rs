//! Build-time evaluation of machine-backed integer-domain classifiers used by
//! concrete const-generic instances.
//!
//! Generic instance synthesis runs before symbol resolution. It can discharge
//! closed arithmetic classifiers there, but a classifier such as
//! `when is_buffer_size(self)` must wait until its callee has a typed symbol and
//! an inferred transitive effect surface. This pass runs immediately after the
//! other typed const-evaluation pass and replaces a proven concrete membership
//! with the ordinary `true` fact consumed by checking.

use omega_core::arena::Handle;
use omega_core::diagnostics::Diagnostic;
use omega_interpreter::BuildTimeValue;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::domain::{ProofFact, ProofMembershipFact};
use omega_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression, UnaryOperator,
};
use omega_typed_trees::types::PrimitiveType;

struct PendingMembership {
    fact: Handle<ProofFact>,
    data_symbol: omega_core::symbols::SymbolHandle,
    instance_name: String,
    membership: ProofMembershipFact,
}

/// Evaluate direct `machine(self)` classifiers for literal memberships copied
/// into synthesized const-generic data definitions.
pub(super) fn evaluate_const_domain_classifiers(
    typed: &mut TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
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

    let effect_plan = omega_effects::infer_effects(typed);
    let mut replacements = Vec::new();
    let mut affected_data = Vec::new();
    let mut diagnostics = Vec::new();

    for pending in pending {
        match evaluate_membership(typed, &effect_plan, &pending) {
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
                "const domain classifier for generic instance `{}` failed: {reason}",
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
    effect_plan: &omega_effects::EffectPlan,
    pending: &PendingMembership,
) -> Result<Option<bool>, String> {
    let Some(domain) = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == pending.membership.domain_symbol)
    else {
        return Ok(None);
    };
    let ExpressionNode::Call(call) = typed.expression_table.expression(domain.classifier) else {
        return Ok(None);
    };
    if !is_direct_self_call(typed, call) {
        return Ok(None);
    }

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

    let Some(classifier_holds) =
        evaluate_machine_classifier(typed, effect_plan, domain.classifier, value)?
    else {
        return Ok(None);
    };
    if !classifier_holds {
        return Ok(Some(false));
    }

    evaluate_domain_facts(typed, effect_plan, domain, value, &mut vec![domain.symbol])
}

fn evaluate_machine_classifier(
    typed: &TypedTrees,
    effect_plan: &omega_effects::EffectPlan,
    classifier: ExpressionHandle,
    self_value: i64,
) -> Result<Option<bool>, String> {
    let ExpressionNode::Call(call) = typed.expression_table.expression(classifier) else {
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
        // Domain classifiers are lowered outside a machine body, where a free
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

    let transitive = effect_plan
        .machines()
        .iter()
        .find(|entry| entry.symbol == machine.symbol)
        .map(|entry| entry.transitive)
        .unwrap_or_else(omega_effects::EffectSet::empty);
    if !transitive.is_empty() {
        return Err(format!(
            "machine `{machine_name}` is not effect-free: it reaches effects `{}`; only effect-free machines may be evaluated at compile time",
            transitive.names().collect::<Vec<_>>().join(", ")
        ));
    }

    let parameters = typed.state_parameters(state);
    if parameters.len() != 1
        || parameters[0].is_mutable
        || matches!(
            typed
                .type_reference_table
                .type_reference(parameters[0].type_reference),
            omega_typed_trees::types::TypeReferenceNode::Reference { .. }
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
            "machine `{machine_name}` must return `bool` when used as a domain classifier"
        ));
    }

    let classifier_holds = match omega_interpreter::evaluate_build_time_machine(
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
    Ok(Some(classifier_holds))
}

fn evaluate_domain_facts(
    typed: &TypedTrees,
    effect_plan: &omega_effects::EffectPlan,
    domain: &omega_typed_trees::domain::DomainDefinition,
    self_value: i64,
    visiting: &mut Vec<omega_core::symbols::SymbolHandle>,
) -> Result<Option<bool>, String> {
    for fact in typed.proof_facts.span_or_empty(domain.facts) {
        match fact {
            ProofFact::Expression(expression) => {
                match evaluate_domain_fact_expression(typed, *expression, self_value)? {
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
                    effect_plan,
                    membership.domain_symbol,
                    nested_value,
                    visiting,
                )? {
                    Some(true) => {}
                    Some(false) => return Ok(Some(false)),
                    None => return Ok(None),
                }
            }
        }
    }
    Ok(Some(true))
}

fn evaluate_nested_domain_membership(
    typed: &TypedTrees,
    effect_plan: &omega_effects::EffectPlan,
    domain_symbol: omega_core::symbols::SymbolHandle,
    self_value: i64,
    visiting: &mut Vec<omega_core::symbols::SymbolHandle>,
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

    if domain.classifier.is_valid() {
        let classifier =
            match evaluate_machine_classifier(typed, effect_plan, domain.classifier, self_value)? {
                Some(value) => Some(ConstProofValue::Boolean(value)),
                None => evaluate_domain_fact_expression(typed, domain.classifier, self_value)?,
            };
        match classifier {
            Some(ConstProofValue::Boolean(true)) => {}
            Some(ConstProofValue::Boolean(false)) => return Ok(Some(false)),
            Some(ConstProofValue::Integer(_)) => {
                return Err(format!(
                    "domain `{}` has a non-boolean classifier",
                    domain.name
                ));
            }
            None => return Ok(None),
        }
    }

    visiting.push(domain_symbol);
    let result = evaluate_domain_facts(typed, effect_plan, domain, self_value, visiting);
    visiting.pop();
    result
}

#[derive(Clone, Copy)]
enum ConstProofValue {
    Integer(i64),
    Boolean(bool),
}

fn evaluate_domain_fact_expression(
    typed: &TypedTrees,
    expression: ExpressionHandle,
    self_value: i64,
) -> Result<Option<ConstProofValue>, String> {
    match typed.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value
            .value_i64()
            .map(ConstProofValue::Integer)
            .map(Some)
            .ok_or_else(|| {
                format!(
                    "proof operand `{value}` does not fit the build-time evaluator's signed integer boundary"
                )
            }),
        ExpressionNode::Boolean(value) => Ok(Some(ConstProofValue::Boolean(*value))),
        ExpressionNode::Name(path) => {
            let [name] = typed.expression_table.name_path_members(path.members) else {
                return Ok(None);
            };
            Ok((name.as_str() == "self")
                .then_some(ConstProofValue::Integer(self_value)))
        }
        ExpressionNode::Binary(binary) => {
            let Some(left) = evaluate_domain_fact_expression(typed, binary.left, self_value)? else {
                return Ok(None);
            };
            let Some(right) = evaluate_domain_fact_expression(typed, binary.right, self_value)?
            else {
                return Ok(None);
            };
            evaluate_domain_fact_binary(binary.operator, left, right).map(Some)
        }
        ExpressionNode::Unary(unary) => {
            let Some(operand) = evaluate_domain_fact_expression(typed, unary.operand, self_value)?
            else {
                return Ok(None);
            };
            match (unary.operator, operand) {
                (UnaryOperator::LogicalNot, ConstProofValue::Boolean(value)) => {
                    Ok(Some(ConstProofValue::Boolean(!value)))
                }
                (UnaryOperator::LogicalNot, ConstProofValue::Integer(_)) => {
                    Err("logical negation requires a boolean proof operand".to_string())
                }
            }
        }
        _ => Ok(None),
    }
}

fn evaluate_domain_fact_binary(
    operator: BinaryOperator,
    left: ConstProofValue,
    right: ConstProofValue,
) -> Result<ConstProofValue, String> {
    use BinaryOperator::*;
    match (left, right) {
        (ConstProofValue::Integer(left), ConstProofValue::Integer(right)) => match operator {
            Add => left
                .checked_add(right)
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof addition overflows `i64`".to_string()),
            Subtract => left
                .checked_sub(right)
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof subtraction overflows `i64`".to_string()),
            Multiply => left
                .checked_mul(right)
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof multiplication overflows `i64`".to_string()),
            Divide => left
                .checked_div(right)
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof division is invalid".to_string()),
            Modulo => left
                .checked_rem(right)
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof remainder is invalid".to_string()),
            ShiftLeft => u32::try_from(right)
                .ok()
                .and_then(|amount| left.checked_shl(amount))
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof left shift exceeds the `i64` width".to_string()),
            ShiftRight => u32::try_from(right)
                .ok()
                .and_then(|amount| left.checked_shr(amount))
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof right shift exceeds the `i64` width".to_string()),
            BitwiseAnd => Ok(ConstProofValue::Integer(left & right)),
            BitwiseOr => Ok(ConstProofValue::Integer(left | right)),
            BitwiseXor => Ok(ConstProofValue::Integer(left ^ right)),
            Equal => Ok(ConstProofValue::Boolean(left == right)),
            NotEqual => Ok(ConstProofValue::Boolean(left != right)),
            Greater => Ok(ConstProofValue::Boolean(left > right)),
            GreaterOrEqual => Ok(ConstProofValue::Boolean(left >= right)),
            Less => Ok(ConstProofValue::Boolean(left < right)),
            LessOrEqual => Ok(ConstProofValue::Boolean(left <= right)),
            And | Or => Err("logical proof operators require boolean operands".to_string()),
        },
        (ConstProofValue::Boolean(left), ConstProofValue::Boolean(right)) => match operator {
            And => Ok(ConstProofValue::Boolean(left && right)),
            Or => Ok(ConstProofValue::Boolean(left || right)),
            Equal => Ok(ConstProofValue::Boolean(left == right)),
            NotEqual => Ok(ConstProofValue::Boolean(left != right)),
            _ => Err("arithmetic proof operators require integer operands".to_string()),
        },
        _ => Err("const proof operands have incompatible types".to_string()),
    }
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
