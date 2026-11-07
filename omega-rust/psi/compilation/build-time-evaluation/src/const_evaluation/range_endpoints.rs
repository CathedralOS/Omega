//! Evaluate closed integer expressions retained by source contracts.
//!
//! A contract keeps its symbolic comparison and exact lexical context. Only
//! its closed integer expression becomes an evaluation root. Calls retain
//! their selected declarations, argument custody and provider dependencies;
//! the shared admission owner checks them before interpretation. Completed
//! folds publish transactionally so all later proof and layout readers agree.
//!
//! Provider-dependent calls defer to the owning pre-check continuation. Their
//! exact selected bodies execute on a private prepared program without
//! replacing the caller's selected operation or borrowing a callee activation
//! to interpret the caller's expression.

use crate::build_time_evaluation::EvaluationDomain;
use crate::const_evaluation::const_generic_expressions::value::evaluate_integer_endpoint;
use crate::machine_execution::admission::require_closed_expression_custody;
use crate::unique::extend_unique;
use crate::{BuildTimeAdmissionPlan, SelectedBuildTimeOperators};
use diagnostics::Diagnostic;
use numerics::literals::LandedIntegerType;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::state::state_by_symbol;
use typed_trees::types::PrimitiveType;

pub(super) mod arguments;
mod integer_type;

/// The builtin integer carrier's literal landing for endpoint literal
/// spellings — snapshot argument substitution and discharge stand-ins.
pub(super) fn integer_landing(carrier: PrimitiveType) -> Result<LandedIntegerType, String> {
    match carrier {
        PrimitiveType::I8 => Ok(LandedIntegerType::I8),
        PrimitiveType::I16 => Ok(LandedIntegerType::I16),
        PrimitiveType::I32 => Ok(LandedIntegerType::I32),
        PrimitiveType::I64 => Ok(LandedIntegerType::I64),
        PrimitiveType::U8 => Ok(LandedIntegerType::U8),
        PrimitiveType::U16 => Ok(LandedIntegerType::U16),
        PrimitiveType::U32 => Ok(LandedIntegerType::U32),
        PrimitiveType::U64 => Ok(LandedIntegerType::U64),
        _ => Err("range endpoint lost its builtin integer carrier".into()),
    }
}

#[derive(PartialEq, Eq)]
pub(crate) struct PendingEndpoint {
    pub(crate) expression: ExpressionHandle,
    pub(crate) root: ExpressionHandle,
    /// The resolved callee in the working tree: a plain machine, or the
    /// generic template of a static application, explicit or inferred.
    pub(crate) machine: symbols::SymbolHandle,
    /// The authored target is generic; the executable callee must be the
    /// prepared tree's specialized instance.
    pub(crate) static_application: bool,
    pub(crate) contract_expression: bool,
}

pub(crate) struct IntegerRoot {
    pub(crate) context: typed_trees::constant_recipes::ConstantExpressionContext,
    pub(crate) expression: ExpressionHandle,
}

/// Every nonliteral bound is a candidate for the shared evaluator. Calls
/// separately name required constant invocations and provider dependencies;
/// an unclosed call-free bound remains owned by ordinary range validation.
pub(crate) struct IntegerRootPlan {
    pub(crate) roots: Vec<IntegerRoot>,
    pub(crate) calls: Vec<PendingEndpoint>,
}

impl IntegerRootPlan {
    /// The legacy source fold writes expression storage once. Its work list
    /// is separate from the complete lexical context roster retained for
    /// portable entries; sharing work never discards an owner's context.
    fn evaluation_roots(&self) -> impl Iterator<Item = &IntegerRoot> {
        let mut visited = std::collections::HashSet::new();
        self.roots
            .iter()
            .filter(move |root| visited.insert(root.expression))
    }
}

#[cfg(test)]
mod root_plan_tests {
    use super::*;
    use typed_trees::constant_recipes::ConstantExpressionContext;

