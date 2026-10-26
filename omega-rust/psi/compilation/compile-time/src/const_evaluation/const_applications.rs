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
//!
//! Folding the argument is only half the settlement: the enclosing `Generic`
//! site still reads open because the pre-resolution normalizer skipped it
//! while the call awaited selection. `realize_folded_applications` closes the
//! site to the same shape a provider-free application carries — rejoining or
//! synthesizing the generated data instance, substituting const parameters in
//! member types with the folded literals, and rewriting the site to `Named`.

use crate::build_time_evaluation::EvaluationDomain;
use std::collections::{HashMap, HashSet};

use arena::HandleSpan;
use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::{
    TypedTrees,
    data::{DataMember, TypeParameterKind},
    expression::ExpressionHandle,
    name::Identifier,
    types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode},
};

use crate::machine_execution::admission::{expression_children, require_closed_expression_custody};

/// One `ConstExpression` retained in a declared const-generic argument slot.
pub(crate) struct PendingConstApplication {
    /// The `ConstExpression` type-reference node occupying the argument slot.
    pub(crate) site: TypeReferenceHandle,
    /// Its retained authored root expression.
    pub(crate) root: ExpressionHandle,
    /// The const parameter's declared carrier type, when the generic base
    /// resolves to a declaration with const parameters.
    destination: Option<TypeReferenceHandle>,
}

/// One authored call reachable inside a pending const application. Call nodes
/// cover both named machine calls and boundary-operator token uses; facts
/// separate which uses actually require Omega's selected providers.
pub(crate) struct PendingApplicationCall {
    pub(crate) site: TypeReferenceHandle,
    pub(crate) root: ExpressionHandle,
    pub(crate) expression: ExpressionHandle,
    /// The callee's working-tree machine, or invalid when the call names no
    /// ordinary machine (evidence, dispatch, and boundary-operator forms).
    pub(crate) machine: SymbolHandle,
    /// Whether `machine` is a generic template requiring its prepared
    /// instance.
    pub(crate) static_application: bool,
}

/// Pending const applications and the calls their evaluation must run.
pub(crate) struct ConstApplicationPlan {
    pub(crate) applications: Vec<PendingConstApplication>,
    pub(crate) calls: Vec<PendingApplicationCall>,
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
    domain: &EvaluationDomain,
) -> Result<bool, Vec<Diagnostic>> {
    let plan = const_application_plan(typed, domain);
    if plan.applications.is_empty() {
        return Ok(false);
    }
    let prepared = crate::PreparedBuildMachineProgram::prepare_in(typed, domain)?;
    let execution = prepared.typed();
    let facts = validation::derive_pre_flow_operator_selections(execution);
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
    domain: &EvaluationDomain,
) -> Result<(), Vec<Diagnostic>> {
    let plan = const_application_plan(typed, domain);
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
    domain: &mut EvaluationDomain,
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
        let plan = const_application_plan(typed, domain);
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
            domain,
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
    realize_folded_applications(typed, &published, domain);
    for warning in warnings {
        eprintln!("{warning}");
    }
    Ok(())
}

