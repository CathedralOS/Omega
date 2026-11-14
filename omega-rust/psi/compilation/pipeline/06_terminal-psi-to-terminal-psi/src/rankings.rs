//! A loop that promises to finish must decrease on every cycle. Check the actual
//! graph, the measure passed to each successor, and each required comparison.

use crate::{graph::Graph, places::Storage, refusal};
use semantic_vocabulary::{
    BlockId, CycleComponentId, EdgeId, IntegerType, IntegerValue, MachineId, Proposition,
    ScalarTerm, ScalarType, ValueId,
};
use std::collections::VecDeque;
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{Checked, Construct, MachineTables, ProgramPoint, RefusalReason};
use terminal_psi::{
    OperationKind, StructuralArgument, TerminalIntegerRankValue, TerminalMachine,
    TerminalNaturalCycle, TerminalNaturalRankComparison, TerminalRankedScc,
};

/// A finite integer tuple cannot decrease forever in lexicographic order.
/// Keep its carriers and private value bindings with this judgment, so the
/// graph alone cannot stand in for the relation that was checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiniteIntegerRanking {
    pub component: CycleComponentId,
    pub carriers: Vec<IntegerType>,
    pub bindings: TerminalNaturalCycle,
}

fn foreign(machine: MachineId, detail: &str) -> terminal_check_tables::Refusal {
    refusal(
        RefusalReason::ForeignIdentity,
        machine,
        Construct::Machine,
        detail,
    )
}

fn cycles(machine: &TerminalMachine) -> &[TerminalNaturalCycle] {
    match &machine.ranked_scc {
        Some(TerminalRankedScc::Natural(cycles)) => cycles,
        None => &[],
    }
}

/// A ranked cycle owes one well-foundedness question and a comparison at
/// every internal edge. The lowered form fixes each question's location.
pub(crate) fn owed(
    machine: &TerminalMachine,
) -> impl Iterator<Item = (ObligationKind, ProgramPoint, Construct)> + '_ {
    cycles(machine).iter().flat_map(move |cycle| {
        let component = terminal_semantics::control_graph::control_cycle_identity(machine, cycle);
        cycle
            .ranks
            .first()
            .map(|rank| {
                (
                    ObligationKind::WellFoundedness { component },
                    ProgramPoint::BlockEntry(rank.block),
                    Construct::Block(rank.block),
                )
            })
            .into_iter()
            .chain(cycle.edges.iter().map(move |edge| {
                (
                    ObligationKind::RankComparison {
                        component,
                        edge: edge.edge,
                    },
                    ProgramPoint::Edge(edge.edge),
                    Construct::Edge(edge.edge),
                )
            }))
    })
}

pub(crate) fn claims_strict(machine: &TerminalMachine, edge: EdgeId) -> Option<bool> {
    cycles(machine)
        .iter()
        .flat_map(|cycle| &cycle.edges)
        .find(|row| row.edge == edge)
        .map(|row| row.comparison == TerminalNaturalRankComparison::Strict)
}