    #[test]
    fn constant_expression_contexts_share_legacy_folds_without_losing_owners() {
        let expression = ExpressionHandle::from_parts(17, 1);
        let plan = IntegerRootPlan {
            roots: [23, 29]
                .map(|owner| IntegerRoot {
                    expression,
                    context: ConstantExpressionContext {
                        owner: symbols::SymbolHandle::from_parts(owner, 1),
                        result_type: TypeReferenceHandle::from_parts(31, 1),
                        ..Default::default()
                    },
                })
                .into(),
            calls: Vec::new(),
        };
        assert_eq!(plan.evaluation_roots().count(), 1);
        assert_eq!(plan.roots.len(), 2);
    }
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
        &EvaluationDomain::all(),
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
    domain: &EvaluationDomain,
) -> Result<bool, Vec<Diagnostic>> {
    let pending = pending_endpoints(typed, domain)?;
    if pending.is_empty() {
        return Ok(false);
    }
    // Admission and the deferral scan must see the same closed static
    // applications as the eventual evaluation; prepare once and reuse it.
    let prepared = crate::PreparedBuildMachineProgram::prepare_in(typed, domain)?;
    let execution = prepared.typed();
    let facts = validation::derive_pre_flow_operator_selections(execution);
    let admission = domain.admission(execution, selection_authority);
    Ok(pending.iter().any(|endpoint| {
        // A type argument awaiting its own bound keeps this application
        // unspecialized. Its original template can still demand a provider;
        // this conservative deferral scan grants no invocation admission.
        let machine = resolve_endpoint_callee(execution, endpoint)
            .map_or(endpoint.machine, |callee| callee.instance);
        admission.closure_needs_execution_preparation(execution, machine, &facts)
    }))
}

/// The executable instance and the arena owning its prepared signature.
#[derive(Clone, Copy)]
pub(super) struct EndpointCallee {
    pub(super) instance: symbols::SymbolHandle,
    pub(super) static_application: bool,
}

impl EndpointCallee {}