/// Close every generic application site whose retained const argument just
/// folded. The pre-resolution normalizer skipped these sites exactly because
/// the call awaited provider selection, so they still read `Generic` — member
/// resolution would rejoin the open template and keep its const parameters
/// symbolic. Once the argument is a closed literal the site must land the
/// same shape a provider-free application produces: rejoin an existing
/// generated instance or synthesize it, then rewrite the site to `Named`.
/// Nested applications converge outward: each realized site is itself a
/// closed argument, so an enclosing `Generic` becomes realizable on the next
/// pass. A site that still names a generic binder, or whose template needs a
/// synthesis shape this pass does not own (case payloads, where-fact
/// evaluation, lifetime forwarding, attached machines), stays open and is
/// checked exactly as before.
fn realize_folded_applications(
    typed: &mut TypedTrees,
    published: &[(
        TypeReferenceHandle,
        TypeReferenceNode,
        Vec<ExpressionHandle>,
    )],
    domain: &mut EvaluationDomain,
) {
    if published.is_empty() {
        return;
    }
    let mut closed: HashSet<TypeReferenceHandle> =
        published.iter().map(|(site, _, _)| *site).collect();
    let mut declined: HashSet<TypeReferenceHandle> = HashSet::new();
    loop {
        // Instance origins are provenance, not applications: a `Generic` node
        // recorded as `generic_instance` must keep that shape, or readers that
        // follow `Named` -> origin -> `Named` loop forever.
        let origins: HashSet<TypeReferenceHandle> = typed
            .data_definitions()
            .iter()
            .filter_map(|definition| definition.generic_instance)
            .collect();
        let mut progress = false;
        // Recollect after publication: a realized instance can add live field
        // types, while detached backing nodes must not acquire instances.
        for site in domain.references(typed).type_references {
            let TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } = typed.type_reference_table.type_reference(site)
            else {
                continue;
            };
            let base_symbol = *base_symbol;
            if closed.contains(&site) || declined.contains(&site) || origins.contains(&site) {
                continue;
            }
            let argument_handles = typed
                .type_reference_table
                .type_reference_handles(*arguments)
                .to_vec();
            if !argument_handles
                .iter()
                .any(|argument| argument_contains(typed, *argument, &closed))
            {
                continue;
            }
            if !argument_handles
                .iter()
                .all(|argument| closed_argument(typed, *argument))
            {
                continue;
            }
            let Some((symbol, name)) = realize_data_instance(typed, site, base_symbol) else {
                declined.insert(site);
                continue;
            };
            // The selected application owns the realized fields and any nested
            // applications they introduce. Keep their canonical symbol identity.
            domain.include(symbol);
            typed.type_reference_table.substitute_node(
                site,
                TypeReferenceNode::Named {
                    symbol,
                    name: Identifier::generated(name),
                },
            );
            closed.insert(site);
            progress = true;
        }
        if !progress {
            return;
        }
    }
}

/// Whether the argument subtree contains a site this pass already folded or
/// closed — the dependency edge that pulls an enclosing `Generic` into scope.
fn argument_contains(
    typed: &TypedTrees,
    reference: TypeReferenceHandle,
    closed: &HashSet<TypeReferenceHandle>,
) -> bool {
    if closed.contains(&reference) {
        return true;
    }
    match typed.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Generic { arguments, .. } => typed
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .any(|argument| argument_contains(typed, *argument, closed)),
        TypeReferenceNode::Constrained { base_type, .. } => {
            argument_contains(typed, *base_type, closed)
        }
        TypeReferenceNode::Reference { referee, .. } => argument_contains(typed, *referee, closed),
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            argument_contains(typed, *element_type, closed)
        }
        TypeReferenceNode::Named { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => false,
    }
}

/// Whether a generic argument is a closed concrete substitution: a literal
/// constant, a named builtin or data type, or a nested application whose own
/// arguments are closed. A `TypeParameter` symbol keeps the site open for
/// ordinary specialization.
fn closed_argument(typed: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    match typed.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => {
            !symbol.is_valid()
                || matches!(
                    typed.symbols.get(*symbol).kind,
                    SymbolKind::BuiltinType | SymbolKind::Data
                )
        }
        TypeReferenceNode::Generic { arguments, .. } => typed
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .all(|argument| closed_argument(typed, *argument)),
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Constrained { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => false,
    }
}

/// The ordinary zero-argument machine a const argument applies directly
/// (`Data<limit()>`): the one argument shape a fixed-array extent can carry
/// symbolically. Any other expression keeps the application open.
fn direct_const_call(
    typed: &TypedTrees,
    root: ExpressionHandle,
) -> Option<(SymbolHandle, Identifier)> {
    let call = typed.expression_table.call_node(root)?;
    if !typed
        .expression_table
        .expression_handles(call.arguments)
        .is_empty()
        || !call.machine_arguments.is_empty()
    {
        return None;
    }
    let (machine, static_application) =
        crate::const_evaluation::range_endpoints::selected_endpoint_machine(typed, root)?;
    if static_application || !machine.symbol.is_valid() {
        return None;
    }
    Some((machine.symbol, machine.name.clone()))
}

