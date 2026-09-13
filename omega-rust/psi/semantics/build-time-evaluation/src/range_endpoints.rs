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
//! A type qualifier is not a runtime receiver. Reuse the typed call's resolved
//! entry and receiver classification, then evaluate that exact machine symbol;
//! rebuilding a name could select an unrelated same-spelled machine. Calls with
//! runtime receivers or unresolved arguments remain outside this closed route.
//! Closed integer arguments share the type system's exact numeric evaluation,
//! with argument carrier and declaration-selection checks before interpreter
//! snapshots erase their authored types. No runtime flow bound supplies a value.
//!
//! Walk strict integer arithmetic and call arguments in postorder. Temporary
//! call results keep their declared integer landing: making a returned u8
//! anonymous would let surrounding arithmetic widen it or initialize u64.
//! Argument selections still read the original prepared tree, while numeric
//! queries read the working substitutions. Machine bodies always execute in
//! the immutable prepared tree; a rollback journal avoids a clone per call.
//! Remaining endpoint arithmetic stays authored for ordinary checking, including
//! its overflow obligations and fractional diagnostics.

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

struct PendingEndpoint {
    constrained_type: TypeReferenceHandle,
    expression: ExpressionHandle,
    machine: symbols::SymbolHandle,
    source_span: source::SourceSpan,
}

pub fn evaluate_const_range_endpoints_with_authority(
    typed: &mut TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<(), Vec<Diagnostic>> {
    let pending = pending_endpoints(typed)?;
    if pending.is_empty() {
        return Ok(());
    }

    let prepared = crate::PreparedBuildMachineProgram::prepare(typed)?;
    let execution = prepared.typed();
    let admission = BuildTimeAdmissionPlan::infer_with_selection_authority(
        execution,
        selection_authority.clone(),
    );

    let mut diagnostics = Vec::new();
    let mut substitutions = Vec::new();
    let mut originals = Vec::new();
    let mut warnings = Vec::new();
    for endpoint in &pending {
        // Admit this original call and qualifier before execution. The
        // invocation floor checks its callee, not every retained occurrence;
        // waiting for a parent argument check would be too late for this call.
        let result = crate::admission::require_call_expression_selection(
            execution,
            endpoint.expression,
            selection_authority.as_deref(),
        )
        .and_then(|()| {
            arguments::evaluate(
                typed,
                execution,
                endpoint.expression,
                endpoint.machine,
                selection_authority.as_deref(),
            )
        })
        .and_then(|(arguments, argument_warnings)| {
            warnings.extend(argument_warnings);
            admission.evaluate_const_evaluable_machine_symbol_for_invocation(
                execution,
                endpoint.machine,
                arguments,
                crate::BuildTimeInvocationCustody::Source(endpoint.source_span),
            )
        })
        .and_then(|value| {
            let machine = execution
                .machines()
                .iter()
                .find(|machine| machine.symbol == endpoint.machine)
                .ok_or_else(|| "range endpoint lost its selected machine".to_owned())?;
            let entry = execution
                .machine_states(machine)
                .first()
                .ok_or("range endpoint machine has no entry state")?;
            let primitive = crate::const_generic_expressions::exact_probe_destination(
                execution,
                entry.return_type,
            )
            .filter(|primitive| primitive.accepts_integer_literal())
            .ok_or("range endpoint call requires an unconstrained exact builtin integer result")?;
            let value = crate::const_lengths::decode_integer_result(execution, machine, value)?;
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
                literal.with_landing(IntegerLanding {
                    landed_type,
                    domain: ArithmeticDomain::Exact,
                })
            })
            .map_err(|reason| format!("invalid evaluated range endpoint: {reason}"))
        });
        match result {
            Ok(literal) => {
                let original = std::mem::replace(
                    typed.expression_table.expression_mut(endpoint.expression),
                    ExpressionNode::Integer(literal.clone()),
                );
                originals.push((endpoint.expression, original));
                substitutions.push((endpoint.expression, literal));
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
    // argument arithmetic. Restore it before publishing any folds so a failed
    // outer call cannot leave its successfully evaluated children erased.
    for (expression, original) in originals.into_iter().rev() {
        *typed.expression_table.expression_mut(expression) = original;
    }

    if diagnostics.is_empty() {
        for (expression, literal) in substitutions {
            *typed.expression_table.expression_mut(expression) = ExpressionNode::Integer(literal);
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
                            work.push((binary.right, false));
                            work.push((binary.left, false));
                        }
                        ExpressionNode::Call(call) => {
                            work.extend(
                                typed
                                    .expression_table
                                    .expression_handles(call.arguments)
                                    .iter()
                                    .rev()
                                    .map(|argument| (*argument, false)),
                            );
                        }
                        _ => {}
                    }
                    continue;
                }
                active.pop();
                visited.push(expression);
                let ExpressionNode::Call(call) = typed.expression_table.expression(expression)
                else {
                    continue;
                };
                if !call.machine_arguments.is_empty()
                    || !call.evidence_arguments.is_empty()
                    || call.static_machine_parameter.is_valid()
                    || call.static_requirement_dispatch.is_some()
                    || call.quotient_operation.is_some()
                    || call.private_layout_operation.is_some()
                {
                    continue;
                }
                let Some(machine) = typed.machines().iter().find(|machine| {
                    // Ordinary value arguments do not close generic binders.
                    // Folding must not erase an underdetermined generic
                    // application before ordinary call validation can reject it.
                    machine.type_parameters.is_empty()
                        && typed.machine_states(machine).first().is_some_and(|entry| {
                            typed.call_has_no_runtime_receiver(call, machine, entry)
                        })
                }) else {
                    continue;
                };
                pending.push(PendingEndpoint {
                    constrained_type,
                    expression,
                    machine: machine.symbol,
                    source_span: typed.expression_table.source_span(expression),
                });
            }
        }
    }
    Ok(pending)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod selection_tests;
