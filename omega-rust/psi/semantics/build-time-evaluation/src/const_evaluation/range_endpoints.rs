//! COMPTIME evaluation of NAMED RANGE ENDPOINTS.
//!
//! `u64[0..=limit(256)]` puts a build-time-admissible machine call
//! in a declared integer range endpoint. The endpoint is a constant position
//! exactly as a fixed-array length is, so it takes the same route as
//! `const_lengths`: admit the callee through the shared
//! [`BuildTimeAdmissionPlan`], execute it with the checked interpreter under
//! the callee's declared integer return type, and substitute the resulting
//! decimal literal into the endpoint expression before checking.
//!
//! Folding here, in the typed trees, is what lets every later reader agree on
//! one value: declaration validation, structural generic inference in
//! `typed-trees-to-checked-trees/src/monomorphization/range_arguments.rs`,
//! proof, and layout all read an ordinary literal endpoint. The
//! context-free `i64` interval evaluator in `validation` never sees the call,
//! so it cannot become the identity of a named computation.
//!
//! An endpoint callee whose reachable closure holds a resolved
//! boundary-operator use cannot run before provider selection: it defers with
//! the owning pre-check continuation, then evaluates under the same selected
//! rows the deferred fixed-array lengths use -- sealed primitive-float
//! meanings plus each selected provider's ordinary checked body, rebound on a
//! private execution copy so the caller's tree keeps its authored selection.
//!
//! A type qualifier is not a runtime receiver. Reuse the typed call's resolved
//! entry and receiver classification, then evaluate that exact machine symbol;
//! rebuilding a name could select an unrelated same-spelled machine. Calls with
//! runtime receivers or unresolved arguments remain outside this closed route.
//! Closed integer arguments share the type system's exact numeric evaluation,
//! with argument carrier and declaration-selection checks before interpreter
//! snapshots erase their authored types. No runtime flow bound supplies a value.
//! Closed range refinements use the same exact bound queries: input values must
//! satisfy every range before invocation, and returned values before folding.
//! A declared, argument-free integer domain on a parameter or result is proved
//! for the same concrete value through the shared domain-fact evaluator, so
//! `bounded(value: u64 in Positive)` admits `bounded(256)` and rejects
//! `bounded(0)` before the callee runs. Neither concrete check replaces
//! ordinary body checking; arithmetic policies and parameterized domains stay
//! outside this route. Calls computing signature bounds are invocation
//! dependencies, independent of declaration order.
//!
//! Walk strict integer arithmetic and call arguments in postorder. Temporary
//! call results keep their declared integer landing: making a returned u8
//! anonymous would let surrounding arithmetic widen it or initialize u64.
//! An argument-position helper may instead return `bool`; it folds to a
//! Boolean literal for the enclosing call's Boolean parameter. The range
//! bound itself is always prepared as an integer position, so a Boolean
//! result can never become an endpoint.
//! Argument selections still read the original prepared tree, while numeric
//! queries read the working substitutions. Machine bodies always execute in
//! the immutable prepared tree; a rollback journal avoids a clone per call.
//! Remaining endpoint arithmetic stays authored for ordinary checking, including
//! its overflow obligations and fractional diagnostics.

use crate::SelectedBuildTimeOperators;
use diagnostics::Diagnostic;
use numerics::{
    arithmetic::ArithmeticDomain,
    literals::{IntegerLanding, IntegerLiteral, IntegerRadix, LandedIntegerType},
};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle};

use crate::BuildTimeAdmissionPlan;

mod arguments;
mod integer_type;

struct PendingEndpoint {
    constrained_type: TypeReferenceHandle,
    expression: ExpressionHandle,
    machine: symbols::SymbolHandle,
    source_span: source::SourceSpan,
    /// Whether this call is the authored range bound itself, whose result
    /// must land as an integer, or an argument of an enclosing call, whose
    /// declared result may be Boolean.
    range_bound: bool,
}