/// Realize every retained const-generic application whose arguments are
/// closed or direct recipe calls as a symbolic instance: the instance's
/// array extents carry the recipe call, so the shared judgment, the index
/// obligations and the target closure treat the application exactly as a
/// retained array length. The site becomes the instance's `Named` node and
/// the instance joins the evaluation domain; an argument of another shape
/// stays pending and refuses as before.
pub(crate) fn realize_symbolic_applications(
    typed: &mut TypedTrees,
    domain: &mut EvaluationDomain,
) -> Result<(), Vec<Diagnostic>> {
    loop {
        let mut progress = false;
        // An instance's recorded origin is bookkeeping, not an application
        // site; rewriting it would detach the instance from its application.
        let origins: Vec<TypeReferenceHandle> = typed
            .data_definitions()
            .iter()
            .filter_map(|definition| definition.generic_instance)
            .collect();
        for site in domain.references(typed).type_references {
            if origins.contains(&site) {
                continue;
            }
            let TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } = typed.type_reference_table.type_reference(site)
            else {
                continue;
            };
            let base_symbol = *base_symbol;
            let argument_handles = typed
                .type_reference_table
                .type_reference_handles(*arguments)
                .to_vec();
            let symbolic = |argument: &TypeReferenceHandle| {
                matches!(
                    typed.type_reference_table.type_reference(*argument),
                    TypeReferenceNode::ConstExpression(root)
                        if direct_const_call(typed, *root).is_some()
                )
            };
            if !argument_handles.iter().any(symbolic)
                || !argument_handles
                    .iter()
                    .all(|argument| closed_argument(typed, *argument) || symbolic(argument))
            {
                continue;
            }
            let Some((symbol, name)) = realize_data_instance(typed, site, base_symbol) else {
                continue;
            };
            domain.include(symbol);
            typed.type_reference_table.substitute_node(
                site,
                TypeReferenceNode::Named {
                    symbol,
                    name: Identifier::generated(name),
                },
            );
            progress = true;
            break;
        }
        if !progress {
            return Ok(());
        }
    }
}

/// Rejoin or synthesize the concrete data instance a now-closed `Generic`
/// site names, returning the instance symbol and its canonical spelling.
/// `None` declines: the site stays `Generic` and checking owns the result.
fn realize_data_instance(
    typed: &mut TypedTrees,
    site: TypeReferenceHandle,
    base_symbol: SymbolHandle,
) -> Option<(SymbolHandle, String)> {
    let TypeReferenceNode::Generic {
        base_name,
        lifetime_arguments,
        arguments,
        ..
    } = typed.type_reference_table.type_reference(site).clone()
    else {
        return None;
    };
    let argument_handles = typed
        .type_reference_table
        .type_reference_handles(arguments)
        .to_vec();
    let template = typed.data_definition_by_symbol(base_symbol).cloned()?;
    if template.generic_instance.is_some()
        || template.quotient.is_some()
        || !template.where_facts.is_empty()
        || template.zero_gated
        || !template.lifetime_parameters.is_empty()
        || typed
            .machines()
            .iter()
            .any(|machine| machine.attached_data_symbol == base_symbol)
    {
        return None;
    }
    let parameters = typed.data_type_parameters(&template).to_vec();
    if parameters.len() != argument_handles.len() {
        return None;
    }
    let mut bindings = HashMap::new();
    for (parameter, argument) in parameters.iter().zip(&argument_handles) {
        match parameter.kind {
            TypeParameterKind::Type
            | TypeParameterKind::Const { .. }
            | TypeParameterKind::Value { .. } => {
                bindings.insert(parameter.symbol, *argument);
            }
            _ => return None,
        }
    }
    let synthetic_name = format!(
        "{}<{}>",
        template.name.as_str(),
        argument_handles
            .iter()
            .map(|argument| argument_slug(typed, *argument))
            .collect::<Option<Vec<_>>>()?
            .join(", ")
    );
    if let Some(existing) = typed.data_definitions().iter().find(|definition| {
        definition.generic_instance.is_some_and(|origin| {
            instance_origin_matches(typed, origin, base_symbol, &argument_handles)
        })
    }) {
        return Some((existing.symbol, synthetic_name));
    }
    let template_members = typed.data_members(&template).to_vec();
    let mut member_types = Vec::with_capacity(template_members.len());
    for member in &template_members {
        let field = member.as_field()?;
        let type_reference = substitute_member_type(typed, field.type_reference, &bindings)?;
        member_types.push((field.clone(), type_reference));
    }
    let instance_symbol =
        typed
            .symbols
            .insert_generated_root_from(base_symbol, SymbolKind::Data, &synthetic_name);
    let member_span = typed.symbols.insert_generated_children(
        instance_symbol,
        member_types
            .iter()
            .map(|(field, _)| (SymbolKind::Field, field.name.as_str())),
    );
    let origin = typed
        .type_reference_table
        .insert(TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments,
            arguments,
        });
    let mut instance = template.clone();
    instance.symbol = instance_symbol;
    instance.name = Identifier::generated(synthetic_name.clone());
    instance.type_parameters = HandleSpan::empty();
    instance.generic_instance = Some(origin);
    instance.members = HandleSpan::empty();
    for ((field, type_reference), member) in member_types.iter().zip(member_span.handles()) {
        let mut field = field.clone();
        field.symbol = member;
        field.type_reference = *type_reference;
        typed.push_data_member(&mut instance, DataMember::Field(field));
    }
    typed.push_data_definition(instance);
    Some((instance_symbol, synthetic_name))
}

