//! Proof of entry/edge formation and every-cycle descent for computed integer
//! ranks in closed bodies. Other progress premises are independent judgments.
use omega_psi::*;
use std::collections::{BTreeSet, HashSet};
mod facts;
mod graph;
mod numbers;
use facts::State;
use numbers::Number;

#[derive(Debug)]
pub struct ComputedRankError {
    pub site: Site,
    pub message: String,
}
fn error(
    site: &Site,
    msg: impl Into<String>,
) -> ComputedRankError {
    ComputedRankError {
        site: site.clone(),
        message: msg.into(),
    }
}
const NAT: ScalarType = ScalarType::Integer { signed: false, width: 64 };

fn validate_view(
    rank: &MachineRanking,
    descending: &DeclarationId,
    bounded_distance: &DeclarationId,
) -> Result<(), ComputedRankError> {
    let expected = match rank.rank {
        RankExpression::SaturatingDistance { .. } => bounded_distance,
        _ => descending,
    };
    if rank.relation.carrier != NAT || rank.relation.order != RankOrder::NaturalDescending || &rank.view != expected {
        return Err(error(&rank.site, "rank requires its admitted natural view and u64 carrier"));
    }
    Ok(())
}

fn eval(
    rank: &MachineRanking,
    expr: &RankExpression<ValueId>,
    state: &State,
) -> Result<Number, ComputedRankError> {
    let ty = rank.relation.carrier;
    let operand = |input: &RankOperand<ValueId>, carrier| match input {
        RankOperand::Constant(n) => Number::literal(carrier, n.clone()).map_err(|e| error(&rank.site, e)),
        RankOperand::Input(id) => state
            .numbers
            .get(id)
            .filter(|n| n.ty == carrier)
            .cloned()
            .ok_or_else(|| error(&rank.site, "rank input needs a dominating value and its integer formation evidence")),
    };
    let mut sites = rank.arithmetic_sites.iter();
    let mut run = |op: &IntegerOperation, a: &Number, b: Option<&Number>| {
        let site = sites
            .next()
            .ok_or_else(|| error(&rank.site, "rank operation lacks its source occurrence"))?;
        numbers::apply(op, a, b, &state.numbers).map_err(|e| error(site, e))
    };
    let value = match expr {
        RankExpression::Operand(a) => operand(a, ty)?,
        RankExpression::SaturatingDistance { cursor, limit } => {
            let op = IntegerOperation {
                primitive: IntegerPrimitive::Subtract,
                policy: ArithmeticPolicy::Saturating,
                obligations: vec![],
            };
            numbers::apply(&op, &operand(limit, ty)?, Some(&operand(cursor, ty)?), &state.numbers).map_err(|e| error(&rank.site, e))?
        }
        RankExpression::ExactDifference { minuend, subtrahend } => {
            let op = IntegerOperation {
                primitive: IntegerPrimitive::Subtract,
                policy: ArithmeticPolicy::Exact,
                obligations: vec![IntegerObligation::ResultFitsCarrier],
            };
            run(&op, &operand(minuend, ty)?, Some(&operand(subtrahend, ty)?))?
        }
        RankExpression::Arithmetic(steps) => {
            let mut stack = Vec::new();
            let mut applications = 0;
            for step in steps {
                match step {
                    RankArithmeticStep::Operand { operand: arg, carrier } => stack.push(operand(arg, *carrier)?),
                    RankArithmeticStep::Apply(op) => {
                        let missing = || error(&rank.site, "rank arithmetic lacks an operand");
                        let b = if matches!(op.primitive, IntegerPrimitive::Negate | IntegerPrimitive::BitwiseNot) {
                            None
                        } else {
                            Some(stack.pop().ok_or_else(missing)?)
                        };
                        let a = stack.pop().ok_or_else(missing)?;
                        stack.push(run(op, &a, b.as_ref())?);
                        applications += 1;
                    }
                }
            }
            if applications == 0 || stack.len() != 1 {
                return Err(error(&rank.site, "arithmetic rank must compute one result"));
            }
            stack.pop().unwrap()
        }
    };
    if sites.next().is_some() || value.ty != ty {
        return Err(error(&rank.site, "rank result or arithmetic occurrences disagree with the view"));
    }
    if let Some(bounds) = &rank.bounds
        && (value.range.lo < bounds.lower
            || if bounds.upper_inclusive {
                value.range.hi > bounds.upper
            } else {
                value.range.hi >= bounds.upper
            })
    {
        return Err(error(&bounds.site, "cannot prove the rank lies within its authored interval"));
    }
    Ok(value)
}