pub(crate) fn admit(
    machine: &TerminalMachine,
    tables: &MachineTables,
    graph: &Graph,
    storage: &Storage<'_>,
) -> Checked<()> {
    let ranked = cycles(machine);
    let actual = if machine.ranked_scc.is_some()
        || machine.contract.termination == semantic_vocabulary::Termination::Terminates
    {
        terminal_semantics::control_graph::control_cycle_members(machine).map_err(|_| {
            foreign(
                machine.id,
                "ranking requires a complete valid reachable control graph",
            )
        })?
    } else {
        Vec::new()
    };
    if machine.contract.termination == semantic_vocabulary::Termination::Terminates
        && ranked.is_empty()
        && let Some(members) = actual.first()
    {
        return Err(refusal(
            RefusalReason::Unsupported,
            machine.id,
            members
                .first()
                .copied()
                .map_or(Construct::Machine, Construct::Block),
            "contract termination promise over an unranked cycle",
        ));
    }
    if machine.ranked_scc.is_some() && ranked.len() != actual.len() {
        return Err(foreign(
            machine.id,
            "ranked components do not cover the actual cyclic control graph",
        ));
    }
    let mut expected = Vec::new();
    let normalizer = Normalize {
        machine,
        tables,
        graph,
        storage,
    };
    for (cycle, members) in ranked.iter().zip(&actual) {
        if cycle
            .ranks
            .iter()
            .map(|rank| rank.block)
            .collect::<Vec<_>>()
            != *members
        {
            return Err(foreign(
                machine.id,
                "rank bindings do not follow exact cyclic component membership",
            ));
        }
        let internal = machine
            .blocks
            .iter()
            .filter(|block| members.binary_search(&block.id).is_ok())
            .flat_map(|block| {
                block
                    .terminator
                    .successor_edges()
                    .into_iter()
                    .filter(|(_, target)| members.binary_search(target).is_ok())
                    .map(move |(edge, target)| (edge, block.id, target))
            })
            .collect::<Vec<_>>();
        if cycle
            .edges
            .iter()
            .map(|edge| (edge.edge, edge.source, edge.target))
            .collect::<Vec<_>>()
            != internal
        {
            return Err(foreign(
                machine.id,
                "rank edges do not match every actual internal edge in canonical order",
            ));
        }
        let first = cycle
            .ranks
            .first()
            .ok_or_else(|| foreign(machine.id, "empty ranking component"))?;
        let signature = cycle
            .rank_values(first.values)
            .ok_or_else(|| foreign(machine.id, "ranking has no complete nonempty carrier tuple"))?;
        let carriers = signature
            .iter()
            .map(|rank| rank.integer_type)
            .collect::<Vec<_>>();
        if carriers
            .iter()
            .any(|integer| integer.is_address() || integer.bits().is_none_or(|bits| bits == 0))
        {
            return Err(refusal(
                RefusalReason::Unsupported,
                machine.id,
                Construct::Machine,
                "ranking requires closed finite integer carriers, not addresses or parameter widths",
            ));
        }
        let mut used = vec![false; cycle.observations.len() + 1];
        for rank in &cycle.ranks {
            let values = cycle
                .rank_values(rank.values)
                .ok_or_else(|| foreign(machine.id, "rank tuple changes carrier or arity"))?;
            for handle in rank.values.handles() {
                let slot = used
                    .get_mut(handle.arena_index() as usize)
                    .ok_or_else(|| foreign(machine.id, "rank observation is outside its arena"))?;
                *slot = true;
            }
            for value in values {
                normalizer.typed(*value)?;
                let at = machine
                    .block(&rank.block)
                    .ok_or_else(|| foreign(machine.id, "rank has no block"))?;
                for (edge, _) in at.terminator.successor_edges() {
                    normalizer.available(value.value, ProgramPoint::Edge(edge))?;
                }
            }
        }
        let component = terminal_semantics::control_graph::control_cycle_identity(machine, cycle);
        expected.push(((component, None), ProgramPoint::BlockEntry(first.block)));
        for edge in &cycle.edges {
            let target = cycle
                .ranks
                .iter()
                .find(|rank| rank.block == edge.target)
                .ok_or_else(|| foreign(machine.id, "rank edge has no target rank"))?;
            let target_values = cycle
                .rank_values(target.values)
                .ok_or_else(|| foreign(machine.id, "target rank has an invalid tuple"))?;
            let successors = cycle
                .rank_values(edge.successor_ranks)
                .ok_or_else(|| foreign(machine.id, "successor rank changes carrier or arity"))?;
            for handle in edge.successor_ranks.handles() {
                let slot = used.get_mut(handle.arena_index() as usize).ok_or_else(|| {
                    foreign(machine.id, "successor observation is outside its arena")
                })?;
                *slot = true;
            }
            let (actuals, _) = edge_actuals(machine, edge.edge)?;
            for (target_rank, successor) in target_values.iter().zip(successors) {
                normalizer.typed(*successor)?;
                normalizer.available(successor.value, ProgramPoint::Edge(edge.edge))?;
                normalizer.corresponds(
                    edge.target,
                    target_rank.value,
                    successor.value,
                    edge.edge,
                    actuals,
                )?;
            }
            expected.push(((component, Some(edge.edge)), ProgramPoint::Edge(edge.edge)));
        }
        if cycle.observations.iter().any(|(handle, _)| {
            !used
                .get(handle.arena_index() as usize)
                .copied()
                .unwrap_or(false)
        }) {
            return Err(foreign(
                machine.id,
                "ranking has surplus unbound observations",
            ));
        }
        strict_cut(machine, cycle)?;
    }
    expected.sort_by_key(|(key, _)| *key);
    if expected.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(foreign(
            machine.id,
            "duplicate ranking component or edge identity",
        ));
    }
    let mut claimed = vec![false; expected.len()];
    for (_, row) in tables.obligations.iter() {
        let key = match row.kind {
            ObligationKind::WellFoundedness { component } => (component, None),
            ObligationKind::RankComparison { component, edge } => (component, Some(edge)),
            _ => continue,
        };
        let matched = expected.binary_search_by_key(&key, |(key, _)| *key).ok();
        if matched.is_none_or(|index| {
            expected[index].1 != row.created_at || std::mem::replace(&mut claimed[index], true)
        }) {
            return Err(foreign(
                machine.id,
                "ranking obligation is duplicate, stale, surplus or at another point",
            ));
        }
    }
    if claimed.contains(&false) {
        return Err(foreign(
            machine.id,
            "ranking is missing an exact relation or edge obligation",
        ));
    }
    Ok(())
}

