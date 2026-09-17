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
//! An explicit static application `identity<256>()` is admitted only when
//! every binder is supplied by a closed const spelling: the prepared program's
//! ordinary static specialization (its all-expression scan covers calls in
//! type positions) rewrites the call to a concrete instance, and the endpoint
//! then resolves that instance from the prepared tree. Its signature types
//! live in the prepared tree, so positions read there while argument values
//! keep the working tree and the template's context. An application that
//! still needs inference is not pending here; a partially supplied one is
//! pending so it can report the missing argument.
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
//! Evaluation runs in rounds. Each round prepares the execution program from
//! the current working tree and folds every call that closes; the driver
//! publishes those folds and prepares again only when a static application
//! failed while progress was made, because its instance's cloned signature
//! bounds are read from the prepared tree and a template bound that folded
//! this round (`bounded<const N>(value: u64[0..=limit()])`) is only visible
//! there after re-preparation. A round with no progress reports its
//! failures and every published fold is restored, so a rejected program
//! keeps its authored calls.
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
    /// The resolved callee in the working tree: a plain machine, or the
    /// generic template of an explicit static application.
    machine: symbols::SymbolHandle,
    source_span: source::SourceSpan,
    /// The authored call carries explicit static machine arguments; the
    /// executable callee is the prepared tree's specialized instance.
    static_application: bool,
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
        // An unresolvable application reports at evaluation, not here.
        resolve_endpoint_callee(execution, endpoint).is_ok_and(|callee| {
            admission.closure_needs_operator_selection(execution, callee.instance, &facts)
        })
    }))
}

/// The machine the endpoint actually invokes, plus the template whose entry
/// supplies value-evaluation context in the working tree.
#[derive(Clone, Copy)]
pub(super) struct EndpointCallee {
    pub(super) instance: symbols::SymbolHandle,
    pub(super) template: symbols::SymbolHandle,
    pub(super) static_application: bool,
}

impl EndpointCallee {
    #[cfg(test)]
    pub(super) fn plain(machine: symbols::SymbolHandle) -> Self {
        Self {
            instance: machine,
            template: machine,
            static_application: false,
        }
    }
}

/// Resolve the executable callee in the prepared tree. A static application
/// must have been rewritten there to its concrete instance; a call that still
/// carries static arguments was not specialized, which after `prepare` means
/// a binder is missing or open, so ask for the explicit argument.
fn resolve_endpoint_callee(
    execution: &TypedTrees,
    endpoint: &PendingEndpoint,
) -> Result<EndpointCallee, String> {
    if !endpoint.static_application {
        return Ok(EndpointCallee {
            instance: endpoint.machine,
            template: endpoint.machine,
            static_application: false,
        });
    }
    let ExpressionNode::Call(call) = execution.expression_table.expression(endpoint.expression)
    else {
        return Err("range endpoint lost its authored static application".to_owned());
    };
    let template = execution
        .machines()
        .iter()
        .find(|machine| machine.symbol == endpoint.machine)
        .ok_or("range endpoint lost its generic template")?;
    if !call.machine_arguments.is_empty() {
        return Err(format!(
            "static application of `{}` supplies {} of {} static arguments; supply every static argument explicitly",
            template.name,
            call.machine_arguments.len(),
            template.type_parameters.len(),
        ));
    }
    let instance = execution
        .machines()
        .iter()
        .find(|machine| {
            execution
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == call.target_symbol)
        })
        .filter(|machine| machine.type_parameters.is_empty())
        .ok_or_else(|| {
            format!(
                "static application of `{}` was not specialized to a concrete instance",
                template.name
            )
        })?;
    Ok(EndpointCallee {
        instance: instance.symbol,
        template: endpoint.machine,
        static_application: true,
    })
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
    // Folds published by earlier rounds, with their authored originals and
    // whether the deferral path had marked them. A round that ends in
    // rejection restores every one of them so a rejected program keeps its
    // authored calls, exactly as a single failing round did.
    let mut published: Vec<(ExpressionHandle, ExpressionNode, bool)> = Vec::new();
    let mut warnings = Vec::new();
    let restore = |typed: &mut TypedTrees,
                   published: Vec<(ExpressionHandle, ExpressionNode, bool)>| {
        for (expression, original, marked) in published.into_iter().rev() {
            *typed.expression_table.expression_mut(expression) = original;
            if marked {
                typed.pending_const_range_endpoints.insert(expression);
            }
        }
    };
    loop {
        let pending = match pending_endpoints(typed) {
            Ok(pending) => pending,
            Err(errors) => {
                restore(typed, published);
                return Err(errors);
            }
        };
        if pending.is_empty() {
            break;
        }
        let round = match evaluate_round(
            typed,
            &pending,
            selection_authority.clone(),
            operators,
            provider_bodies,
            &mut warnings,
        ) {
            Ok(round) => round,
            Err(errors) => {
                restore(typed, published);
                return Err(errors);
            }
        };
        let progress = !round.folds.is_empty();
        for (expression, folded) in round.folds {
            let original =
                std::mem::replace(typed.expression_table.expression_mut(expression), folded);
            let marked = typed.pending_const_range_endpoints.remove(&expression);
            published.push((expression, original, marked));
        }
        if round.diagnostics.is_empty() {
            break;
        }
        // Another preparation can only change the outcome of a static
        // application: its instance's cloned signature bounds are read from
        // the prepared tree, which this round's folds have just changed. A
        // plain callee reads the working tree directly, and a round without
        // progress would only repeat the same failures.
        if !(progress && round.failed_static_application) {
            restore(typed, published);
            return Err(round.diagnostics);
        }
    }
    for warning in warnings {
        eprintln!("{warning}");
    }
    Ok(())
}