/// Substitute bound parameter occurrences inside a template member's type.
/// `None` declines shapes whose substitution this pass cannot replay exactly.
fn substitute_member_type(
    typed: &mut TypedTrees,
    reference: TypeReferenceHandle,
    bindings: &HashMap<SymbolHandle, TypeReferenceHandle>,
) -> Option<TypeReferenceHandle> {
    match typed.type_reference_table.type_reference(reference).clone() {
        TypeReferenceNode::Named { symbol, .. } => {
            bindings.get(&symbol).copied().or(Some(reference))
        }
        TypeReferenceNode::Reference {
            referee,
            access,
            lifetime,
        } => {
            let referee = substitute_member_type(typed, referee, bindings)?;
            Some(
                typed
                    .type_reference_table
                    .insert(TypeReferenceNode::Reference {
                        referee,
                        access,
                        lifetime,
                    }),
            )
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let element_type = substitute_member_type(typed, element_type, bindings)?;
            let length = match length {
                FixedArrayLength::Literal(_) => length,
                FixedArrayLength::ConstParameter { symbol, .. } => {
                    let argument = bindings.get(&symbol).copied()?;
                    match typed.type_reference_table.type_reference(argument) {
                        TypeReferenceNode::Named { name, .. } => {
                            FixedArrayLength::Literal(name.as_str().parse().ok()?)
                        }
                        // A retained recipe argument keeps the instance's
                        // extent symbolic: the same application, now in
                        // array-length position, closes under the target.
                        TypeReferenceNode::ConstExpression(root) => {
                            let (machine, name) = direct_const_call(typed, *root)?;
                            FixedArrayLength::ConstCall {
                                machine,
                                name,
                                source_span: typed.expression_table.source_span(*root),
                            }
                        }
                        _ => return None,
                    }
                }
                FixedArrayLength::Expression(_) | FixedArrayLength::ConstCall { .. } => {
                    return None;
                }
            };
            Some(
                typed
                    .type_reference_table
                    .insert(TypeReferenceNode::FixedArray {
                        element_type,
                        length,
                    }),
            )
        }
        TypeReferenceNode::Slice { element_type } => {
            let element_type = substitute_member_type(typed, element_type, bindings)?;
            Some(
                typed
                    .type_reference_table
                    .insert(TypeReferenceNode::Slice { element_type }),
            )
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments,
            arguments,
        } => {
            let mut substituted = Vec::new();
            for argument in typed
                .type_reference_table
                .type_reference_handles(arguments)
                .to_vec()
            {
                substituted.push(substitute_member_type(typed, argument, bindings)?);
            }
            let arguments = typed
                .type_reference_table
                .insert_type_reference_handles(substituted);
            Some(
                typed
                    .type_reference_table
                    .insert(TypeReferenceNode::Generic {
                        base_symbol,
                        base_name,
                        lifetime_arguments,
                        arguments,
                    }),
            )
        }
        TypeReferenceNode::Constrained { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. } => None,
        TypeReferenceNode::Unit => Some(reference),
    }
}

