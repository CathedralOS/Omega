//! Propose the interval a two-way selection join gives its result.
//!
//! Integer `min`/`max`, like any compare-and-select computation, lowers to a
//! comparison, one `Conditional`, and a join whose two arrivals each forward
//! one compared operand as the join's result parameter. Reconstruction
//! intersects the two arms' path facts, so the result reaches the join with
//! no bound even though each arm bounds what it forwards: the arm taken when
//! `a <= b` forwards an `a` no larger than `b`. A sum of two clamped values
//! then has no operand bound left to prove its exact arithmetic.
//!
//! This producer evaluates integer intervals over one machine's values:
//! literals, values a block parameter receives unchanged on every arrival,
//! and the results of earlier selection joins. At each selection join it
//! proposes the hull of what the two arms forward, each narrowed by the
//! comparison outcome its arm is taken under. Unknown values keep their
//! carrier extremes, and only sides tighter than the carrier are proposed.
//! The proposal is a hypothesis: the shared roster transaction proves it on
//! both actual arrivals before it is retained.
//!
//! As with literal join hulls, a machine with no reconstructed question
//! receives no proposal: the row alone would only manufacture its own
//! arrival goals.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{
    BlockId, IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType, ValueId,
};
use terminal_psi::{
    Block, OperationKind, OperationResult, ScalarBlockInvariant, TerminalMachine, TerminalModule,
    Terminator,
};
use terminal_verifier::ReconstructedTerminalObligationSet;

/// Jump-only blocks walked back from a join arrival to the deciding
/// conditional, and block parameters followed while resolving a value's
/// source. Lowered selections thread their operands through a handful of
/// blocks; the bound only keeps malformed graphs finite.
const MAXIMUM_WALK: usize = 64;

pub(super) fn candidates(
    module: &TerminalModule,
    questions: &ReconstructedTerminalObligationSet,
) -> Vec<ScalarBlockInvariant> {
    let mut candidates = Vec::new();
    for machine in &module.machines {
        if !questions
            .obligations()
            .iter()
            .any(|site| site.owner.machine() == machine.id)
        {
            continue;
        }
        let mut values = MachineValues::new(machine);
        for header in &machine.blocks {
            let Some(selection) = values.selection(header.id) else {
                continue;
            };
            let mut members = Vec::new();
            for (position, parameter) in header.parameters.iter().enumerate() {
                let ScalarType::Integer(integer) = parameter.scalar_type else {
                    continue;
                };
                let Some(interval) = values.selection_interval(&selection, position, integer)
                else {
                    continue;
                };
                members.extend(interval.members(parameter.id, integer));
            }
            let predicate = match members.len() {
                0 => continue,
                1 => members.pop().expect("one member"),
                _ => Proposition::Conjunction(members),
            };
            candidates.push(ScalarBlockInvariant {
                machine: machine.id,
                header: header.id,
                predicate,
                arrivals: Vec::new(),
            });
        }
    }
    candidates
}

/// One actual edge into a block: its source block, its arguments, and the
/// polarity when it is an arm of the source's conditional.
#[derive(Clone, Copy)]
struct Arrival<'machine> {
    from: BlockId,
    arguments: &'machine [ValueId],
    polarity: Option<bool>,
}

/// A join whose two arrivals descend from the two arms of one conditional on
/// an integer comparison.
struct Selection<'machine> {
    /// Each arrival's arguments with the polarity of the conditional arm it
    /// descends from.
    arms: [(&'machine [ValueId], bool); 2],
    comparison: Comparison,
}

/// `smaller <= larger` or `smaller < larger`, as sources.
#[derive(Clone, Copy)]
struct Order {
    smaller: ValueId,
    larger: ValueId,
    strict: bool,
}

/// The deciding comparison `left <= right` or `left < right`, as sources.
#[derive(Clone, Copy)]
struct Comparison {
    left: ValueId,
    right: ValueId,
    strict: bool,
}

impl Comparison {
    /// The order that holds on the arm taken with this polarity.
    fn holds(self, polarity: bool) -> Order {
        if polarity {
            Order {
                smaller: self.left,
                larger: self.right,
                strict: self.strict,
            }
        } else {
            Order {
                smaller: self.right,
                larger: self.left,
                strict: !self.strict,
            }
        }
    }
}

/// An integer interval; both ends lie within the value's carrier.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Interval {
    minimum: i128,
    maximum: i128,
}

impl Interval {
    fn carrier(integer: IntegerType) -> Option<Self> {
        Some(Self {
            minimum: integer_bound(integer.minimum_value())?,
            maximum: integer_bound(integer.maximum_value())?,
        })
    }

    fn hull(self, other: Self) -> Self {
        Self {
            minimum: self.minimum.min(other.minimum),
            maximum: self.maximum.max(other.maximum),
        }
    }