/// One round over the still-authored endpoint calls against a program
/// prepared from the current working tree.
struct Round {
    /// Successfully folded calls, in evaluation order.
    folds: Vec<(ExpressionHandle, ExpressionNode)>,
    diagnostics: Vec<Diagnostic>,
    /// At least one failure was an explicit static application, whose
    /// instance bounds only a later preparation can close.
    failed_static_application: bool,
}

fn evaluate_round(
    typed: &mut TypedTrees,
    pending: &[PendingEndpoint],
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    operators: &[crate::SelectedBuildTimeBinaryOperator],
    provider_bodies: &[crate::SelectedBuildTimeProviderBody],
    warnings: &mut Vec<Diagnostic>,
) -> Result<Round, Vec<Diagnostic>> {
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

    let mut round = Round {
        folds: Vec::new(),
        diagnostics: Vec::new(),
        failed_static_application: false,
    };
    let mut originals = Vec::new();
    for endpoint in pending {
        let result = evaluate_endpoint(
            typed,
            execution,
            &admission,
            endpoint,
            selection_authority.as_deref(),
            warnings,
        );
        match result {
            Ok(folded) => {
                // Later endpoints in this round read this fold from the
                // working tree (surrounding arithmetic, enclosing arguments).
                let original = std::mem::replace(
                    typed.expression_table.expression_mut(endpoint.expression),
                    folded.clone(),
                );
                originals.push((endpoint.expression, original));
                round.folds.push((endpoint.expression, folded));
            }
            Err(reason) => {
                round.failed_static_application |= endpoint.static_application;
                round.diagnostics.push(Diagnostic::error(format!(
                    "range endpoint of `{}`: const evaluation of `{}` failed: {reason}",
                    typed
                        .type_reference_table
                        .display_name(endpoint.constrained_type),
                    typed.symbols.display_path(endpoint.machine, "::"),
                )));
            }
        }
    }

    // Execute against the immutable prepared program, never a graph changed
    // underneath its admission plan. The working tree only supplies closed
    // argument arithmetic and signature bounds. Restore it before the driver
    // publishes any folds so a failed outer call cannot leave its
    // successfully evaluated children erased.
    for (expression, original) in originals.into_iter().rev() {
        *typed.expression_table.expression_mut(expression) = original;
    }
    Ok(round)
}