/// The canonical argument spelling used in a synthesized instance name.
fn argument_slug(typed: &TypedTrees, reference: TypeReferenceHandle) -> Option<String> {
    match typed.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { name, .. } => Some(name.as_str().to_owned()),
        TypeReferenceNode::ConstExpression(root) => {
            direct_const_call(typed, *root).map(|(_, name)| format!("{}()", name.as_str()))
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => Some(format!(
            "{}<{}>",
            base_name.as_str(),
            typed
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .map(|argument| argument_slug(typed, *argument))
                .collect::<Option<Vec<_>>>()?
                .join(", ")
        )),
        _ => None,
    }
}

/// Whether an existing instance's recorded origin names this exact
/// application, so repeated sites rejoin the one generated definition.
fn instance_origin_matches(
    typed: &TypedTrees,
    origin: TypeReferenceHandle,
    base_symbol: SymbolHandle,
    arguments: &[TypeReferenceHandle],
) -> bool {
    let TypeReferenceNode::Generic {
        base_symbol: origin_base,
        arguments: origin_arguments,
        ..
    } = typed.type_reference_table.type_reference(origin)
    else {
        return false;
    };
    let origin_handles = typed
        .type_reference_table
        .type_reference_handles(*origin_arguments);
    *origin_base == base_symbol
        && origin_handles.len() == arguments.len()
        && origin_handles
            .iter()
            .zip(arguments)
            .all(|(origin, argument)| closed_argument_eq(typed, *origin, *argument))
}

fn closed_argument_eq(
    typed: &TypedTrees,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
) -> bool {
    match (
        typed.type_reference_table.type_reference(left),
        typed.type_reference_table.type_reference(right),
    ) {
        (TypeReferenceNode::ConstExpression(left), TypeReferenceNode::ConstExpression(right)) => {
            match (
                direct_const_call(typed, *left),
                direct_const_call(typed, *right),
            ) {
                (Some((left, _)), Some((right, _))) => left == right,
                _ => false,
            }
        }
        (
            TypeReferenceNode::Named {
                symbol: left_symbol,
                name: left_name,
            },
            TypeReferenceNode::Named {
                symbol: right_symbol,
                name: right_name,
            },
        ) => {
            if left_symbol.is_valid() && right_symbol.is_valid() {
                left_symbol == right_symbol
            } else {
                left_name == right_name
            }
        }
        (
            TypeReferenceNode::Generic {
                base_symbol: left_base,
                arguments: left_arguments,
                ..
            },
            TypeReferenceNode::Generic {
                base_symbol: right_base,
                arguments: right_arguments,
                ..
            },
        ) => {
            let left_handles = typed
                .type_reference_table
                .type_reference_handles(*left_arguments);
            let right_handles = typed
                .type_reference_table
                .type_reference_handles(*right_arguments);
            left_base == right_base
                && left_handles.len() == right_handles.len()
                && left_handles
                    .iter()
                    .zip(right_handles)
                    .all(|(left, right)| closed_argument_eq(typed, *left, *right))
        }
        (
            TypeReferenceNode::FixedArray {
                element_type: left_element,
                length: left_length,
            },
            TypeReferenceNode::FixedArray {
                element_type: right_element,
                length: right_length,
            },
        ) => {
            left_length == right_length && closed_argument_eq(typed, *left_element, *right_element)
        }
        (
            TypeReferenceNode::Slice {
                element_type: left_element,
            },
            TypeReferenceNode::Slice {
                element_type: right_element,
            },
        ) => closed_argument_eq(typed, *left_element, *right_element),
        (
            TypeReferenceNode::Reference {
                referee: left_referee,
                access: left_access,
                lifetime: left_lifetime,
            },
            TypeReferenceNode::Reference {
                referee: right_referee,
                access: right_access,
                lifetime: right_lifetime,
            },
        ) => {
            left_access == right_access
                && left_lifetime == right_lifetime
                && closed_argument_eq(typed, *left_referee, *right_referee)
        }
        (TypeReferenceNode::Unit, TypeReferenceNode::Unit) => true,
        _ => false,
    }
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
    domain: &EvaluationDomain,
) -> Result<ApplicationRound, Vec<Diagnostic>> {
    // Bind exact authored provider occurrences before cloning specializations,
    // on a private copy: the caller's tree keeps its authored selection and
    // source-owned handles for final checking.
    let (prepared, admission) = crate::const_evaluation::prepared_admission(
        typed,
        provider_bodies,
        operators,
        selection_authority.clone(),
        domain,
    )?;
    let execution = prepared.typed();

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
pub(crate) fn const_application_plan(
    typed: &TypedTrees,
    domain: &EvaluationDomain,
) -> ConstApplicationPlan {
    let mut plan = ConstApplicationPlan {
        applications: Vec::new(),
        calls: Vec::new(),
    };
    for site in domain.references(typed).type_references {
        let TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } = typed.type_reference_table.type_reference(site)
        else {
            continue;
        };
        let parameters = generic_type_parameters(typed, *base_symbol);
        for (index, argument) in typed
            .type_reference_table
            .type_reference_handles(*arguments)
            .iter()
            .enumerate()
        {
            visit_argument(
                typed,
                *argument,
                parameters.and_then(|parameters| parameters.get(index)),
                &mut plan,
            );
        }
    }
    plan
}

