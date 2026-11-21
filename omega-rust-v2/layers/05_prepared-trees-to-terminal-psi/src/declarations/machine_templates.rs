mod lexical;

use super::{
    contracts::{self, ClauseOwner},
    type_expression::type_expression,
};
use crate::lowering::applied_tree::LoweringTree;
use crate::{
    ContractClause, LoweringError, MachineTemplate, MachineTemplateBinder, PredicateStep, TemplateBinder, TemplateCall, TemplateCallTarget, TemplateCallable,
    TemplateOperand, TemplateRequirementSlot, TemplateStep, TypeExpression, site_at,
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
    traits: &[super::traits::TraitSchema<'_>],
) -> Result<Vec<MachineTemplate>, LoweringError> {
    use std::collections::{HashMap, HashSet};
    let instantiated: HashSet<_> = instances
        .iter()
        .map(|i| i.application.machine.as_ref())
        .collect();
    let by_id: HashMap<_, _> = signatures.iter().map(|s| (&s.machine, *s)).collect();
    let member_owners: HashMap<_, _> = traits.iter()
        .flat_map(|t| t.unapplied_members().map(|member| (member, t.declaration)))
        .collect();
    let mut supplied_calls = HashMap::new();
    let mut calls: HashMap<_, Vec<_>> = HashMap::new();
    for call in &prepared.typed().calls {
        if let Some(owner) = &call.caller
            && call.use_site == CallUse::Execution
        {
            if let Some(home) = member_owners.get(&call.machine)
                && let Some(binder) = evidence_member(prepared, owner, home, call)
            {
                // A supplied member is packaged on its trait, not as a standalone machine.
                supplied_calls.insert(call.node, binder);
            } else {
                calls.entry(owner).or_default().push(call);
            }
        }
    }
    // An exported machine whose caller supplies an operation, through a
    // callable bound or an operation record such as `const E: Local`, keeps
    // its interface for external callers beside any local instance.
    let mut operation_owners: HashSet<_> = prepared
        .typed()
        .callable_bounds
        .iter()
        .map(|(owner, _, _)| owner)
        .collect();
    operation_owners.extend(
        prepared
            .typed()
            .generic_parameters
            .iter()
            .filter_map(|(owner, telescope)| {
                telescope
                    .parameters
                    .iter()
                    .any(|p| p.kind == NodeKind::ConstParam && matches!(p.carrier.as_deref(), Some(ValueType::Evidence(_))))
                    .then_some(owner)
            }),
    );
    let mut open: HashSet<_> = signatures
        .iter()
        .filter(|s| !instantiated.contains(&s.machine) || trees[s.node.file].resolved().is_exported(s.node.node) && operation_owners.contains(&s.machine))
        .map(|s| &s.machine)
        .collect();
    let mut work: Vec<_> = open.iter().copied().collect();
    while let Some(owner) = work.pop() {
        // A captured binder needs its declaring header, even if another caller
        // already selected a concrete application of that declaration.
        for enclosing in std::iter::successors(owner.enclosing_machine.as_ref(), |d| d.enclosing_machine.as_ref()) {
            if prepared.typed().generic_parameters.get(enclosing).is_some_and(|p| !p.parameters.is_empty())
                && open.insert(enclosing)
            {
                work.push(enclosing);
            }
        }
        for call in calls.get(owner).into_iter().flatten() {
            // Calls through an operation-record binder were routed to
            // `supplied_calls` above. Any other evidence call, such as one to a
            // requirement inherited from a parent trait, has no packaged
            // declaration and refuses here.
            let target = by_id
                .get(&call.machine)
                .ok_or_else(|| LoweringError::new(&call.site, "template call needs its packaged declaration"))?;
            if (!call.type_arguments.is_empty() || !call.enclosing_arguments.is_empty()) && open.insert(&target.machine) {
                work.push(&target.machine);
            }
            for arg in &call.type_arguments {
                if let ValueType::MachineSelection(app) = arg {
                    let selected = by_id.get(app.machine.as_ref()).ok_or_else(|| LoweringError::new(&call.site, "template selection needs its packaged declaration"))?;
                    if (!app.arguments.is_empty() || !app.enclosing_arguments.is_empty()) && open.insert(&selected.machine) {
                        work.push(&selected.machine);
                    }
                }
            }
        }
    }
    signatures
        .iter()
        .filter(|s| open.contains(&s.machine))
        .map(|s| lower(prepared, &trees[s.node.file], s, traits, &supplied_calls))
        .collect()
}

fn lower(
    prepared: &PreparedTrees<'_>,
    tree: &TypedTree<'_>,
    sig: &TypedSignature,
    traits: &[super::traits::TraitSchema<'_>],
    supplied_calls: &SuppliedCalls,
) -> Result<MachineTemplate, LoweringError> {
    let typed = prepared.typed();
    let file = tree.resolved().file;
    // A nested declaration need not introduce parameters of its own.
    let empty = Default::default();
    let params = typed.generic_parameters.get(&sig.machine).unwrap_or(&empty);
    if file.nodes[sig.node.node].has_boundary_modifier()
        || params.witness_count != 0
        || !params.equations.is_empty()
        || file.generic_parameters(sig.node.node).count() != params.parameters.len()
    {
        return Err(LoweringError::new(&sig.site, OPEN_TEMPLATE));
    }
    let mut parameters = Vec::new();
    let mut bounds = Vec::new();
    for (index, ((_, _, kind), param)) in file
        .generic_parameters(sig.node.node)
        .zip(&params.parameters)
        .enumerate()
    {
        if kind != param.kind {
            return Err(LoweringError::new(&sig.site, OPEN_TEMPLATE));
        }
        let binder = match kind {
            NodeKind::TypeParam if param.carrier.is_none() => MachineTemplateBinder::Type(TemplateBinder::Type {
                multiplicity: param.multiplicity,
            }),
            // A static value binder such as `const N: u8` keeps its declared
            // carrier; the caller's application supplies the value.
            NodeKind::ConstParam
                if param.multiplicity.is_none()
                    && let Some(carrier) = param.carrier.as_deref()
                    && lexical::static_value_carrier(carrier) =>
            {
                MachineTemplateBinder::Const(type_expression(carrier, &sig.site)?)
            }
            // A whole-trait binder such as `const Audio: Player` has no callable
            // bound. Its calls name requirements of that trait; the caller's
            // application supplies the selected member. Only traits without
            // their own parameters package unapplied members, so an applied
            // trait binder still refuses.
            NodeKind::ConstParam
                if param.multiplicity.is_none()
                    && let Some(ValueType::Evidence(slot)) = param.carrier.as_deref()
                    && slot.trait_arguments.is_empty()
                    && traits.iter().any(|t| t.declaration == &slot.declaration)
                    && !typed.callable_bounds.iter().any(|(owner, i, _)| owner == &sig.machine && *i == index) => {
                MachineTemplateBinder::Implementation(super::members::slot(&slot.declaration, &slot.trait_arguments, &sig.site)?)
            }
            NodeKind::ConstParam if param.multiplicity.is_none() => {
                let (_, _, bound) = typed
                    .callable_bounds
                    .iter()
                    .find(|(d, i, _)| d == &sig.machine && *i == index)
                    .ok_or_else(|| LoweringError::new(&sig.site, OPEN_TEMPLATE))?;
                bounds.push((index, bound));
                let (requirement, interface) = if let Some(slot) = &bound.requirement {
                    // Nominal predicates refer to the exact requirement declaration.
                    if bound.requires || bound.has_guarantee {
                        return Err(LoweringError::new(&sig.site, OPEN_TEMPLATE));
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
                    // The bound names an alias; its signature owns the contract clauses.
                    let (origin, input) = prepared
                        .typed()
                        .callable_origin(&bound.contract)
                        .ok_or_else(|| LoweringError::new(&sig.site, OPEN_TEMPLATE))?;
                    let interface = super::type_expression::callable_interface(prepared, &origin, input, bound)?;
                    (None, interface)
                };
                MachineTemplateBinder::Callable {
                    requirement,
                    signature: Box::new(interface),
                }
            }
            _ => return Err(LoweringError::new(&sig.site, OPEN_TEMPLATE)),
        };
        parameters.push(binder);
    }
    // A template needs some binder a caller supplies: a callable bound, a
    // static value, an operation record, or (for a nested declaration) the
    // enclosing machine's arguments.
    if bounds.is_empty()
        && !parameters.iter().any(|p| matches!(p, MachineTemplateBinder::Const(_) | MachineTemplateBinder::Implementation(_)))
        && sig.machine.enclosing_machine.is_none()
    {
        return Err(LoweringError::new(&sig.site, OPEN_TEMPLATE));
    }
    let effects = typed
        .machine_effects
        .iter()
        .find(|e| e.machine == sig.machine)
        .ok_or_else(|| LoweringError::new(&sig.site, OPEN_TEMPLATE))?;
    if !effects.reach.is_empty() || effects.fixed_pause != Default::default() {
        return Err(LoweringError::new(&sig.site, OPEN_TEMPLATE));
    }
    let mut invokes = Vec::new();
    let mut invokes_machines = Vec::new();
    for entry in &effects.invokes {
        match entry {
            InvocationTarget::StaticParameter { owner, index } if owner == &sig.machine && bounds.iter().any(|b| b.0 == *index) => invokes.push(*index),
            InvocationTarget::AppliedMachine(app) => invokes_machines.push(lexical::selection(typed, app, &sig.site)?),
            _ => return Err(LoweringError::new(&sig.site, OPEN_TEMPLATE)),
        }
    }
    let body_node = file.child(sig.node.node, NodeKind::Body).ok_or_else(|| LoweringError::new(&sig.site, OPEN_TEMPLATE))?;
    let mut steps = Steps {
        inputs: CallInputs { prepared, supplied_calls },
        tree,
        sig,
        bounds: &bounds,
        parameters: &parameters,
        invokes: &invokes,
        body: Vec::new(),
    };
    let mut result = None;
    for &statement in &file.nodes[body_node].children {
        if file.nodes[statement].kind == NodeKind::Machine {
            continue; // Lowered separately with its own identity and signature.
        }
        let [outer] = *file.nodes[statement].children.as_slice() else {
            return Err(LoweringError::new(&sig.site, OPEN_TEMPLATE));
        };
        if file.nodes[statement].kind == NodeKind::Tail && result.is_none() && tree.semantics_at(statement).next().is_none()
            && file.nodes[outer].kind == NodeKind::PathExpression && file.nodes[outer].children.is_empty()
            && let Some(binding) = lexical::static_result(typed, tree, sig, outer)
        {
            result = Some(binding);
            continue;
        }
        if result.is_some() || tree.semantics_at(statement).next().is_some() {
            return Err(LoweringError::new(&sig.site, OPEN_TEMPLATE));
        }
        let (operand, ty) = steps.expression(outer)?;
        match file.nodes[statement].kind {
            NodeKind::Tail if ty == &sig.result && ty != &ValueType::NoResult => result = Some(crate::TemplateResult::Operand(operand)),
            // Only a call without a result stands alone, with or without its
            // semicolon; no value is discarded. A void tail in a machine with
            // a result leaves `result` empty and refuses below.
            NodeKind::ExpressionStatement | NodeKind::Tail
                if ty == &ValueType::NoResult && matches!(operand, TemplateOperand::Step(i) if matches!(steps.body.get(i), Some(TemplateStep::Call(_)))) => {}
            _ => return Err(LoweringError::new(&sig.site, OPEN_TEMPLATE)),
        }
    }
    let body = steps.body;
    if effects
        .callback_pause
        .keys()
        .any(|slot| !invokes.contains(slot))
    {
        return Err(LoweringError::new(&sig.site, OPEN_TEMPLATE));
    }
    if body.is_empty() && result.is_none() || result.is_some() != (sig.result != ValueType::NoResult) {
        return Err(LoweringError::new(&sig.site, OPEN_TEMPLATE));
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
        invokes_machines,
        body,
        result,
    })
}

const OPEN_TEMPLATE: &str = "open generic machine requires a closed application before lowering";

/// The open body under construction. Each source expression becomes the
/// operand that reads its value; a call or an integer operation appends a step.
struct Steps<'p, 't, 'a, 'y> {
    inputs: CallInputs<'p, 't>,
    tree: &'a TypedTree<'y>,
    sig: &'a TypedSignature,
    bounds: &'a [(usize, &'a CallableType)],
    parameters: &'a [MachineTemplateBinder],
    invokes: &'a [usize],
    body: Vec<TemplateStep>,
}

