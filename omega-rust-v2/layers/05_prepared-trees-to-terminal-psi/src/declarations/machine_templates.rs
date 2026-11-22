mod lexical;

use super::{
    contracts::{self, ClauseOwner},
    type_expression::{indexed_expression, type_expression},
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

/// The selection Psi publishes for `application`: its declaration, its
/// operands and the checked operands of each enclosing generic machine. An
/// open template's `Describe` step and a closed body's `DescribeMachine`
/// carry this one record.
pub(crate) fn selection(
    typed: &omega_typed::TypedStage,
    application: &omega_typed::MachineApplication,
    site: &Site,
) -> Result<crate::TemplateSelection, LoweringError> {
    lexical::selection(typed, application, site)
}

fn result_type(
    ty: &ValueType,
    site: &Site,
) -> Result<Option<TypeExpression>, LoweringError> {
    (ty != &ValueType::NoResult)
        .then(|| indexed_expression(ty, site))
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
    if sig.satisfies.is_some() || !sig.multiplicity_bounds.is_empty() {
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
            // Trait lowering publishes these clauses alongside the requirement signature.
            NodeKind::Invokes if file.child(sig.node.node, NodeKind::Body).is_none()
                && file.nodes[sig.node.node].parent.is_some_and(|n| file.nodes[n].kind == NodeKind::Trait) => {}
            // Routes, including guards over the entry formals, are lowered below.
            NodeKind::Crashes => {}
            NodeKind::OperatorToken if sig.operator.as_ref().is_some_and(|binding| binding.node == file.node_id(child)) => {}
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
            .map(|(_, ty)| indexed_expression(ty, &sig.site))
            .collect::<Result<_, _>>()?,
        result: result_type(&sig.result, &sig.site)?,
        result_lifetime_input: super::callables::result_lifetime_input(&sig.result, sig.parameters.iter().map(|(_, ty)| ty)),
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
                        .map(|v| indexed_expression(v, site))
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
            // A direct call of a toolchain declaration names its provider's
            // declaration; this package neither copies nor opens a template for it.
            if let Some(target) = by_id.get(&call.machine) {
                if (!call.type_arguments.is_empty() || !call.enclosing_arguments.is_empty()) && open.insert(&target.machine) {
                    work.push(&target.machine);
                }
            } else if call.evidence.is_some() || prepared.typed().imported_machine_definition(&call.machine).is_none() {
                return Err(LoweringError::new(&call.site, "template call needs its packaged declaration"));
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
            NodeKind::LifetimeParam if param.carrier.is_none() && param.multiplicity.is_none() => MachineTemplateBinder::Type(TemplateBinder::Lifetime),
            // A static value binder such as `const N: u8` keeps its declared
            // carrier; the caller's application supplies the value.
            NodeKind::ConstParam
                if param.multiplicity.is_none()
                    && let Some(carrier) = param.carrier.as_deref()
                    && (lexical::static_value_carrier(carrier)
                        || matches!(carrier, ValueType::Nominal(id) if typed.record_fields.contains_key(id.as_ref()) && !typed.case_owners.values().any(|owner| owner == id.as_ref()))) =>
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
                callback_binder(prepared, bound)?
            }
            _ => return Err(LoweringError::new(&sig.site, OPEN_TEMPLATE)),
        };
        parameters.push(binder);
    }
    // A template needs some binder a caller supplies: a callable bound, a
    // static value, an operation record, or (for a nested declaration) the
    // enclosing machine's arguments. A telescope of lifetimes alone reaches
    // this planner only when one concrete body cannot name a lifetime as a
    // region (`regions::single_body`): it sits in a type with no region slot,
    // such as the toolchain's `TypeSchema<'t>`, or the body applies it at a
    // call. The template is then the machine's only Psi form.
    let lifetimes_only = !parameters.is_empty()
        && parameters
            .iter()
            .all(|p| matches!(p, MachineTemplateBinder::Type(TemplateBinder::Lifetime)));
    if bounds.is_empty()
        && !lifetimes_only
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
        locals: Vec::new(),
    };
    let mut result = None;
    for &statement in &file.nodes[body_node].children {
        if file.nodes[statement].kind == NodeKind::Machine {
            continue; // Lowered separately with its own identity and signature.
        }
        if file.nodes[statement].kind == NodeKind::Let && result.is_none() && tree.semantics_at(statement).next().is_none() {
            steps.bind(statement)?;
            continue;
        }
        let [outer] = *file.nodes[statement].children.as_slice() else {
            return Err(LoweringError::new(&sig.site, OPEN_TEMPLATE));
        };
        if file.nodes[statement].kind == NodeKind::Tail && result.is_none() && tree.semantics_at(statement).next().is_none()
            && file.nodes[outer].kind == NodeKind::PathExpression && file.nodes[outer].children.is_empty()
            && steps.local(outer).is_none()
            && let Some(binding) = lexical::static_result(typed, tree, sig, outer)
        {
            result = Some(binding);
            continue;
        }
        if result.is_some() || tree.semantics_at(statement).next().is_some() {
            return Err(LoweringError::new(&sig.site, OPEN_TEMPLATE));
        }
        let (operand, ty) = steps.expression(outer, file.nodes[statement].kind == NodeKind::Tail)?;
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
    let mut positive_widths = Vec::new();
    for row in &typed.obligations {
        let ObligationKind::PositiveWidth(width) = &row.kind else { continue };
        let at = row.node.node;
        if row.node.file == sig.node.file && row.site.package == sig.site.package && row.site.file == sig.site.file
            && (sig.node.node..file.nodes[sig.node.node].end).contains(&at)
            && std::iter::successors(Some(at), |&n| file.nodes[n].parent)
                .find(|&n| matches!(file.nodes[n].kind, NodeKind::Machine | NodeKind::Data)) == Some(sig.node.node)
        {
            positive_widths.push((row.site.clone(), indexed_expression(width, &row.site)?));
        }
    }
    Ok(MachineTemplate {
        declaration: crate::identity::declaration(&sig.machine, &sig.site)?,
        public: tree.resolved().is_exported(sig.node.node),
        operator: sig.operator.as_ref().map(super::machine::lower_operator).transpose()?,
        positive_widths,
        parameters,
        signature: signature(prepared, tree, sig)?,
        discarded_fields: residuals(typed, sig, &body)?,
        declared_pause: sig.pause,
        fixed_pause: effects.fixed_pause,
        callback_pause: effects.callback_pause.iter().map(|(i, p)| (*i, *p)).collect(),
        invokes,
        invokes_machines,
        body,
        result,
    })
}