/// Checks only components that contain a computed integer ranking expression.
/// Every rank is interpreted under the declared machine entry requirements.
pub fn check_computed_ranks(
    unit: &LoweredPackage,
    descending: &DeclarationId,
    bounded_distance: &DeclarationId,
) -> Result<(), ComputedRankError> {
    let computed = |m: &LoweredMachine| {
        m.contract.ranking.as_ref().is_some_and(|r| {
            !r.arithmetic_sites.is_empty()
                || matches!(r.rank, RankExpression::Arithmetic(_) | RankExpression::ExactDifference { .. })
                || r.edges
                    .iter()
                    .any(|edge| matches!(edge.successor, Some(RankExpression::Arithmetic(_) | RankExpression::ExactDifference { .. })))
        })
    };
    if !unit.machines.iter().any(computed) {
        return Ok(());
    }
    let calls = unit
        .machines
        .iter()
        .map(|m| graph::calls(unit, m))
        .collect::<Result<Vec<_>, _>>()?;
    let forward: Vec<Vec<usize>> = calls
        .iter()
        .map(|rows| rows.iter().map(|r| r.target).collect())
        .collect();
    let mut back = vec![vec![]; forward.len()];
    for (i, next) in forward.iter().enumerate() {
        for &j in next {
            back[j].push(i)
        }
    }
    let mut checked = BTreeSet::new();
    for (root, m) in unit.machines.iter().enumerate() {
        if checked.contains(&root) || !computed(m) {
            continue;
        }
        let predecessors = graph::reachable(&back, root);
        let group: BTreeSet<_> = graph::reachable(&forward, root)
            .intersection(&predecessors)
            .copied()
            .collect();
        let ids: Vec<_> = group.iter().map(|&i| unit.machines[i].id).collect();
        let mut preserving = vec![Vec::new(); group.len()];
        for (ordinal, &i) in group.iter().enumerate() {
            let machine = &unit.machines[i];
            let ranking = machine
                .contract
                .ranking
                .as_ref()
                .ok_or_else(|| error(&machine.body.diagnostics.body, "computed ranking cycle has an unranked member"))?;
            validate_view(ranking, descending, bounded_distance)?;
            if ranking.component != ids {
                return Err(error(&ranking.site, "declared rank component differs from the executable calls"));
            }
            let internal: Vec<_> = calls[i].iter().filter(|r| group.contains(&r.target)).collect();
            let entry = facts::entry(unit, machine)?;
            let source = ranking
                .source_rank(&machine.body.parameters)
                .ok_or_else(|| error(&ranking.site, "entry rank operand is not a scalar formal"))?;
            eval(ranking, &source, &entry)?;
            if internal.len() != ranking.edges.len() {
                return Err(error(&ranking.site, "rank claims do not cover every internal call"));
            }
            visit_calls(unit, machine, entry, &internal, |call, state| {
                let mut rows = ranking
                    .edges
                    .iter()
                    .filter(|r| r.call == call.at && r.block == call.block);
                let row = rows
                    .next()
                    .ok_or_else(|| error(call.site, "missing claim on the actual recursive call"))?;
                let target = &unit.machines[call.target];
                if rows.next().is_some() || target.id != row.target {
                    return Err(error(call.site, "rank claim names an incorrect recursive call target"));
                }
                let dest = target
                    .contract
                    .ranking
                    .as_ref()
                    .ok_or_else(|| error(call.site, "recursive target has no ranking witness"))?;
                validate_view(dest, descending, bounded_distance)?;
                if target.parameter_types.len() != call.arguments.len() {
                    return Err(error(call.site, "rank substitution needs the callee's actual argument list"));
                }
                for ((arg, formal), expected) in call
                    .arguments
                    .iter()
                    .zip(&target.body.parameters)
                    .zip(&target.parameter_types)
                {
                    match (arg, formal) {
                        (CallArgument::Scalar(v), Parameter::Scalar(_))
                            if machine
                                .body
                                .values
                                .get(v.index())
                                .is_some_and(|r| r.ty == *expected) => {}
                        (CallArgument::Scalar(_), _) | (_, Parameter::Scalar(_)) => {
                            return Err(error(call.site, "recursive actual differs from the callee's scalar formal"));
                        }
                        (_, Parameter::Structural(_)) => {}
                    }
                }
                let instantiated = dest
                    .rank
                    .try_map(|i| match call.arguments.get(i) {
                        Some(CallArgument::Scalar(id)) => Some(*id),
                        _ => None,
                    })
                    .ok_or_else(|| error(call.site, "rank substitution requires a scalar argument"))?;
                if row.successor.as_ref() != Some(&instantiated) {
                    return Err(error(call.site, "rank successor differs from the callee rank on actual arguments"));
                }
                let target_entry = facts::entry(unit, target)?;
                for (formal, arg) in target.body.parameters.iter().zip(call.arguments) {
                    if let (Parameter::Scalar(id), CallArgument::Scalar(actual)) = (formal, arg)
                        && let Some(required) = target_entry.numbers.get(id)
                        && Number::input(required.ty, *id).is_ok_and(|full| full.range != required.range)
                        && state
                            .numbers
                            .get(actual)
                            .is_none_or(|v| !required.range.contains(&v.range))
                    {
                        return Err(error(call.site, "rank call does not establish its target's entry bound"));
                    }
                }
                let before = eval(ranking, &source, state)?;
                let after = eval(dest, &instantiated, state)?;
                let delta = after.difference_bounds(&before, &state.numbers);
                if delta.hi > 0.into() {
                    return Err(error(call.site, "cannot prove the computed rank is non-increasing on this recursive call"));
                }
                if delta.hi == 0.into() {
                    if row.comparison == RankComparison::Strict {
                        return Err(error(call.site, "the claimed strict decrease is unproved"));
                    }
                    preserving[ordinal].push(
                        ids.binary_search_by_key(&target.id.get(), |id| id.get())
                            .expect("member of the reconstructed component"),
                    );
                }
                Ok(())
            })?;
        }
        if !graph::acyclic(&preserving) {
            return Err(error(
                &m.contract.ranking.as_ref().unwrap().site,
                "computed rank admits a recursive cycle without a proven strict decrease",
            ));
        }
        checked.extend(group);
    }
    Ok(())
}