impl<'a> Steps<'_, '_, 'a, '_> {
    // Each refusing branch keeps its own diagnostic origin.
    #[cfg_attr(feature = "diagnostic-origins", track_caller)]
    fn refuse(&self) -> LoweringError {
        LoweringError::new(&self.sig.site, OPEN_TEMPLATE)
    }

    fn push(
        &mut self,
        step: TemplateStep,
    ) -> TemplateOperand {
        self.body.push(step);
        TemplateOperand::Step(self.body.len() - 1)
    }

    fn expression(
        &mut self,
        node: usize,
    ) -> Result<(TemplateOperand, &'a ValueType), LoweringError> {
        let (tree, sig) = (self.tree, self.sig);
        let file = tree.resolved().file;
        match file.nodes[node].kind {
            NodeKind::Paren => {
                let [inner] = *file.nodes[node].children.as_slice() else {
                    return Err(self.refuse());
                };
                let (operand, ty) = self.expression(inner)?;
                // Parentheses add no typed record beyond the value itself.
                if tree
                    .semantics_at(node)
                    .any(|row| !matches!(row, NodeSemantics::Expression(e) if &e.ty == ty && e.static_argument_slot.is_none()))
                {
                    return Err(self.refuse());
                }
                Ok((operand, ty))
            }
            NodeKind::CallMarker | NodeKind::Call => self.call(node),
            NodeKind::Binary | NodeKind::Unary => self.integer(node),
            // Reading a whole scalar formal copies it. A structural formal would
            // need a published move or disposal, which a template does not carry.
            NodeKind::PathExpression
                if file.nodes[node].children.is_empty()
                    && let Some(NodeSemantics::Expression(e)) = tree.semantics_at(node).next()
                    && typed_subject(tree, node, &e.ty)
                    && crate::lowering::types::scalar_carrier(&e.ty).is_ok()
                    && let Some(formal) = formal(tree, sig, node, &e.ty) =>
            {
                Ok((TemplateOperand::Formal(formal), &e.ty))
            }
            _ => Err(self.refuse()),
        }
    }

    fn call(
        &mut self,
        outer: usize,
    ) -> Result<(TemplateOperand, &'a ValueType), LoweringError> {
        let (tree, sig) = (self.tree, self.sig);
        let file = tree.resolved().file;
        let mut call = outer;
        while file.nodes[call].kind == NodeKind::CallMarker {
            let [inner] = *file.nodes[call].children.as_slice() else {
                return Err(self.refuse());
            };
            call = inner;
        }
        if file.nodes[call].kind != NodeKind::Call {
            return Err(self.refuse());
        }
        let (operation, ty) = lower_call(Some(self.inputs), tree, sig, call, self.bounds)?;
        if outer != call && (operation.acknowledgements == Default::default() || (outer..call).any(|n| !typed_subject(tree, n, ty))) {
            return Err(self.refuse());
        }
        match &operation.target {
            TemplateCallTarget::StaticBinder(slot) if !self.invokes.contains(slot) => return Err(self.refuse()),
            TemplateCallTarget::Member { binder, .. } if !matches!(self.parameters.get(*binder), Some(MachineTemplateBinder::Implementation(_))) => {
                return Err(LoweringError::new(&operation.site, "supplied member call lacks its operation-record binder"));
            }
            _ => {}
        }
        Ok((self.push(TemplateStep::Call(operation)), ty))
    }

    /// The classifier a closed body uses selects the primitive, policy and
    /// checker questions. A trapping operator would need a crash edge in the
    /// template, and a folded constant a literal operand; both still refuse.
    fn integer(
        &mut self,
        node: usize,
    ) -> Result<(TemplateOperand, &'a ValueType), LoweringError> {
        let (tree, sig) = (self.tree, self.sig);
        let file = tree.resolved().file;
        let mut selected = None;
        let mut obligations = Vec::new();
        for row in tree.semantics_at(node) {
            match row {
                NodeSemantics::Operation(op) if selected.is_none() => selected = Some(op),
                NodeSemantics::Obligation(o) => obligations.push(o),
                NodeSemantics::Expression(e) if e.static_argument_slot.is_none() => {}
                _ => return Err(self.refuse()),
            }
        }
        let op = selected.ok_or_else(|| self.refuse())?;
        let fixed = |ty: &ValueType| matches!(ty, ValueType::Integer { width: Some(w), .. } if *w > 0);
        if !fixed(&op.result) && op.result != ValueType::Bool
            || !op.operands.iter().all(fixed)
            || op.operands.len() != file.nodes[node].children.len()
            || tree
                .semantics_at(node)
                .any(|row| matches!(row, NodeSemantics::Expression(e) if e.ty != op.result))
        {
            return Err(self.refuse());
        }
        let crate::machines::arithmetic::LoweredInteger::Ordinary(operation) = crate::machines::arithmetic::lower_integer(op, &obligations, &sig.machine)?
        else {
            return Err(self.refuse());
        };
        let mut operands = Vec::with_capacity(op.operands.len());
        for (&child, carrier) in file.nodes[node].children.iter().zip(&op.operands) {
            let (operand, ty) = self.expression(child)?;
            if ty != carrier {
                return Err(self.refuse());
            }
            operands.push(operand);
        }
        let step = crate::TemplateInteger {
            operation,
            operands,
            result_type: type_expression(&op.result, &op.site)?,
            site: op.site.clone(),
        };
        Ok((self.push(TemplateStep::Integer(step)), &op.result))
    }
}