pub(super) fn callback_binder(prepared: &PreparedTrees<'_>, bound: &CallableType) -> Result<MachineTemplateBinder, LoweringError> {
    let (requirement, interface) = if let Some(slot) = &bound.requirement {
        // Nominal predicates refer to the exact requirement declaration.
        if bound.requires || bound.has_guarantee {
            return Err(LoweringError::new(&bound.contract, OPEN_TEMPLATE));
        }
        let interface = TemplateCallable {
            parameters: bound
                .parameters
                .iter()
                .map(|v| type_expression(v, &bound.contract))
                .collect::<Result<_, _>>()?,
            result: result_type(&bound.result, &bound.contract)?,
            result_lifetime_input: super::callables::result_lifetime_input(&bound.result, bound.parameters.iter()),
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
            .ok_or_else(|| LoweringError::new(&bound.contract, OPEN_TEMPLATE))?;
        let interface = super::type_expression::callable_interface(prepared, &origin, input, bound)?;
        (None, interface)
    };
    Ok(MachineTemplateBinder::Callable { requirement, signature: Box::new(interface) })
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
    /// The operand each `let` binding holds, by its binding pattern node.
    locals: Vec<(usize, TemplateOperand)>,
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

    /// Whether `node`'s expression kind is one this builder lowers. A call
    /// operand of any other kind keeps the call's refusal.
    fn lowers(
        &self,
        node: usize,
    ) -> bool {
        let file = self.tree.resolved().file;
        matches!(
            file.nodes[node].kind,
            NodeKind::Paren
                | NodeKind::Call
                | NodeKind::CallMarker
                | NodeKind::Literal
                | NodeKind::Binary
                | NodeKind::Unary
                | NodeKind::RecordLiteral
                | NodeKind::Dereference
                | NodeKind::ArrayLiteral
                | NodeKind::Match
        ) || file.nodes[node].kind == NodeKind::PathExpression && file.nodes[node].children.is_empty()
    }

    /// L3 hands the returned value to the lifetime checker with `ReturnBorrow`
    /// on that value's node. The template result keeps the value, so the row
    /// needs no other record; anywhere else it refuses.
    fn returned_row(
        &self,
        row: &NodeSemantics<'_>,
        node: usize,
        returned: bool,
    ) -> bool {
        returned
            && matches!(row, NodeSemantics::Obligation(o) if o.node == self.tree.resolved().file.node_id(node)
                && matches!(&o.kind, ObligationKind::ReturnBorrow { machine } if machine == &self.sig.machine))
    }

    /// The node's one typed expression; besides it only the returned row may appear.
    fn typed_value(
        &self,
        node: usize,
        returned: bool,
    ) -> Result<&'a omega_typed::TypedExpression, LoweringError> {
        let mut found = None;
        for row in self.tree.semantics_at(node) {
            match row {
                NodeSemantics::Expression(e) if found.is_none() && e.static_argument_slot.is_none() => found = Some(e),
                ref row if self.returned_row(row, node, returned) => {}
                _ => return Err(self.refuse()),
            }
        }
        found.ok_or_else(|| self.refuse())
    }

    /// The `let` binding pattern that `node` reads, and the operand it holds.
    fn local(
        &self,
        node: usize,
    ) -> Option<(usize, &TemplateOperand)> {
        let Some(Symbol::Binding { binder_node, .. }) = self.tree.resolved().symbol(node) else {
            return None;
        };
        self.locals
            .iter()
            .find(|(pattern, _)| pattern == binder_node)
            .map(|(pattern, operand)| (*pattern, operand))
    }

    /// `let name[: T] = value;` binds the initializer's operand to an immutable name.
    fn bind(
        &mut self,
        statement: usize,
    ) -> Result<(), LoweringError> {
        let tree = self.tree;
        let file = tree.resolved().file;
        let children = &file.nodes[statement].children;
        let (&pattern, &value) = match children.as_slice() {
            [pattern, value] | [pattern, _, value] => (pattern, value),
            _ => return Err(self.refuse()),
        };
        let mut rows = tree.semantics_at(pattern);
        let Some(NodeSemantics::Expression(bound)) = rows.next() else {
            return Err(self.refuse());
        };
        if rows.next().is_some()
            || bound.static_argument_slot.is_some()
            || file.nodes[pattern].kind != NodeKind::BindingPattern
            || !file.nodes[pattern].children.is_empty()
            || self
                .inputs
                .prepared
                .typed()
                .mutable_bindings
                .contains(&file.node_id(pattern))
        {
            return Err(self.refuse());
        }
        if let [_, annotation, _] = children.as_slice()
            && (*annotation..file.nodes[*annotation].end).any(|part| tree.semantics_at(part).next().is_some())
        {
            return Err(self.refuse());
        }
        let (operand, ty) = self.expression(value, false)?;
        if ty != &bound.ty {
            return Err(self.refuse());
        }
        self.locals.push((pattern, operand));
        Ok(())
    }

    /// The operand for the value of `node`, appending the steps that produce it.
    /// `returned` admits L3's `ReturnBorrow` row on the value the machine returns.
    fn expression(
        &mut self,
        node: usize,
        returned: bool,
    ) -> Result<(TemplateOperand, &'a ValueType), LoweringError> {
        let (tree, sig) = (self.tree, self.sig);
        let file = tree.resolved().file;
        match file.nodes[node].kind {
            NodeKind::Paren => {
                let [inner] = *file.nodes[node].children.as_slice() else {
                    return Err(self.refuse());
                };
                let (operand, ty) = self.expression(inner, returned)?;
                // Parentheses add no typed record beyond the value itself.
                if tree
                    .semantics_at(node)
                    .any(|row| !matches!(row, NodeSemantics::Expression(e) if &e.ty == ty && e.static_argument_slot.is_none()))
                {
                    return Err(self.refuse());
                }
                Ok((operand, ty))
            }
            NodeKind::Call if tree.declaration_selection(node).is_some() => self.describe(node, returned),
            NodeKind::CallMarker | NodeKind::Call => self.call(node, returned),
            NodeKind::Binary | NodeKind::Unary => self.integer(node),
            NodeKind::Cast => self.qualify(node, returned),
            NodeKind::Literal if file.nodes[node].children.is_empty() => {
                let mut landing = None;
                let mut expression = None;
                for row in tree.semantics_at(node) {
                    match row {
                        NodeSemantics::Expression(e) if e.static_argument_slot.is_none() && expression.is_none() => expression = Some(&e.ty),
                        NodeSemantics::Landing(l) if landing.is_none() && l.value.is_integer() && l.ty.contains_integer(l.value.numer()) => landing = Some(l),
                        _ => return Err(self.refuse()),
                    }
                }
                let landing = landing.ok_or_else(|| self.refuse())?;
                if !expression.is_some_and(|ty| *ty == landing.ty || matches!(ty, ValueType::Exact { value, .. } if value == &landing.value)) {
                    return Err(self.refuse());
                }
                Ok((TemplateOperand::Constant(crate::ScalarLiteral::Integer(landing.value.numer().clone())), &landing.ty))
            }
            NodeKind::RecordLiteral => self.construct(node, returned),
            NodeKind::Dereference => self.copy_referent(node, returned),
            NodeKind::ArrayLiteral => self.array(node, returned),
            NodeKind::Match => self.select(node, returned),
            // A `let` local names the value its initializer produced.
            NodeKind::PathExpression
                if file.nodes[node].children.is_empty()
                    && let Some((_, local)) = self.local(node) =>
            {
                let local = local_copy(local).ok_or_else(|| self.refuse())?;
                Ok((local, &self.typed_value(node, returned)?.ty))
            }
            NodeKind::PathExpression
                if file.nodes[node].children.is_empty()
                    && let Some(Symbol::Builtin(name)) = tree.resolved().symbol(node)
                    && matches!(name.as_str(), "true" | "false")
                    && typed_subject(tree, node, &ValueType::Bool) =>
            {
                let literal = crate::ScalarLiteral::Boolean(name == "true");
                Ok((TemplateOperand::Constant(literal), &self.typed_value(node, false)?.ty))
            }
            // Reading a whole scalar formal copies it. A structural formal would
            // need a published move or disposal, which a template does not carry.
            NodeKind::PathExpression
                if file.nodes[node].children.is_empty()
                    && let Some(NodeSemantics::Expression(e)) = tree.semantics_at(node).next()
                    && typed_subject(tree, node, &e.ty)
                    && (crate::lowering::types::scalar_carrier(e.ty.carrier()).is_ok() || matches!(e.ty.carrier(), ValueType::IndexedInteger { .. }))
                    && e.ty.domains().iter().all(|atom| !tree.forest().cleanups.iter().any(|cleanup| cleanup.owner == atom.declaration))
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
        returned: bool,
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
        let bounds = self.bounds;
        let (operation, ty) = lower_call(Some(&mut *self), tree, sig, call, bounds, returned)?;
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

    fn qualify(&mut self, node: usize, returned: bool) -> Result<(TemplateOperand, &'a ValueType), LoweringError> {
        let tree = self.tree;
        let file = tree.resolved().file;
        let mut operation = None;
        let mut result = None;
        let mut establishes = Vec::new();
        for row in tree.semantics_at(node) {
            match row {
                NodeSemantics::Operation(op) if op.token == "as" && operation.is_none() => operation = Some(op),
                NodeSemantics::Expression(e) if e.static_argument_slot.is_none() && result.is_none() => result = Some(&e.ty),
                NodeSemantics::Obligation(o) if o.node == file.node_id(node) => match &o.kind {
                    ObligationKind::EstablishDomain(atom) => establishes.push(atom),
                    _ if self.returned_row(&row, node, returned) => {}
                    _ => return Err(self.refuse()),
                },
                _ => return Err(self.refuse()),
            }
        }
        let op = operation.ok_or_else(|| self.refuse())?;
        let [source] = op.operands.as_slice() else { return Err(self.refuse()) };
        let result = result.filter(|ty| **ty == op.result && ty.carrier() == source.carrier()).ok_or_else(|| self.refuse())?;
        let added = result.domains().iter().filter(|atom| !source.domains().contains(atom));
        if !added.eq(establishes.iter().copied()) || result.domains() == source.domains() {
            return Err(self.refuse());
        }
        let child = *file.nodes[node].children.first().ok_or_else(|| self.refuse())?;
        let (operand, actual) = self.expression(child, false)?;
        if actual != source {
            return Err(self.refuse());
        }
        let establishes = establishes.into_iter().map(|atom| crate::LoweringTypes::domain_atom(atom, &op.site)).collect::<Result<_, _>>()?;
        let step = TemplateStep::Qualify { operand, source: indexed_expression(source, &op.site)?, result: indexed_expression(result, &op.site)?, establishes, site: op.site.clone() };
        Ok((self.push(step), result))
    }

    /// The classifier a closed body uses selects the primitive, policy and
    /// checker questions. A trapping operator retains its crash site in the
    /// template. Literal operands retain their checked numeric landings.
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
        let fixed = |ty: &ValueType| matches!(ty, ValueType::Integer { width: Some(w), .. } if *w > 0) || matches!(ty, ValueType::IndexedInteger { .. });
        if !fixed(&op.result) && op.result != ValueType::Bool
            || !op.operands.iter().all(fixed)
            || op.operands.len() != file.nodes[node].children.len()
            || tree
                .semantics_at(node)
                .any(|row| matches!(row, NodeSemantics::Expression(e) if e.ty != op.result))
        {
            return Err(self.refuse());
        }
        let operation = crate::machines::arithmetic::lower_integer(op, &obligations, &sig.machine)?;
        let mut operands = Vec::with_capacity(op.operands.len());
        for (&child, carrier) in file.nodes[node].children.iter().zip(&op.operands) {
            let (operand, ty) = self.expression(child, false)?;
            if ty != carrier {
                return Err(self.refuse());
            }
            operands.push(operand);
        }
        let result_type = indexed_expression(&op.result, &op.site)?;
        let site = op.site.clone();
        let step = match operation {
            crate::machines::arithmetic::LoweredInteger::Ordinary(operation) => TemplateStep::Integer(crate::TemplateInteger { operation, operands, result_type, site }),
            crate::machines::arithmetic::LoweredInteger::Trapping(primitive) => TemplateStep::TrappingInteger { primitive, operands, result_type, site },
        };
        Ok((self.push(step), &op.result))
    }

    fn copy_referent(&mut self, node: usize, returned: bool) -> Result<(TemplateOperand, &'a ValueType), LoweringError> {
        let tree = self.tree;
        let file = tree.resolved().file;
        let [place] = *file.nodes[node].children.as_slice() else { return Err(self.refuse()) };
        if file.nodes[place].kind != NodeKind::PathExpression || !file.nodes[place].children.is_empty() {
            return Err(self.refuse());
        }
        let reference = &self.typed_value(place, false)?.ty;
        let Some((omega_typed::Access::Shared, referent)) = reference.reference_parts() else { return Err(self.refuse()) };
        let formal = formal(tree, self.sig, place, reference).ok_or_else(|| self.refuse())?;
        let site = site_at(tree, node);
        let mut result = None;
        let mut copied = false;
        for row in tree.semantics_at(node) {
            match row {
                NodeSemantics::Expression(e) if result.is_none() && e.static_argument_slot.is_none() && &e.ty == referent => result = Some(&e.ty),
                NodeSemantics::Obligation(o) if !copied && o.node == file.node_id(node) && o.site == site
                    && matches!(&o.kind, ObligationKind::CopyRead(ty) if ty == referent) => copied = true,
                ref row if self.returned_row(row, node, returned) => {}
                _ => return Err(self.refuse()),
            }
        }
        let result = result.filter(|_| copied).ok_or_else(|| self.refuse())?;
        let step = TemplateStep::CopyReferent { formal, ty: type_expression(result, &site)?, site };
        Ok((self.push(step), result))
    }

    /// A record or case literal. Written fields lower in source order; each
    /// omitted field named by a `ZeroField` row reads that field's zero. An
    /// `EstablishData` row stays on the step as the data's owed `requires`.
    fn construct(
        &mut self,
        node: usize,
        returned: bool,
    ) -> Result<(TemplateOperand, &'a ValueType), LoweringError> {
        let tree = self.tree;
        let file = tree.resolved().file;
        let site = site_at(tree, node);
        let construction = tree.construction(node).ok_or_else(|| self.refuse())?;
        let (mut typed_result, mut establishes) = (false, false);
        let Some((data, arguments)) = construction.carrier.nominal_parts() else { return Err(self.refuse()) };
        if data != &construction.owner { return Err(self.refuse()); }
        let mut omitted = Vec::new();
        for row in tree.semantics_at(node) {
            match row {
                NodeSemantics::Expression(e) if !typed_result && e.ty == construction.carrier && e.static_argument_slot.is_none() => typed_result = true,
                ref row if self.returned_row(row, node, returned) => {}
                NodeSemantics::Obligation(o) if o.site == site && o.node == file.node_id(node) => match &o.kind {
                    ObligationKind::ZeroField { field, ty } => omitted.push((field, ty)),
                    ObligationKind::EstablishData(owner) if !establishes && owner == &construction.owner => establishes = true,
                    _ => return Err(self.refuse()),
                },
                _ => return Err(self.refuse()),
            }
        }
        let owner = construction.case.as_ref().unwrap_or(&construction.owner);
        let roster = field_roster(self.inputs.prepared, owner).ok_or_else(|| self.refuse())?;
        let [path, ..] = file.nodes[node].children.as_slice() else {
            return Err(self.refuse());
        };
        if !typed_result
            || file.nodes[*path].kind != NodeKind::PathExpression
            || (*path..file.nodes[*path].end).any(|part| tree.semantics_at(part).next().is_some())
        {
            return Err(self.refuse());
        }
        let mut filled = vec![false; roster.len()];
        let mut fields = Vec::with_capacity(roster.len());
        for initializer in &construction.initializers {
            let Some(slot) = filled.get_mut(initializer.position).filter(|slot| !**slot) else {
                return Err(self.refuse());
            };
            *slot = true;
            let input = initializer.expression;
            let value = if returned && let Some(moved) = self.move_into_result(input, &roster[initializer.position].ty.substitute(data, arguments))? {
                moved
            } else {
                self.expression(input, false)?.0
            };
            fields.push((crate::identity::declaration(&roster[initializer.position].declaration, &site)?, value));
        }
        for (field, ty) in omitted {
            let Some(position) = roster.iter().position(|row| &row.declaration == field) else {
                return Err(self.refuse());
            };
            if std::mem::replace(&mut filled[position], true) || roster[position].ty.substitute(data, arguments) != *ty {
                return Err(self.refuse());
            }
            fields.push((crate::identity::declaration(field, &site)?, zero(field, ty, &site)?));
        }
        if filled.contains(&false) {
            return Err(self.refuse());
        }
        let step = crate::TemplateConstruct {
            declaration: crate::identity::declaration(owner, &site)?,
            ty: type_expression(&construction.carrier, &site)?,
            fields,
            establishes,
            site,
        };
        Ok((self.push(TemplateStep::Construct(step)), &construction.carrier))
    }

    /// A field of the returned construction takes an entire owned formal.
    /// An affine formal moves even when its data has no explicit attributes.
    fn move_into_result(&mut self, node: usize, destination: &ValueType) -> Result<Option<TemplateOperand>, LoweringError> {
        let tree = self.tree;
        let file = tree.resolved().file;
        if file.nodes[node].kind != NodeKind::PathExpression || !file.nodes[node].children.is_empty() {
            return Ok(None);
        }
        let Some(NodeSemantics::Expression(value)) = tree.semantics_at(node).next() else { return Ok(None) };
        let Some((owner, _)) = value.ty.nominal_parts() else { return Ok(None) };
        let Some(formal) = formal(tree, self.sig, node, &value.ty) else { return Ok(None) };
        if owner.kind != NodeKind::Data || &value.ty != destination || !typed_subject(tree, node, &value.ty) { return Err(self.refuse()); }
        let forest = tree.forest();
        let declared = forest.data_properties.iter().chain(forest.library.into_iter().flat_map(|lib| &lib.data_properties))
            .find(|(id, _)| id == owner);
        if declared.is_some_and(|(_, p)| p.multiplicity == omega_typed::Multiplicity::Copy) {
            return Ok(Some(TemplateOperand::Formal(formal)));
        }
        let site = site_at(tree, node);
        let step = TemplateStep::MoveFormal { formal, ty: type_expression(&value.ty, &site)?, site };
        Ok(Some(self.push(step)))
    }

    /// An array literal at its typed destination, an owned `[T]` or `[T; N]`.
    /// Its elements lower in order and each must land at `T`.
    fn array(
        &mut self,
        node: usize,
        returned: bool,
    ) -> Result<(TemplateOperand, &'a ValueType), LoweringError> {
        let file = self.tree.resolved().file;
        let site = site_at(self.tree, node);
        let literal = self.typed_value(node, returned)?;
        let children = &file.nodes[node].children;
        let element = match &literal.ty {
            ValueType::Slice(element) => element.as_ref(),
            ValueType::Array(element, length) if u64::try_from(children.len()).is_ok_and(|n| n == *length) => element.as_ref(),
            _ => return Err(self.refuse()),
        };
        let mut elements = Vec::with_capacity(children.len());
        for &child in children {
            let (value, ty) = self.expression(child, false)?;
            if ty != element {
                return Err(self.refuse());
            }
            elements.push(value);
        }
        let step = crate::TemplateArray {
            element: type_expression(element, &site)?,
            ty: type_expression(&literal.ty, &site)?,
            elements,
            site,
        };
        Ok((self.push(TemplateStep::Array(step)), &literal.ty))
    }

    /// `reflect\callable(pick)`: L3 selected the described machine and the
    /// descriptor type. It is a value, not a call, and grants nothing; no node
    /// below it carries a row of its own.
    fn describe(
        &mut self,
        node: usize,
        returned: bool,
    ) -> Result<(TemplateOperand, &'a ValueType), LoweringError> {
        let tree = self.tree;
        let file = tree.resolved().file;
        let site = site_at(tree, node);
        let Some(omega_typed::DeclarationSelection::Machine(application)) = tree.declaration_selection(node) else {
            return Err(self.refuse());
        };
        let descriptor = self.typed_value(node, returned)?;
        if (node + 1..file.nodes[node].end).any(|part| tree.semantics_at(part).next().is_some()) {
            return Err(self.refuse());
        }
        let step = crate::TemplateDescriptor {
            selection: lexical::selection(self.inputs.prepared.typed(), application, &site)?,
            ty: type_expression(&descriptor.ty, &site)?,
            site,
        };
        Ok((self.push(TemplateStep::Describe(step)), &descriptor.ty))
    }

    /// A `match` on a Boolean value, classified as a closed body classifies
    /// it. Each arm's steps follow the select step, in arm order.
    fn select(
        &mut self,
        node: usize,
        returned: bool,
    ) -> Result<(TemplateOperand, &'a ValueType), LoweringError> {
        let tree = self.tree;
        let file = tree.resolved().file;
        let site = site_at(tree, node);
        let ty = &self.typed_value(node, returned)?.ty;
        let (subject, arms) = crate::machines::branches::boolean_dispatch(&LoweringTree::plain(tree), node)?;
        let (subject, subject_ty) = self.expression(subject, false)?;
        if subject_ty != &ValueType::Bool {
            return Err(self.refuse());
        }
        let index = self.body.len();
        let placeholder = crate::TemplateSelect {
            subject,
            arms: Vec::new(),
            ty: type_expression(ty, &site)?,
            site,
        };
        self.body.push(TemplateStep::Select(placeholder));
        let mut lowered = Vec::with_capacity(arms.len());
        for arm in arms {
            let start = self.body.len();
            let (result, arm_ty) = self.expression(arm.expression, false)?;
            let Some(arm_node) = file.nodes[arm.expression].parent else {
                return Err(self.refuse());
            };
            if arm_ty != ty {
                return Err(self.refuse());
            }
            lowered.push(crate::TemplateArm {
                when: arm.sense,
                steps: start..self.body.len(),
                result,
                site: site_at(tree, arm_node),
            });
        }
        let TemplateStep::Select(select) = &mut self.body[index] else {
            unreachable!("the select step stays at its index")
        };
        select.arms = lowered;
        Ok((TemplateOperand::Step(index), ty))
    }
}

/// A local use names the operand of its initializer. Reusing that operand in
/// Psi preserves its identity for the checker's ownership judgment.
fn local_copy(operand: &TemplateOperand) -> Option<TemplateOperand> {
    Some(match operand {
        TemplateOperand::Formal(i) => TemplateOperand::Formal(*i),
        TemplateOperand::Step(i) => TemplateOperand::Step(*i),
        TemplateOperand::Constant(value) => TemplateOperand::Constant(value.clone()),
        _ => return None,
    })
}

/// The typed field roster of a record or case, from this forest or the
/// imported toolchain forest that declares it.
fn field_roster<'t>(
    prepared: &'t PreparedTrees<'_>,
    owner: &omega_resolved::DeclarationId,
) -> Option<&'t [omega_typed::TypedField]> {
    let typed = prepared.typed();
    typed
        .record_fields
        .get(owner)
        .or_else(|| typed.library?.record_fields.get(owner))
        .map(|fields| fields.as_slice())
}