fn generic_type_parameters(
    typed: &TypedTrees,
    base_symbol: SymbolHandle,
) -> Option<&[typed_trees::data::TypeParameter]> {
    if let Some(data) = typed.data_definition_by_symbol(base_symbol) {
        return Some(typed.data_type_parameters(data));
    }
    if let Some(machine) = typed.machine(base_symbol) {
        return Some(typed.machine_type_parameters(machine));
    }
    if let Some(trait_definition) = typed.trait_definition_by_symbol(base_symbol) {
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
    parameter: Option<&typed_trees::data::TypeParameter>,
    plan: &mut ConstApplicationPlan,
) {
    let destination = parameter.and_then(|parameter| match parameter.kind {
        TypeParameterKind::Const { type_reference }
        | TypeParameterKind::Value { type_reference } => Some(type_reference),
        _ => None,
    });
    match typed.type_reference_table.type_reference(reference) {
        TypeReferenceNode::ConstExpression(root) => {
            // Const slots owe static execution; Value slots only fold closed
            // recipes. Runtime subjects and reference formations belong to
            // ordinary operand capture, including their observable effects.
            if parameter
                .is_some_and(|parameter| matches!(parameter.kind, TypeParameterKind::Value { .. }))
                && expression_subtree(typed, *root).iter().any(|expression| {
                    match typed.expression_table.expression(*expression) {
                        typed_trees::expression::ExpressionNode::Borrow(_) => true,
                        typed_trees::expression::ExpressionNode::Name(path)
                            if path.symbol.is_valid() =>
                        {
                            matches!(
                                typed.symbols.get(path.symbol).kind,
                                SymbolKind::Local
                                    | SymbolKind::Parameter
                                    | SymbolKind::Field
                                    | SymbolKind::TypeParameter
                            )
                        }
                        _ => false,
                    }
                })
            {
                return;
            }
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
            visit_argument(typed, *base_type, parameter, plan);
        }
        TypeReferenceNode::Reference { referee, .. } => {
            visit_argument(typed, *referee, parameter, plan);
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            visit_argument(typed, *element_type, parameter, plan);
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
                visit_argument(
                    typed,
                    *nested,
                    parameters.and_then(|parameters| parameters.get(index)),
                    plan,
                );
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