pub fn evaluate_const_range_endpoints(
    typed: &mut TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<(), Vec<Diagnostic>> {
    evaluate_selected_range_endpoints(
        typed,
        selection_authority,
        SelectedBuildTimeOperators {
            operators: &[],
            provider_bodies: &[],
        },
    )
}

/// Whether any still-authored endpoint call's reachable machine closure holds
/// a resolved boundary-operator use that only exact selected execution can
/// run. Mirrors the fixed-array-length deferral gate: the owning continuation
/// waits for Omega's settled rows rather than failing such an endpoint against
/// an unselected admission.
pub(crate) fn pending_endpoint_calls_need_operator_selection(
    typed: &TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<bool, Vec<Diagnostic>> {
    let pending = pending_endpoints(typed)?;
    if pending.is_empty() {
        return Ok(false);
    }
    // Admission and the deferral scan must see the same closed static
    // applications as the eventual evaluation; prepare once and reuse it.
    let prepared = crate::PreparedBuildMachineProgram::prepare(typed)?;
    let execution = prepared.typed();
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(execution);
    let admission = BuildTimeAdmissionPlan::infer(execution, selection_authority);
    Ok(pending.iter().any(|endpoint| {
        admission.closure_needs_operator_selection(execution, endpoint.machine, &facts)
    }))
}

/// Record every still-authored endpoint call as deferred to selected
/// execution. Interim checking passes over the typed tree before the owning
/// continuation resumes, so the marks keep a pending endpoint from being
/// misread as a non-constant or dependent bound; each fold removes its mark.
pub(crate) fn defer_pending_endpoint_calls(typed: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    for endpoint in pending_endpoints(typed)? {
        typed
            .pending_const_range_endpoints
            .insert(endpoint.expression);
    }
    Ok(())
}

/// Finish still-authored endpoint calls under the exact selected rows Omega
/// settled for this program. `operators` supplies the sealed primitive-float
/// meanings; `provider_bodies` rebinds each retained boundary-operator
/// occurrence to its selected provider's ordinary checked body on the private
/// execution copy -- the caller's tree keeps its authored selection and
/// source-owned handles for final checking, exactly as deferred fixed-array
/// lengths do.
pub(crate) fn evaluate_selected_range_endpoints(
    typed: &mut TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    selected: crate::SelectedBuildTimeOperators<'_>,
) -> Result<(), Vec<Diagnostic>> {
    let crate::SelectedBuildTimeOperators {
        operators,
        provider_bodies,
    } = selected;
    let pending = pending_endpoints(typed)?;
    if pending.is_empty() {
        return Ok(());
    }

    let prepared = crate::PreparedBuildMachineProgram::prepare(typed)?;
    let selected_execution = if provider_bodies.is_empty() {
        None
    } else {
        Some(
            crate::machine_execution::selected_operators::apply_selected_provider_bodies(
                prepared.typed(),
                provider_bodies,
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?,
        )
    };
    let execution = selected_execution
        .as_ref()
        .unwrap_or_else(|| prepared.typed());
    let admission = BuildTimeAdmissionPlan::infer(execution, selection_authority.clone())
        .with_selected_operators(execution, operators)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;

    let mut diagnostics = Vec::new();
    let mut substitutions = Vec::new();
    let mut originals = Vec::new();
    let mut warnings = Vec::new();
    for endpoint in &pending {
        // Admit this original call and qualifier before execution. The
        // invocation floor checks its callee, not every retained occurrence;
        // waiting for a parent argument check would be too late for this call.
        let result = crate::machine_execution::admission::require_call_expression_selection(
            execution,
            endpoint.expression,
            selection_authority.as_deref(),
        )
        .and_then(|()| {
            arguments::evaluate(
                typed,
                execution,
                &admission,
                endpoint.expression,
                endpoint.machine,
                selection_authority.as_deref(),
            )
        })
        .and_then(|(arguments, argument_warnings)| {
            let machine = execution
                .machines()
                .iter()
                .find(|machine| machine.symbol == endpoint.machine)
                .ok_or_else(|| "range endpoint lost its selected machine".to_owned())?;
            let entry = execution
                .machine_states(machine)
                .first()
                .ok_or("range endpoint machine has no entry state")?;
            let position = if endpoint.range_bound {
                integer_type::ScalarPosition::Integer(integer_type::IntegerPosition::prepare(
                    typed,
                    execution,
                    entry.return_type,
                    selection_authority.as_deref(),
                )?)
            } else {
                integer_type::ScalarPosition::prepare(
                    typed,
                    execution,
                    entry.return_type,
                    selection_authority.as_deref(),
                )?
            };
            warnings.extend(argument_warnings);
            // A domain-qualified parameter is a generated `requires` premise
            // on the entry state. `arguments::evaluate` has just proved each
            // concrete argument's membership through the shared domain-fact
            // evaluator, which is the invocation proof the closure fence asks
            // for; the fence stands down only when those parameter-domain
            // premises are the closure's sole premises.
            let custody = crate::BuildTimeInvocationCustody::Source(endpoint.source_span);
            let value = if admission.closure_includes_authored_requires(execution, machine)
                && admission.closure_requires_are_entry_parameter_domains(execution, machine)
            {
                admission.evaluate_const_evaluable_machine_symbol_for_concrete_premise_invocation(
                    execution,
                    endpoint.machine,
                    arguments,
                    custody,
                )?
            } else {
                admission.evaluate_const_evaluable_machine_symbol_for_invocation(
                    execution,
                    endpoint.machine,
                    arguments,
                    custody,
                )?
            };
            let position = match position {
                integer_type::ScalarPosition::Integer(position) => position,
                integer_type::ScalarPosition::Boolean => {
                    return match value {
                        crate::BuildTimeValue::Bool(value) => Ok(ExpressionNode::Boolean(value)),
                        other => Err(format!(
                            "machine `{}` returned `{other:?}` instead of `bool`",
                            machine.name
                        )),
                    };
                }
            };
            let value = crate::const_evaluation::const_lengths::decode_integer_result(
                execution, machine, value,
            )?;
            position.require_value(execution, &admission, &value)?;
            let primitive = position.primitive;
            let landed_type = match primitive {
                typed_trees::types::PrimitiveType::I8 => LandedIntegerType::I8,
                typed_trees::types::PrimitiveType::I16 => LandedIntegerType::I16,
                typed_trees::types::PrimitiveType::I32 => LandedIntegerType::I32,
                typed_trees::types::PrimitiveType::I64 => LandedIntegerType::I64,
                typed_trees::types::PrimitiveType::U8 => LandedIntegerType::U8,
                typed_trees::types::PrimitiveType::U16 => LandedIntegerType::U16,
                typed_trees::types::PrimitiveType::U32 => LandedIntegerType::U32,
                typed_trees::types::PrimitiveType::U64 => LandedIntegerType::U64,
                _ => {
                    return Err("range endpoint result requires a fixed integer carrier".to_owned());
                }
            };
            IntegerLiteral::from_parts(
                value.is_negative(),
                IntegerRadix::Decimal,
                &value.abs().to_string(),
            )
            .map(|literal| {
                ExpressionNode::Integer(literal.with_landing(IntegerLanding {
                    landed_type,
                    domain: ArithmeticDomain::Exact,
                }))
            })
            .map_err(|reason| format!("invalid evaluated range endpoint: {reason}"))
        });
        match result {
            Ok(folded) => {
                let original = std::mem::replace(
                    typed.expression_table.expression_mut(endpoint.expression),
                    folded.clone(),
                );
                originals.push((endpoint.expression, original));
                substitutions.push((endpoint.expression, folded));
            }
            Err(reason) => diagnostics.push(Diagnostic::error(format!(
                "range endpoint of `{}`: const evaluation of `{}` failed: {reason}",
                typed
                    .type_reference_table
                    .display_name(endpoint.constrained_type),
                typed.symbols.display_path(endpoint.machine, "::"),
            ))),
        }
    }

    // Execute against the immutable prepared program, never a graph changed
    // underneath its admission plan. The working tree only supplies closed
    // argument arithmetic and signature bounds. Restore it before publishing
    // any folds so a failed outer call cannot leave its successfully evaluated
    // children erased.
    for (expression, original) in originals.into_iter().rev() {
        *typed.expression_table.expression_mut(expression) = original;
    }

    if diagnostics.is_empty() {
        for (expression, folded) in substitutions {
            *typed.expression_table.expression_mut(expression) = folded;
            typed.pending_const_range_endpoints.remove(&expression);
        }
        for warning in warnings {
            eprintln!("{warning}");
        }
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn pending_endpoints(typed: &TypedTrees) -> Result<Vec<PendingEndpoint>, Vec<Diagnostic>> {
    let mut pending = Vec::new();
    let mut visited = Vec::new();
    for (constrained_type, _, constraints) in typed
        .type_reference_table
        .constrained_type_reference_sites()
    {
        for constraint in typed.type_reference_table.constraints(constraints) {
            let TypeConstraintNode::Range {
                minimum, maximum, ..
            } = constraint
            else {
                continue;
            };
            let mut work = vec![(*maximum, false), (*minimum, false)];
            let mut active = Vec::new();
            // Expressions beneath a call's argument list. Everything else,
            // including arithmetic around the bound and the callee's own
            // signature bounds, is a range bound whose value must land as an
            // integer.
            let mut argument_positions = Vec::new();
            while let Some((expression, leaving)) = work.pop() {
                if !typed.expression_table.expression_is_valid(expression) {
                    continue;
                }
                if !leaving {
                    if active.contains(&expression) {
                        return Err(vec![Diagnostic::error("cyclic range endpoint expression")]);
                    }
                    if visited.contains(&expression) {
                        continue;
                    }
                    active.push(expression);
                    work.push((expression, true));
                    match typed.expression_table.expression(expression) {
                        ExpressionNode::Binary(binary) => {
                            if argument_positions.contains(&expression) {
                                argument_positions.push(binary.left);
                                argument_positions.push(binary.right);
                            }
                            work.push((binary.right, false));
                            work.push((binary.left, false));
                        }
                        ExpressionNode::Call(call) => {
                            let arguments =
                                typed.expression_table.expression_handles(call.arguments);
                            argument_positions.extend(arguments.iter().copied());
                            work.extend(arguments.iter().rev().map(|argument| (*argument, false)));
                            if let Some(machine) = selected_endpoint_machine(typed, expression) {
                                append_signature_bounds(typed, machine, &mut work)?;
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                active.pop();
                visited.push(expression);
                let Some(machine) = selected_endpoint_machine(typed, expression) else {
                    continue;
                };
                pending.push(PendingEndpoint {
                    constrained_type,
                    expression,
                    machine: machine.symbol,
                    source_span: typed.expression_table.source_span(expression),
                    range_bound: !argument_positions.contains(&expression),
                });
            }
        }
    }
    Ok(pending)
}

fn selected_endpoint_machine(
    typed: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<&typed_trees::machine::Machine> {
    let ExpressionNode::Call(call) = typed.expression_table.expression(expression) else {
        return None;
    };
    if !call.machine_arguments.is_empty()
        || !call.evidence_arguments.is_empty()
        || call.static_machine_parameter.is_valid()
        || call.static_requirement_dispatch.is_some()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
    {
        return None;
    }
    // Ordinary arguments do not close generic binders. Preserve unresolved
    // applications and runtime receivers for ordinary call validation.
    typed.machines().iter().find(|machine| {
        machine.type_parameters.is_empty()
            && typed
                .machine_states(machine)
                .first()
                .is_some_and(|entry| typed.call_has_no_runtime_receiver(call, machine, entry))
    })
}

fn append_signature_bounds(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    work: &mut Vec<(ExpressionHandle, bool)>,
) -> Result<(), Vec<Diagnostic>> {
    let Some(entry) = typed.machine_states(machine).first() else {
        return Ok(());
    };
    // A called declaration may appear later in source. Its static input/result
    // bounds are dependencies of invocation admission, not a reason to rely on
    // the type arena's order or repeatedly retry unsuccessful evaluations.
    for mut reference in std::iter::once(entry.return_type).chain(
        typed
            .state_parameters(entry)
            .iter()
            .map(|parameter| parameter.type_reference),
    ) {
        let mut visited = Vec::new();
        while let typed_trees::types::TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = typed.type_reference_table.type_reference(reference)
        {
            if visited.contains(&reference) {
                return Err(vec![Diagnostic::error(
                    "cyclic range endpoint integer type",
                )]);
            }
            visited.push(reference);
            for constraint in typed.type_reference_table.constraints(*constraints) {
                if let TypeConstraintNode::Range {
                    minimum, maximum, ..
                } = constraint
                {
                    work.push((*maximum, false));
                    work.push((*minimum, false));
                }
            }
            reference = *base_type;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod selection_tests;

#[cfg(test)]
mod integer_type_tests;
