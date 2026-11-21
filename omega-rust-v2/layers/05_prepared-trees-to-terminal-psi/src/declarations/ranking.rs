//! Authored `terminates by` witnesses and their in-component edge claims.
//!
//! The header records the exact view, relation, entry rank and bounds. After
//! every body exists, the component pass reconstructs the actual call graph,
//! checks that each claim names a real input and edge, and attaches one claim
//! to every internal edge. It proves no comparison, bound or well-foundedness;
//! those remain checker obligations. So does a termination promise on a cycle
//! that a member's missing `terminates by` leaves without a joint ranking:
//! lowering publishes that component and leaves the verdict to the checker.
//! Unsupported shapes refuse.
use crate::lowering::applied_tree::LoweringTree;
use crate::{
    ArithmeticPolicy, BlockTerminator, CallArgument, IntegerPrimitive, LoweredPackage, LoweringError, MachineBody, MachineId, MachineRanking, OperationKind, Parameter, RankBounds,
    RankComparison, RankExpression, RankOperand, RankOrder, RankRelation, RankedEdge, ScalarType, Site, TypeShape, ValueOrigin, site_at,
};
mod calls;
mod comparisons;
use calls::{Invocation, RankedTransfer};
use num_bigint::BigInt;
use omega_prepared::{PreparedTrees, PreparedValue};
use omega_resolved::Symbol;
use omega_syntax::{NodeKind, TerminationClause};
use omega_typed::{NodeSemantics, ObligationKind, TypedSignature, ValueType};

const VIEW_PACKAGE: &str = "@omega/toolchain";
const DESCENDING: &str = "language\\core\\Nat\\Descending";
const BOUNDED_DISTANCE: &str = "language\\core\\Nat\\BoundedDistance";

#[derive(Clone, Copy)]
enum View {
    Descending,
    BoundedDistance,
}