/// Resolve the executable callee in the prepared tree. A static application
/// must have been rewritten there to its concrete instance; a call that still
/// carries static arguments was not specialized, which after `prepare` means
/// a binder is missing or open, so ask for the explicit argument.
pub(super) fn resolve_endpoint_callee(
    execution: &TypedTrees,
    endpoint: &PendingEndpoint,
) -> Result<EndpointCallee, String> {
    if !endpoint.static_application {
        return Ok(EndpointCallee {
            instance: endpoint.machine,
            static_application: false,
        });
    }
    let Some(call) = execution.expression_table.call_node(endpoint.expression) else {
        return Err("range endpoint lost its authored static application".to_owned());
    };
    let template = execution
        .machine(endpoint.machine)
        .ok_or("range endpoint lost its generic template")?;
    if !call.machine_arguments.is_empty() {
        if call.machine_arguments.len() == template.type_parameters.len() {
            return Err(format!(
                "static application of `{}` awaits a complete validated specialization",
                template.name,
            ));
        }
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
            state_by_symbol(execution.machine_states(machine), call.target_symbol).is_some()
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
    defer_pending_endpoint_calls_in(typed, &EvaluationDomain::all())
}

pub(crate) fn defer_pending_endpoint_calls_in(
    typed: &mut TypedTrees,
    domain: &EvaluationDomain,
) -> Result<(), Vec<Diagnostic>> {
    for endpoint in pending_endpoints(typed, domain)? {
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
    domain: &EvaluationDomain,
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
        let plan = match integer_root_plan(typed, domain) {
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
            domain,
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
        // A static tuple may belong to this endpoint or a transitive callee.
        // Reprepare after a fold so either closure sees its newly closed types.
        // Each progressing round permanently removes a nonliteral root; no
        // progress means another round would repeat the same failures.
        if !progress {
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
}

fn evaluate_round(
    typed: &mut TypedTrees,
    plan: &IntegerRootPlan,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    operators: &[crate::SelectedBuildTimeBinaryOperator],
    provider_bodies: &[crate::SelectedBuildTimeProviderBody],
    warnings: &mut Vec<Diagnostic>,
    domain: &EvaluationDomain,
) -> Result<Round, Vec<Diagnostic>> {
    // Bind exact authored provider occurrences before cloning specializations.
    // Rebinding only the template after preparation would leave its cloned
    // body with an unselected operator. Both steps remain private, and the
    // existing provider validator checks the original occurrence first.
    let (prepared, admission) = crate::const_evaluation::prepared_admission(
        typed,
        provider_bodies,
        operators,
        selection_authority.clone(),
        domain,
    )?;
    let execution = prepared.typed();

    let mut round = Round {
        folds: Vec::new(),
        diagnostics: Vec::new(),
    };
    let mut originals = Vec::new();
    for endpoint in plan.evaluation_roots() {
        let result = evaluate_root(
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
                let required = plan
                    .calls
                    .iter()
                    .any(|call| call.root == endpoint.expression);
                if !required {
                    continue;
                }
                round.diagnostics.push(
                    Diagnostic::error(format!(
                        "contract integer expression: const evaluation failed: {reason}"
                    ))
                    .with_source_span(typed.expression_table.source_span(endpoint.expression)),
                );
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

fn evaluate_root(
    typed: &TypedTrees,
    execution: &TypedTrees,
    admission: &BuildTimeAdmissionPlan,
    endpoint: &IntegerRoot,
    selection_authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<ExpressionNode, String> {
    require_closed_expression_custody(execution, endpoint.expression, selection_authority)?;
    let invocation = arguments::Invocation {
        working: typed,
        execution,
        admission,
        authority: selection_authority,
    };
    let (literal, additions) =
        evaluate_integer_endpoint(execution, endpoint.expression, &invocation)?;
    extend_unique(warnings, additions);
    Ok(ExpressionNode::Integer(literal))
}

pub(crate) fn pending_endpoints(
    typed: &TypedTrees,
    domain: &EvaluationDomain,
) -> Result<Vec<PendingEndpoint>, Vec<Diagnostic>> {
    Ok(integer_root_plan(typed, domain)?.calls)
}

pub(crate) fn integer_root_plan(
    typed: &TypedTrees,
    domain: &EvaluationDomain,
) -> Result<IntegerRootPlan, Vec<Diagnostic>> {
    let mut pending = Vec::new();
    let mut roots = Vec::new();
    for root in super::contract_expressions::roots(typed, domain) {
        roots.push(IntegerRoot {
            context: root.context,
            expression: root.expression,
        });
        extend_unique(&mut pending, root.invocations);
    }
    Ok(IntegerRootPlan {
        roots,
        calls: pending,
    })
}

/// The endpoint's callee in the working tree and whether the call is an
/// static application. Returns `None` for calls this route never
/// folds: runtime receivers and evidence/dispatch forms. Generic callees still
/// require an exact concrete instance from ordinary specialization.
pub(super) fn selected_endpoint_machine(
    typed: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(&typed_trees::machine::Machine, bool)> {
    let call = typed.expression_table.call_node(expression)?;
    if !call.evidence_arguments.is_empty()
        || call.static_machine_parameter.is_valid()
        || !call.selects_only_nominal_route()
    {
        return None;
    }
    // Discover type/const applications, not a second specialization
    // rule. Preparation validates types in the caller's scope and resolves the
    // complete tuple; resolve_endpoint_callee then requires its concrete
    // instance. The instance's substituted signature owns scalar admission.
    // Machine/evidence applications still need their full dispatch context.
    if !call.machine_arguments.iter().all(|argument| {
        argument.application.is_none()
            && argument.evidence_projection.is_none()
            && (argument.type_reference.is_valid()
                || matches!(
                    typed.symbols.get(argument.symbol).kind,
                    symbols::SymbolKind::BuiltinType | symbols::SymbolKind::Data
                )
                || argument.const_literal.is_some()
                || typed.const_declaration_by_symbol(argument.symbol).is_some())
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
    // Discovery is not inference or admission. Let ordinary specialization
    // infer from declared argument types, then require its complete instance
    // in resolve_endpoint_callee before interpreting any value. An unused,
    // underdetermined binder cannot disappear merely because this is a bound.
    machine
        .conformance_bounds
        .is_empty()
        .then_some((machine, true))
}