fn visit_calls(
    unit: &LoweredPackage,
    m: &LoweredMachine,
    entry: State,
    calls: &[&graph::Call<'_>],
    mut visit: impl FnMut(&graph::Call<'_>, &State) -> Result<(), ComputedRankError>,
) -> Result<(), ComputedRankError> {
    let b = &m.body;
    let fail = |s: &str| error(&b.diagnostics.body, s);
    let edges = |block: &BranchBlock| -> Vec<(BlockId, Option<(ValueId, bool)>)> {
        match &block.terminator {
            BlockTerminator::Jump(e) => vec![(e.target, None)],
            BlockTerminator::Conditional {
                condition,
                when_true,
                when_false,
            } => vec![
                (when_true.target, Some((*condition, true))),
                (when_false.target, Some((*condition, false))),
            ],
            BlockTerminator::TerminalCall { completion: Some(target), .. } => vec![(*target, None)],
            _ => vec![],
        }
    };
    let mut seen_ops = vec![false; b.operations.len()];
    let mut seen_values = BTreeSet::new();
    let mut seen_edges = HashSet::new();
    let mut degree = vec![0; b.blocks.len()];
    let mut arrivals = vec![None; b.blocks.len()];
    for p in &b.parameters {
        if let Parameter::Scalar(id) = p {
            seen_values.insert(*id);
        }
    }
    for (idx, block) in b.blocks.iter().enumerate() {
        if block.id.get() != idx + 1 {
            return Err(fail("rank block identity differs from its position"));
        }
        for id in &block.scalar_parameters {
            if !seen_values.insert(*id) {
                return Err(fail("duplicate scalar definition in rank body"));
            }
        }
        let edge_ids = match &block.terminator {
            BlockTerminator::Jump(e) => [Some(e.id), None],
            BlockTerminator::Conditional { when_true, when_false, .. } => [Some(when_true.id), Some(when_false.id)],
            BlockTerminator::Return { id, .. } | BlockTerminator::TerminalCall { id, .. } => [Some(*id), None],
            BlockTerminator::ExternalExit(e) => [Some(e.id), None],
        };
        for id in edge_ids.into_iter().flatten() {
            if !seen_edges.insert(id) {
                return Err(fail("duplicate rank edge identity"));
            }
        }
        for (to, _) in edges(block) {
            *degree
                .get_mut(to.get() - 1)
                .ok_or_else(|| fail("unknown rank control-flow target"))? += 1;
        }
        for pos in block.operations.clone() {
            let seen = seen_ops
                .get_mut(pos)
                .ok_or_else(|| fail("invalid rank operation range"))?;
            if *seen || b.operations[pos].id.index() != pos {
                return Err(fail("invalid rank operation identity"));
            }
            *seen = true;
            if let OperationResult::Scalar(id) = b.operations[pos].result
                && !seen_values.insert(id)
            {
                return Err(fail("duplicate scalar definition in rank body"));
            }
        }
    }
    if seen_ops.contains(&false) || degree.first() != Some(&0) || !b.blocks[0].scalar_parameters.is_empty() {
        return Err(fail("rank proof requires a unique CFG entry and complete operation coverage"));
    }
    arrivals[0] = Some(entry);
    let mut stack = vec![0];
    let mut visited = 0;
    let mut visited_calls = HashSet::new();
    while let Some(i) = stack.pop() {
        visited += 1;
        let mut state = arrivals[i]
            .take()
            .ok_or_else(|| fail("rank block lacks incoming evidence"))?;
        let block = &b.blocks[i];
        let mut inspect = |at, state: &State| -> Result<(), ComputedRankError> {
            for call in calls.iter().filter(|c| c.block == block.id && c.at == at) {
                if !visited_calls.insert(at) {
                    return Err(fail("rank graph has duplicate call identities"));
                }
                visit(call, state)?;
            }
            Ok(())
        };
        for op in &b.operations[block.operations.clone()] {
            inspect(CallSite::Operation(op.id), &state)?;
            state.operation(unit, m, op).map_err(|s| {
                error(
                    b.diagnostics
                        .operations
                        .get(op.id.index())
                        .unwrap_or(&b.diagnostics.body),
                    s,
                )
            })?;
        }
        if let BlockTerminator::TerminalCall { id, .. } = &block.terminator {
            inspect(CallSite::Edge(*id), &state)?;
        }
        for (target, guard) in edges(block) {
            let to = target.get() - 1;
            let mut next = state.clone();
            let edge = match &block.terminator {
                BlockTerminator::Jump(e) => Some(e),
                BlockTerminator::Conditional { when_true, when_false, .. } => {
                    let (id, positive) = guard.expect("conditional edge carries its predicate");
                    next.branch(id, positive);
                    Some(if positive { when_true } else { when_false })
                }
                // The result is a fresh call output. No caller rank fact transfers to it.
                BlockTerminator::TerminalCall { .. } => None,
                _ => unreachable!("terminator has no successor"),
            };
            if let Some(edge) = edge {
                next.bind_edge(b, edge, &b.blocks[to]).map_err(|s| fail(&s))?;
            }
            if let Some(prev) = &mut arrivals[to] {
                prev.merge(&next)
            } else {
                arrivals[to] = Some(next)
            }
            degree[to] -= 1;
            if degree[to] == 0 {
                stack.push(to)
            }
        }
    }
    if visited != b.blocks.len() || visited_calls.len() != calls.len() {
        return Err(fail(
            "computed rank requires independent evidence for a cyclic block, unreachable block or cleanup call",
        ));
    }
    Ok(())
}