fn edge_actuals(
    machine: &TerminalMachine,
    edge: EdgeId,
) -> Checked<(&[ValueId], &[StructuralArgument])> {
    for block in &machine.blocks {
        match &block.terminator {
            terminal_psi::Terminator::Jump {
                edge: actual,
                arguments,
                structural_arguments,
                ..
            } if *actual == edge => return Ok((arguments, structural_arguments)),
            terminal_psi::Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for successor in [when_true, when_false] {
                    if successor.edge == edge {
                        return Ok((&successor.arguments, &successor.structural_arguments));
                    }
                }
            }
            terminal_psi::Terminator::StructuralCase { cases, .. } => {
                if let Some(successor) = cases.iter().find(|successor| successor.edge == edge) {
                    return Ok((&[], &successor.structural_arguments));
                }
            }
            _ => {}
        }
    }
    Err(foreign(
        machine.id,
        "rank edge has no canonical successor arguments",
    ))
}

fn strict_cut(machine: &TerminalMachine, cycle: &TerminalNaturalCycle) -> Checked<()> {
    let index = |block: BlockId| usize::try_from(block.get() - 1).unwrap_or(usize::MAX);
    let mut incoming = vec![0usize; machine.blocks.len()];
    let mut outgoing = vec![Vec::new(); machine.blocks.len()];
    for edge in &cycle.edges {
        if edge.comparison == TerminalNaturalRankComparison::Preserving {
            incoming[index(edge.target)] += 1;
            outgoing[index(edge.source)].push(edge.target);
        }
    }
    let mut ready = cycle
        .ranks
        .iter()
        .filter_map(|rank| (incoming[index(rank.block)] == 0).then_some(rank.block))
        .collect::<VecDeque<_>>();
    let mut removed = 0;
    while let Some(block) = ready.pop_front() {
        removed += 1;
        for target in &outgoing[index(block)] {
            let count = &mut incoming[index(*target)];
            *count -= 1;
            if *count == 0 {
                ready.push_back(*target);
            }
        }
    }
    if removed != cycle.ranks.len() {
        return Err(foreign(
            machine.id,
            "removing strict rank edges leaves a control cycle",
        ));
    }
    Ok(())
}

fn component(
    machine: &TerminalMachine,
    identity: CycleComponentId,
) -> Checked<&TerminalNaturalCycle> {
    let mut matching = cycles(machine).iter().filter(|cycle| {
        terminal_semantics::control_graph::control_cycle_identity(machine, cycle) == identity
    });
    let cycle = matching
        .next()
        .ok_or_else(|| foreign(machine.id, "ranking names no actual component"))?;
    if matching.next().is_some() {
        return Err(foreign(
            machine.id,
            "ranking component identity is ambiguous",
        ));
    }
    Ok(cycle)
}

pub(crate) fn well_founded(
    machine: &TerminalMachine,
    identity: CycleComponentId,
) -> Checked<FiniteIntegerRanking> {
    let cycle = component(machine, identity)?;
    let rank = cycle
        .ranks
        .first()
        .and_then(|rank| cycle.rank_values(rank.values))
        .ok_or_else(|| foreign(machine.id, "ranking has no finite relation"))?;
    Ok(FiniteIntegerRanking {
        component: identity,
        carriers: rank.iter().map(|rank| rank.integer_type).collect(),
        bindings: cycle.clone(),
    })
}