/// Entry formals can be substituted directly only when execution reaches
/// this binder unconditionally, with no preceding assignments or path tests.
pub(crate) fn tail_binder_formals(
    tree: &TypedTree<'_>,
    sig: &TypedSignature,
    node: usize,
    binder: usize,
    bound: &CallableType,
) -> Result<Vec<usize>, LoweringError> {
    let file = tree.resolved().file;
    let bad = || LoweringError::new(&site_at(tree, node), "callback crash route needs unconditional entry-formal operands");
    let body = file.child(sig.node.node, NodeKind::Body).ok_or_else(bad)?;
    let [tail] = file.nodes[body].children.as_slice() else { return Err(bad()) };
    if file.nodes[*tail].kind != NodeKind::Tail || file.nodes[*tail].children != [node] {
        return Err(bad());
    }
    // Without the prepared product only binder invocations on whole formals lower.
    let (call, _) = lower_call(None, tree, sig, node, &[(binder, bound)])?;
    if call.target != TemplateCallTarget::StaticBinder(binder) {
        return Err(bad());
    }
    call.operands
        .into_iter()
        .map(|operand| match operand {
            crate::TemplateOperand::Formal(formal) => Ok(formal),
            crate::TemplateOperand::RecordField { .. } | crate::TemplateOperand::Step(_) => Err(bad()),
        })
        .collect()
}