/// Lower one authored `terminates by` clause. Edge claims are attached by
/// [`bind_component_edges`] once the bodies exist.
pub(crate) fn authored(
    prepared: &PreparedTrees<'_>,
    tree: &LoweringTree<'_>,
    sig: &TypedSignature,
    clause: usize,
) -> Result<MachineRanking, LoweringError> {
    let resolved = tree.resolved();
    let file = resolved.file;
    let site = site_at(tree, clause);
    let refuse = |node: usize, reason: &str| LoweringError::new(&site_at(tree, node), reason);
    let Some(TerminationClause::By { measure, order, bound }) = file.nodes[clause].termination_clause() else {
        return Err(refuse(clause, "termination clause has no ranking witness form"));
    };
    let mut witness = None;
    let mut obligation = false;
    for record in tree.semantics_at(clause) {
        match record {
            NodeSemantics::RankingWitness(w) if witness.is_none() && w.machine == sig.machine && w.node == file.node_id(clause) => witness = Some(w),
            NodeSemantics::Obligation(o) if !obligation && matches!(&o.kind, ObligationKind::Termination(machine) if *machine == sig.machine) => {
                obligation = true
            }
            _ => return Err(refuse(clause, "unsupported typed termination record")),
        }
    }
    let witness = witness
        .filter(|_| obligation)
        .ok_or_else(|| refuse(clause, "termination clause lacks its typed ranking witness"))?;
    let view = match (witness.view.package.as_str(), witness.view.path.as_str(), witness.view.kind) {
        (VIEW_PACKAGE, DESCENDING, NodeKind::Machine) => View::Descending,
        (VIEW_PACKAGE, BOUNDED_DISTANCE, NodeKind::Machine) => View::BoundedDistance,
        _ => return Err(refuse(clause, "ranking view has no supported Psi normalization")),
    };
    // An authored view must name the exact selected declaration, with no arguments.
    if let Some(order) = order {
        let head = file.ungroup(order);
        if !matches!(resolved.symbol(head), Some(Symbol::Declaration(id)) if *id == witness.view) {
            return Err(refuse(order, "ranking view must name its exact selected declaration"));
        }
    }
    let carrier = match &witness.rank {
        ValueType::Integer {
            signed: false,
            width: Some(width),
        } => ScalarType::Integer { signed: false, width: *width },
        _ => return Err(refuse(clause, "ranking view has no unsigned natural carrier")),
    };
    // An integer literal in the rank carrier: a bound endpoint or a constant
    // rank operand, named by `what` in refusals.
    let literal = |node: usize, what: &str| -> Result<BigInt, LoweringError> {
        let node = file.ungroup(node);
        if file.nodes[node].kind != NodeKind::Literal {
            return Err(refuse(node, &format!("{what} must be an integer literal")));
        }
        let mut value: Option<&BigInt> = None;
        for record in tree.semantics_at(node) {
            let candidate = match record {
                NodeSemantics::Landing(l) if l.ty == witness.rank && l.value.is_integer() => Some(l.value.numer()),
                NodeSemantics::Expression(e) if e.static_argument_slot.is_none() => match &e.ty {
                    ValueType::Exact {
                        value,
                        fractional_origin: None,
                    } if value.is_integer() => Some(value.numer()),
                    ty if ty == &witness.rank => None,
                    _ => return Err(refuse(node, &format!("{what} has a foreign carrier"))),
                },
                _ => return Err(refuse(node, &format!("unsupported typed {what} record"))),
            };
            if let Some(candidate) = candidate {
                if value.is_some_and(|previous| previous != candidate) {
                    return Err(refuse(node, &format!("{what} has conflicting values")));
                }
                value = Some(candidate);
            }
        }
        value
            .filter(|v| witness.rank.contains_integer(v))
            .cloned()
            .ok_or_else(|| refuse(node, &format!("{what} has no value in the rank carrier")))
    };
    let endpoint = |node: usize| literal(node, "rank bound endpoint");
    // A rank operand is an immutable formal of the carrier or a closed
    // integer constant. A named `const` contributes its prepared value, the
    // same value its other uses read; the declaration adds no runtime input.
    let operand = |node: usize| -> Result<RankOperand<usize>, LoweringError> {
        let node = file.ungroup(node);
        let symbol = resolved.symbol(node);
        if file.nodes[node].kind == NodeKind::Literal {
            return Ok(RankOperand::Constant(literal(node, "rank literal")?));
        }
        if let Some(Symbol::Declaration(id)) = symbol
            && id.kind == NodeKind::Const
        {
            let (mut typed, mut named) = (false, false);
            for record in tree.semantics_at(node) {
                match record {
                    NodeSemantics::Expression(e) if !typed && e.static_argument_slot.is_none() && e.ty == witness.rank => typed = true,
                    NodeSemantics::Constant(constant) if !named && constant == id => named = true,
                    _ => return Err(refuse(node, "unsupported typed ranking subject record")),
                }
            }
            if !(typed && named) {
                return Err(refuse(node, "ranking constant has no typed carrier"));
            }
            return match prepared.constant(id) {
                Some(PreparedValue::Integer(value)) if witness.rank.contains_integer(value) => Ok(RankOperand::Constant(value.clone())),
                _ => Err(refuse(node, "ranking constant has no prepared value in the rank carrier")),
            };
        }
        let Some(Symbol::Binding {
            span, kind: NodeKind::Param, ..
        }) = symbol
        else {
            return Err(refuse(node, "lowering only represents rank operands that name machine formals or integer constants"));
        };
        let index = sig
            .parameters
            .iter()
            .position(|(candidate, ty)| candidate == span && ty == &witness.rank)
            .ok_or_else(|| refuse(node, "ranking subject must be an unqualified formal of the rank carrier"))?;
        let mut typed = false;
        for record in tree.semantics_at(node) {
            match record {
                NodeSemantics::Expression(e) if e.static_argument_slot.is_none() && e.ty == witness.rank => typed = true,
                _ => return Err(refuse(node, "unsupported typed ranking subject record")),
            }
        }
        if !typed {
            return Err(refuse(node, "ranking subject has no typed carrier"));
        }
        Ok(RankOperand::Input(index))
    };
    // Terminal Psi also admits one exact subtraction of two such operands,
    // such as `MAX - index`. Keep the authored operand order. The typed
    // operation and its formation question must be the ones any exact integer
    // subtraction carries; the checker proves that question at entry and on
    // each successor. A wrapping, saturating or nested subject has no form.
    let difference = |node: usize, minuend: usize, subtrahend: usize| -> Result<RankExpression<usize>, LoweringError> {
        let computed = || {
            refuse(
                node,
                "lowering only represents a computed rank that is one exact subtraction of formals or integer constants",
            )
        };
        let (mut typed, mut operation, mut contract) = (false, None, None);
        for record in tree.semantics_at(node) {
            match record {
                NodeSemantics::Expression(e) if !typed && e.static_argument_slot.is_none() && e.ty == witness.rank => typed = true,
                NodeSemantics::Operation(op) if operation.is_none() => operation = Some(op),
                NodeSemantics::Obligation(o) if contract.is_none() && matches!(o.kind, ObligationKind::OperationContract { .. }) => contract = Some(o),
                _ => return Err(refuse(node, "unsupported typed ranking subject record")),
            }
        }
        let (true, Some(operation), Some(contract)) = (typed, operation, contract) else {
            return Err(refuse(node, "ranking subtraction has no typed carrier"));
        };
        if operation.token != "-" {
            return Err(computed());
        }
        if operation.result != witness.rank || operation.operands.iter().any(|ty| ty != &witness.rank) {
            return Err(refuse(node, "ranking subtraction has a foreign carrier"));
        }
        let lowered = crate::machines::arithmetic::index_operation(operation, contract, &sig.machine)?;
        if lowered.primitive != IntegerPrimitive::Subtract || lowered.policy != ArithmeticPolicy::Exact {
            return Err(computed());
        }
        if [minuend, subtrahend]
            .into_iter()
            .any(|child| matches!(file.nodes[file.ungroup(child)].kind, NodeKind::Binary | NodeKind::Unary))
        {
            return Err(computed());
        }
        Ok(RankExpression::ExactDifference {
            minuend: operand(minuend)?,
            subtrahend: operand(subtrahend)?,
        })
    };
    let subject = file.ungroup(measure);
    let rank = match (view, file.nodes[subject].kind, file.nodes[subject].children.as_slice()) {
        (View::Descending, NodeKind::Binary, &[minuend, subtrahend]) => difference(subject, minuend, subtrahend)?,
        (View::Descending, kind, _) if kind != NodeKind::Tuple => RankExpression::Operand(operand(subject)?),
        (View::BoundedDistance, NodeKind::Tuple, &[cursor, limit]) => {
            if tree
                .semantics_at(subject)
                .any(|record| !matches!(record, NodeSemantics::Expression(_)))
            {
                return Err(refuse(subject, "unsupported typed ranking subject record"));
            }
            RankExpression::SaturatingDistance {
                cursor: operand(cursor)?,
                limit: operand(limit)?,
            }
        }
        _ => return Err(refuse(subject, "ranking subject does not match its view")),
    };
    let bounds = match bound {
        None => None,
        Some(bound) => {
            let range = file
                .range_bounds(bound)
                .ok_or_else(|| refuse(bound, "rank bound requires a range"))?;
            let (Some(lower), Some(upper)) = (range.start, range.end) else {
                return Err(refuse(bound, "rank bound requires both endpoints"));
            };
            Some(RankBounds {
                lower: endpoint(lower)?,
                upper: endpoint(upper)?,
                upper_inclusive: range.inclusive,
                site: site_at(tree, bound),
            })
        }
    };
    Ok(MachineRanking {
        view: crate::identity::declaration(&witness.view, &site)?,
        relation: RankRelation {
            carrier,
            order: RankOrder::NaturalDescending,
        },
        rank,
        bounds,
        site,
        component: Vec::new(),
        edges: Vec::new(),
    })
}

