//! A conditional edge provides a necessary bound only when every route to
//! the operation traverses that edge. Guard operands use the same checked
//! value correspondence as the arithmetic itself, at the branch's own cut.

use super::*;
use terminal_psi::Terminator;

pub(super) struct Branch {
    entered: BlockId,
    condition: ValueId,
    at: Point,
    when_true: bool,
}

pub(super) fn branches(machine: &TerminalMachine) -> Vec<Branch> {
    let mut targets = machine
        .blocks
        .iter()
        .flat_map(|block| block.terminator.successor_edges())
        .map(|(_, target)| target)
        .collect::<Vec<_>>();
    targets.sort_unstable();
    let mut result = Vec::new();
    for block in &machine.blocks {
        let Terminator::Conditional {
            condition,
            when_true,
            when_false,
        } = &block.terminator
        else {
            continue;
        };
        for (taken, edge) in [(true, when_true), (false, when_false)] {
            let start = targets.partition_point(|target| *target < edge.target);
            if edge.target == machine.entry || targets.get(start + 1) == Some(&edge.target) {
                continue;
            }
            result.push(Branch {
                entered: edge.target,
                condition: *condition,
                at: (block.id, block.operations.len()),
                when_true: taken,
            });
        }
    }
    result
}

impl GroundRange<'_, '_> {
    pub(super) fn guard_indicators(&self, block: BlockId, branches: &[Branch]) -> Vec<Step> {
        if block == self.index.machine().entry || branches.is_empty() {
            return Vec::new();
        }
        let mut ground = GroundRange {
            module: self.module,
            index: self.index,
            storage: self.storage,
            dominators: self.dominators,
            active: Vec::new(),
            steps: Vec::new(),
            remaining: 4096,
        };
        let Some(tree) = self.tree() else {
            return Vec::new();
        };
        for branch in branches
            .iter()
            .filter(|branch| tree.dominates(branch.entered, block))
        {
            let start = ground.steps.len();
            if ground
                .indicator(branch.condition, branch.at, branch.when_true, 0)
                .is_some()
            {
                ground.steps.push(Step::Multiply);
            } else {
                // An unknown runtime guard supplies no restriction.
                ground.steps.truncate(start);
            }
        }
        ground.steps
    }

    fn indicator(
        &mut self,
        condition: ValueId,
        at: Point,
        positive: bool,
        depth: usize,
    ) -> Option<()> {
        self.visit(depth)?;
        let (source, position) = self.definition(condition, at)?;
        let operation = &source.operations[position];
        let cut = (source.id, position);
        let (left, right, comparison, positive) = match operation.kind {
            OperationKind::BooleanNot { operand } => {
                return self.indicator(operand, cut, !positive, depth + 1);
            }
            OperationKind::BooleanEqual { left, right } => {
                for (tested, constant) in [(left, right), (right, left)] {
                    if let Some((owner, pos)) = self.definition(constant, cut)
                        && let OperationKind::BooleanConstant { value } = owner.operations[pos].kind
                    {
                        return self.indicator(tested, cut, value == positive, depth + 1);
                    }
                }
                return None;
            }
            OperationKind::IntegerEqual { left, right } => (left, right, Step::Equal, positive),
            OperationKind::IntegerLessThan { left, right } => {
                (left, right, Step::LessThan, positive)
            }
            OperationKind::IntegerLessOrEqual { left, right } => {
                (right, left, Step::LessThan, !positive)
            }
            _ => return None,
        };
        if !positive {
            self.steps
                .push(Step::Observation(ScalarRangeObservation::Literal(
                    BigInt::from_u64(1),
                )));
        }
        let ty = self.index.value_type(left)?;
        self.value(left, ty, cut, depth + 1)?;
        self.value(right, ty, cut, depth + 1)?;
        self.steps.push(comparison);
        if !positive {
            self.steps.push(Step::Subtract);
        }
        Some(())
    }
}
