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
//! live in the prepared tree, so their positions and original argument
//! expressions read that same tree. An application that
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
//! ordinary body checking. Wrapping/Saturating parameters accept exact initial
//! anonymous landings and retain the callee policy during execution; policy-bearing
//! results and parameterized domains remain outside this route. Calls computing signature bounds are invocation
//! dependencies, independent of declaration order.
//!
//! Evaluation runs in rounds. Each round prepares the execution program from
//! the current working tree and folds every endpoint that closes; the driver
//! publishes those folds and prepares again only when a static application
//! failed while progress was made, because its instance's cloned signature
//! bounds are read from the prepared tree and a template bound that folded
//! this round (`bounded<const N>(value: u64[0..=limit()])`) is only visible
//! there after re-preparation. A round with no progress reports its
//! failures and every published fold is restored, so a rejected program
//! keeps its authored calls.
//!
//! Each complete endpoint runs through the shared closed scalar evaluator.
//! Discovery collects roots independently of calls: removing a helper cannot
//! prevent a closed composition from reaching evaluation. Unclosed optional
//! roots remain authored for ordinary dependent/runtime range validation.
//! Its shape pass checks all calls and operators before selective execution;
//! the separate exhaustive custody walk preserves every original selection,
//! including skipped branches. Call results retain their declared carriers,
//! while an anonymous root remains an exact proof integer (including an
//! exclusive bound one beyond the subject carrier). No callee activation is
//! borrowed to interpret the caller's expression.
//! The evaluator reads an immutable prepared tree. Whole endpoint literals
//! and their deferral marks publish transactionally; the working tree supplies
//! previously folded signature bounds between rounds.

use crate::SelectedBuildTimeOperators;
use diagnostics::Diagnostic;
#[cfg(test)]
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
    expression: ExpressionHandle,
    root: ExpressionHandle,
    /// The resolved callee in the working tree: a plain machine, or the
    /// generic template of an explicit static application.
    machine: symbols::SymbolHandle,
    /// The authored call carries explicit static machine arguments; the
    /// executable callee is the prepared tree's specialized instance.
    static_application: bool,
}

struct EndpointRoot {
    constrained_type: TypeReferenceHandle,
    expression: ExpressionHandle,
}