pub(crate) fn goal(
    machine: &TerminalMachine,
    identity: CycleComponentId,
    edge: EdgeId,
) -> Checked<Proposition> {
    let cycle = component(machine, identity)?;
    let edge = cycle
        .edges
        .iter()
        .find(|claim| claim.edge == edge)
        .ok_or_else(|| foreign(machine.id, "ranking has no exact edge comparison"))?;
    let before = cycle
        .ranks
        .iter()
        .find(|rank| rank.block == edge.source)
        .and_then(|rank| cycle.rank_values(rank.values))
        .ok_or_else(|| foreign(machine.id, "ranking comparison has no source tuple"))?;
    let after = cycle
        .rank_values(edge.successor_ranks)
        .ok_or_else(|| foreign(machine.id, "ranking comparison has no successor tuple"))?;
    let pair = |index: usize| {
        let scalar = ScalarType::Integer(before[index].integer_type);
        (
            ScalarTerm::value(after[index].value, scalar),
            ScalarTerm::value(before[index].value, scalar),
        )
    };
    let last = before
        .len()
        .checked_sub(1)
        .ok_or_else(|| foreign(machine.id, "ranking comparison has an empty tuple"))?;
    let (after, before_value) = pair(last);
    let mut goal = match edge.comparison {
        TerminalNaturalRankComparison::Strict => Proposition::LessThan(after, before_value),
        TerminalNaturalRankComparison::Preserving => Proposition::LessOrEqual(after, before_value),
    };
    for index in (0..last).rev() {
        let (after, before) = pair(index);
        goal = Proposition::Disjunction(vec![
            Proposition::LessThan(after.clone(), before.clone()),
            Proposition::Conjunction(vec![Proposition::Equal(after, before), goal]),
        ]);
    }
    Ok(goal)
}

#[derive(Debug, PartialEq, Eq)]
enum RankValue {
    Scalar(ValueId),
    Integer(IntegerValue),
    Difference(Box<RankValue>, Box<RankValue>),
    Length(
        semantic_vocabulary::PlaceId,
        Vec<terminal_psi::StructuralPathSegment>,
    ),
}

struct Normalize<'a, 'storage> {
    machine: &'a TerminalMachine,
    tables: &'a MachineTables,
    graph: &'a Graph,
    storage: &'a Storage<'storage>,
}

