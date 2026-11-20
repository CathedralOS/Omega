use super::{
    contracts::{self, ClauseOwner},
    type_expression::type_expression,
};
use crate::lowering::applied_tree::LoweringTree;
use crate::{
    ContractClause, LoweringError, MachineTemplate, MachineTemplateBinder, PredicateStep, TemplateBinder, TemplateCall, TemplateCallTarget, TemplateCallable,
    TemplateRequirementSlot, TypeExpression, site_at,
};
use omega_prepared::PreparedTrees;
use omega_resolved::Symbol;
use omega_syntax::NodeKind;
use omega_typed::{CallUse, CallableType, InvocationTarget, NodeSemantics, ObligationKind, Site, TypedSignature, TypedTree, ValueType};

fn result_type(
    ty: &ValueType,
    site: &Site,
) -> Result<Option<TypeExpression>, LoweringError> {
    (ty != &ValueType::NoResult)
        .then(|| type_expression(ty, site))
        .transpose()
}

pub(super) fn signature(
    prepared: &PreparedTrees<'_>,
    tree: &TypedTree<'_>,
    sig: &TypedSignature,
) -> Result<TemplateCallable, LoweringError> {
    let file = tree.resolved().file;
    let plain = LoweringTree::plain(tree);
    let mut requires = Vec::new();
    let mut ensures = Vec::new();
    let bad = || LoweringError::new(&sig.site, "unsupported template machine contract");
    if sig.operator.is_some() || sig.satisfies.is_some() || !sig.multiplicity_bounds.is_empty() {
        return Err(bad());
    }
    for &child in &file.nodes[sig.node.node].children {
        match file.nodes[child].kind {
            NodeKind::Path
            | NodeKind::Params
            | NodeKind::Result
            | NodeKind::GenericParams
            | NodeKind::Body
            | NodeKind::Reaches
            | NodeKind::Blocks
            | NodeKind::Suspends => {}
            // Routes, including guards over the entry formals, are lowered below.
            NodeKind::Crashes => {}
            NodeKind::Terminates
                if file.nodes[child].children.is_empty()
                    && tree.semantics_at(child).all(
                        |row| matches!(row,NodeSemantics::Obligation(o) if matches!(&o.kind,ObligationKind::Termination(owner) if owner==&sig.machine)),
                    ) => {}
            NodeKind::Requires => requires.push(contracts::lower_clause(prepared, &plain, ClauseOwner::Machine(sig), child)?),
            NodeKind::Ensures => ensures.push(match projection(prepared, tree, sig, child)? {
                Some(clause) => clause,
                None => contracts::lower_clause(prepared, &plain, ClauseOwner::Machine(sig), child)?,
            }),
            _ => return Err(bad()),
        }
    }
    if sig.requires == requires.is_empty() || sig.has_guarantee == ensures.is_empty() {
        return Err(bad());
    }
    let crash_routes = super::crash_routes::lower_routes(
        prepared,
        &plain,
        ClauseOwner::Machine(sig),
        sig.node.node,
        &sig.site,
        sig.crash_ceiling.as_deref(),
    )?;
    Ok(TemplateCallable {
        parameters: sig
            .parameters
            .iter()
            .map(|(_, ty)| type_expression(ty, &sig.site))
            .collect::<Result<_, _>>()?,
        result: result_type(&sig.result, &sig.site)?,
        requires,
        ensures,
        reach: reach(sig.reach.as_deref(), &sig.site)?,
        crash_ceiling: sig.crash_ceiling.clone(),
        crash_routes,
        pause: sig.pause.declared(),
        terminates: sig.terminates,
        site: sig.site.clone(),
    })
}

fn reach(
    rows: Option<&[omega_typed::ServiceApplication]>,
    site: &Site,
) -> Result<Option<Vec<(crate::DeclarationId, Vec<TypeExpression>)>>, LoweringError> {
    rows.map(|rows| {
        rows.iter()
            .map(|row| {
                Ok((
                    crate::identity::declaration(&row.service, site)?,
                    row.arguments
                        .iter()
                        .map(|v| type_expression(v, site))
                        .collect::<Result<Vec<_>, _>>()?,
                ))
            })
            .collect()
    })
    .transpose()
}

