//! Provider-dependent const-generic applications retained in type position.
//!
//! A const-generic argument carrying an authored machine call ordinarily folds
//! on the pre-resolution probe in `const_generic_calls`. When the call's
//! reachable closure holds a resolved boundary-operator use, no authored
//! selection or visible satisfier can authorize its execution: the argument
//! keeps its `ConstExpression` node and its root expression is marked in
//! `pending_const_range_endpoints`, the shared pending-constant set that keeps
//! consuming call tuples open and fails checked lowering if it survives.
//!
//! Omega's settled provider rows fold it here, exactly as deferred
//! fixed-array lengths and range endpoints do: the authored occurrence rebinds
//! to its selected provider's ordinary checked body on a private execution
//! copy, the argument evaluates under its declared carrier, and the site
//! becomes a `Named` literal. Distinct build plans therefore produce distinct
//! results while an unselected satisfier stays inert.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees,
    data::TypeParameterKind,
    expression::ExpressionHandle,
    name::Identifier,
    types::{TypeReferenceHandle, TypeReferenceNode},
};

use crate::machine_execution::admission::{expression_children, require_closed_expression_custody};

/// One `ConstExpression` retained in a declared const-generic argument slot.
struct PendingConstApplication {
    /// The `ConstExpression` type-reference node occupying the argument slot.
    site: TypeReferenceHandle,
    /// Its retained authored root expression.
    root: ExpressionHandle,
    /// The const parameter's declared carrier type, when the generic base
    /// resolves to a declaration with const parameters.
    destination: Option<TypeReferenceHandle>,
}

/// One authored call reachable inside a pending const application. Call nodes
/// cover both named machine calls and boundary-operator token uses; facts
/// separate which uses actually require Omega's selected providers.
struct PendingApplicationCall {
    site: TypeReferenceHandle,
    root: ExpressionHandle,
    expression: ExpressionHandle,
    /// The callee's working-tree machine, or invalid when the call names no
    /// ordinary machine (evidence, dispatch, and boundary-operator forms).
    machine: SymbolHandle,
    /// Whether `machine` is a generic template requiring its prepared
    /// instance.
    static_application: bool,
}

/// Pending const applications and the calls their evaluation must run.
struct ConstApplicationPlan {
    applications: Vec<PendingConstApplication>,
    calls: Vec<PendingApplicationCall>,
}