impl Normalize<'_, '_> {
    fn typed(&self, rank: TerminalIntegerRankValue) -> Checked<()> {
        if self.tables.values.get(rank.value).is_none_or(|row| {
            row.rigid.is_some() || row.scalar_type != ScalarType::Integer(rank.integer_type)
        }) {
            return Err(foreign(
                self.machine.id,
                "rank observation has another carrier or no executable value",
            ));
        }
        Ok(())
    }
    fn available(&self, value: ValueId, used: ProgramPoint) -> Checked<()> {
        let point = match self.tables.values.get(value).map(|row| row.origin) {
            Some(ValueOrigin::MachineParameter { .. }) => {
                ProgramPoint::BlockEntry(self.machine.entry)
            }
            Some(ValueOrigin::BlockParameter { block, .. }) => ProgramPoint::BlockEntry(block),
            Some(ValueOrigin::Operation(operation)) => ProgramPoint::AfterOperation(operation),
            _ => {
                return Err(foreign(
                    self.machine.id,
                    "ranking names a non-executable or undefined value",
                ));
            }
        };
        if !self.graph.dominates(point, used)? {
            return Err(foreign(
                self.machine.id,
                "rank observation is outside its actual scope",
            ));
        }
        Ok(())
    }
    fn corresponds(
        &self,
        target: BlockId,
        rank: ValueId,
        successor: ValueId,
        edge: EdgeId,
        actuals: &[ValueId],
    ) -> Checked<()> {
        let origin = self
            .tables
            .values
            .get(rank)
            .ok_or_else(|| foreign(self.machine.id, "target rank is undefined"))?
            .origin;
        match origin {
            ValueOrigin::BlockParameter { block, position } if block == target => {
                if actuals.get(position as usize).copied() == Some(successor) {
                    return Ok(());
                }
            }
            ValueOrigin::MachineParameter { .. } => {
                if rank == successor {
                    return Ok(());
                }
            }
            ValueOrigin::BlockParameter { block, .. } if block != target => {
                self.available(rank, ProgramPoint::BlockEntry(target))?;
                if rank == successor {
                    return Ok(());
                }
            }
            ValueOrigin::Operation(operation) => {
                let (owner, _) = self.graph.locate(ProgramPoint::AfterOperation(operation))?;
                if owner != target {
                    self.available(rank, ProgramPoint::BlockEntry(target))?;
                    if rank == successor {
                        return Ok(());
                    }
                } else {
                    let expected = self.value(
                        rank,
                        Some((target, edge, actuals)),
                        ProgramPoint::AfterOperation(operation),
                        &mut (self.tables.values.len() + 1),
                    )?;
                    let actual = self.value(
                        successor,
                        None,
                        ProgramPoint::Edge(edge),
                        &mut (self.tables.values.len() + 1),
                    )?;
                    if expected == actual {
                        return Ok(());
                    }
                }
            }
            _ => {}
        }
        Err(foreign(
            self.machine.id,
            "successor rank is not the target observation under its exact actual arguments",
        ))
    }
    fn value(
        &self,
        value: ValueId,
        substitution: Option<(BlockId, EdgeId, &[ValueId])>,
        used: ProgramPoint,
        budget: &mut usize,
    ) -> Checked<RankValue> {
        *budget = budget
            .checked_sub(1)
            .ok_or_else(|| foreign(self.machine.id, "rank normalization does not end"))?;
        self.available(value, used)?;
        let row = self
            .tables
            .values
            .get(value)
            .ok_or_else(|| foreign(self.machine.id, "rank value is undefined"))?;
        match row.origin {
            ValueOrigin::MachineParameter { .. } => Ok(RankValue::Scalar(value)),
            ValueOrigin::BlockParameter { block, position } => {
                if let Some((target, edge, actuals)) = substitution
                    && target == block
                {
                    let actual = actuals.get(position as usize).copied().ok_or_else(|| {
                        foreign(self.machine.id, "rank substitution lost a scalar actual")
                    })?;
                    self.value(actual, None, ProgramPoint::Edge(edge), budget)
                } else {
                    Ok(RankValue::Scalar(value))
                }
            }
            ValueOrigin::Operation(producer) => {
                let before = self.graph.locate(ProgramPoint::AfterOperation(producer))?;
                let operation = self
                    .machine
                    .blocks
                    .get(usize::try_from(before.0.get() - 1).unwrap_or(usize::MAX))
                    .and_then(|block| block.operations.get(before.1 - 1))
                    .filter(|operation| operation.id == producer)
                    .ok_or_else(|| foreign(self.machine.id, "rank producer is absent"))?;
                match &operation.kind {
                    OperationKind::IntegerConstant { value } => Ok(RankValue::Integer(*value)),
                    OperationKind::ExactIntegerSubtract { left, right, .. } => {
                        Ok(RankValue::Difference(
                            Box::new(self.value(
                                *left,
                                substitution,
                                ProgramPoint::AfterOperation(producer),
                                budget,
                            )?),
                            Box::new(self.value(
                                *right,
                                substitution,
                                ProgramPoint::AfterOperation(producer),
                                budget,
                            )?),
                        ))
                    }
                    OperationKind::ByteSequenceLength { source } => {
                        let mut source = source.clone();
                        if let Some((target, edge, _)) = substitution {
                            if before.0 != target
                                || !self.storage.rank_descriptor_kept(
                                    &source,
                                    target,
                                    0,
                                    before.1 - 1,
                                )?
                            {
                                return Err(foreign(
                                    self.machine.id,
                                    "target rank descriptor changes before its observation",
                                ));
                            }
                            if let semantic_vocabulary::StructuralPlaceKind::BlockParameter {
                                block,
                                position,
                            } = self
                                .machine
                                .structural_places
                                .iter()
                                .find(|place| place.id == source.place)
                                .ok_or_else(|| {
                                    foreign(self.machine.id, "rank descriptor has no root")
                                })?
                                .kind
                                && block == target
                            {
                                let actual = edge_actuals(self.machine, edge)?
                                    .1
                                    .get(position as usize)
                                    .ok_or_else(|| {
                                        foreign(
                                            self.machine.id,
                                            "rank substitution lost its structural actual",
                                        )
                                    })?;
                                source.path =
                                    actual.path.iter().cloned().chain(source.path).collect();
                                source.place = actual.place;
                            }
                        } else {
                            let (block, last) = self.graph.locate(used)?;
                            let end = self
                                .machine
                                .block(&block)
                                .ok_or_else(|| foreign(self.machine.id, "rank edge has no block"))?
                                .operations
                                .len()
                                .min(last);
                            if before.0 != block
                                || !self
                                    .storage
                                    .rank_descriptor_kept(&source, block, before.1, end)?
                            {
                                return Err(foreign(
                                    self.machine.id,
                                    "successor descriptor changed after its rank observation",
                                ));
                            }
                        }
                        Ok(RankValue::Length(source.place, source.path))
                    }
                    _ if substitution.is_none() => Ok(RankValue::Scalar(value)),
                    _ => Err(refusal(
                        RefusalReason::Unsupported,
                        self.machine.id,
                        Construct::Operation(producer),
                        "ranking observation outside constants, exact subtraction and descriptor length",
                    )),
                }
            }
            _ => Err(foreign(
                self.machine.id,
                "ranking uses a non-executable value",
            )),
        }
    }
}