fn formal(
    tree: &TypedTree<'_>,
    sig: &TypedSignature,
    node: usize,
    ty: &ValueType,
) -> Option<usize> {
    let Some(Symbol::Binding {
        kind: NodeKind::Param, span, ..
    }) = tree.resolved().symbol(node)
    else {
        return None;
    };
    sig.parameters
        .iter()
        .position(|(name, carrier)| name == span && carrier == ty)
}

fn projection(
    prepared: &PreparedTrees<'_>,
    tree: &TypedTree<'_>,
    sig: &TypedSignature,
    clause: usize,
) -> Result<Option<ContractClause>, LoweringError> {
    let file = tree.resolved().file;
    let [node] = *file.nodes[clause].children.as_slice() else { return Ok(None) };
    let Some((binder, contract, operands)) = tree.semantics_at(node).find_map(|row| match row {
        NodeSemantics::Obligation(o) => match &o.kind {
            ObligationKind::ApplyCallbackGuarantee { binder, contract, operands } => Some((binder, contract, operands)),
            _ => None,
        },
        _ => None,
    }) else {
        return Ok(None);
    };
    let bad = || LoweringError::new(&site_at(tree, node), "callback projection lacks its exact binder, operands or proof obligation");
    let ValueType::Parameter { owner, index } = binder else {
        return Err(bad());
    };
    let owner = owner.as_ref();
    let (_, _, bound) = prepared
        .typed()
        .callable_bounds
        .iter()
        .find(|(decl, i, c)| decl == owner && owner == &sig.machine && i == index && &c.contract == contract && c.has_guarantee)
        .ok_or_else(bad)?;
    let [callee, args] = *file.nodes[node].children.as_slice() else {
        return Err(bad());
    };
    let (binder_name, _, _) = file
        .generic_parameters(sig.node.node)
        .nth(*index)
        .ok_or_else(bad)?;
    if !matches!(tree.resolved().symbol(callee),Some(Symbol::StaticMember{binder,member}) if *binder==binder_name.span && member=="ensures") {
        return Err(bad());
    }
    let nodes = &file.nodes[args].children;
    if nodes.len() != operands.len() || nodes.len() != bound.parameters.len() + 1 || bound.result == ValueType::NoResult {
        return Err(bad());
    }
    let (mut proved, mut applied, mut boolean) = (false, false, false);
    for row in tree.semantics_at(node) {
        match row {
            NodeSemantics::Expression(e) if e.ty == ValueType::Bool && e.static_argument_slot.is_none() && !boolean => boolean = true,
            NodeSemantics::Obligation(o) => match &o.kind {
                ObligationKind::ApplyCallbackGuarantee { .. } if !applied => applied = true,
                ObligationKind::ProveGuarantee { machine, predicate } if machine == owner && predicate == &site_at(tree, node) && !proved => proved = true,
                _ => return Err(bad()),
            },
            _ => return Err(bad()),
        }
    }
    if !proved || !boolean || !applied {
        return Err(bad());
    }
    let mut predicate = Vec::new();
    for ((&n, site), ty) in nodes
        .iter()
        .zip(operands)
        .zip(bound.parameters.iter().chain([&bound.result]))
    {
        if site != &site_at(tree, n) || !typed_subject(tree, n, ty) {
            return Err(bad());
        }
        predicate.push(if let Some(i) = formal(tree, sig, n, ty) {
            PredicateStep::Formal(i)
        } else if matches!(tree.resolved().symbol(n),Some(Symbol::Builtin(name)) if name=="result" && &sig.result==ty) {
            PredicateStep::Result
        } else {
            return Err(bad());
        });
    }
    predicate.push(PredicateStep::CallbackGuarantee {
        binder: *index,
        operand_count: operands.len(),
    });
    Ok(Some(ContractClause {
        predicate,
        site: site_at(tree, clause),
    }))
}

