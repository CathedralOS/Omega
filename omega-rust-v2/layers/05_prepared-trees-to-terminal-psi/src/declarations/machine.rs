use super::contracts;
use crate::{LoweringError, MachineContract, site_at};
use omega_prepared::PreparedTrees;
use omega_syntax::NodeKind;
use omega_typed::MachineEffects;

pub(crate) fn machine_header(
    prepared: &PreparedTrees<'_>,
    instance: &crate::lowering::applications::MachineInstance<'_>,
    effects: &MachineEffects,
    realized_pause: Option<crate::PauseEffects>,
    domains: &[super::domains::DomainSchema<'_>],
) -> Result<(MachineContract, Option<crate::LoweredOperator>), LoweringError> {
    let tree = &instance.tree;
    let sig = &instance.signature;
    let file = tree.resolved().file;
    let node = &file.nodes[sig.node.node];
    if node.has_boundary_modifier() || (node.kind!=NodeKind::AsmMachine && file.child(sig.node.node, NodeKind::Body).is_none()) {
        return Err(LoweringError::new(
            &sig.site,
            "boundary and bodyless declarations require callable declaration lowering",
        ));
    }
    let mut operator = None;
    let mut requires = Vec::new();
    let mut ensures = Vec::new();
    let mut ranking = None;
    for &child in &node.children {
        let kind = file.nodes[child].kind;
        match kind {
            NodeKind::GenericParams
            | NodeKind::Path
            | NodeKind::Params
            | NodeKind::Result
            | NodeKind::Body
            | NodeKind::Reaches
            | NodeKind::Crashes
            | NodeKind::Blocks
            | NodeKind::Suspends => {}
            NodeKind::Instruction|NodeKind::AsmMachine|NodeKind::AsmTransition|NodeKind::Registers|NodeKind::Clobbers if node.kind == NodeKind::AsmMachine=>{}
            NodeKind::OperatorToken => {
                let binding = sig
                    .operator
                    .as_ref()
                    .filter(|op| op.node == file.node_id(child))
                    .ok_or_else(|| LoweringError::new(&site_at(tree, child), "operator token has no matching typed declaration"))?;
                operator = Some(crate::LoweredOperator {
                    token: binding.token.clone(),
                    home: binding
                        .home
                        .as_ref()
                        .map(|id| crate::identity::declaration(id, &binding.site))
                        .transpose()?,
                    public: binding.public,
                    site: binding.site.clone(),
                });
            }
            NodeKind::Requires => requires.push(contracts::lower_clause(prepared, tree, contracts::ClauseOwner::Machine(sig), child)?),
            NodeKind::Ensures => ensures.push(contracts::lower_clause(prepared, tree, contracts::ClauseOwner::Machine(sig), child)?),
            NodeKind::Terminates if file.nodes[child].children.is_empty() => {}
            NodeKind::Terminates if ranking.is_none() => ranking = Some(super::ranking::authored(tree, sig, child)?),
            _ => return Err(LoweringError::new(&site_at(tree, child), format!("unsupported machine header: {kind:?}"))),
        }
    }
    if sig.operator.is_some() != operator.is_some() {
        return Err(LoweringError::new(&sig.site, "typed operator has no matching token in the machine header"));
    }
    if sig.requires == requires.is_empty()
        || sig.has_guarantee == ensures.is_empty()
        || sig.satisfies.is_some()
        || !sig.multiplicity_bounds.is_empty()
    {
        return Err(LoweringError::new(&sig.site, "machine requires contract or effects lowering"));
    }
    // The specialized envelope must exist, never block, and stay within the
    // instance's declared ceiling.
    let realized_pause = realized_pause
        .filter(|p| !p.block && !(p.suspend && sig.pause.suspend == Some(false)))
        .ok_or_else(|| LoweringError::new(&sig.site, "machine requires contract or effects lowering"))?;
    // Calls reference these clauses by index. Append the domain predicates after
    // the authored clauses, binding self to its signature slot.
    for (index, (_, ty)) in sig.parameters.iter().enumerate() {
        append_domain_predicates(&mut requires, ty, crate::PredicateStep::Formal(index), domains);
    }
    append_domain_predicates(&mut ensures, &sig.result, crate::PredicateStep::Result, domains);
    let crash_routes = super::crash_routes::lower_routes(
        prepared,
        tree,
        contracts::ClauseOwner::Machine(sig),
        sig.node.node,
        &sig.site,
        sig.crash_ceiling.as_deref(),
    )?;
    super::crash_routes::check_invocations(prepared, sig)?;
    let contract = MachineContract {
        public: tree.resolved().is_exported(sig.node.node),
        promises_termination: sig.terminates,
        reach: sig.reach.as_deref().map(|row|super::members::reach(row,|v|instance.application.instantiate(v),&sig.site)).transpose()?,
        realized_reach: super::members::reach(&effects.reach,|v|instance.application.instantiate(v),&sig.site)?,
        has_empty_crash_ceiling: sig.crash_ceiling.as_ref().is_some_and(Vec::is_empty),
        crash_routes,
        pause_bounds: sig.pause,
        realized_pause,
        requires,
        ensures,
        ranking,
    };
    Ok((contract, operator))
}

pub(super) fn append_domain_predicates(
    clauses: &mut Vec<crate::ContractClause>,
    ty: &omega_typed::ValueType,
    subject: crate::PredicateStep,
    domains: &[super::domains::DomainSchema<'_>],
) {
    for member in ty.domains() {
        // Type interning reports unsupported memberships at the declared type.
        if let Ok(index) = domains.binary_search_by(|d| d.declaration.cmp(&member.declaration)) {
            for template in domains[index].predicates().unwrap_or_default() {
                let mut clause = template.clone();
                for step in &mut clause.predicate {
                    if *step == crate::PredicateStep::SelfValue {
                        *step = subject.clone();
                    }
                }
                clauses.push(clause);
            }
        }
    }
}