    /// `literal <= value` and `value <= literal` for each side tighter than
    /// the carrier.
    fn members(self, value: ValueId, integer: IntegerType) -> Vec<Proposition> {
        let Some(carrier) = Self::carrier(integer) else {
            return Vec::new();
        };
        let subject = ScalarTerm::value(value, ScalarType::Integer(integer));
        let literal = |bound: i128| {
            let value = match integer.sign() {
                IntegerSign::Signed => IntegerValue::Signed(bound),
                IntegerSign::Unsigned => IntegerValue::Unsigned(u128::try_from(bound).ok()?),
            };
            ScalarTerm::integer(integer, value).ok()
        };
        let mut members = Vec::new();
        if self.minimum > carrier.minimum
            && let Some(minimum) = literal(self.minimum)
        {
            members.push(Proposition::LessOrEqual(minimum, subject.clone()));
        }
        if self.maximum < carrier.maximum
            && let Some(maximum) = literal(self.maximum)
        {
            members.push(Proposition::LessOrEqual(subject, maximum));
        }
        members
    }
}

fn integer_bound(value: IntegerValue) -> Option<i128> {
    match value {
        IntegerValue::Signed(value) => Some(value),
        IntegerValue::Unsigned(value) => i128::try_from(value).ok(),
    }
}

/// One machine's definitions, parameters and arrivals, with memoized value
/// sources and intervals.
struct MachineValues<'machine> {
    blocks: BTreeMap<BlockId, &'machine Block>,
    definitions: BTreeMap<ValueId, (&'machine OperationKind, ScalarType)>,
    /// Block parameter value → `(block, position, type)`.
    parameters: BTreeMap<ValueId, (BlockId, usize, ScalarType)>,
    arrivals: BTreeMap<BlockId, Vec<Arrival<'machine>>>,
    sources: BTreeMap<ValueId, ValueId>,
    intervals: BTreeMap<ValueId, Option<Interval>>,
}