fn evaluate_endpoint(
    typed: &TypedTrees,
    execution: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    endpoint: &PendingEndpoint,
    selection_authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<ExpressionNode, String> {
    // Admit this original call and qualifier before execution. The
    // invocation floor checks its callee, not every retained occurrence;
    // waiting for a parent argument check would be too late for this call.
    crate::machine_execution::admission::require_call_expression_selection(
        execution,
        endpoint.expression,
        selection_authority,
    )
    .and_then(|()| resolve_endpoint_callee(execution, endpoint))
    .and_then(|callee| {
        arguments::evaluate(
            typed,
            execution,
            admission,
            endpoint.expression,
            callee,
            selection_authority,
        )
        .map(|(arguments, argument_warnings)| (callee, arguments, argument_warnings))
    })
    .and_then(|(callee, arguments, argument_warnings)| {
        let machine = execution
            .machines()
            .iter()
            .find(|machine| machine.symbol == callee.instance)
            .ok_or_else(|| "range endpoint lost its selected machine".to_owned())?;
        let entry = execution
            .machine_states(machine)
            .first()
            .ok_or("range endpoint machine has no entry state")?;
        // A specialized instance's signature exists only in the prepared
        // tree; a plain callee's positions read the working tree so
        // already folded inner calls are visible.
        let types: &TypedTrees = if callee.static_application {
            execution
        } else {
            typed
        };
        let position = if endpoint.range_bound {
            integer_type::ScalarPosition::Integer(integer_type::IntegerPosition::prepare(
                types,
                execution,
                entry.return_type,
                selection_authority,
            )?)
        } else {
            integer_type::ScalarPosition::prepare(
                types,
                execution,
                entry.return_type,
                selection_authority,
            )?
        };
        for warning in argument_warnings {
            if !warnings.contains(&warning) {
                warnings.push(warning);
            }
        }
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
                callee.instance,
                arguments,
                custody,
            )?
        } else {
            admission.evaluate_const_evaluable_machine_symbol_for_invocation(
                execution,
                callee.instance,
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
        position.require_value(execution, admission, &value)?;
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
    })
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
                            if let Some((machine, _)) = selected_endpoint_machine(typed, expression)
                            {
                                append_signature_bounds(typed, machine, &mut work)?;
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                active.pop();
                visited.push(expression);
                let Some((machine, static_application)) =
                    selected_endpoint_machine(typed, expression)
                else {
                    continue;
                };
                pending.push(PendingEndpoint {
                    constrained_type,
                    expression,
                    machine: machine.symbol,
                    source_span: typed.expression_table.source_span(expression),
                    static_application,
                    range_bound: !argument_positions.contains(&expression),
                });
            }
        }
    }
    Ok(pending)
}

/// The endpoint's callee in the working tree and whether the call is an
/// explicit static application. Returns `None` for calls this route never
/// folds: runtime receivers, evidence/dispatch forms, and generic callees
/// whose binders would need inference from ordinary arguments.
fn selected_endpoint_machine(
    typed: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(&typed_trees::machine::Machine, bool)> {
    let ExpressionNode::Call(call) = typed.expression_table.expression(expression) else {
        return None;
    };
    if !call.evidence_arguments.is_empty()
        || call.static_machine_parameter.is_valid()
        || call.static_requirement_dispatch.is_some()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
    {
        return None;
    }
    // Only closed const spellings are static arguments here: a literal or a
    // const declaration. Type, machine, evidence and nested applications keep
    // ordinary call validation.
    if !call.machine_arguments.iter().all(|argument| {
        argument.application.is_none()
            && argument.evidence_projection.is_none()
            && (argument.const_literal.is_some()
                || typed
                    .const_declarations()
                    .iter()
                    .any(|declaration| declaration.symbol == argument.symbol))
    }) {
        return None;
    }
    let machine = typed.machines().iter().find(|machine| {
        typed
            .machine_states(machine)
            .first()
            .is_some_and(|entry| typed.call_has_no_runtime_receiver(call, machine, entry))
    })?;
    if machine.type_parameters.is_empty() {
        return call
            .machine_arguments
            .is_empty()
            .then_some((machine, false));
    }
    // Ordinary arguments do not close generic binders: an application with
    // no static arguments needs inference and stays with ordinary call
    // validation. A partially supplied one is pending so evaluation can name
    // the missing argument.
    (!call.machine_arguments.is_empty() && machine.conformance_bounds.is_empty())
        .then_some((machine, true))
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
