//! Proof-only theorems with machine-shaped logical hypotheses.
//!
//! A hypothesis is a universal premise supplied by checked evidence, never by
//! an executable selection. Its theorem therefore publishes no machine, body,
//! call or callback entry: only erased contract, hypothesis and proof-step
//! rows. This module records those statements; it proves none of them. Shapes
//! outside the supported fragment refuse instead of degrading to executable
//! callbacks or runtime work.

use super::contracts::{self, ClauseOwner};
use super::type_expression::type_expression;
use crate::lowering::applied_tree::LoweringTree;
use crate::{LoweringError, PredicateStep, ProofInput, ProofStep, ProofSubject, TheoremBinder, TypeExpression, site_at};
use omega_prepared::PreparedTrees;
use omega_resolved::{DeclarationId, Symbol};
use omega_syntax::NodeKind;
use omega_typed::{CallUse, NodeSemantics, ObligationKind, Site, TypedSignature, TypedTree, ValueType};
use std::collections::HashMap;

/// The site that introduces the evidence for a universal premise.
#[derive(Clone, Copy)]
enum Introduction {
    TheoremBinder(usize),
    EnclosingHypothesis(usize),
}

/// A checked hypothesis and the location of its declaration within this theorem.
#[derive(Clone, Copy)]
pub(crate) struct Bound<'a> {
    typed: &'a omega_typed::LogicalHypothesis,
    pub(super) node: usize,
    introduction: Introduction,
}

impl<'a> Bound<'a> {
    fn parent(self) -> Option<usize> {
        match self.introduction {
            Introduction::TheoremBinder(_) => None,
            Introduction::EnclosingHypothesis(parent) => Some(parent),
        }
    }

    pub(super) fn result(&self) -> Option<&'a ValueType> {
        (self.typed.result != ValueType::NoResult).then_some(&self.typed.result)
    }
}