impl<'machine> MachineValues<'machine> {
    fn new(machine: &'machine TerminalMachine) -> Self {
        let mut values = Self {
            blocks: BTreeMap::new(),
            definitions: BTreeMap::new(),
            parameters: BTreeMap::new(),
            arrivals: BTreeMap::new(),
            sources: BTreeMap::new(),
            intervals: BTreeMap::new(),
        };
        for block in &machine.blocks {
            values.blocks.insert(block.id, block);
            for (position, parameter) in block.parameters.iter().enumerate() {
                values
                    .parameters
                    .insert(parameter.id, (block.id, position, parameter.scalar_type));
            }
            for operation in &block.operations {
                if let OperationResult::Scalar(result) = &operation.result {
                    values
                        .definitions
                        .insert(result.id, (&operation.kind, result.scalar_type));
                }
            }
            match &block.terminator {
                Terminator::Jump {
                    target, arguments, ..
                } => values.arrivals.entry(*target).or_default().push(Arrival {
                    from: block.id,
                    arguments,
                    polarity: None,
                }),
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for (successor, polarity) in [(when_true, true), (when_false, false)] {
                        values
                            .arrivals
                            .entry(successor.target)
                            .or_default()
                            .push(Arrival {
                                from: block.id,
                                arguments: &successor.arguments,
                                polarity: Some(polarity),
                            });
                    }
                }
                _ => {}
            }
        }
        values
    }

    /// The value `value` always equals: a block parameter every arrival binds
    /// from one common source resolves to that source.
    fn source(&mut self, value: ValueId) -> ValueId {
        self.source_within(value, &mut BTreeSet::new())
    }

    fn source_within(&mut self, value: ValueId, active: &mut BTreeSet<ValueId>) -> ValueId {
        if let Some(&source) = self.sources.get(&value) {
            return source;
        }
        let Some(&(block, position, _)) = self.parameters.get(&value) else {
            return value;
        };
        if active.len() >= MAXIMUM_WALK || !active.insert(value) {
            return value;
        }
        let arguments = self.arrivals.get(&block).map_or_else(Vec::new, |arrivals| {
            arrivals
                .iter()
                .map(|arrival| arrival.arguments.get(position).copied())
                .collect::<Vec<_>>()
        });
        let mut common = None;
        for argument in arguments {
            let Some(argument) = argument else {
                common = None;
                break;
            };
            let source = self.source_within(argument, active);
            if common.is_some_and(|common| common != source) {
                common = None;
                break;
            }
            common = Some(source);
        }
        active.remove(&value);
        let source = common.unwrap_or(value);
        self.sources.insert(value, source);
        source
    }

    /// The conditional block and polarity an arrival descends from, through
    /// blocks that perform no operation and have exactly one arrival.
    fn deciding_arm(&self, arrival: Arrival) -> Option<(BlockId, bool)> {
        let mut arrival = arrival;
        for _ in 0..MAXIMUM_WALK {
            if let Some(polarity) = arrival.polarity {
                return Some((arrival.from, polarity));
            }
            let block = self.blocks.get(&arrival.from)?;
            if !block.operations.is_empty() {
                return None;
            }
            let [previous] = self.arrivals.get(&arrival.from)?.as_slice() else {
                return None;
            };
            arrival = *previous;
        }
        None
    }

    fn selection(&mut self, header: BlockId) -> Option<Selection<'machine>> {
        let [first, second] = *self.arrivals.get(&header)?.as_slice() else {
            return None;
        };
        let (first_conditional, first_polarity) = self.deciding_arm(first)?;
        let (second_conditional, second_polarity) = self.deciding_arm(second)?;
        if first_conditional != second_conditional || first_polarity == second_polarity {
            return None;
        }
        let Terminator::Conditional { condition, .. } =
            &self.blocks.get(&first_conditional)?.terminator
        else {
            return None;
        };
        let condition = self.source(*condition);
        let (kind, _) = self.definitions.get(&condition)?;
        let (left, right, strict) = match kind {
            OperationKind::IntegerLessOrEqual { left, right } => (*left, *right, false),
            OperationKind::IntegerLessThan { left, right } => (*left, *right, true),
            _ => return None,
        };
        let comparison = Comparison {
            left: self.source(left),
            right: self.source(right),
            strict,
        };
        Some(Selection {
            arms: [
                (first.arguments, first_polarity),
                (second.arguments, second_polarity),
            ],
            comparison,
        })
    }

    /// The hull of the values the two arms forward at `position`, each
    /// narrowed by its arm's comparison outcome; `None` when both arms
    /// forward one common value, which is threaded rather than selected.
    fn selection_interval(
        &mut self,
        selection: &Selection,
        position: usize,
        integer: IntegerType,
    ) -> Option<Interval> {
        let mut forwarded = Vec::with_capacity(2);
        for (arguments, polarity) in selection.arms {
            let argument = *arguments.get(position)?;
            forwarded.push((self.source(argument), polarity));
        }
        if forwarded[0].0 == forwarded[1].0 {
            return None;
        }
        let mut hull: Option<Interval> = None;
        for (value, polarity) in forwarded {
            let interval = self.narrowed(value, selection.comparison.holds(polarity), integer)?;
            hull = Some(hull.map_or(interval, |hull| hull.hull(interval)));
        }
        hull
    }

    /// `value`'s interval narrowed by an order that holds where it is
    /// forwarded.
    fn narrowed(&mut self, value: ValueId, order: Order, integer: IntegerType) -> Option<Interval> {
        let mut interval = self.interval(value, integer)?;
        let step = i128::from(order.strict);
        if value == order.smaller {
            let larger = self.interval(order.larger, integer)?;
            interval.maximum = interval.maximum.min(larger.maximum.checked_sub(step)?);
        }
        if value == order.larger {
            let smaller = self.interval(order.smaller, integer)?;
            interval.minimum = interval.minimum.max(smaller.minimum.checked_add(step)?);
        }
        // An empty interval means the arm cannot be taken; keep the operand's
        // own interval rather than proposing from an impossible arm.
        if interval.minimum > interval.maximum {
            return self.interval(value, integer);
        }
        Some(interval)
    }

    /// The interval a source value lies in: a literal's point, an earlier
    /// selection result's hull, or else the carrier.
    fn interval(&mut self, value: ValueId, integer: IntegerType) -> Option<Interval> {
        let value = self.source(value);
        if let Some(&known) = self.intervals.get(&value) {
            return known;
        }
        // A selection that reaches itself through its own operands keeps the
        // carrier while it is being evaluated.
        let carrier = Interval::carrier(integer);
        self.intervals.insert(value, carrier);
        let interval = self.evaluate(value, integer).or(carrier);
        self.intervals.insert(value, interval);
        interval
    }

    fn evaluate(&mut self, value: ValueId, integer: IntegerType) -> Option<Interval> {
        if let Some(&(kind, scalar_type)) = self.definitions.get(&value) {
            if scalar_type != ScalarType::Integer(integer) {
                return None;
            }
            let OperationKind::IntegerConstant { value } = kind else {
                return None;
            };
            let point = integer_bound(*value)?;
            return Some(Interval {
                minimum: point,
                maximum: point,
            });
        }
        let &(block, position, scalar_type) = self.parameters.get(&value)?;
        if scalar_type != ScalarType::Integer(integer) {
            return None;
        }
        let selection = self.selection(block)?;
        self.selection_interval(&selection, position, integer)
    }
}

#[cfg(test)]
mod tests;
