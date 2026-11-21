//! Proof-only theorems with machine-shaped logical hypotheses.
//!
//! A hypothesis is a universal premise supplied by checked evidence, never by
//! an executable selection. Its theorem therefore publishes no machine, body,
//! call or callback entry: only erased contract and hypothesis rows. This
//! module records those statements; it proves none of them. Shapes outside the
//! supported fragment refuse instead of degrading to executable callbacks.

use super::contracts::{self, ClauseOwner};
use super::type_expression::type_expression;
use crate::lowering::applied_tree::LoweringTree;
use crate::{LoweringError, PredicateStep, TheoremBinder, TypeExpression, site_at};
use omega_prepared::PreparedTrees;
use omega_resolved::{DeclarationId, Symbol};
use omega_syntax::NodeKind;
use omega_typed::{CallUse, NodeSemantics, ObligationKind, Site, TypedSignature, TypedTree, ValueType};

/// One hypothesis of the theorem being lowered, with its syntax node and the
/// generic slot of the `machine` binder that alone supplies it.
#[derive(Clone, Copy)]
pub(crate) struct Bound<'a> {
    typed: &'a omega_typed::LogicalHypothesis,
    pub(super) node: usize,
    binder: usize,
}

impl<'a> Bound<'a> {
    pub(super) fn result(&self) -> Option<&'a ValueType> {
        (self.typed.result != ValueType::NoResult).then_some(&self.typed.result)
    }
}

pub(crate) fn lower(
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
    let body = file
        .child(node, NodeKind::Body)
        .ok_or_else(|| refuse("a logical theorem must be an ordinary resultless machine with an authored body"))?;
    if !file.nodes[body].children.is_empty() {
        return Err(refuse("a logical theorem body requires checked proof-term lowering"));
    }
    let effects = typed
        .machine_effects
        .iter()
        .find(|effects| effects.machine == sig.machine)
        .ok_or_else(|| refuse("logical theorem has no normalized effect row"))?;
    if !effects.invokes.is_empty() || !effects.reach.is_empty() || effects.fixed_pause != Default::default() || !effects.callback_pause.is_empty() {
        return Err(refuse("a logical theorem cannot reach services, pause or invoke callbacks"));
    }
    // Citations are erased proof applications. Until they are lowered as such,
    // an executable or static-argument use must not acquire a runtime target.
    if let Some(call) = typed
        .calls
        .iter()
        .find(|call| call.machine == sig.machine && call.use_site != CallUse::Fact)
    {
        return Err(LoweringError::new(
            &call.site,
            "a logical theorem citation requires erased proof-application lowering",
        ));
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

    let mut found = Vec::new();
    for &child in &file.nodes[node].children {
        match file.nodes[child].kind {
            NodeKind::GenericParams | NodeKind::Path | NodeKind::Params | NodeKind::Body | NodeKind::Requires | NodeKind::Ensures => {}
            NodeKind::Terminates if file.nodes[child].children.is_empty() => {}
            NodeKind::Hypothesis => {
                let site = site_at(tree, child);
                let hypothesis = typed
                    .logical_hypotheses
                    .iter()
                    .find(|h| h.site == site && h.owner == sig.machine && h.parent.is_none())
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
    let bounds = found
        .into_iter()
        .map(|(typed, node, binder)| {
            binder.map(|binder| Bound { typed, node, binder }).ok_or_else(|| {
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

    let plain = LoweringTree::plain(tree);
    let mut hypotheses = Vec::with_capacity(bounds.len());
    for (index, bound) in bounds.iter().enumerate() {
        let hypothesis = bound.typed;
        let mut result = None;
        let owner = ClauseOwner::Theorem(sig, bounds.as_slice(), Some(index));
        let (mut premises, mut conclusions) = (Vec::new(), Vec::new());
        for &part in &file.nodes[bound.node].children {
            let (clauses, expected) = match file.nodes[part].kind {
                NodeKind::Params => continue,
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
    })
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

/// Proof formals are identified by their binding and exact checked logical type,
/// not by whether their carrier is supported as a runtime contract operand.
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
        binder_node,
        kind: NodeKind::Param,
        ..
    }) = tree.resolved().symbol(node)
    else {
        return None;
    };
    let matches = |(name, carrier): &_| name == span && carrier == ty;
    if let Some(hypothesis) = local {
        let bound = bounds.get(hypothesis)?;
        if (bound.node..tree.resolved().file.nodes[bound.node].end).contains(binder_node) {
            let index = bound.typed.parameters.iter().position(matches)?;
            return Some(PredicateStep::HypothesisParameter { hypothesis, index });
        }
    }
    theorem.parameters.iter().position(matches).map(PredicateStep::Formal)
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
    let ClauseOwner::Theorem(theorem, bounds, _) = owner else {
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
    let names = [
        file.nodes[bound.node].binder_name().map(|name| name.span),
        file.generic_parameters(theorem.node.node)
            .nth(bound.binder)
            .map(|(name, _, _)| name.span),
    ];
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
