//! A loop keeps its declared measure on the values passed to each iteration.
//! Observations are ordinary operations; the checker independently validates
//! their types, descriptor bindings, comparisons, and coverage of every cycle.

use super::places::Bound;
use super::{Lowering, Value};
use crate::body::{Expression, ExpressionKind, RankRange, Signature, Strict, ValueType};
use crate::{SourceOrigin, Unpositioned, Unserved, refuse};
use arena::IdTable;
use semantic_vocabulary::{BlockId, MachineId, OperationId, ScalarType, ValueId};
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{Checked, Construct, ProgramPoint};
use terminal_psi::{
    Operation, OperationKind, StructuralAccess, StructuralArgument, TerminalBlockNaturalRank,
    TerminalIntegerRankValue, TerminalMachine, TerminalNaturalCycle, TerminalNaturalRankComparison,
    TerminalNaturalRankEdge, TerminalRankedScc,
};

#[derive(Clone)]
enum Input {
    Scalar(ValueId),
    Structural(StructuralArgument),
    Unavailable,
}

impl Lowering<'_> {
    pub(super) fn inherit_ranking(&mut self, block: BlockId) {
        let inserted = self.block_ranks.push(self.active_rank.clone());
        debug_assert_eq!(block, inserted);
    }

    pub(super) fn enter_ranking(&mut self, block: BlockId) {
        self.active_rank = self.block_ranks.get(block).cloned().flatten();
    }

    pub(super) fn begin_ranking(&mut self, member: MachineId) -> Checked<()> {
        self.member_rank_bound = None;
        let signature = self.signatures.get(member.into(), self.id)?;
        let Some(ranking) = &signature.ranking else {
            return Ok(());
        };
        let inputs = signature
            .parameters
            .iter()
            .enumerate()
            .map(|(slot, _)| match self.slots.get(slot).copied().flatten() {
                Some(Bound::Scalar(value)) => Input::Scalar(value),
                Some(Bound::Record(place) | Bound::Home(place)) => Input::Structural(
                    StructuralArgument::whole(place, StructuralAccess::SharedBorrow),
                ),
                // Unused erased bindings need no runtime operand. Reading
                // one as a measure still refuses in rank_expression.
                _ => Input::Unavailable,
            })
            .collect::<Vec<_>>();
        let values = self.observe_ranks(&ranking.measures, &inputs)?;
        self.member_rank_bound = ranking
            .fixed_bound
            .as_deref()
            .map(|bound| self.observe_rank(bound, &inputs))
            .transpose()?;
        let block = self.current()?;
        let missing = self.internal("ranked block has no construction row");
        *self.block_ranks.get_mut(block).ok_or(missing)? = Some(values.clone());
        self.active_rank = Some(values);
        if let Some(range) = &ranking.range {
            let Some([measure]) = self.active_rank.as_deref() else {
                return Err(self.unserved_rank("a scalar rank range needs exactly one measure"));
            };
            self.establish_rank_range(*measure, range, &inputs)?;
        }
        Ok(())
    }

    /// Each source invocation checks its private interval on the normalized
    /// rank. The ordinary invariant edge proves the two inequalities before
    /// its destination can assume them, including on first entry to a loop.
    fn establish_rank_range(
        &mut self,
        rank: TerminalIntegerRankValue,
        range: &RankRange,
        inputs: &[Input],
    ) -> Checked<()> {
        use semantic_vocabulary::{Proposition, ScalarTerm};

        let lower = self.rank_expression(&range.lower, inputs)?;
        let upper = self.rank_expression(&range.upper, inputs)?;
        let source = range.lower.source;
        let established = self.block(source);
        self.sources.predicate_establishments.push((
            established,
            terminal_check_tables::program::EstablishedCondition::RankRange,
        ));
        let scalar_type = ScalarType::Integer(rank.integer_type);
        let mut bind = |position| -> Checked<ScalarTerm> {
            let id = self.value(
                scalar_type,
                ValueOrigin::BlockParameter {
                    block: established,
                    position,
                },
            );
            self.parameter(established, id, scalar_type)?;
            Ok(ScalarTerm::Value { id, scalar_type })
        };
        let floor = bind(0)?;
        let measure = bind(1)?;
        let ceiling = bind(2)?;
        let below_ceiling = if range.upper_inclusive {
            Proposition::LessOrEqual(measure.clone(), ceiling)
        } else {
            Proposition::LessThan(measure.clone(), ceiling)
        };
        self.invariants.push(terminal_psi::ScalarBlockInvariant {
            machine: self.id,
            header: established,
            predicate: crate::canonical::conjunction(
                vec![Proposition::LessOrEqual(floor, measure), below_ceiling],
                self.id,
            )?,
            arrivals: Vec::new(),
        });
        self.jump_with_structural(
            established,
            vec![lower, rank.value, upper],
            Vec::new(),
            source,
        )?;
        self.enter(established);
        Ok(())
    }

    pub(super) fn successor_ranking(
        &mut self,
        member: MachineId,
        scalars: &[ValueId],
        structural: &[StructuralArgument],
        recursive_transfer: bool,
        source: SourceOrigin,
    ) -> Checked<()> {
        let signature = self.signatures.get(member.into(), self.id)?;
        let Some(ranking) = &signature.ranking else {
            return Ok(());
        };
        let inputs = self.rank_inputs(signature, scalars, structural)?;
        let values = self.observe_ranks(&ranking.measures, &inputs)?;
        if recursive_transfer {
            match (self.member_rank_bound, ranking.fixed_bound.as_deref()) {
                (Some(before), Some(bound)) => {
                    let after = self.observe_rank(bound, &inputs)?;
                    self.establish_fixed_rank_bound(before, after, source)?;
                }
                (None, None) => {}
                _ => {
                    return Err(self
                        .unserved_rank("recursive transfers must share the fixed ranking bound"));
                }
            }
        }
        self.successor_ranks.push((self.current()?, member, values));
        Ok(())
    }

    fn establish_fixed_rank_bound(
        &mut self,
        before: TerminalIntegerRankValue,
        after: TerminalIntegerRankValue,
        source: SourceOrigin,
    ) -> Checked<()> {
        use semantic_vocabulary::ScalarTerm;
        if before.integer_type != after.integer_type {
            return Err(self.unserved_rank("the fixed ranking bound changes carrier"));
        }
        if before.value == after.value {
            return Ok(());
        }
        let established = self.block(source);
        let scalar_type = ScalarType::Integer(before.integer_type);
        let mut bind = |position| -> Checked<ScalarTerm> {
            let id = self.value(
                scalar_type,
                ValueOrigin::BlockParameter {
                    block: established,
                    position,
                },
            );
            self.parameter(established, id, scalar_type)?;
            Ok(ScalarTerm::Value { id, scalar_type })
        };
        let predicate = crate::canonical::equal(bind(0)?, bind(1)?, self.id)?;
        self.sources.predicate_establishments.push((
            established,
            terminal_check_tables::program::EstablishedCondition::FixedRankBound,
        ));
        self.invariants.push(terminal_psi::ScalarBlockInvariant {
            machine: self.id,
            header: established,
            predicate,
            arrivals: Vec::new(),
        });
        self.jump_with_structural(
            established,
            vec![before.value, after.value],
            Vec::new(),
            source,
        )?;
        self.enter(established);
        Ok(())
    }

    fn rank_inputs(
        &self,
        signature: &Signature,
        scalars: &[ValueId],
        structural: &[StructuralArgument],
    ) -> Checked<Vec<Input>> {
        let mut scalar = scalars.iter();
        let mut places = structural.iter();
        let inputs = signature
            .parameters
            .iter()
            .enumerate()
            .map(|(position, ty)| {
                if signature.borrowed(position).is_some() || matches!(ty, ValueType::Record(..)) {
                    places.next().cloned().map(Input::Structural)
                } else {
                    scalar.next().copied().map(Input::Scalar)
                }
                .ok_or_else(|| self.internal("rank substitution has a missing actual argument"))
            })
            .collect::<Checked<Vec<_>>>()?;
        if scalar.next().is_some() || places.next().is_some() {
            return Err(self.internal("rank substitution has surplus actual arguments"));
        }
        Ok(inputs)
    }

    fn unserved_rank(&self, detail: &'static str) -> terminal_check_tables::Refusal {
        refuse(self.id, Construct::Machine, Unserved::Contract(detail))
    }

    fn observe_ranks(
        &mut self,
        expressions: &[Expression],
        inputs: &[Input],
    ) -> Checked<Vec<TerminalIntegerRankValue>> {
        expressions
            .iter()
            .map(|expression| self.observe_rank(expression, inputs))
            .collect()
    }

    fn observe_rank(
        &mut self,
        expression: &Expression,
        inputs: &[Input],
    ) -> Checked<TerminalIntegerRankValue> {
        let Some(ValueType::Scalar(ScalarType::Integer(integer_type), _)) = expression.result
        else {
            return Err(self.unserved_rank("ranking observation is not an integer"));
        };
        let value = self.rank_expression(expression, inputs)?;
        Ok(TerminalIntegerRankValue {
            integer_type,
            value,
        })
    }

    fn rank_expression(&mut self, expression: &Expression, inputs: &[Input]) -> Checked<ValueId> {
        let scalar_type = expression
            .result
            .and_then(ValueType::scalar)
            .ok_or_else(|| self.unserved_rank("ranking expression has no scalar carrier"))?;
        let emitted = match &expression.kind {
            ExpressionKind::Slot(slot) => {
                return match inputs.get(*slot) {
                    Some(Input::Scalar(value)) => Ok(*value),
                    _ => Err(self
                        .unserved_rank("integer ranking subject does not name a scalar argument")),
                };
            }
            ExpressionKind::Integer(value) => {
                let operation = self.operation_identity(expression.source);
                self.emit(
                    operation,
                    Some(scalar_type),
                    OperationKind::IntegerConstant { value: *value },
                )?
            }
            ExpressionKind::Strict {
                operation: Strict::Subtract,
                left,
                right,
                ..
            } => {
                let left = self.rank_expression(left, inputs)?;
                let right = self.rank_expression(right, inputs)?;
                self.strict_expression(expression, left, right)?
            }
            ExpressionKind::CollectionMeasure {
                base,
                measure: crate::source_state::expression::CollectionMeasure::Length,
            } => {
                let ExpressionKind::Slot(slot) = base.kind else {
                    return Err(
                        self.unserved_rank("length ranking subject is not a resolved formal")
                    );
                };
                let Some(Input::Structural(argument)) = inputs.get(slot) else {
                    return Err(self.unserved_rank(
                        "length ranking subject does not name a structural argument",
                    ));
                };
                let mut argument = argument.clone();
                argument.access = StructuralAccess::SharedBorrow;
                let record = self
                    .roots
                    .get(argument.place)
                    .map(|(_, record)| *record)
                    .ok_or_else(|| self.internal("ranking descriptor has no root declaration"))?;
                let exact = self.exact_root(argument.place)?;
                self.name_contents(exact, ValueType::record(record))?;
                let operation = self.operation_identity(expression.source);
                let read = self
                    .record_type(record)?
                    .shape
                    .length_operation()
                    .ok_or_else(|| {
                        self.unserved_rank("ranking length has no collection descriptor")
                    })?;
                self.emit(operation, Some(scalar_type), read(argument))?
            }
            _ => {
                return Err(
                    self.unserved_rank("ranking expression has no pure integer normalization")
                );
            }
        };
        match emitted {
            Value::Scalar(value) => Ok(value),
            _ => Err(self.internal("ranking observation did not produce a scalar")),
        }
    }

    pub(super) fn publish_rankings(&mut self, machine: &mut TerminalMachine) -> Checked<()> {
        if self.block_ranks.values().all(Option::is_none) {
            return Ok(());
        }
        let components = terminal_semantics::control_graph::control_cycle_members(machine)
            .map_err(|_| self.internal("ranking has an invalid completed control graph"))?;
        let mut operations = self.sources.operations.map(|_, _| None);
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            *operations
                .get_mut(operation.id)
                .ok_or_else(|| self.internal("rank producer has no operation coordinate"))? =
                Some(operation);
        }
        // The producer already knows each head and each transfer. Prepare
        // their associations once rather than rediscovering them per edge.
        let mut heads = self
            .heads
            .iter()
            .map(|head| (head.block, head.member))
            .collect::<Vec<_>>();
        heads.sort_unstable_by_key(|(block, _)| *block);
        let mut transfers = (0..self.successor_ranks.len()).collect::<Vec<_>>();
        transfers.sort_unstable_by_key(|index| {
            let (source, member, _) = &self.successor_ranks[*index];
            (*source, *member)
        });
        let mut cycles = Vec::with_capacity(components.len());
        for members in components {
            let mut cycle = TerminalNaturalCycle {
                observations: arena::Arena::default(),
                ranks: Vec::with_capacity(members.len()),
                edges: Vec::new(),
            };
            for block in &members {
                let rank = self
                    .block_ranks
                    .get(*block)
                    .and_then(Option::as_ref)
                    .ok_or_else(|| {
                        let mut refusal =
                            self.unserved_rank("a cyclic block has no ranking observation");
                        refusal.site = terminal_check_tables::RefusalSite::Machine(
                            self.id,
                            Construct::Block(*block),
                        );
                        let head = heads
                            .binary_search_by_key(block, |(block, _)| *block)
                            .ok()
                            .map(|position| heads[position].1);
                        refusal.detail.push_str(&format!(
                            " (machine {:?}, block {:?}, head {:?}, source {:?})",
                            self.id,
                            block,
                            head,
                            self.sources.blocks.get(*block),
                        ));
                        refusal
                    })?;
                let values = cycle.observations.insert_many(rank.clone());
                cycle.ranks.push(TerminalBlockNaturalRank {
                    block: *block,
                    values,
                });
            }
            for member in &members {
                let block = machine
                    .blocks
                    .get(usize::try_from(member.get() - 1).unwrap_or(usize::MAX))
                    .filter(|block| block.id == *member)
                    .ok_or_else(|| self.internal("cyclic member has no completed block"))?;
                let source_rank = self
                    .block_ranks
                    .get(block.id)
                    .and_then(Option::as_ref)
                    .ok_or_else(|| self.internal("cyclic source has no rank"))?;
                for (edge, target) in block
                    .terminator
                    .successor_edges()
                    .into_iter()
                    .filter(|(_, target)| members.binary_search(target).is_ok())
                {
                    let transferred = heads
                        .binary_search_by_key(&target, |(block, _)| *block)
                        .ok()
                        .and_then(|head| {
                            let key = (block.id, heads[head].1);
                            transfers
                                .binary_search_by_key(&key, |index| {
                                    let (source, member, _) = &self.successor_ranks[*index];
                                    (*source, *member)
                                })
                                .ok()
                                .map(|position| &self.successor_ranks[transfers[position]])
                        });
                    let target_rank = self
                        .block_ranks
                        .get(target)
                        .and_then(Option::as_ref)
                        .ok_or_else(|| self.internal("cyclic target has no rank"))?;
                    let successor = transferred.map_or(target_rank, |(_, _, values)| values);
                    // A helper's inferred observation and its recursive actual
                    // may compute the same subtraction in distinct operations.
                    // Those edges preserve rank; different SSA identities alone
                    // cannot demand strict descent. Both kinds of claim are
                    // still proved independently by the checker.
                    let same = successor.len() == source_rank.len()
                        && successor.iter().zip(source_rank).all(|(after, before)| {
                            let mut budget = self.tables.values.len();
                            after.integer_type == before.integer_type
                                && same_rank_computation(
                                    &self.tables.values,
                                    &operations,
                                    after.value,
                                    before.value,
                                    &mut budget,
                                )
                        });
                    let comparison = if transferred.is_some() && !same {
                        TerminalNaturalRankComparison::Strict
                    } else {
                        TerminalNaturalRankComparison::Preserving
                    };
                    let successor_ranks = cycle.observations.insert_many(successor.clone());
                    cycle.edges.push(TerminalNaturalRankEdge {
                        edge,
                        source: block.id,
                        target,
                        successor_ranks,
                        comparison,
                    });
                }
            }
            let component =
                terminal_semantics::control_graph::control_cycle_identity(machine, &cycle);
            let source = SourceOrigin::Unpositioned(Unpositioned::LoopHead);
            self.obligation(
                ObligationKind::WellFoundedness { component },
                ProgramPoint::BlockEntry(members[0]),
                source,
            );
            for edge in &cycle.edges {
                self.obligation(
                    ObligationKind::RankComparison {
                        component,
                        edge: edge.edge,
                    },
                    ProgramPoint::Edge(edge.edge),
                    source,
                );
            }
            cycles.push(cycle);
        }
        if !cycles.is_empty() {
            machine.ranked_scc = Some(TerminalRankedScc::Natural(cycles));
        }
        Ok(())
    }
}

