//! Run selected passes on the untouched, unsealed input of the first judgment.
//! Only the executable closure is transformed. Its proof tables, the generic
//! forms and every conformance witness go through the ordinary checker again.
use ::optimization::{PsiOptimization, PsiOptimizationSelections};
use diagnostics::Diagnostic;
use semantic_vocabulary::{BlockId, IntegerValue, ValueId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_check_tables::compilation::TerminalCompilation;
use terminal_psi::{Block, OperationKind, TerminalMachine, Terminator};

pub(super) fn apply(
    mut program: TerminalCompilation,
    selections: &PsiOptimizationSelections,
) -> Result<TerminalCompilation, Vec<Diagnostic>> {
    let TerminalCompilation::Prepared(prepared) = &mut program else {
        return Err(vec![Diagnostic::error(
            "compiler defect: accepted Psi has no lowered compilation",
        )]);
    };
    let closure = prepared.closure.as_mut().map_err(|_| {
        vec![Diagnostic::error(
            "compiler defect: accepted Psi has no executable closure",
        )]
    })?;
    for &pass in selections.as_slice() {
        match pass {
            PsiOptimization::SparseConditionalConstantPropagation => {
                for machine in &mut closure.module.machines {
                    propagate_constants(machine);
                }
            }
            other => {
                return Err(vec![Diagnostic::error(format!(
                    "not supported yet: source Psi optimization `{}` has no implemented pass",
                    other.name()
                ))]);
            }
        }
    }
    Ok(program)
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Fact {
    /// No executable predecessor or definition has supplied this value yet.
    #[default]
    Unseen,
    Boolean(bool),
    Integer(IntegerValue),
    Varying,
}

impl Fact {
    fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unseen, value) | (value, Self::Unseen) => value,
            (left, right) if left == right => left,
            _ => Self::Varying,
        }
    }
}

/// Track executable predecessors and constants together. A new predecessor
/// can invalidate an earlier constant. Its readers must run again before
/// any operation is rewritten.
#[derive(Default)]
struct Propagation<'a> {
    blocks: BTreeMap<BlockId, &'a Block>,
    reached: BTreeSet<BlockId>,
    pending: BTreeSet<BlockId>,
    values: BTreeMap<ValueId, Fact>,
    readers: BTreeMap<ValueId, BTreeSet<BlockId>>,
}

impl Propagation<'_> {
    fn read(&mut self, block: BlockId, value: ValueId) -> Fact {
        self.readers.entry(value).or_default().insert(block);
        self.values.get(&value).copied().unwrap_or(Fact::Varying)
    }

    fn merge(&mut self, value: ValueId, incoming: Fact) {
        let old = self.values.entry(value).or_default();
        let joined = old.join(incoming);
        if joined != *old {
            *old = joined;
            if let Some(readers) = self.readers.get(&value) {
                self.pending.extend(readers);
            }
        }
    }

    fn reach(&mut self, target: BlockId) {
        if self.reached.insert(target) {
            self.pending.insert(target);
        }
    }

    fn arrive(&mut self, from: BlockId, target: BlockId, arguments: Option<&[ValueId]>) {
        self.reach(target);
        let destination = self.blocks[&target];
        for (position, formal) in destination.parameters.iter().enumerate() {
            let incoming = arguments
                .and_then(|args| args.get(position))
                .map(|&argument| self.read(from, argument))
                .unwrap_or(Fact::Varying);
            self.merge(formal.id, incoming);
        }
    }

    fn evaluate(&mut self, block: BlockId, operation: &OperationKind) -> Fact {
        use OperationKind as Op;
        match *operation {
            Op::BooleanConstant { value } => Fact::Boolean(value),
            Op::IntegerConstant { value } => Fact::Integer(value),
            Op::BooleanNot { operand } => match self.read(block, operand) {
                Fact::Boolean(value) => Fact::Boolean(!value),
                Fact::Unseen => Fact::Unseen,
                _ => Fact::Varying,
            },
            Op::BooleanEqual { left, right }
            | Op::IntegerEqual { left, right }
            | Op::IntegerLessThan { left, right }
            | Op::IntegerLessOrEqual { left, right } => {
                let ordering = match (self.read(block, left), self.read(block, right)) {
                    (Fact::Varying, _) | (_, Fact::Varying) => return Fact::Varying,
                    (Fact::Unseen, _) | (_, Fact::Unseen) => return Fact::Unseen,
                    (Fact::Boolean(left), Fact::Boolean(right)) => left.cmp(&right),
                    (
                        Fact::Integer(IntegerValue::Signed(left)),
                        Fact::Integer(IntegerValue::Signed(right)),
                    ) => left.cmp(&right),
                    (
                        Fact::Integer(IntegerValue::Unsigned(left)),
                        Fact::Integer(IntegerValue::Unsigned(right)),
                    ) => left.cmp(&right),
                    _ => return Fact::Varying,
                };
                Fact::Boolean(match operation {
                    Op::IntegerLessThan { .. } => ordering.is_lt(),
                    Op::IntegerLessOrEqual { .. } => ordering.is_le(),
                    _ => ordering.is_eq(),
                })
            }
            _ => Fact::Varying,
        }
    }

    fn visit(&mut self, id: BlockId) {
        let block = self.blocks[&id];
        for operation in &block.operations {
            if let Some(result) = operation.result.scalar() {
                let value = self.evaluate(id, &operation.kind);
                self.merge(result.id, value);
            }
        }
        match &block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } => self.arrive(id, *target, Some(arguments)),
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                let value = self.read(id, *condition);
                for (required, edge) in [(true, when_true), (false, when_false)] {
                    if value == Fact::Varying || value == Fact::Boolean(required) {
                        self.arrive(id, edge.target, Some(&edge.arguments));
                    }
                }
            }
            // Case payloads depend on the tested storage. Every case may run.
            other => {
                for target in other.successor_targets() {
                    self.arrive(id, target, None);
                }
            }
        }
    }
}

fn propagate_constants(machine: &mut TerminalMachine) {
    if machine.blocks.is_empty() {
        return;
    }
    let values = {
        let mut propagation = Propagation {
            blocks: machine
                .blocks
                .iter()
                .map(|block| (block.id, block))
                .collect(),
            values: machine
                .operations()
                .filter_map(|op| op.result.scalar())
                .chain(machine.block_parameters().copied())
                .map(|value| (value.id, Fact::Unseen))
                .collect(),
            ..Default::default()
        };
        propagation.reach(machine.entry);
        let entry = propagation.blocks[&machine.entry];
        for value in machine.parameters.iter().chain(&entry.parameters) {
            propagation.merge(value.id, Fact::Varying);
        }
        while let Some(block) = propagation.pending.pop_first() {
            propagation.visit(block);
        }
        propagation.values
    };
    for block in &mut machine.blocks {
        for operation in &mut block.operations {
            let Some(result) = operation.result.scalar() else {
                continue;
            };
            if let Some(&Fact::Boolean(value)) = values.get(&result.id) {
                // Only a pure comparison or negation can infer a new Boolean.
                // Keep operation and result identities so all uses, source
                // correspondences and operation-local check rows retain their site.
                operation.kind = OperationKind::BooleanConstant { value };
            }
        }
    }
}