/// A structural zero names the type whose active-case fields and default
/// domain the checker must establish. A reference is never supplied.
fn zero(
    field: &omega_resolved::DeclarationId,
    ty: &ValueType,
    site: &Site,
) -> Result<TemplateOperand, LoweringError> {
    match ty {
        ValueType::Bool | ValueType::Integer { width: Some(_), .. } | ValueType::Slice(_)
        | ValueType::Array(..) | ValueType::Nominal(_) | ValueType::Applied(..) => Ok(TemplateOperand::Zero(type_expression(ty, site)?)),
        _ if ty
            .reference_parts()
            .is_some_and(|(_, referent)| !matches!(referent, ValueType::Slice(_))) =>
        {
            Err(LoweringError::new(
                site,
                format!(
                    "omitted field `{}` of type `{ty}` needs a borrowed reference, which `..` never supplies",
                    field.path
                ),
            ))
        }
        _ => Err(LoweringError::new(
            site,
            format!("cannot lower implicit zero of type `{ty}` for omitted field `{}`", field.path),
        )),
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
    let (call, _) = lower_call(None, tree, sig, node, &[(binder, bound)], true)?;
    if call.target != TemplateCallTarget::StaticBinder(binder) {
        return Err(bad());
    }
    call.operands
        .into_iter()
        .map(|operand| match operand {
            crate::TemplateOperand::Formal(formal) => Ok(formal),
            _ => Err(bad()),
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

/// Without `steps`, only operands that need no prepared product lower: whole
/// formals. With it, an operand may also be a borrow of a formal, a projected
/// field, or any value the step builder lowers.
fn lower_call<'a>(
    mut steps: Option<&mut Steps<'_, '_, '_, '_>>,
    tree: &'a TypedTree<'_>,
    sig: &TypedSignature,
    node: usize,
    bounds: &[(usize, &CallableType)],
    returned: bool,
) -> Result<(TemplateCall, &'a ValueType), LoweringError> {
    let inputs = steps.as_deref().map(|builder| builder.inputs);
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
            // L3 hands a returned call result to the lifetime checker with the
            // selected signature. The returned call keeps that signature and its
            // arguments, so the row needs no further record here.
            NodeSemantics::Obligation(o)
                if returned
                    && o.node == file.node_id(node)
                    && o.site == site
                    && matches!(&o.kind, ObligationKind::ReturnBorrow { machine } if machine == &sig.machine) => {}
            _ => return Err(bad()),
        }
    }
    if !refinements.is_empty() { return Err(bad()) }
    let (target, arguments, actual, ty, acknowledgements) = selection.ok_or_else(bad)?;
    if expr != Some(ty) || actual.len() != file.nodes[args].children.len() {
        return Err(bad());
    }
    let mut operands = Vec::with_capacity(actual.len());
    // Operand position, formal, next step at formation, and source node for earlier borrows.
    let mut borrowed = Vec::new();
    for (&node, (id, subject)) in file.nodes[args].children.iter().zip(actual) {
        if file.node_id(node) != id {
            return Err(bad());
        }
        let at = |declaration: &Site| matches!(subject, ValueType::Subject { declaration: d, version: None } if d == declaration);
        let temporary = at(&site_at(tree, node));
        if file.nodes[node].kind == NodeKind::FieldAccess {
            if !temporary {
                return Err(bad());
            }
            operands.push(projected_operand(inputs.ok_or_else(bad)?.prepared, tree, sig, node).ok_or_else(bad)?);
            continue;
        }
        if file.nodes[node].kind == NodeKind::Borrow && temporary {
            let operand = borrowed_formal(inputs.ok_or_else(bad)?.prepared, tree, sig, node).ok_or_else(bad)?;
            if let TemplateOperand::Borrow { formal, .. } = &operand
                && let Some(builder) = steps.as_deref()
            {
                borrowed.push((operands.len(), *formal, builder.body.len(), node));
            }
            operands.push(operand);
            continue;
        }
        let mut rows = tree.semantics_at(node);
        if file.nodes[node].kind == NodeKind::PathExpression
            && file.nodes[node].children.is_empty()
            && let Some(NodeSemantics::Expression(e)) = rows.next()
            && rows.next().is_none()
            && e.static_argument_slot.is_none()
            && let Some(formal) = formal(tree, sig, node, &e.ty)
        {
            let mut declaration = sig.site.clone();
            declaration.span = sig.parameters[formal].0;
            if !at(&declaration) {
                return Err(bad());
            }
            operands.push(TemplateOperand::Formal(formal));
            continue;
        }
        let builder = steps
            .as_deref_mut()
            .filter(|builder| builder.lowers(node))
            .ok_or_else(bad)?;
        // A `let` local names the value its binding pattern declares.
        let expected = match builder.local(node) {
            Some((pattern, _)) => at(&site_at(tree, pattern)),
            None => temporary,
        };
        if !expected {
            return Err(bad());
        }
        let start = builder.body.len();
        let (operand, _) = builder.expression(node, false)?;
        if builder.body.len() != start && operands.iter().any(|operand| matches!(operand, TemplateOperand::MoveField { .. })) {
            return Err(LoweringError::new(&site, "template field transfer before a later operand step needs ordered custody"));
        }
        // Preserve the birth of earlier loans whose borrowers overlap these
        // new steps. Other borrows remain ordinary call-scoped operands.
        for &(position, formal, first_step, origin) in &borrowed {
            if builder.body[start..].iter().any(|step| reads_formal(step, &[formal]))
                && let TemplateOperand::Borrow { access, .. } = &operands[position]
            {
                operands[position] = TemplateOperand::BorrowBefore { formal, access: access.clone(), step: first_step, site: site_at(tree, origin) };
            }
        }
        operands.push(operand);
    }
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

/// `@formal` for the duration of the call, with the typed borrow of exactly
/// that formal. The template publishes no disposal, so a borrowed formal must
/// be one that discarding drops: a scalar, a `copy` carrier, or a value
/// `copied_whole` admits. A borrow of a field or of anything else refuses.
fn borrowed_formal(
    prepared: &PreparedTrees<'_>,
    tree: &TypedTree<'_>,
    sig: &TypedSignature,
    node: usize,
) -> Option<TemplateOperand> {
    let file = tree.resolved().file;
    let [place] = *file.nodes[node].children.as_slice() else { return None };
    let (mut reference, mut borrowed) = (None, None);
    for row in tree.semantics_at(node) {
        match row {
            NodeSemantics::Expression(e) if reference.is_none() && e.static_argument_slot.is_none() => reference = Some(&e.ty),
            NodeSemantics::Obligation(o) if borrowed.is_none() && o.node == file.node_id(node) => match &o.kind {
                ObligationKind::Borrow { access, place: at } if *at == site_at(tree, place) => borrowed = Some(access),
                _ => return None,
            },
            _ => return None,
        }
    }
    let ValueType::Reference(access, referent) = reference? else { return None };
    let mut rows = tree.semantics_at(place);
    let Some(NodeSemantics::Expression(read)) = rows.next() else { return None };
    if borrowed? != access
        || rows.next().is_some()
        || read.static_argument_slot.is_some()
        || &read.ty != referent.as_ref()
        || file.nodes[place].kind != NodeKind::PathExpression
        || !file.nodes[place].children.is_empty()
    {
        return None;
    }
    let formal = formal(tree, sig, place, &read.ty)?;
    let typed = prepared.typed();
    let declared = |id: &omega_resolved::DeclarationId| {
        typed
            .data_properties
            .iter()
            .find(|(data, _)| data == id)
            .map(|(_, p)| p.multiplicity)
    };
    let copy = read.ty.multiplicity_with(&declared, &|_, _| None) == Some(omega_typed::Multiplicity::Copy);
    if !copy && crate::lowering::types::scalar_carrier(&read.ty).is_err() && !copied_whole(typed, &read.ty, &mut Vec::new()) {
        return None;
    }
    let access = match access {
        omega_typed::Access::Shared => crate::TemplateAccess::Shared,
        omega_typed::Access::Mut => crate::TemplateAccess::Mut,
        omega_typed::Access::Write => crate::TemplateAccess::Write,
    };
    Some(TemplateOperand::Borrow { formal, access })
}

/// Whether a step reads one of these entry formals directly.
fn reads_formal(
    step: &TemplateStep,
    formals: &[usize],
) -> bool {
    use TemplateOperand as O;
    let direct = |operand: &O| matches!(operand, O::Formal(f) | O::RecordField { formal: f, .. } | O::Borrow { formal: f, .. } | O::BorrowBefore { formal: f, .. } | O::MoveField { formal: f, .. } if formals.contains(f));
    match step {
        TemplateStep::CopyReferent { formal, .. } | TemplateStep::MoveFormal { formal, .. } => formals.contains(formal),
        TemplateStep::Call(call) => call.operands.iter().any(direct),
        TemplateStep::Integer(integer) => integer.operands.iter().any(direct),
        TemplateStep::TrappingInteger { operands, .. } => operands.iter().any(direct),
        TemplateStep::Qualify { operand, .. } => direct(operand),
        TemplateStep::Construct(construct) => construct.fields.iter().any(|(_, value)| direct(value)),
        TemplateStep::Array(array) => array.elements.iter().any(direct),
        TemplateStep::Describe(_) => false,
        TemplateStep::Select(select) => direct(&select.subject) || select.arms.iter().any(|arm| direct(&arm.result)),
    }
}

/// A copy of a field below a formal record, such as `track.volume` or
/// `track.tag.id`. Every step is a plain record field of the value the previous
/// step reached, starting from the formal itself; the operand keeps the whole
/// path. The copied leaf must be unrestricted: a scalar, or a fixed array or
/// `[copy]` record of such values. The read then leaves the formal whole, so
/// the formal, its record and its siblings owe the same cleanup or linear use
/// as when the field is not read; the receiver's and siblings' obligations
/// are not conditions of the read. A direct affine field of a plain
/// record transfers with its return residual. A case payload needs its
/// active case established before the transfer.
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
    } else if path.is_empty() && plain_affine_record(prepared.typed(), receiver) && plain_affine_record(prepared.typed(), &root.ty) {
        return Some(TemplateOperand::MoveField { formal, field, ty });
    } else {
        return None;
    };
    Some(crate::TemplateOperand::RecordField { formal, carrier: path, field, argument })
}