/// Reconstruct each component from the actual lowered calls and attach one
/// claim to every internal edge. Productive unranked cycles stay unranked.
pub(crate) fn bind_component_edges(package: &mut LoweredPackage) -> Result<(), LoweringError> {
    let machines = &package.machines;
    if !machines
        .iter()
        .any(|m| m.contract.ranking.is_some() || m.contract.promises_termination)
    {
        return Ok(());
    }
    let local_target = |callee: &crate::CallTarget, site: &Site| -> Result<Option<usize>, LoweringError> {
        let id = match *callee {
            crate::CallTarget::Machine(id) => id,
            crate::CallTarget::Runtime { .. } => return Ok(None),
            crate::CallTarget::Member(slot) => match &package
                .member_calls
                .get(slot)
                .ok_or_else(|| LoweringError::new(site, "rank graph has an unknown member selection"))?
                .target
            {
                crate::MemberRealization::Conformance { machine, .. } => *machine,
                crate::MemberRealization::Boundary { .. } => return Ok(None),
            },
        };
        let index = usize::try_from(id.get() - 1)
            .ok()
            .filter(|&i| machines.get(i).is_some_and(|m| m.id == id))
            .ok_or_else(|| LoweringError::new(site, "rank graph has an unknown local machine"))?;
        Ok(Some(index))
    };
    let mut calls = vec![Vec::new(); machines.len()];
    for (position, machine) in machines.iter().enumerate() {
        let diagnostics = &machine.body.diagnostics;
        // A runtime descriptor or boundary member is outside every local
        // component, ranked caller or not. Its call stays in the body with its
        // callable contract; the checker applies that contract's termination.
        let mut add = |callee: &crate::CallTarget, site: &Site, invocation| -> Result<(), LoweringError> {
            if let Some(target) = local_target(callee, site)? {
                calls[position].push((target, invocation));
            }
            Ok(())
        };
        for row in &machine.body.blocks {
            for op in &machine.body.operations[row.operations.clone()] {
                let site = &diagnostics.operations[op.id.index()];
                match &op.kind {
                    OperationKind::Call { callee, .. } => add(callee, site, Invocation::Ordinary(row, op))?,
                    OperationKind::StructuralStore { disposition: crate::StructuralStoreDisposition::Dispose(actions), .. } => {
                        for action in actions {
                            if let crate::CleanupAction::Invoke { target, .. } = action {
                                add(&crate::CallTarget::Machine(*target), site, Invocation::Cleanup(site))?;
                            }
                        }
                    }
                    _ => {}
                }
            }
            let mut add_cleanups = |actions: &[crate::CleanupAction], id: crate::EdgeId| -> Result<(), LoweringError> {
                let site = edge_site(&machine.body, id.get());
                for action in actions {
                    if let crate::CleanupAction::Invoke { target, .. } = action {
                        add(&crate::CallTarget::Machine(*target), site, Invocation::Cleanup(site))?;
                    }
                }
                Ok(())
            };
            match &row.terminator {
                BlockTerminator::Return { id, cleanup, .. } => add_cleanups(cleanup, *id)?,
                BlockTerminator::TerminalCall { id, callee, cleanup, .. } => {
                    add_cleanups(cleanup, *id)?;
                    add(callee, edge_site(&machine.body, id.get()), Invocation::Terminal(row))?;
                }
                BlockTerminator::Jump(edge) => add_cleanups(&edge.cleanup, edge.id)?,
                BlockTerminator::Conditional { when_true, when_false, .. } => {
                    add_cleanups(&when_true.cleanup, when_true.id)?;
                    add_cleanups(&when_false.cleanup, when_false.id)?;
                }
                // An exit leaf has no successor, cleanup or local cycle edge.
                BlockTerminator::ExternalExit(_) => {}
            }
        }
    }
    let successors: Vec<Vec<usize>> = calls.iter()
        .map(|outgoing| outgoing.iter().map(|&(target, _)| target).collect())
        .collect();
    let component = strongly_connected(&successors);
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for (position, &group) in component.iter().enumerate() {
        if groups.len() <= group {
            groups.resize_with(group + 1, Vec::new);
        }
        groups[group].push(position);
    }
    let mut updates = Vec::new();
    for (group, members) in groups.iter_mut().enumerate() {
        members.sort_unstable_by_key(|&m| machines[m].id.get());
        let cyclic = members.len() > 1 || successors[members[0]].contains(&members[0]);
        // Members without an authored rank stay in the component. Their own
        // edges are the calls and transfers of their bodies; judging a
        // promise that relies on them belongs to the checker.
        let ranked: Vec<usize> = members
            .iter()
            .copied()
            .filter(|&m| machines[m].contract.ranking.is_some())
            .collect();
        for &m in &ranked {
            if let Some(ranking) = &machines[m].contract.ranking {
                check_entry_rank(&machines[m].body, &machines[m].parameter_types, ranking, &package.types)?;
            }
        }
        let identities: Vec<MachineId> = members.iter().map(|&m| machines[m].id).collect();
        if !cyclic {
            if !ranked.is_empty() {
                updates.push((members[0], identities, Vec::new()));
            }
            continue;
        }
        // A cycle with no authored rank has no claim to attach. A termination
        // promise on it stays in the machine contract over the same edges.
        let Some(relation) = ranked
            .first()
            .and_then(|&m| machines[m].contract.ranking.as_ref())
            .map(|r| r.relation)
        else {
            continue;
        };
        for &m in &ranked {
            let machine = &machines[m];
            let ranking = machine
                .contract
                .ranking
                .as_ref()
                .expect("ranked members have a witness");
            if ranking.relation != relation {
                return Err(LoweringError::new(&ranking.site, "a recursive component requires one common rank relation"));
            }
            match has_block_cycle(&machine.body) {
                Some(false) => {}
                Some(true) => return Err(LoweringError::new(&machine.body.diagnostics.body, "ranked block backedges are unsupported")),
                None => {
                    return Err(LoweringError::new(
                        &machine.body.diagnostics.body,
                        "ranked body has an unresolved successor block",
                    ));
                }
            }
            let source = ranking
                .source_rank(&machine.body.parameters)
                .ok_or_else(|| LoweringError::new(&ranking.site, "ranking input is not a scalar formal"))?;
            let comparisons = comparisons::Comparisons::new(&machine.body);
            // Preserve body order while checking identity in the call's own
            // namespace. A member has few component edges, so a flat list of
            // claimed call sites suffices.
            let mut claimed = Vec::new();
            let mut edges = Vec::new();
            for &(target, invocation) in calls[m].iter().filter(|&&(target, _)| component[target] == group) {
                let RankedTransfer { block, call, forwarding_return, arguments, site: edge_site } = invocation.transfer(&machine.body, &package.types)?;
                if claimed.contains(&call) {
                    return Err(LoweringError::new(edge_site, "ranked edge identity is not unique"));
                }
                claimed.push(call);
                let callee_machine = &machines[target];
                if arguments.len() != callee_machine.parameter_types.len() {
                    return Err(LoweringError::new(edge_site, "ranked edge arguments disagree with the target's formals"));
                }
                let successor = callee_machine
                    .contract
                    .ranking
                    .as_ref()
                    .map(|target_rank| {
                        target_rank
                            .rank
                            .try_map(|formal| match arguments.get(formal) {
                                Some(CallArgument::Scalar(value))
                                    if machine
                                        .body
                                        .values
                                        .get(value.index())
                                        .is_some_and(|v| Some(&v.ty) == callee_machine.parameter_types.get(formal)) =>
                                {
                                    Some(*value)
                                }
                                _ => None,
                            })
                            .ok_or_else(|| LoweringError::new(edge_site, "ranked edge argument is not a scalar actual of the target rank"))
                    })
                    .transpose()?;
                // Without a target rank there is nothing to compare.
                let comparison = successor
                    .as_ref()
                    .map_or(RankComparison::Pending, |successor| comparisons.comparison(block, &source, successor));
                edges.push(RankedEdge {
                    block,
                    call,
                    forwarding_return,
                    target: callee_machine.id,
                    successor,
                    comparison,
                });
            }
            updates.push((m, identities.clone(), edges));
        }
    }
    for (m, component, edges) in updates {
        let ranking = package.machines[m]
            .contract
            .ranking
            .as_mut()
            .expect("updates name ranked machines");
        ranking.component = component;
        ranking.edges = edges;
    }
    Ok(())
}