type SuppliedCalls = std::collections::HashMap<omega_resolved::TreeNodeId, usize>;

/// What a template body call may consult beyond its own typed rows. Declaration
/// calls, supplied member calls and projected operands need the prepared product.
#[derive(Clone, Copy)]
struct CallInputs<'p, 't> {
    prepared: &'p PreparedTrees<'t>,
    supplied_calls: &'p SuppliedCalls,
}

/// Match the binder bound, the typed call's enclosing application and the owner
/// of the packaged requirement. An equal signature does not select a member.
/// A restricted bound such as `Local=Log` declares no members of its own; a
/// call through it names a requirement of its base trait.
fn evidence_member(
    prepared: &PreparedTrees<'_>,
    caller: &omega_resolved::DeclarationId,
    packaged_owner: &omega_resolved::DeclarationId,
    call: &omega_typed::TypedCall,
) -> Option<usize> {
    let ValueType::Parameter { owner, index } = call.evidence.as_ref()? else { return None };
    let typed = prepared.typed();
    let param = typed.generic_parameters.get(caller)?.parameters.get(*index)?;
    let ValueType::Evidence(slot) = param.carrier.as_deref()? else { return None };
    // Typing records a call through a restriction as the restriction, then its
    // base, as the closed member path reads it (`members::Key::from_call`).
    let (home, args) = match (call.enclosing_arguments.as_slice(), typed.refinement_bases.get(&slot.declaration)) {
        ([(home, args)], None) if home == &slot.declaration => (home, args),
        ([(alias, alias_args), (home, args)], Some(base))
            if alias == &slot.declaration && alias_args.is_empty() && base.trait_arguments.is_empty() && &base.declaration == home =>
        {
            (home, args)
        }
        _ => return None,
    };
    (owner.as_ref() == caller
        && param.kind == NodeKind::ConstParam
        && home == packaged_owner
        && args.is_empty()
        && slot.trait_arguments.is_empty()
        && call.type_arguments.is_empty())
    .then_some(*index)
}