/// Whether any retained const application can only execute once Omega
/// supplies exact provider bodies. The deferral scan mirrors
/// `pending_endpoint_calls_need_operator_selection`: prepare once so static
/// applications resolve to their instances, then ask admission's call closure
/// about resolved boundary uses. A boundary token use inside the argument
/// expression itself (e.g. `Buffer<a % b>`) carries the same dependency
/// without naming a machine.
pub(crate) fn pending_const_applications_need_operator_selection(
    typed: &TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<bool, Vec<Diagnostic>> {
    let plan = const_application_plan(typed);
    if plan.applications.is_empty() {
        return Ok(false);
    }
    let prepared = crate::PreparedBuildMachineProgram::prepare(typed)?;
    let execution = prepared.typed();
    let facts = typed_trees_to_checked_trees::derive_pre_flow_operator_selections(execution);
    let admission = crate::BuildTimeAdmissionPlan::infer(execution, selection_authority.clone());
    let boundary_use = |selected_operator_symbol: SymbolHandle| {
        execution
            .operators()
            .iter()
            .any(|operator| operator.symbol == selected_operator_symbol && operator.is_boundary)
    };
    let boundary_use_expression = |expression| {
        facts
            .uses_with_status(checked_trees::CheckedOperatorResolutionStatus::Resolved)
            .any(|fact| {
                fact.expression == expression && boundary_use(fact.selected_operator_symbol)
            })
            || facts.named_uses().any(|fact| {
                fact.expression == expression && boundary_use(fact.selected_operator_symbol)
            })
    };
    Ok(plan.applications.iter().any(|application| {
        let subtree = expression_subtree(execution, application.root);
        subtree
            .iter()
            .any(|expression| boundary_use_expression(*expression))
            || plan
                .calls
                .iter()
                .filter(|call| call.site == application.site)
                .any(|call| {
                    call.machine.is_valid()
                        && admission.closure_needs_operator_selection(
                            execution,
                            resolve_application_callee(execution, call)
                                .map_or(call.machine, |callee| callee.instance),
                            &facts,
                        )
                })
    }))
}

/// Record every retained const application's expressions as deferred to
/// selected execution, the same marks `defer_pending_endpoint_calls` uses:
/// interim checking reads them as pending constants, consuming call tuples
/// stay open, and each fold removes its marks.
pub(crate) fn defer_pending_const_applications(
    typed: &mut TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let plan = const_application_plan(typed);
    for application in &plan.applications {
        typed.pending_const_range_endpoints.insert(application.root);
    }
    for call in &plan.calls {
        typed.pending_const_range_endpoints.insert(call.expression);
    }
    Ok(())
}

/// Finish retained const-generic applications under the exact selected rows
/// Omega settled for this program. Each fold substitutes the `ConstExpression`
/// node for the `Named` literal its canonical value spells. An application
/// whose required provider body never settled reports the dependency instead
/// of disappearing silently.
pub(crate) fn evaluate_selected_const_applications(
    typed: &mut TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    selected: crate::SelectedBuildTimeOperators<'_>,
) -> Result<(), Vec<Diagnostic>> {
    let crate::SelectedBuildTimeOperators {
        operators,
        provider_bodies,
    } = selected;
    // Folds published by earlier rounds, with their authored originals and the
    // marks the deferral path set. A round that ends in rejection restores
    // every one so a rejected program keeps its authored applications.
    let mut published: Vec<(
        TypeReferenceHandle,
        TypeReferenceNode,
        Vec<ExpressionHandle>,
    )> = Vec::new();
    let mut warnings = Vec::new();
    let restore = |typed: &mut TypedTrees,
                   published: Vec<(
        TypeReferenceHandle,
        TypeReferenceNode,
        Vec<ExpressionHandle>,
    )>| {
        for (site, original, marked) in published.into_iter().rev() {
            typed.type_reference_table.substitute_node(site, original);
            typed.pending_const_range_endpoints.extend(marked);
        }
    };
    loop {
        let plan = const_application_plan(typed);
        if plan.applications.is_empty() {
            break;
        }
        let round = match evaluate_application_round(
            typed,
            &plan,
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
        for fold in round.folds {
            let original = typed.type_reference_table.type_reference(fold.site).clone();
            typed
                .type_reference_table
                .substitute_node(fold.site, fold.node);
            let mut marked: Vec<_> = plan
                .calls
                .iter()
                .filter(|call| call.site == fold.site)
                .filter_map(|call| {
                    typed
                        .pending_const_range_endpoints
                        .remove(&call.expression)
                        .then_some(call.expression)
                })
                .collect();
            if typed.pending_const_range_endpoints.remove(&fold.root) {
                marked.push(fold.root);
            }
            published.push((fold.site, original, marked));
        }
        if round.diagnostics.is_empty() {
            break;
        }
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

/// One round over still-retained applications against a program prepared from
/// the current working tree, mirroring the range-endpoint round.
struct ApplicationRound {
    folds: Vec<ApplicationFold>,
    diagnostics: Vec<Diagnostic>,
}

struct ApplicationFold {
    site: TypeReferenceHandle,
    root: ExpressionHandle,
    node: TypeReferenceNode,
}

fn evaluate_application_round(
    typed: &mut TypedTrees,
    plan: &ConstApplicationPlan,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
    operators: &[crate::SelectedBuildTimeBinaryOperator],
    provider_bodies: &[crate::SelectedBuildTimeProviderBody],
    warnings: &mut Vec<Diagnostic>,
) -> Result<ApplicationRound, Vec<Diagnostic>> {
    // Bind exact authored provider occurrences before cloning specializations,
    // on a private copy: the caller's tree keeps its authored selection and
    // source-owned handles for final checking.
    let selected_execution = if provider_bodies.is_empty() {
        None
    } else {
        Some(
            crate::machine_execution::selected_operators::apply_selected_provider_bodies(
                typed,
                provider_bodies,
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?,
        )
    };
    let prepared =
        crate::PreparedBuildMachineProgram::prepare(selected_execution.as_ref().unwrap_or(typed))?;
    let execution = prepared.typed();
    let admission = crate::BuildTimeAdmissionPlan::infer(execution, selection_authority.clone())
        .with_selected_operators(execution, operators)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;

    let mut round = ApplicationRound {
        folds: Vec::new(),
        diagnostics: Vec::new(),
    };
    for application in &plan.applications {
        match evaluate_const_application(
            typed,
            execution,
            &admission,
            application,
            selection_authority.as_deref(),
            warnings,
        ) {
            Ok(fold) => round.folds.push(fold),
            Err(reason) => {
                round.diagnostics.push(
                    Diagnostic::error(format!(
                        "const-generic application: const evaluation failed: {reason}"
                    ))
                    .with_source_span(typed.expression_table.source_span(application.root)),
                );
            }
        }
    }
    Ok(round)
}

fn evaluate_const_application(
    typed: &TypedTrees,
    execution: &TypedTrees,
    admission: &crate::BuildTimeAdmissionPlan,
    application: &PendingConstApplication,
    selection_authority: Option<&dyn crate::BuildTimeSelectionAuthority>,
    warnings: &mut Vec<Diagnostic>,
) -> Result<ApplicationFold, String> {
    require_closed_expression_custody(execution, application.root, selection_authority)?;
    let destination = application
        .destination
        .ok_or("const-generic application has no declared const destination".to_owned())?;
    let carrier = crate::const_evaluation::const_generic_expressions::exact_probe_destination(
        execution,
        destination,
    )
    .ok_or(
        "const-generic application needs an unconstrained exact builtin integer or Boolean carrier"
            .to_owned(),
    )?;
    let invocation = crate::const_evaluation::range_endpoints::arguments::Invocation {
        working: typed,
        execution,
        admission,
        authority: selection_authority,
    };
    let (value, additions) =
        crate::const_evaluation::const_generic_expressions::value::evaluate_closed_const_argument(
            execution,
            application.root,
            carrier,
            &invocation,
        )?;
    for warning in additions {
        if !warnings.contains(&warning) {
            warnings.push(warning);
        }
    }
    let spelling = match value.decode_encoding() {
        Some(language_semantics::const_value::DecodedCanonicalConstValue::Integer {
            value,
            ..
        }) => value.to_string(),
        Some(language_semantics::const_value::DecodedCanonicalConstValue::Boolean(_)) => {
            value.atom()
        }
        _ => {
            return Err("const application returned an unsupported canonical value".to_owned());
        }
    };
    Ok(ApplicationFold {
        site: application.site,
        root: application.root,
        node: TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated(spelling),
        },
    })
}

/// Resolve a pending call's executable callee in the prepared tree through the
/// shared endpoint machinery: a static application must name its concrete
/// instance, a plain callee is itself.
fn resolve_application_callee(
    execution: &TypedTrees,
    call: &PendingApplicationCall,
) -> Result<crate::const_evaluation::range_endpoints::EndpointCallee, String> {
    crate::const_evaluation::range_endpoints::resolve_endpoint_callee(
        execution,
        &crate::const_evaluation::range_endpoints::PendingEndpoint {
            expression: call.expression,
            root: call.root,
            machine: call.machine,
            static_application: call.static_application,
        },
    )
}

/// Depth-first handle set of one expression's subtree, for matching
/// boundary-operator uses recorded directly on argument expressions.
fn expression_subtree(typed: &TypedTrees, root: ExpressionHandle) -> Vec<ExpressionHandle> {
    let mut subtree = Vec::new();
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if !typed.expression_table.expression_is_valid(expression) {
            continue;
        }
        if subtree.contains(&expression) {
            continue;
        }
        subtree.push(expression);
        pending.extend(expression_children(typed, expression));
    }
    subtree
}

/// Every `ConstExpression` node that occupies a declared const-generic
/// argument position and still carries authored calls. Positions under a
/// domain constraint's own arguments are owned by `const_domain_facts`, so
/// this scan descends only through type constructors.
fn const_application_plan(typed: &TypedTrees) -> ConstApplicationPlan {
    let mut plan = ConstApplicationPlan {
        applications: Vec::new(),
        calls: Vec::new(),
    };
    for (_, base_symbol, arguments) in typed.type_reference_table.generic_type_reference_sites() {
        let parameters = generic_type_parameters(typed, base_symbol);
        for (index, argument) in typed
            .type_reference_table
            .type_reference_handles(arguments)
            .iter()
            .enumerate()
        {
            let destination = parameters
                .and_then(|parameters| parameters.get(index))
                .and_then(|parameter| match &parameter.kind {
                    TypeParameterKind::Const { type_reference }
                    | TypeParameterKind::Value { type_reference } => Some(*type_reference),
                    _ => None,
                });
            visit_argument(typed, *argument, destination, &mut plan);
        }
    }
    plan
}

fn generic_type_parameters(
    typed: &TypedTrees,
    base_symbol: SymbolHandle,
) -> Option<&[typed_trees::data::TypeParameter]> {
    if let Some(data) = typed
        .data_definitions()
        .iter()
        .find(|data| data.symbol == base_symbol)
    {
        return Some(typed.data_type_parameters(data));
    }
    if let Some(machine) = typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == base_symbol)
    {
        return Some(typed.machine_type_parameters(machine));
    }
    if let Some(trait_definition) = typed
        .traits()
        .iter()
        .find(|candidate| candidate.symbol == base_symbol)
    {
        return Some(typed.trait_type_parameters(trait_definition));
    }
    None
}

/// Walk one generic argument's type constructors; only `ConstExpression`
/// nodes reached this way bind to the generic's declared parameter, while a
/// nested `Generic` is its own parameter-scoped site.
fn visit_argument(
    typed: &TypedTrees,
    reference: TypeReferenceHandle,
    destination: Option<TypeReferenceHandle>,
    plan: &mut ConstApplicationPlan,
) {
    match typed.type_reference_table.type_reference(reference) {
        TypeReferenceNode::ConstExpression(root) => {
            // Only a root that names an ordinary machine call is an
            // application this machinery may finish. A boundary-operator
            // token or any other expression at the root is a malformed
            // argument shape that checking rejects on its own terms.
            if crate::const_evaluation::range_endpoints::selected_endpoint_machine(typed, *root)
                .is_none()
            {
                return;
            }
            let calls = collect_calls(typed, *root);
            if calls.is_empty() {
                return;
            }
            plan.applications.push(PendingConstApplication {
                site: reference,
                root: *root,
                destination,
            });
            plan.calls.extend(calls.into_iter().map(|call| {
                let (machine, static_application) =
                    crate::const_evaluation::range_endpoints::selected_endpoint_machine(
                        typed, call,
                    )
                    .map(|(machine, static_application)| (machine.symbol, static_application))
                    .unwrap_or((SymbolHandle::invalid(), false));
                PendingApplicationCall {
                    site: reference,
                    root: *root,
                    expression: call,
                    machine,
                    static_application,
                }
            }));
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            visit_argument(typed, *base_type, destination, plan);
        }
        TypeReferenceNode::Reference { referee, .. } => {
            visit_argument(typed, *referee, destination, plan);
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            visit_argument(typed, *element_type, destination, plan);
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            let parameters = generic_type_parameters(typed, *base_symbol);
            for (index, nested) in typed
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .enumerate()
            {
                let destination = parameters
                    .and_then(|parameters| parameters.get(index))
                    .and_then(|parameter| match &parameter.kind {
                        TypeParameterKind::Const { type_reference }
                        | TypeParameterKind::Value { type_reference } => Some(*type_reference),
                        _ => None,
                    });
                visit_argument(typed, *nested, destination, plan);
            }
        }
        _ => {}
    }
}

/// Every call node inside an expression's own subtree, in source order. Calls
/// nested inside a callee's signature bounds stay the callee's responsibility.
fn collect_calls(typed: &TypedTrees, root: ExpressionHandle) -> Vec<ExpressionHandle> {
    let mut calls = Vec::new();
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if !typed.expression_table.expression_is_valid(expression) {
            continue;
        }
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        if matches!(
            typed.expression_table.expression(expression),
            typed_trees::expression::ExpressionNode::Call(_)
        ) {
            calls.push(expression);
        }
        pending.extend(expression_children(typed, expression));
    }
    calls
}

#[cfg(test)]
mod tests;
