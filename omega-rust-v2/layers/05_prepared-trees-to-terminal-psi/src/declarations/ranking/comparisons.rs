//! Select an edge obligation, not a termination judgment. In particular a
//! different value id says nothing about order. Only use the actual operands
//! and comparisons on paths that must have been taken to reach this call.
use crate::{
    ArithmeticPolicy, BlockId, BlockTerminator, IntegerPrimitive, MachineBody, OperationKind, RankComparison, RankExpression, RankOperand, ScalarLiteral, ValueId,
    ValueOrigin,
};
use std::cmp::Ordering::{self, Equal, Greater, Less};

#[derive(Clone, Copy)]
enum Arrival {
    None,
    One(BlockId, Option<(ValueId, bool)>),
    Multiple,
}

pub(super) struct Comparisons<'a> {
    body: &'a MachineBody,
    arrivals: Vec<Arrival>,
}

impl<'a> Comparisons<'a> {
    /// Called after the ranked body's block graph has been checked acyclic.
    pub(super) fn new(body: &'a MachineBody) -> Self {
        let mut arrivals = vec![Arrival::None; body.blocks.len()];
        for block in &body.blocks {
            let mut arrive = |target: BlockId, guard| {
                let arrival = &mut arrivals[target.get() - 1];
                *arrival = match arrival {
                    Arrival::None => Arrival::One(block.id, guard),
                    _ => Arrival::Multiple,
                };
            };
            match &block.terminator {
                BlockTerminator::Jump(edge) => arrive(edge.target, None),
                BlockTerminator::Conditional {
                    condition,
                    when_true,
                    when_false,
                } => {
                    arrive(when_true.target, Some((*condition, true)));
                    arrive(when_false.target, Some((*condition, false)));
                }
                _ => {}
            }
        }
        Self { body, arrivals }
    }

    fn definition(
        &self,
        v: ValueId,
    ) -> Option<&OperationKind> {
        let ValueOrigin::Operation(def) = self.body.values.get(v.index())?.origin else {
            return None;
        };
        Some(&self.body.operations.get(def.index())?.kind)
    }

    /// An exact positive step is an equation on the successful result. Its
    /// independent arithmetic safety obligation is still on the operation.
    fn positive_step(
        &self,
        next: ValueId,
        source: ValueId,
        primitive: IntegerPrimitive,
    ) -> bool {
        let Some(OperationKind::Integer { operation, operands }) = self.definition(next) else {
            return false;
        };
        operation.primitive == primitive
            && operation.policy == ArithmeticPolicy::Exact
            && matches!(operands[..], [lhs, rhs] if lhs == source && matches!(self.definition(rhs),
                Some(OperationKind::Constant(ScalarLiteral::Integer(n))) if n.sign() == num_bigint::Sign::Plus))
    }

    /// Every structurally bound edge asks a comparison question. Recognizing
    /// a specific obligation is optional; failure to recognize one must not
    /// drop the edge or guess descent from a change of operand identity.
    pub(super) fn comparison(
        &self,
        block: BlockId,
        source: &RankExpression<ValueId>,
        successor: &RankExpression<ValueId>,
    ) -> RankComparison {
        use RankExpression::{ExactDifference, Operand, SaturatingDistance};
        use RankOperand::Input;
        // Only an input value can carry a step or a guard. A constant
        // operand that differs between source and successor stays pending.
        if successor == source {
            return RankComparison::Preserving;
        }
        match (source, successor) {
            (Operand(Input(prev)), Operand(Input(next))) if self.positive_step(*next, *prev, IntegerPrimitive::Subtract) => RankComparison::Strict,
            (Operand(Input(prev)), Operand(Input(next))) => self.guarded_role(block, *next, *prev).unwrap_or(RankComparison::Pending),
            (
                SaturatingDistance {
                    cursor: Input(cursor),
                    limit: Input(limit),
                },
                SaturatingDistance {
                    cursor: Input(next),
                    limit: Input(same_limit),
                },
            ) if limit == same_limit && self.positive_step(*next, *cursor, IntegerPrimitive::Add) => {
                // At or beyond the limit both distances can be zero. Do not
                // infer strict distance descent from the cursor step alone.
                if self.guarded_role(block, *cursor, *limit) == Some(RankComparison::Strict) {
                    RankComparison::Strict
                } else if cursor == limit || self.guarded_role(block, *limit, *cursor).is_some() {
                    RankComparison::Preserving
                } else {
                    RankComparison::Pending
                }
            }
            // An exact difference has no floor at zero: wherever the successor
            // rank is defined, a positive step on one side with the other side
            // unchanged makes it strictly smaller. Its definedness stays a
            // checker question on the same subtraction.
            (
                ExactDifference {
                    minuend,
                    subtrahend: Input(prev),
                },
                ExactDifference {
                    minuend: same,
                    subtrahend: Input(next),
                },
            ) if minuend == same && self.positive_step(*next, *prev, IntegerPrimitive::Add) => RankComparison::Strict,
            (
                ExactDifference {
                    minuend: Input(prev),
                    subtrahend,
                },
                ExactDifference {
                    minuend: Input(next),
                    subtrahend: same,
                },
            ) if subtrahend == same && self.positive_step(*next, *prev, IntegerPrimitive::Subtract) => RankComparison::Strict,
            _ => RankComparison::Pending,
        }
    }

    fn guarded_role(
        &self,
        mut block: BlockId,
        next: ValueId,
        prev: ValueId,
    ) -> Option<RankComparison> {
        // Stop at joins rather than borrowing one predecessor's facts. Use
        // all recognized guards on this single-predecessor chain, independent
        // of arm order and of which operand was written first.
        let mut possible = [true; 3];
        while let Arrival::One(parent, guard) = self.arrivals[block.get() - 1] {
            block = parent;
            let Some((condition, taken)) = guard else { continue };
            let Some(OperationKind::Integer { operation, operands }) = self.definition(condition) else {
                continue;
            };
            let reversed = if operands.as_slice() == [next, prev] {
                false
            } else if operands.as_slice() == [prev, next] {
                true
            } else {
                continue;
            };
            for (allow, order) in possible.iter_mut().zip([Less, Equal, Greater]) {
                let order = if reversed { order.reverse() } else { order };
                if let Some(test) = compare(operation.primitive, order) {
                    *allow &= test == taken;
                }
            }
        }
        match possible {
            [true, false, false] => Some(RankComparison::Strict),
            [_, true, false] => Some(RankComparison::Preserving),
            _ => None,
        }
    }
}

fn compare(
    op: IntegerPrimitive,
    order: Ordering,
) -> Option<bool> {
    Some(match op {
        IntegerPrimitive::Equal => order == Equal,
        IntegerPrimitive::NotEqual => order != Equal,
        IntegerPrimitive::LessThan => order == Less,
        IntegerPrimitive::LessOrEqual => order != Greater,
        IntegerPrimitive::GreaterThan => order == Greater,
        IntegerPrimitive::GreaterOrEqual => order != Less,
        _ => return None,
    })
}