fn lower_call<'a>(
    inputs: Option<CallInputs<'_, '_>>,
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
    let mut enclosing_arguments = Vec::new();
    let mut refinements: Vec<_> = tree.semantics_at(node).filter_map(|row| match row {
        NodeSemantics::Obligation(o) => match &o.kind {
            ObligationKind::RefineCallable { formal: Some((owner, index)), contract, selection } => Some((owner, *index, contract.as_ref(), selection)),
            _ => None,
        },
        _ => None,
    }).collect();
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
                    && c.layout_query.is_none()
                    && c.operands.iter().all(|o| o.callable.is_none()) =>
            {
                let inputs = inputs.ok_or_else(bad)?;
                let typed = inputs.prepared.typed();
                let actual = c
                    .operands
                    .iter()
                    .map(|o| (o.node, &o.subject))
                    .collect::<Vec<_>>();
                let declaration = crate::identity::declaration(&c.machine, &site)?;
                let target = if let Some(&binder) = inputs.supplied_calls.get(&c.node) {
                    // The trait application stays on the binder's slot; it is
                    // not a lexical capture of an enclosing machine.
                    TemplateCallTarget::Member { binder, requirement: declaration }
                } else if c.evidence.is_none() {
                    enclosing_arguments = lexical::captures(typed, &c.machine, &c.enclosing_arguments, &site)?;
                    TemplateCallTarget::Declaration(declaration)
                } else {
                    return Err(bad());
                };
                selection = Some((
                    target,
                    c.type_arguments
                        .iter()
                        .enumerate().map(|(slot, v)| {
                            let ValueType::MachineSelection(app) = v else { return type_expression(v, &site) };
                            // The call's declaration and argument position identify the bound
                            // in Psi. Require the exact L3 refinement, not a compatible signature.
                            let i = refinements.iter().position(|(owner, index, contract, selected)| {
                                **owner == c.machine && *index == slot && *selected == app
                                    && typed.callable_bounds.iter().any(|(d, n, b)| d == *owner && *n == slot && b == *contract)
                            }).ok_or_else(bad)?;
                            refinements.remove(i);
                            Ok(TypeExpression(vec![crate::TypeExpressionStep::MachineSelection(Box::new(lexical::selection(typed, app, &site)?))]))
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                    actual,
                    &c.result,
                    c.acknowledgements,
                ));
            }
            NodeSemantics::Obligation(o) if matches!(&o.kind, ObligationKind::RefineCallable { formal: Some(_), .. }) => {}
            _ => return Err(bad()),
        }
    }
    if !refinements.is_empty() { return Err(bad()) }
    let (target, arguments, actual, ty, acknowledgements) = selection.ok_or_else(bad)?;
    if expr != Some(ty) || actual.len() != file.nodes[args].children.len() {
        return Err(bad());
    }
    let operands = file.nodes[args]
        .children
        .iter()
        .zip(actual)
        .map(|(&node, (id, subject))| {
            if file.node_id(node) != id {
                return Err(bad());
            }
            if file.nodes[node].kind == NodeKind::FieldAccess {
                if !matches!(subject, ValueType::Subject { declaration, version: None } if declaration == &site_at(tree, node)) {
                    return Err(bad());
                }
                return projected_operand(inputs.ok_or_else(bad)?.prepared, tree, sig, node).ok_or_else(bad);
            }
            let mut rows = tree.semantics_at(node);
            let Some(NodeSemantics::Expression(e)) = rows.next() else {
                return Err(bad());
            };
            if file.nodes[node].kind != NodeKind::PathExpression
                || !file.nodes[node].children.is_empty()
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
            Ok(crate::TemplateOperand::Formal(formal))
        })
        .collect::<Result<_, _>>()?;
    Ok((
        TemplateCall {
            target,
            arguments,
            enclosing_arguments,
            acknowledgements,
            operands,
            result_type: result_type(ty, &site)?,
            site,
        },
        ty,
    ))
}

