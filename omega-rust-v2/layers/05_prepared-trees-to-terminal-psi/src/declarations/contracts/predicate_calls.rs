use super::{ClauseOwner, LoweringTree, PredicateStep};
use crate::lowering::types::scalar_carrier;
use crate::{LoweringError, site_at};
use omega_prepared::PreparedTrees;
use omega_psi::PredicateProgress;
use omega_resolved::DeclarationId;
use omega_syntax::NodeKind;
use omega_typed::{CallUse, NodeSemantics, ObligationKind, ValueType};

/// Unlike an executable call, a fact application consumes predicate terms
/// from its enclosing clause. It neither allocates a result nor enters a body.
/// Restrict targets to plain local scalar machines with an exact body;
/// member selection, specialization and loans need additional identities.
pub(super) fn application<'a>(
    prepared: &'a PreparedTrees<'_>,
    tree: &LoweringTree<'_>,
    owner: ClauseOwner<'_>,
    node: usize,
    ty: &ValueType,
    guarantee: Option<&DeclarationId>,
    crash_guard: bool,
) -> Result<(PredicateStep, Vec<(usize, &'a ValueType)>), LoweringError> {
    let file = tree.resolved().file;
    let id = file.node_id(node);
    let site = site_at(tree, node);
    let refuse = |message| LoweringError::new(&site, message);
    let mut selected = None;
    let (mut typed, mut proved, mut total_guard) = (false, false, false);
    let mut precondition = None;
    for row in tree.semantics_at(node) {
        match row {
            NodeSemantics::Call(c) if selected.is_none() && c.node == id && c.site == site && c.use_site == CallUse::Fact => selected = Some(c),
            NodeSemantics::Expression(e) if e.node == id && e.site == site && &e.ty == ty && e.static_argument_slot.is_none() => typed = true,
            NodeSemantics::Obligation(o) if o.node == id && o.site == site => match &o.kind {
                ObligationKind::CallPrecondition(callee) if precondition.is_none() => precondition = Some(callee),
                ObligationKind::ProveGuarantee { machine, predicate } if !proved && Some(machine) == guarantee && predicate == &site => proved = true,
                ObligationKind::TotalCrashGuard if crash_guard && !total_guard => total_guard = true,
                _ => return Err(refuse("unsupported obligation on predicate application")),
            },
            _ => return Err(refuse("unsupported typed predicate application record")),
        }
    }
    let call = selected.ok_or_else(|| refuse("predicate application has no selected fact call"))?;
    let caller = match owner {
        ClauseOwner::Machine(sig) => Some(&sig.machine),
        ClauseOwner::Callable(..) | ClauseOwner::Subject(_) | ClauseOwner::Domain(..) | ClauseOwner::IndexedData(..) | ClauseOwner::Template(_) => None,
        _ => return Err(refuse("unsupported predicate application owner")),
    };
    if !typed || proved != guarantee.is_some() || total_guard != crash_guard || call.caller.as_ref() != caller || &call.result != ty {
        return Err(refuse("predicate application disagrees with its result or clause owner"));
    }
    let unsupported = || LoweringError::new(&site, format!("unsupported machine call `{}` in a Psi contract predicate", call.machine.path));
    if call.machine.package != site.package
        || call.machine.kind != NodeKind::Machine
        || call.evidence.is_some()
        || call.layout_query.is_some()
        || !call.type_arguments.is_empty()
        || !call.enclosing_arguments.is_empty()
        || call.acknowledgements != Default::default()
    {
        return Err(unsupported());
    }
    let forest = prepared.typed();
    let sig = forest
        .signatures
        .iter()
        .find(|sig| sig.machine == call.machine)
        .ok_or_else(unsupported)?;
    let target = forest.trees().nth(sig.node.file).ok_or_else(unsupported)?;
    let def = sig.node.node;
    let source = target.resolved().file;
    // These top-level, non-template definitions are emitted once by the package
    // planner, even when their only use is a fact (not an executable call).
    if !source.nodes[0].children.contains(&def)
        || target.resolved().declaration_at(def) != Some(&call.machine)
        || source.child(def, NodeKind::Body).is_none()
        || source.child(def, NodeKind::Hypothesis).is_some()
        || source.generic_parameters(def).next().is_some()
    {
        return Err(unsupported());
    }
    // L3's effect row retains an authored ceiling. For a private body without a
    // clause it is empty only when every operation and call is route-free.
    let crash_free = forest
        .machine_effects
        .iter()
        .find(|effects| effects.machine == sig.machine)
        .is_some_and(|effects| effects.crash_ceiling.as_ref().is_some_and(Vec::is_empty));
    if !crash_free {
        return Err(refuse("predicate application requires a terminating helper with an empty crash ceiling"));
    }
    let progress = if sig.terminates {
        PredicateProgress::CheckPromise
    } else if !target.resolved().is_exported(def) {
        PredicateProgress::CheckPrivateBody
    } else {
        return Err(refuse("predicate application of an exported helper requires a termination promise"));
    };
    let [callee, args] = *file.nodes[node].children.as_slice() else {
        return Err(unsupported());
    };
    if file.nodes[callee].kind != NodeKind::PathExpression
        || !file.nodes[callee].children.is_empty()
        || tree.semantics_at(callee).next().is_some()
        || file.nodes[args].kind != NodeKind::Args
        || tree.semantics_at(args).next().is_some()
    {
        return Err(unsupported());
    }
    let arguments = &file.nodes[args].children;
    if sig.result != call.result || arguments.len() != call.operands.len() || call.operands.len() != sig.parameters.len() {
        return Err(refuse("predicate application disagrees with its selected signature"));
    }
    let result = scalar_carrier(ty).map_err(|_| unsupported())?;
    let mut operands = Vec::with_capacity(arguments.len());
    let mut terms = Vec::with_capacity(arguments.len());
    for ((&node, actual), (span, formal)) in arguments.iter().zip(&call.operands).zip(&sig.parameters) {
        if actual.node != file.node_id(node)
            || actual.site != site_at(tree, node)
            || actual.parameter.package != sig.site.package
            || actual.parameter.file != sig.site.file
            || actual.parameter.span != *span
            || actual.callable.is_some()
        {
            return Err(refuse("predicate operand disagrees with its selected occurrence or formal"));
        }
        if matches!(actual.subject, ValueType::Subject { version: Some(_), .. }) {
            return Err(unsupported());
        }
        operands.push(scalar_carrier(formal).map_err(|_| unsupported())?);
        terms.push((node, formal));
    }
    // Bare scalar formals have no implicit domain requirements. These indices
    // are precisely the authored prefix of the selected machine contract.
    let count = source.nodes[def]
        .children
        .iter()
        .filter(|&&n| source.nodes[n].kind == NodeKind::Requires)
        .count();
    if precondition != sig.requires.then_some(&sig.machine) || sig.requires != (count != 0) {
        return Err(refuse("predicate application lacks its selected call precondition"));
    }
    let step = PredicateStep::MachineApplication {
        callee: crate::identity::declaration(&call.machine, &site)?,
        operands,
        result,
        requirements: 0..count,
        progress,
        site,
    };
    Ok((step, terms))
}