fn typed_subject(
    tree: &TypedTree<'_>,
    node: usize,
    ty: &ValueType,
) -> bool {
    let mut rows = tree.semantics_at(node);
    matches!(rows.next(),Some(NodeSemantics::Expression(e)) if &e.ty==ty && e.static_argument_slot.is_none()) && rows.next().is_none()
}

/// A wrapper with open static arguments must point at a template even if another
/// caller also requested a concrete body of that same declaration.
pub(crate) fn plan(
    prepared: &PreparedTrees<'_>,
    trees: &[TypedTree<'_>],
    signatures: &[&TypedSignature],
    instances: &[crate::lowering::applications::MachineInstance<'_>],
) -> Result<Vec<MachineTemplate>, LoweringError> {
    use std::collections::{HashMap, HashSet};
    let instantiated: HashSet<_> = instances
        .iter()
        .map(|i| i.application.machine.as_ref())
        .collect();
    let by_id: HashMap<_, _> = signatures.iter().map(|s| (&s.machine, *s)).collect();
    let mut calls: HashMap<_, Vec<_>> = HashMap::new();
    for call in &prepared.typed().calls {
        if let Some(owner) = &call.caller
            && call.use_site == CallUse::Execution
        {
            calls.entry(owner).or_default().push(call);
        }
    }
    let callback_owners: HashSet<_> = prepared
        .typed()
        .callable_bounds
        .iter()
        .map(|(owner, _, _)| owner)
        .collect();
    let mut open: HashSet<_> = signatures
        .iter()
        .filter(|s| !instantiated.contains(&s.machine) || trees[s.node.file].resolved().is_exported(s.node.node) && callback_owners.contains(&s.machine))
        .map(|s| &s.machine)
        .collect();
    let mut work: Vec<_> = open.iter().copied().collect();
    while let Some(owner) = work.pop() {
        for call in calls.get(owner).into_iter().flatten() {
            let target = by_id
                .get(&call.machine)
                .ok_or_else(|| LoweringError::new(&call.site, "template call needs its packaged declaration"))?;
            if !call.type_arguments.is_empty() && open.insert(&target.machine) {
                work.push(&target.machine);
            }
        }
    }
    signatures
        .iter()
        .filter(|s| open.contains(&s.machine))
        .map(|s| lower(prepared, &trees[s.node.file], s))
        .collect()
}

fn lower(
    prepared: &PreparedTrees<'_>,
    tree: &TypedTree<'_>,
    sig: &TypedSignature,
) -> Result<MachineTemplate, LoweringError> {
    let typed = prepared.typed();
    let file = tree.resolved().file;
    let bad = || LoweringError::new(&sig.site, "open generic machine requires a closed application before lowering");
    let params = typed.generic_parameters.get(&sig.machine).ok_or_else(bad)?;
    if file.nodes[sig.node.node].has_boundary_modifier()
        || params.witness_count != 0
        || !params.equations.is_empty()
        || file.generic_parameters(sig.node.node).count() != params.parameters.len()
    {
        return Err(bad());
    }
    let mut parameters = Vec::new();
    let mut bounds = Vec::new();
    for (index, ((_, node, kind), param)) in file
        .generic_parameters(sig.node.node)
        .zip(&params.parameters)
        .enumerate()
    {
        if kind != param.kind {
            return Err(bad());
        }
        let binder = match kind {
            NodeKind::TypeParam if param.carrier.is_none() => MachineTemplateBinder::Type(TemplateBinder::Type {
                multiplicity: param.multiplicity,
            }),
            NodeKind::ConstParam if param.multiplicity.is_none() => {
                let (_, _, bound) = typed
                    .callable_bounds
                    .iter()
                    .find(|(d, i, _)| d == &sig.machine && *i == index)
                    .ok_or_else(bad)?;
                bounds.push((index, bound));
                let (requirement, interface) = if let Some(slot) = &bound.requirement {
                    // Nominal predicates refer to the exact requirement declaration.
                    if bound.requires || bound.has_guarantee {
                        return Err(bad());
                    }
                    let interface = TemplateCallable {
                        parameters: bound
                            .parameters
                            .iter()
                            .map(|v| type_expression(v, &bound.contract))
                            .collect::<Result<_, _>>()?,
                        result: result_type(&bound.result, &bound.contract)?,
                        requires: vec![],
                        ensures: vec![],
                        reach: reach(bound.reach.as_deref(), &bound.contract)?,
                        crash_ceiling: bound.crash_ceiling.clone(),
                        // Like its predicates, the routes stay on the named requirement.
                        crash_routes: Vec::new(),
                        pause: bound.pause,
                        terminates: bound.terminates,
                        site: bound.contract.clone(),
                    };
                    let requirement = TemplateRequirementSlot {
                        declaration: crate::identity::declaration(&slot.declaration, &bound.contract)?,
                        arguments: slot
                            .trait_arguments
                            .iter()
                            .map(|v| type_expression(v, &bound.contract))
                            .collect::<Result<_, _>>()?,
                    };
                    (Some(requirement), interface)
                } else {
                    let input = file
                        .child(node, NodeKind::MachineType)
                        .filter(|n| site_at(tree, *n) == bound.contract)
                        .ok_or_else(bad)?;
                    (None, super::type_expression::callable_interface(prepared, tree, input, bound)?)
                };
                MachineTemplateBinder::Callable {
                    requirement,
                    signature: Box::new(interface),
                }
            }
            _ => return Err(bad()),
        };
        parameters.push(binder);
    }
    if bounds.is_empty() {
        return Err(bad());
    }
    let effects = typed
        .machine_effects
        .iter()
        .find(|e| e.machine == sig.machine)
        .ok_or_else(bad)?;
    if !effects.reach.is_empty() || effects.fixed_pause != Default::default() {
        return Err(bad());
    }
    let invokes: Vec<_> = effects
        .invokes
        .iter()
        .map(|entry| match entry {
            InvocationTarget::StaticParameter { owner, index } if owner == &sig.machine && bounds.iter().any(|b| b.0 == *index) => Ok(*index),
            _ => Err(bad()),
        })
        .collect::<Result<_, _>>()?;
    let body_node = file.child(sig.node.node, NodeKind::Body).ok_or_else(bad)?;
    let mut body = Vec::new();
    let mut result = None;
    for &statement in &file.nodes[body_node].children {
        let [outer] = *file.nodes[statement].children.as_slice() else {
            return Err(bad());
        };
        let mut call = outer;
        while file.nodes[call].kind == NodeKind::CallMarker {
            let [inner] = *file.nodes[call].children.as_slice() else {
                return Err(bad());
            };
            call = inner;
        }
        if result.is_some() || file.nodes[call].kind != NodeKind::Call || tree.semantics_at(statement).next().is_some() {
            return Err(bad());
        }
        let (operation, ty) = lower_call(tree, sig, call, &bounds)?;
        if outer != call && (operation.acknowledgements == Default::default() || (outer..call).any(|n| !typed_subject(tree, n, ty))) {
            return Err(bad());
        }
        if let TemplateCallTarget::StaticBinder(slot) = operation.target
            && !invokes.contains(&slot)
        {
            return Err(bad());
        }
        match file.nodes[statement].kind {
            NodeKind::Tail if ty == &sig.result && ty != &ValueType::NoResult => result = Some(body.len()),
            NodeKind::ExpressionStatement if ty == &ValueType::NoResult => {}
            _ => return Err(bad()),
        }
        body.push(operation);
    }
    if effects
        .callback_pause
        .keys()
        .any(|slot| !invokes.contains(slot))
    {
        return Err(bad());
    }
    if body.is_empty() || result.is_some() != (sig.result != ValueType::NoResult) {
        return Err(bad());
    }
    super::crash_routes::check_template_invocations(prepared, sig)?;
    Ok(MachineTemplate {
        declaration: crate::identity::declaration(&sig.machine, &sig.site)?,
        public: tree.resolved().is_exported(sig.node.node),
        parameters,
        signature: signature(prepared, tree, sig)?,
        declared_pause: sig.pause,
        fixed_pause: effects.fixed_pause,
        callback_pause: effects.callback_pause.iter().map(|(i, p)| (*i, *p)).collect(),
        invokes,
        body,
        result,
    })
}

fn lower_call<'a>(
    tree: &'a TypedTree<'_>,
    sig: &TypedSignature,
    node: usize,
    bounds: &[(usize, &CallableType)],
) -> Result<(TemplateCall, &'a ValueType), LoweringError> {
    let file = tree.resolved().file;
    let site = site_at(tree, node);
    let bad = || LoweringError::new(&site, "template call has unsupported operands, selection or obligations");
    let [callee, args] = *file.nodes[node].children.as_slice() else {
        return Err(bad());
    };
    if file.nodes[callee].kind != NodeKind::PathExpression
        || (callee..file.nodes[callee].end).any(|part| tree.semantics_at(part).next().is_some())
        || file.nodes[args].kind != NodeKind::Args
        || tree.semantics_at(args).next().is_some()
    {
        return Err(bad());
    }
    let mut expr = None;
    let mut selection = None;
    for row in tree.semantics_at(node) {
        match row {
            NodeSemantics::Expression(e) if expr.is_none() && e.static_argument_slot.is_none() => expr = Some(&e.ty),
            NodeSemantics::IndirectCall(c) if selection.is_none() && c.use_site == CallUse::Execution && c.caller.as_ref() == Some(&sig.machine) => {
                let InvocationTarget::StaticParameter { owner, index } = &c.entry else {
                    return Err(bad());
                };
                if owner != &sig.machine
                    || !bounds.iter().any(|(i, b)| i == index && **b == c.contract)
                    || c.operands.len() != c.contract.parameters.len()
                    || c.operands.iter().any(|o| o.callable.is_some())
                {
                    return Err(bad());
                }
                let actual = c
                    .operands
                    .iter()
                    .map(|o| (o.node, &o.subject))
                    .collect::<Vec<_>>();
                selection = Some((TemplateCallTarget::StaticBinder(*index), vec![], actual, &c.contract.result, c.acknowledgements));
            }
            NodeSemantics::Call(c)
                if selection.is_none()
                    && c.use_site == CallUse::Execution
                    && c.caller.as_ref() == Some(&sig.machine)
                    && c.enclosing_arguments.is_empty()
                    && c.evidence.is_none()
                    && c.layout_query.is_none()
                    && c.operands.iter().all(|o| o.callable.is_none()) =>
            {
                let actual = c
                    .operands
                    .iter()
                    .map(|o| (o.node, &o.subject))
                    .collect::<Vec<_>>();
                selection = Some((
                    TemplateCallTarget::Declaration(crate::identity::declaration(&c.machine, &site)?),
                    c.type_arguments
                        .iter()
                        .map(|v| type_expression(v, &site))
                        .collect::<Result<Vec<_>, _>>()?,
                    actual,
                    &c.result,
                    c.acknowledgements,
                ));
            }
            _ => return Err(bad()),
        }
    }
    let (target, arguments, actual, ty, acknowledgements) = selection.ok_or_else(bad)?;
    if expr != Some(ty) || actual.len() != file.nodes[args].children.len() {
        return Err(bad());
    }
    let operands = file.nodes[args]
        .children
        .iter()
        .zip(actual)
        .map(|(&node, (id, subject))| {
            let mut rows = tree.semantics_at(node);
            let Some(NodeSemantics::Expression(e)) = rows.next() else {
                return Err(bad());
            };
            if file.nodes[node].kind != NodeKind::PathExpression
                || !file.nodes[node].children.is_empty()
                || file.node_id(node) != id
                || rows.next().is_some()
                || e.static_argument_slot.is_some()
            {
                return Err(bad());
            }
            let formal = formal(tree, sig, node, &e.ty).ok_or_else(bad)?;
            let mut declaration = sig.site.clone();
            declaration.span = sig.parameters[formal].0;
            if !matches!(subject,ValueType::Subject{declaration:d,version:None} if *d==declaration) {
                return Err(bad());
            }
            Ok(formal)
        })
        .collect::<Result<_, _>>()?;
    Ok((
        TemplateCall {
            target,
            arguments,
            acknowledgements,
            operands,
            result_type: result_type(ty, &site)?,
            site,
        },
        ty,
    ))
}