/// Plain residual disposal requires an affine record with no independent nominal cleanup.
fn plain_affine_record(typed: &omega_typed::TypedTrees<'_>, ty: &ValueType) -> bool {
    let ValueType::Nominal(id) = ty else { return false };
    typed.record_fields.contains_key(id.as_ref()) && !typed.case_owners.values().any(|owner| owner == id.as_ref()) && !typed.cleanups.iter().any(|row| &row.owner == id.as_ref())
        && typed.data_properties.iter().find(|(data, _)| data == id.as_ref())
            .is_none_or(|(_, p)| p.multiplicity == omega_typed::Multiplicity::Affine)
}

/// The direct-field complement of all unconditional transfers, in reverse
/// formal and reverse field order. More complex destruction needs its own
/// ordered cleanup actions. A conditional transfer needs separate exits.
fn residuals(typed: &omega_typed::TypedTrees<'_>, sig: &TypedSignature, body: &[TemplateStep]) -> Result<Vec<crate::TemplateResidual>, LoweringError> {
    use std::collections::{BTreeMap, BTreeSet};
    let mut moved = BTreeMap::<usize, BTreeSet<&crate::DeclarationId>>::new();
    let mut whole_actuals = BTreeSet::new();
    for (index, step) in body.iter().enumerate() {
        if let TemplateStep::MoveFormal { formal, .. } = step { whole_actuals.insert(*formal); }
        let Some(call) = step.call() else { continue };
        for operand in &call.operands {
            match operand {
                TemplateOperand::MoveField { formal, field, .. } => {
                    if body.iter().any(|step| matches!(step, TemplateStep::Select(s) if s.arms.iter().any(|arm| arm.steps.contains(&index))))
                        || !moved.entry(*formal).or_default().insert(field)
                    {
                        return Err(LoweringError::new(&call.site, "template field transfer needs a distinct unconditional source"));
                    }
                }
                TemplateOperand::Formal(formal) => { whole_actuals.insert(*formal); }
                _ => {}
            }
        }
    }
    let mut residual = Vec::new();
    for (formal, holes) in moved.into_iter().rev() {
        if whole_actuals.contains(&formal) {
            return Err(LoweringError::new(&sig.site, "template cannot transfer a whole formal beside its partial transfer"));
        }
        let ValueType::Nominal(owner) = &sig.parameters[formal].1 else { unreachable!("checked direct field of plain record") };
        for field in typed.record_fields[owner.as_ref()].iter().rev() {
            let id = crate::identity::declaration(&field.declaration, &field.site)?;
            if !holes.contains(&id) {
                if !discardable_residual(typed, &field.ty, &mut Vec::new()) {
                    return Err(LoweringError::new(&sig.site, "template field transfer needs a disposition for its surviving subtree"));
                }
                residual.push(crate::TemplateResidual { formal, field: id, ty: type_expression(&field.ty, &field.site)? });
            }
        }
    }
    Ok(residual)
}

/// Discarding a subtree performs no cleanup calls and settles no linear
/// claim. An unopened sum needs variant-specific disposal.
fn discardable_residual<'a>(typed: &'a omega_typed::TypedTrees<'_>, ty: &'a ValueType, open: &mut Vec<&'a omega_resolved::DeclarationId>) -> bool {
    match ty {
        ValueType::Array(element, _) => discardable_residual(typed, element, open),
        ValueType::Nominal(id) => {
            let id = id.as_ref();
            if open.contains(&id) || typed.case_owners.values().any(|owner| owner == id)
                || typed.cleanups.iter().any(|row| &row.owner == id)
                || typed.data_properties.iter().any(|(data, p)| data == id && p.multiplicity == omega_typed::Multiplicity::Linear) {
                return false;
            }
            let Some(fields) = typed.record_fields.get(id) else { return false };
            open.push(id);
            let disposable = fields.iter().all(|f| discardable_residual(typed, &f.ty, open));
            open.pop();
            disposable
        }
        _ => crate::lowering::types::scalar_carrier(ty).is_ok(),
    }
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