/// Each rank input is an immutable scalar formal of exactly the relation
/// carrier. Constant operands were checked against the carrier in the header.
fn check_entry_rank(
    body: &MachineBody,
    parameter_types: &[crate::TypeId],
    ranking: &MachineRanking,
    types: &crate::TypeTable,
) -> Result<(), LoweringError> {
    for formal in ranking.rank.inputs() {
        let scalar = match body.parameters.get(formal) {
            Some(Parameter::Scalar(value)) => body.values.get(value.index()),
            _ => None,
        };
        let carrier = parameter_types.get(formal).and_then(|&ty| types.get(ty));
        let exact = scalar.is_some_and(|v| v.origin == ValueOrigin::Parameter && Some(v.ty) == parameter_types.get(formal).copied())
            && carrier == Some(&TypeShape::PrimitiveScalar(ranking.relation.carrier));
        if !exact {
            return Err(LoweringError::new(&ranking.site, "ranking input is not a scalar formal of the rank carrier"));
        }
    }
    Ok(())
}

fn edge_site(
    body: &MachineBody,
    edge: usize,
) -> &Site {
    body.diagnostics
        .edges
        .get(edge.wrapping_sub(1))
        .unwrap_or(&body.diagnostics.body)
}

/// `None` when a successor names no block of this body.
fn has_block_cycle(body: &MachineBody) -> Option<bool> {
    let position = |id: crate::BlockId| {
        body.blocks
            .get(id.get() - 1)
            .filter(|block| block.id == id)
            .map(|_| id.get() - 1)
    };
    let mut successors = Vec::with_capacity(body.blocks.len());
    for block in &body.blocks {
        let targets = match &block.terminator {
            BlockTerminator::Conditional { when_true, when_false, .. } => vec![when_true.target, when_false.target],
            BlockTerminator::Jump(edge) => vec![edge.target],
            _ => Vec::new(),
        };
        successors.push(targets.into_iter().map(&position).collect::<Option<Vec<_>>>()?);
    }
    let mut state = vec![0u8; successors.len()];
    for root in 0..successors.len() {
        if state[root] != 0 {
            continue;
        }
        state[root] = 1;
        let mut stack = vec![(root, 0usize)];
        while let Some(&(node, next)) = stack.last() {
            if let Some(&successor) = successors[node].get(next) {
                if let Some(top) = stack.last_mut() {
                    top.1 += 1;
                }
                match state[successor] {
                    0 => {
                        state[successor] = 1;
                        stack.push((successor, 0));
                    }
                    1 => return Some(true),
                    _ => {}
                }
            } else {
                state[node] = 2;
                stack.pop();
            }
        }
    }
    Some(false)
}