/// Choose a preserving candidate only for identical immutable computations.
/// Unrecognized producers keep the strict candidate and its proof obligation;
/// this comparison grants no equality fact to any program.
fn same_rank_computation(
    values: &terminal_check_tables::MachineValues,
    operations: &IdTable<OperationId, Option<&Operation>>,
    left: ValueId,
    right: ValueId,
    budget: &mut usize,
) -> bool {
    if left == right {
        return true;
    }
    let Some(remaining) = budget.checked_sub(1) else {
        return false;
    };
    *budget = remaining;
    let (Some(left), Some(right)) = (values.get(left), values.get(right)) else {
        return false;
    };
    if left.scalar_type != right.scalar_type {
        return false;
    }
    let (ValueOrigin::Operation(left), ValueOrigin::Operation(right)) = (left.origin, right.origin)
    else {
        return false;
    };
    let (Some(Some(left)), Some(Some(right))) = (operations.get(left), operations.get(right))
    else {
        return false;
    };
    match (&left.kind, &right.kind) {
        (
            OperationKind::IntegerConstant { value: left },
            OperationKind::IntegerConstant { value: right },
        ) => left == right,
        (
            OperationKind::ExactIntegerSubtract {
                left: a, right: b, ..
            },
            OperationKind::ExactIntegerSubtract {
                left: c, right: d, ..
            },
        )
        | (
            OperationKind::SaturatingIntegerSubtract { left: a, right: b },
            OperationKind::SaturatingIntegerSubtract { left: c, right: d },
        ) => {
            same_rank_computation(values, operations, *a, *c, budget)
                && same_rank_computation(values, operations, *b, *d, budget)
        }
        _ => false,
    }
}