/// A copy of a field below a formal record, such as `track.volume` or
/// `track.tag.id`. Every step is a plain record field of the value the previous
/// step reached, starting from the formal itself; the operand keeps the whole
/// path. The copied leaf must be unrestricted: a scalar, or a fixed array or
/// `[copy]` record of such values. The read then leaves the formal whole, so
/// the formal, its record and its siblings owe the same cleanup or linear use
/// as when the field is not read; the receiver's and siblings' obligations
/// are not conditions of the read. Moving an affine field is a partial
/// transfer with a residual, and a case payload step needs its case
/// established; neither has a template operand form, so both refuse.
fn projected_operand(prepared: &PreparedTrees<'_>, tree: &TypedTree<'_>, sig: &TypedSignature, node: usize) -> Option<crate::TemplateOperand> {
    let file = tree.resolved().file;
    // The selections from the outermost field inward, then the formal below them.
    let mut chain = vec![node];
    let mut base = loop {
        let [base] = *file.nodes[*chain.last()?].children.as_slice() else { return None };
        if file.nodes[base].kind != NodeKind::FieldAccess {
            break base;
        }
        chain.push(base);
    };
    let mut base_rows = tree.semantics_at(base);
    let NodeSemantics::Expression(root) = base_rows.next()? else { return None };
    if base_rows.next().is_some() || root.static_argument_slot.is_some() || file.nodes[base].kind != NodeKind::PathExpression
        || !file.nodes[base].children.is_empty()
    {
        return None;
    }
    let formal = formal(tree, sig, base, &root.ty)?;
    let mut receiver = &root.ty;
    let mut path = Vec::with_capacity(chain.len());
    for &step in chain.iter().rev() {
        let (mut leaf, mut field) = (None, None);
        for row in tree.semantics_at(step) {
            match row {
                NodeSemantics::Expression(e) if e.static_argument_slot.is_none() && leaf.is_none() => leaf = Some(&e.ty),
                NodeSemantics::Field(id) if field.is_none() => field = Some(id),
                _ => return None,
            }
        }
        let leaf = leaf?;
        // `field_segment` checks the typed place step and the field's declared
        // carrier; a case payload field is not a record step.
        let PredicateStep::Field(id) = contracts::field_segment(prepared, &LoweringTree::plain(tree), step, base, receiver, field?, leaf).ok()? else {
            return None;
        };
        path.push(id);
        (base, receiver) = (step, leaf);
    }
    let field = path.pop()?;
    let ty = type_expression(receiver, &site_at(tree, node)).ok()?;
    let argument = if crate::lowering::types::scalar_carrier(receiver).is_ok() {
        crate::Parameter::Scalar(ty)
    } else if copied_whole(prepared.typed(), receiver, &mut Vec::new()) {
        crate::Parameter::Structural(ty)
    } else {
        return None;
    };
    Some(crate::TemplateOperand::RecordField { formal, carrier: path, field, argument })
}

/// A structural value that a field read copies rather than moves: a fixed array
/// of scalars or of such values, or a `[copy]` record without cleanup whose
/// fields are all copied the same way. An affine or linear record, a reference
/// and anything L5 cannot classify are not copies.
fn copied_whole<'a>(typed: &'a omega_typed::TypedTrees<'_>, ty: &'a ValueType, open: &mut Vec<&'a omega_resolved::DeclarationId>) -> bool {
    let copied = |ty: &'a ValueType, open: &mut Vec<_>| crate::lowering::types::scalar_carrier(ty).is_ok() || copied_whole(typed, ty, open);
    match ty {
        ValueType::Array(element, _) => copied(element, open),
        ValueType::Nominal(id) => {
            let id = id.as_ref();
            if open.contains(&id)
                || typed.cleanups.iter().any(|row| &row.owner == id)
                || !typed.data_properties.iter().any(|(data, p)| data == id && p.multiplicity == omega_typed::Multiplicity::Copy)
            {
                return false;
            }
            let Some(fields) = typed.record_fields.get(id) else { return false };
            open.push(id);
            let plain = fields.iter().all(|f| copied(&f.ty, open));
            open.pop();
            plain
        }
        _ => false,
    }
}