/// Every nonliteral bound is a candidate for the shared evaluator. Calls
/// separately name required constant invocations and provider dependencies;
/// an unclosed call-free bound remains owned by ordinary range validation.
struct EndpointPlan {
    roots: Vec<EndpointRoot>,
    calls: Vec<PendingEndpoint>,
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

/// The executable instance and the arena owning its prepared signature.
#[derive(Clone, Copy)]
pub(super) struct EndpointCallee {
    pub(super) instance: symbols::SymbolHandle,
    pub(super) static_application: bool,
}

impl EndpointCallee {
    #[cfg(test)]
    pub(super) fn plain(machine: symbols::SymbolHandle) -> Self {
        Self {
            instance: machine,
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
        typed.pending_const_range_endpoints.insert(endpoint.root);
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
    let mut published: Vec<(ExpressionHandle, ExpressionNode, Vec<ExpressionHandle>)> = Vec::new();
    let mut warnings = Vec::new();
    let restore =
        |typed: &mut TypedTrees,
         published: Vec<(ExpressionHandle, ExpressionNode, Vec<ExpressionHandle>)>| {
            for (expression, original, marked) in published.into_iter().rev() {
                *typed.expression_table.expression_mut(expression) = original;
                typed.pending_const_range_endpoints.extend(marked);
            }
        };
    loop {
        let plan = match endpoint_plan(typed) {
            Ok(plan) => plan,
            Err(errors) => {
                restore(typed, published);
                return Err(errors);
            }
        };
        if plan.roots.is_empty() {
            break;
        }
        let round = match evaluate_round(
            typed,
            &plan,
            selection_authority.clone(),
            operators,
            provider_bodies,
            &mut warnings,
        ) {
            Ok(round) => round,
            // Optional closed probing must not make preparation a new
            // requirement for runtime or dependent range declarations.
            Err(_) if plan.calls.is_empty() => break,
            Err(errors) => {
                restore(typed, published);
                return Err(errors);
            }
        };
        let progress = !round.folds.is_empty();
        for (expression, folded) in round.folds {
            let original =
                std::mem::replace(typed.expression_table.expression_mut(expression), folded);
            let mut marked: Vec<_> = plan
                .calls
                .iter()
                .filter(|call| call.root == expression)
                .filter_map(|call| {
                    typed
                        .pending_const_range_endpoints
                        .remove(&call.expression)
                        .then_some(call.expression)
                })
                .collect();
            if typed.pending_const_range_endpoints.remove(&expression) {
                marked.push(expression);
            }
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

/// One round over the still-authored endpoint roots against a program
/// prepared from the current working tree.
struct Round {
    /// Successfully folded whole bounds, in dependency order.
    folds: Vec<(ExpressionHandle, ExpressionNode)>,
    diagnostics: Vec<Diagnostic>,
    /// At least one failure was an explicit static application, whose
    /// instance bounds only a later preparation can close.
    failed_static_application: bool,
}

fn evaluate_round(
    typed: &mut TypedTrees,
    plan: &EndpointPlan,
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
    for endpoint in &plan.roots {
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
                let mut required_calls = plan
                    .calls
                    .iter()
                    .filter(|call| call.root == endpoint.expression)
                    .peekable();
                if required_calls.peek().is_none() {
                    continue;
                }
                round.failed_static_application |=
                    required_calls.any(|call| call.static_application);
                round.diagnostics.push(Diagnostic::error(format!(
                    "range endpoint of `{}`: const evaluation failed: {reason}",
                    typed
                        .type_reference_table
                        .display_name(endpoint.constrained_type),
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
    endpoint: &EndpointRoot,
    selection_authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<ExpressionNode, String> {
    crate::machine_execution::admission::require_closed_expression_custody(
        execution,
        endpoint.expression,
        selection_authority,
    )?;
    let invocation = arguments::Invocation {
        working: typed,
        execution,
        admission,
        authority: selection_authority,
    };
    let (literal, additions) =
        crate::const_evaluation::const_generic_expressions::value::evaluate_integer_endpoint(
            execution,
            endpoint.expression,
            &invocation,
        )?;
    for warning in additions {
        if !warnings.contains(&warning) {
            warnings.push(warning);
        }
    }
    Ok(ExpressionNode::Integer(literal))
}

fn pending_endpoints(typed: &TypedTrees) -> Result<Vec<PendingEndpoint>, Vec<Diagnostic>> {
    Ok(endpoint_plan(typed)?.calls)
}

fn endpoint_plan(typed: &TypedTrees) -> Result<EndpointPlan, Vec<Diagnostic>> {
    let mut pending = Vec::new();
    let mut roots = Vec::new();
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
            let mut work = vec![(*maximum, false, *maximum), (*minimum, false, *minimum)];
            let mut active = Vec::new();
            while let Some((expression, leaving, root)) = work.pop() {
                if !typed.expression_table.expression_is_valid(expression) {
                    continue;
                }
                if !leaving {
                    if active.contains(&expression) {
                        return Err(vec![Diagnostic::error("cyclic range endpoint expression")]);
                    }
                    if visited.contains(&(expression, root)) {
                        continue;
                    }
                    active.push(expression);
                    work.push((expression, true, root));
                    let children =
                        crate::machine_execution::admission::expression_children(typed, expression);
                    work.extend(children.into_iter().rev().map(|child| (child, false, root)));
                    if let Some((machine, _)) = selected_endpoint_machine(typed, expression) {
                        append_signature_bounds(typed, machine, &mut work)?;
                    }
                    continue;
                }
                active.pop();
                visited.push((expression, root));
                if expression == root
                    && !matches!(
                        typed.expression_table.expression(root),
                        ExpressionNode::Integer(_)
                    )
                {
                    roots.push(EndpointRoot {
                        constrained_type,
                        expression: root,
                    });
                }
                let Some((machine, static_application)) =
                    selected_endpoint_machine(typed, expression)
                else {
                    continue;
                };
                pending.push(PendingEndpoint {
                    expression,
                    root,
                    machine: machine.symbol,
                    static_application,
                });
            }
        }
    }
    Ok(EndpointPlan {
        roots,
        calls: pending,
    })
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
    work: &mut Vec<(ExpressionHandle, bool, ExpressionHandle)>,
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
                    work.push((*maximum, false, *maximum));
                    work.push((*minimum, false, *minimum));
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