pub(crate) fn header(
    prepared: &PreparedTrees<'_>,
    tree: &TypedTree<'_>,
    sig: &TypedSignature,
) -> Result<crate::LogicalTheorem, LoweringError> {
    let typed = prepared.typed();
    let file = tree.resolved().file;
    let node = sig.node.node;
    let refuse = |reason: &str| LoweringError::new(&sig.site, reason);
    if file.nodes[node].kind != NodeKind::Machine
        || file.nodes[node].has_boundary_modifier()
        || sig.operator.is_some()
        || sig.satisfies.is_some()
        || !sig.multiplicity_bounds.is_empty()
        || sig.result != ValueType::NoResult
    {
        return Err(refuse("a logical theorem must be an ordinary resultless machine with an authored body"));
    }
    file.child(node, NodeKind::Body)
        .ok_or_else(|| refuse("a logical theorem must be an ordinary resultless machine with an authored body"))?;
    let effects = typed
        .machine_effects
        .iter()
        .find(|effects| effects.machine == sig.machine)
        .ok_or_else(|| refuse("logical theorem has no normalized effect row"))?;
    if !effects.invokes.is_empty() || !effects.reach.is_empty() || effects.fixed_pause != Default::default() || !effects.callback_pause.is_empty() {
        return Err(refuse("a logical theorem cannot reach services, pause or invoke callbacks"));
    }
    // A body's citation lowers to an erased `ApplyTheorem`. A static argument
    // would select the theorem as a runtime target, which it never has.
    if let Some(call) = typed
        .calls
        .iter()
        .find(|call| call.machine == sig.machine && call.use_site == CallUse::StaticArgument)
    {
        return Err(LoweringError::new(&call.site, "a logical theorem cannot be a static machine argument"));
    }
    if let Some(selection) = typed
        .obligations
        .iter()
        .find(|o| matches!(&o.kind, ObligationKind::RefineCallable { selection, .. } if selection.machine.as_ref() == &sig.machine))
    {
        return Err(LoweringError::new(
            &selection.site,
            "a logical theorem cannot be selected as an executable callable",
        ));
    }

    let mut records = HashMap::new();
    for record in typed.logical_hypotheses.iter().filter(|record| record.owner == sig.machine) {
        if records.insert(&record.site, record).is_some() {
            return Err(LoweringError::new(&record.site, "duplicate checked hypothesis at the same occurrence"));
        }
    }
    let mut found = Vec::new();
    for &child in &file.nodes[node].children {
        match file.nodes[child].kind {
            NodeKind::GenericParams | NodeKind::Path | NodeKind::Params | NodeKind::Body | NodeKind::Requires | NodeKind::Ensures => {}
            NodeKind::Terminates if file.nodes[child].children.is_empty() => {}
            NodeKind::Hypothesis => {
                let site = site_at(tree, child);
                let hypothesis = records.remove(&site).filter(|row| row.parent.is_none())
                    .ok_or_else(|| LoweringError::new(&site, "logical hypothesis has no checked theorem record"))?;
                found.push((hypothesis, child, None));
            }
            kind => {
                return Err(LoweringError::new(
                    &site_at(tree, child),
                    format!("unsupported logical theorem clause: {kind:?}"),
                ));
            }
        }
    }

    let checked = typed.generic_parameters.get(&sig.machine);
    let telescope = checked.map_or(&[][..], |c| c.parameters.as_slice());
    if checked.is_some_and(|c| c.witness_count != 0 || !c.equations.is_empty()) || file.generic_parameters(node).count() != telescope.len() {
        return Err(refuse("logical theorem telescope has unsupported witnesses or equations"));
    }
    let mut binders = Vec::with_capacity(telescope.len());
    for (slot, ((_, _, kind), parameter)) in file.generic_parameters(node).zip(telescope).enumerate() {
        let binder = match (kind, parameter.kind, parameter.carrier.as_deref(), parameter.multiplicity) {
            (NodeKind::MachineParam, NodeKind::MachineParam, Some(ValueType::LogicalRequirement(site)), None) => {
                let index = found
                    .iter()
                    .position(|(h, _, _)| &h.site == site)
                    .ok_or_else(|| refuse("machine binder names no hypothesis of this theorem"))?;
                if found[index].2.replace(slot).is_some() {
                    return Err(refuse("duplicate logical requirement for binder"));
                }
                TheoremBinder::Hypothesis(index)
            }
            (NodeKind::KindedParam, NodeKind::KindedParam, Some(bound), None) => TheoremBinder::Mathematical(type_expression(bound, &sig.site)?),
            _ => return Err(refuse("unsupported logical theorem binder")),
        };
        binders.push(binder);
    }
    let mut bounds = found
        .into_iter()
        .map(|(typed, node, binder)| {
            binder.map(|slot| Bound {
                typed,
                node,
                introduction: Introduction::TheoremBinder(slot),
            }).ok_or_else(|| {
                let mut site = typed.site.clone();
                if let Some(name) = file.nodes[node].binder_name() {
                    site.span = name.span;
                }
                LoweringError::new(
                    &site,
                    format!("lowering has no matching `machine {0}` binder for logical hypothesis `{0}`", typed.name),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Append children in source order. The existing root slots remain stable,
    // and every parent precedes its children at arbitrary nesting depth.
    let mut parent = 0;
    while let Some(bound) = bounds.get(parent).copied() {
        for &child in &file.nodes[bound.node].children {
            if file.nodes[child].kind == NodeKind::Hypothesis {
                let site = site_at(tree, child);
                let record = records
                    .remove(&site)
                    .filter(|row| row.parent.as_ref() == Some(&bound.typed.site))
                    .ok_or_else(|| LoweringError::new(&site, "nested hypothesis disagrees with its checked parent"))?;
                bounds.push(Bound {
                    typed: record,
                    node: child,
                    introduction: Introduction::EnclosingHypothesis(parent),
                });
            }
        }
        parent += 1;
    }
    if !records.is_empty() {
        return Err(refuse("checked hypothesis has no declaration in its theorem"));
    }

    let plain = LoweringTree::plain(tree);
    let mut hypotheses = Vec::with_capacity(bounds.len());
    for (index, bound) in bounds.iter().enumerate() {
        let hypothesis = bound.typed;
        let mut result = None;
        let owner = ClauseOwner::Theorem(sig, bounds.as_slice(), Some(index));
        let (mut premises, mut conclusions) = (Vec::new(), Vec::new());
        for &part in &file.nodes[bound.node].children {
            let (clauses, expected) = match file.nodes[part].kind {
                NodeKind::Params | NodeKind::Hypothesis => continue,
                NodeKind::Result if result.is_none() && hypothesis.result != ValueType::NoResult => {
                    result = Some(logical_type(&hypothesis.result, &site_at(tree, part))?);
                    continue;
                }
                NodeKind::Requires => (&mut premises, &hypothesis.premises),
                NodeKind::Ensures => (&mut conclusions, &hypothesis.conclusions),
                kind => {
                    return Err(LoweringError::new(
                        &site_at(tree, part),
                        format!("unsupported logical hypothesis clause: {kind:?}"),
                    ));
                }
            };
            if !matches!(file.nodes[part].children.as_slice(), [root] if expected.get(clauses.len()) == Some(&site_at(tree, *root))) {
                return Err(LoweringError::new(
                    &site_at(tree, part),
                    "logical hypothesis clause disagrees with its checked record",
                ));
            }
            clauses.push(contracts::lower_clause(prepared, &plain, owner, part)?);
        }
        if premises.len() != hypothesis.premises.len() || conclusions.len() != hypothesis.conclusions.len() {
            return Err(LoweringError::new(
                &hypothesis.site,
                "logical hypothesis clauses disagree with its checked record",
            ));
        }
        if result.is_none() && hypothesis.result != ValueType::NoResult {
            return Err(LoweringError::new(&hypothesis.site, "logical hypothesis lacks its checked result annotation"));
        }
        let parameters = hypothesis
            .parameters
            .iter()
            .map(|(span, ty)| {
                let mut site = hypothesis.site.clone();
                site.span = *span;
                logical_type(ty, &site)
            })
            .collect::<Result<_, _>>()?;
        hypotheses.push(crate::LogicalHypothesis {
            parent: bound.parent(),
            site: hypothesis.site.clone(),
            parameters,
            result,
            premises,
            conclusions,
        });
    }

    let owner = ClauseOwner::Theorem(sig, bounds.as_slice(), None);
    let (mut requires, mut ensures) = (Vec::new(), Vec::new());
    for &child in &file.nodes[node].children {
        match file.nodes[child].kind {
            NodeKind::Requires => requires.push(contracts::lower_clause(prepared, &plain, owner, child)?),
            NodeKind::Ensures => ensures.push(contracts::lower_clause(prepared, &plain, owner, child)?),
            _ => {}
        }
    }
    if sig.requires != (!requires.is_empty() || !bounds.is_empty()) || sig.has_guarantee == ensures.is_empty() {
        return Err(refuse("logical theorem clauses disagree with its checked signature"));
    }
    let parameters = sig
        .parameters
        .iter()
        .map(|(span, ty)| {
            let mut site = sig.site.clone();
            site.span = *span;
            logical_type(ty, &site)
        })
        .collect::<Result<_, _>>()?;
    Ok(crate::LogicalTheorem {
        declaration: crate::identity::declaration(&sig.machine, &sig.site)?,
        public: tree.resolved().is_exported(node),
        site: sig.site.clone(),
        binders,
        parameters,
        hypotheses,
        requires,
        ensures,
        proof: Vec::new(),
    })
}

/// The theorem body as erased proof steps, in authored order. A `let` names
/// an erased subject and a statement call cites a checked local machine. No
/// step becomes an operation: the theorem has no executable body, so nothing
/// here may evaluate a value, enter a helper or require a runtime carrier.
pub(crate) fn proof_steps(
    prepared: &PreparedTrees<'_>,
    tree: &TypedTree<'_>,
    sig: &TypedSignature,
    callees: &crate::machines::theorem_citations::TheoremCallees<'_>,
) -> Result<Vec<ProofStep>, LoweringError> {
    let file = tree.resolved().file;
    let tree = &LoweringTree::plain(tree);
    let body = file.child(sig.node.node, NodeKind::Body).ok_or_else(|| LoweringError::new(&sig.site, "logical theorem has no body"))?;
    let resolved = tree.resolved();
    let typed = prepared.typed();
    // Each local's binder node, its step position and its checked carrier.
    let mut locals: Vec<(usize, usize, &ValueType)> = Vec::new();
    let mut steps = Vec::with_capacity(file.nodes[body].children.len());
    for &statement in &file.nodes[body].children {
        let site = site_at(tree, statement);
        let refuse = |reason: &str| LoweringError::new(&site, reason);
        match (file.nodes[statement].kind, file.nodes[statement].children.as_slice()) {
            (NodeKind::Let, [binder, initializer] | [binder, _, initializer])
                if file.nodes[*binder].kind == NodeKind::BindingPattern && resolved.pattern_binding(*binder).is_some() =>
            {
                // A zero-initialized `let x: T;` has no subject, and a mutable
                // binding would name a changing value.
                if tree.semantics_at(statement).next().is_some() || typed.mutable_bindings.contains(&file.node_id(*binder)) {
                    return Err(refuse("a proof binding must name one immutable subject"));
                }
                let carrier = tree
                    .semantics_at(*binder)
                    .find_map(|row| match row {
                        NodeSemantics::Expression(e) if e.static_argument_slot.is_none() => Some(&e.ty),
                        _ => None,
                    })
                    .ok_or_else(|| refuse("a proof binding has no checked carrier"))?;
                let subject = proof_subject(prepared, tree, sig, &locals, *initializer, carrier)?;
                locals.push((*binder, steps.len(), carrier));
                steps.push(ProofStep::Bind { subject, site });
            }
            (NodeKind::ExpressionStatement, [call]) if file.nodes[*call].kind == NodeKind::Call => {
                if let Some(citation) = crate::machines::theorem_citations::select(prepared, tree, sig, *call, &ValueType::NoResult, callees, |node, ty| {
                    proof_input(tree, sig, &locals, node, ty)
                })? {
                    steps.push(ProofStep::ApplyTheorem {
                        theorem: citation.theorem,
                        arguments: citation.arguments,
                        subjects: citation.subjects,
                        requirements: 0..citation.requirement_count,
                        site: site_at(tree, *call),
                    });
                } else {
                    steps.push(cite_machine(prepared, tree, sig, &locals, *call)?);
                }
            }
            _ => return Err(refuse("unsupported proof step in a logical theorem body")),
        }
    }
    Ok(steps)
}

/// `lemma(x);` in a theorem body cites `lemma` as proof. L3 typed it as an
/// ordinary statement call with the callee's precondition, so this checks
/// that record and the helper's identity, then keeps the subjects erased.
fn cite_machine(
    prepared: &PreparedTrees<'_>,
    tree: &LoweringTree<'_>,
    sig: &TypedSignature,
    locals: &[(usize, usize, &ValueType)],
    node: usize,
) -> Result<ProofStep, LoweringError> {
    let file = tree.resolved().file;
    let id = file.node_id(node);
    let site = site_at(tree, node);
    let refuse = |reason: &str| LoweringError::new(&site, reason);
    let (mut selected, mut statement, mut precondition, mut crash) = (None, false, None, None);
    for row in tree.semantics_at(node) {
        match row {
            NodeSemantics::Call(call) if selected.is_none() && call.node == id && call.site == site => selected = Some(call),
            NodeSemantics::Expression(e) if !statement && e.static_argument_slot.is_none() && e.ty == ValueType::NoResult => statement = true,
            NodeSemantics::Obligation(o) if o.node == id && o.site == site => match &o.kind {
                ObligationKind::CallPrecondition(machine) if precondition.is_none() => precondition = Some(machine),
                // A crash route of the cited helper; the helper check below refuses it.
                kind @ ObligationKind::CrashCoverage { .. } => crash = Some((kind, &o.site)),
                kind => return Err(LoweringError::new(&o.site, format!("unsupported proof citation obligation: {kind:?}"))),
            },
            _ => return Err(refuse("unsupported typed record on a proof citation")),
        }
    }
    let call = selected.ok_or_else(|| refuse("a proof statement must cite a checked machine"))?;
    if !statement || call.use_site != CallUse::Execution || call.caller.as_ref() != Some(&sig.machine) || call.result != ValueType::NoResult {
        return Err(refuse("a proof citation is a resultless statement of this theorem"));
    }
    let (helper, progress, count) =
        super::contracts::predicate_calls::checked_helper(prepared, call, precondition, &site, super::contracts::predicate_calls::HelperUse::Proof)?;
    if let Some((kind, site)) = crash {
        return Err(LoweringError::new(site, format!("unsupported proof citation obligation: {kind:?}")));
    }
    let [_, args] = *file.nodes[node].children.as_slice() else {
        return Err(refuse("a proof citation has no callee and subjects"));
    };
    let arguments = &file.nodes[args].children;
    if file.nodes[args].kind != NodeKind::Args || arguments.len() != call.operands.len() || call.operands.len() != helper.parameters.len() {
        return Err(refuse("a proof citation disagrees with its selected signature"));
    }
    let mut subjects = Vec::with_capacity(arguments.len());
    for ((&argument, applied), (span, formal)) in arguments.iter().zip(&call.operands).zip(&helper.parameters) {
        if applied.node != file.node_id(argument)
            || applied.site != site_at(tree, argument)
            || applied.parameter.package != helper.site.package
            || applied.parameter.file != helper.site.file
            || applied.parameter.span != *span
            || applied.callable.is_some()
        {
            return Err(LoweringError::new(
                &applied.site,
                "a proof citation subject disagrees with its occurrence or formal",
            ));
        }
        if crate::lowering::types::scalar_carrier(formal).is_err() {
            return Err(LoweringError::new(&applied.site, "a proof citation subject needs a scalar carrier"));
        }
        subjects.push(proof_subject(prepared, tree, sig, locals, argument, formal)?);
    }
    Ok(ProofStep::CiteMachine {
        machine: crate::identity::declaration(&call.machine, &site)?,
        subjects,
        requirements: 0..count,
        progress,
        site,
    })
}

/// Lower an erased subject using the theorem formals and earlier proof bindings.
fn proof_subject(
    prepared: &PreparedTrees<'_>,
    tree: &LoweringTree<'_>,
    sig: &TypedSignature,
    locals: &[(usize, usize, &ValueType)],
    node: usize,
    carrier: &ValueType,
) -> Result<ProofSubject<ProofInput>, LoweringError> {
    crate::machines::theorem_citations::erased_subject(prepared, tree, &sig.machine, node, carrier, |node, ty| {
        proof_input(tree, sig, locals, node, ty)
    })
}

fn proof_input(tree: &LoweringTree<'_>, sig: &TypedSignature, locals: &[(usize, usize, &ValueType)], node: usize, ty: &ValueType) -> Option<ProofInput> {
        let Symbol::Binding { span, binder_node, kind, .. } = tree.resolved().symbol(node)? else { return None };
        match kind {
            NodeKind::Param => sig.parameters.iter().position(|(name, formal)| name == span && formal == ty).map(ProofInput::Formal),
            NodeKind::BindingPattern => locals.iter().rev().find(|(binder, _, formal)| binder == binder_node && formal == &ty).map(|(_, step, _)| ProofInput::Local(*step)),
            _ => None,
        }
}

/// Borrowed proof inputs and domain-qualified subjects carry read and domain
/// facts this row does not yet publish, so they refuse here.
fn logical_type(
    ty: &ValueType,
    site: &Site,
) -> Result<TypeExpression, LoweringError> {
    if matches!(ty.carrier(), ValueType::Reference(..) | ValueType::AtLifetime(..)) || !ty.domains().is_empty() {
        return Err(LoweringError::new(site, "a logical subject requires borrowed or domain proof-input lowering"));
    }
    type_expression(ty, site)
}

/// A hypothesis conclusion is assumed evidence, not a guarantee this theorem proves.
pub(super) fn proves_guarantee(owner: ClauseOwner<'_>) -> bool {
    !matches!(owner, ClauseOwner::Theorem(_, _, Some(_)))
}

/// A theorem clause may state a strict proposition rather than a Boolean.
pub(super) fn strict_fact(
    owner: ClauseOwner<'_>,
    tree: &LoweringTree<'_>,
    node: usize,
) -> bool {
    if !matches!(owner, ClauseOwner::Theorem(..)) {
        return false;
    }
    let sort = tree.semantics_at(node).find_map(|row| match row {
        NodeSemantics::Obligation(o) => match &o.kind {
            ObligationKind::MathematicalFact {
                sort: sort @ ValueType::Universe { strict: true, .. },
            } => Some(sort),
            _ => None,
        },
        _ => None,
    });
    sort.is_some_and(|sort| {
        tree.semantics_at(node)
            .any(|row| matches!(row, NodeSemantics::Expression(e) if &e.ty == sort && e.static_argument_slot.is_none()))
    })
}

/// Walk outward from the clause's directly enclosing hypothesis.
fn lexical_scope<'a>(bounds: &'a [Bound<'_>], slot: Option<usize>) -> impl Iterator<Item = usize> + 'a {
    std::iter::successors(slot, |index| bounds[*index].parent())
}

/// Resolve a formal by its binding occurrence, including enclosing hypotheses.
pub(super) fn scoped_parameter<'a>(
    bounds: &[Bound<'a>],
    slot: Option<usize>,
    span: &crate::SourceSpan,
) -> Option<(usize, usize, &'a ValueType)> {
    lexical_scope(bounds, slot).find_map(|hypothesis| {
        bounds[hypothesis]
            .typed
            .parameters
            .iter()
            .enumerate()
            .find(|(_, (name, _))| name == span)
            .map(|(index, (_, carrier))| (hypothesis, index, carrier))
    })
}

/// Proof formals are identified by their binding and exact checked logical type.
pub(super) fn parameter(
    owner: ClauseOwner<'_>,
    tree: &LoweringTree<'_>,
    node: usize,
    ty: &ValueType,
) -> Option<PredicateStep> {
    let ClauseOwner::Theorem(theorem, bounds, local) = owner else {
        return None;
    };
    let Some(Symbol::Binding {
        span,
        kind: NodeKind::Param,
        ..
    }) = tree.resolved().symbol(node)
    else {
        return None;
    };
    if let Some((hypothesis, index, carrier)) = scoped_parameter(bounds, local, span) {
        return (carrier == ty).then_some(PredicateStep::HypothesisParameter { hypothesis, index });
    }
    theorem.parameters.iter().position(|(name, carrier)| name == span && carrier == ty).map(PredicateStep::Formal)
}

/// `Step::ensures(subjects)` applies the hypothesis's conclusions at exact
/// subjects. It returns the step and the operands with their declared carriers.
pub(super) fn guarantee<'o>(
    tree: &LoweringTree<'_>,
    owner: ClauseOwner<'o>,
    node: usize,
    ty: &ValueType,
    proved: Option<&DeclarationId>,
) -> Result<Option<(PredicateStep, Vec<(usize, &'o ValueType)>)>, LoweringError> {
    let Some((requirement, operands)) = tree.semantics_at(node).find_map(|row| match row {
        NodeSemantics::Obligation(o) => match &o.kind {
            ObligationKind::ApplyHypothesis { requirement, operands } => Some((requirement, operands)),
            _ => None,
        },
        _ => None,
    }) else {
        return Ok(None);
    };
    let site = site_at(tree, node);
    let refuse = |reason: &str| LoweringError::new(&site, reason);
    let ClauseOwner::Theorem(theorem, bounds, local) = owner else {
        return Err(refuse("a logical guarantee projection requires its theorem's hypotheses"));
    };
    let hypothesis = bounds
        .iter()
        .position(|b| &b.typed.site == requirement)
        .ok_or_else(|| refuse("logical guarantee projection names no hypothesis of this theorem"))?;
    let bound: &'o Bound<'o> = &bounds[hypothesis];
    let file = tree.resolved().file;
    let [callee, args] = *file.nodes[node].children.as_slice() else {
        return Err(refuse("logical guarantee projection has no callee and subjects"));
    };
    let external_binder = match bound.introduction {
        Introduction::TheoremBinder(slot) => file.generic_parameters(theorem.node.node).nth(slot).map(|(name, _, _)| name.span),
        Introduction::EnclosingHypothesis(parent) if lexical_scope(bounds, local).any(|scope| scope == parent) => None,
        Introduction::EnclosingHypothesis(_) => return Err(refuse("logical guarantee projection escapes its enclosing hypothesis")),
    };
    let names = [file.nodes[bound.node].binder_name().map(|name| name.span), external_binder];
    if !matches!(tree.resolved().symbol(callee), Some(Symbol::StaticMember { binder, member }) if member == "ensures" && names.contains(&Some(*binder))) {
        return Err(refuse("logical guarantee projection disagrees with its hypothesis binder"));
    }
    let (mut boolean, mut applied, mut obligation) = (false, false, false);
    for row in tree.semantics_at(node) {
        match row {
            NodeSemantics::Expression(e) if !boolean && e.ty == ValueType::Bool && &e.ty == ty && e.static_argument_slot.is_none() => boolean = true,
            NodeSemantics::Obligation(o) => match &o.kind {
                ObligationKind::ApplyHypothesis { .. } if !applied => applied = true,
                ObligationKind::ProveGuarantee { machine, predicate } if !obligation && Some(machine) == proved && predicate == &site => obligation = true,
                _ => return Err(refuse("unsupported obligation on a logical guarantee projection")),
            },
            _ => return Err(refuse("unsupported record on a logical guarantee projection")),
        }
    }
    if !boolean || obligation != proved.is_some() {
        return Err(refuse("logical guarantee projection lacks its Boolean or proof obligation record"));
    }
    let subjects = &file.nodes[args].children;
    // The checked relation takes its inputs in order, then the mathematical
    // result subject if present. None of these operands is a runtime return.
    let result = bound.result();
    if file.nodes[args].kind != NodeKind::Args
        || subjects.len() != operands.len()
        || subjects.len() != bound.typed.parameters.len() + usize::from(result.is_some())
        || subjects
            .iter()
            .zip(operands)
            .any(|(&subject, expected)| &site_at(tree, subject) != expected)
    {
        return Err(refuse("logical guarantee projection lacks its exact subjects"));
    }
    let carriers = subjects
        .iter()
        .copied()
        .zip(bound.typed.parameters.iter().map(|(_, ty)| ty).chain(result))
        .collect();
    Ok(Some((
        PredicateStep::HypothesisGuarantee {
            hypothesis,
            operand_count: subjects.len(),
        },
        carriers,
    )))
}