/// Component number of each machine; mutually reachable machines share one.
fn strongly_connected(successors: &[Vec<usize>]) -> Vec<usize> {
    let size = successors.len();
    let mut predecessors = vec![Vec::new(); size];
    for (from, targets) in successors.iter().enumerate() {
        for &to in targets {
            predecessors[to].push(from);
        }
    }
    let mut finished = Vec::with_capacity(size);
    let mut visited = vec![false; size];
    for root in 0..size {
        if std::mem::replace(&mut visited[root], true) {
            continue;
        }
        let mut stack = vec![(root, 0usize)];
        while let Some(&(node, next)) = stack.last() {
            if let Some(&successor) = successors[node].get(next) {
                if let Some(top) = stack.last_mut() {
                    top.1 += 1;
                }
                if !std::mem::replace(&mut visited[successor], true) {
                    stack.push((successor, 0));
                }
            } else {
                finished.push(node);
                stack.pop();
            }
        }
    }
    let mut component = vec![usize::MAX; size];
    let mut count = 0;
    for &root in finished.iter().rev() {
        if component[root] != usize::MAX {
            continue;
        }
        component[root] = count;
        let mut pending = vec![root];
        while let Some(node) = pending.pop() {
            for &caller in &predecessors[node] {
                if component[caller] == usize::MAX {
                    component[caller] = count;
                    pending.push(caller);
                }
            }
        }
        count += 1;
    }
    component
}
