//! Project a read-only retry loop onto the comparisons that can take its
//! backedge. Straight-line computations and short-circuit joins are visited
//! once. The retry must carry the original borrowed resources to its header.

mod predicate;
use super::{Comparison, Facts, Scalar, SpinCondition, Subject, Subjects, bind, constant};
use predicate::{ALWAYS, Guard, NEVER};
pub(in super::super) use predicate::{Observation, Predicate};
use semantic_vocabulary::{BlockId, ValueId};
use std::collections::{BTreeMap, VecDeque};
use terminal_psi::{
    AtomicAccessEvent, Block, Operation, OperationKind, StructuralAccess, StructuralArgument,
    TerminalMachine, Terminator,
};

#[derive(Clone, PartialEq, Eq)]
enum Word {
    Literal(Scalar),
    Cell(Subject),
    Boolean(Guard),
}

struct Analysis<'a> {
    module: &'a terminal_psi::TerminalModule,
    machine: &'a TerminalMachine,
    header: BlockId,
    predicate: Predicate,
    resources: Subjects,
    words: Facts<ValueId, Word>,
    reached: Facts<BlockId, Guard>,
    returned: bool,
    observations: usize,
}

impl Analysis<'_> {
    fn boolean(&mut self, word: &Word) -> Option<Guard> {
        match word {
            Word::Boolean(value) => Some(*value),
            Word::Literal(Scalar::Boolean(value)) => Some(if *value { ALWAYS } else { NEVER }),
            Word::Cell(subject) => Some(self.predicate.observe(
                subject.clone(),
                SpinCondition {
                    comparison: Comparison::equal(Scalar::Boolean(true)),
                    positive: true,
                },
            )),
            _ => None,
        }
    }

    fn operation(&mut self, operation: &Operation) -> Option<()> {
        if operation.declared_invocation().is_some() {
            return super::inert_unit_call(self.module, operation).then_some(());
        }
        let target = operation.result.scalar()?.id;
        let word = if let Some(value) = constant(operation) {
            Word::Literal(value)
        } else {
            match &operation.kind {
                OperationKind::AtomicAccess {
                    place,
                    path,
                    event: AtomicAccessEvent::Load { .. },
                    resident_obligation: None,
                } => {
                    self.observations += 1;
                    Word::Cell(self.resources.subject(*place, path)?)
                }
                OperationKind::BooleanNot { operand } => {
                    let word = self.words.get(operand)?.clone();
                    Word::Boolean(!self.boolean(&word)?)
                }
                kind => {
                    let (a, b, order, positive) = Comparison::operands(kind)?;
                    let left = self.words.get(&a)?.clone();
                    let right = self.words.get(&b)?.clone();
                    let (word, pivot, order) = match (left, right) {
                        (word, Word::Literal(pivot)) => (word, pivot, order),
                        (Word::Literal(pivot), word) => (word, pivot, order.reverse()),
                        _ => return None,
                    };
                    let condition = SpinCondition {
                        comparison: Comparison { pivot, order },
                        positive,
                    };
                    let result = match word {
                        Word::Cell(subject) => self.predicate.observe(subject, condition),
                        Word::Literal(value) => {
                            if condition.matches(value)? {
                                ALWAYS
                            } else {
                                NEVER
                            }
                        }
                        Word::Boolean(value) if order.is_eq() => {
                            let Scalar::Boolean(bit) = pivot else {
                                return None;
                            };
                            if positive == bit { value } else { !value }
                        }
                        _ => return None,
                    };
                    Word::Boolean(result)
                }
            }
        };
        bind(&mut self.words, target, word)
    }

    fn edge(
        &mut self,
        guard: Guard,
        target: BlockId,
        scalars: &[ValueId],
        places: &[StructuralArgument],
        repeat: bool,
    ) -> Option<()> {
        let block = self.machine.block(&target)?;
        if scalars.len() != block.parameters.len()
            || places.len() != block.structural_parameters.len()
        {
            return None;
        }
        for (formal, arg) in block.structural_parameters.iter().zip(places) {
            if formal.access != StructuralAccess::SharedBorrow || arg.access != formal.access {
                return None;
            }
            let subject = self.resources.subject(arg.place, &arg.path)?;
            bind(&mut self.resources.places, formal.place, subject)?;
        }
        if repeat && target == self.header {
            if !scalars.is_empty() {
                return None;
            }
            self.predicate.retry = self.predicate.either(self.predicate.retry, guard);
            return Some(());
        }
        let prior = self.reached.get(&target).copied().unwrap_or(NEVER);
        for (formal, arg) in block.parameters.iter().zip(scalars) {
            let incoming = self.words.get(arg)?.clone();
            let merged = if let Some(previous) = self.words.get(&formal.id).cloned()
                && previous != incoming
            {
                let before = self.boolean(&previous)?;
                let after = self.boolean(&incoming)?;
                Word::Boolean(self.predicate.select(guard, after, before))
            } else {
                incoming
            };
            self.words.insert(formal.id, merged);
        }
        self.reached
            .insert(target, self.predicate.either(prior, guard));
        Some(())
    }

    fn block(&mut self, block: &Block) -> Option<()> {
        let entered = *self.reached.get(&block.id)?;
        for op in &block.operations {
            self.operation(op)?;
        }
        match &block.terminator {
            Terminator::ReturnUnit {
                cleanup_actions, ..
            } if cleanup_actions.is_empty() => self.returned = true,
            Terminator::Jump {
                target,
                arguments,
                structural_arguments,
                cleanup_actions,
                ..
            } if cleanup_actions.is_empty() => {
                self.edge(entered, *target, arguments, structural_arguments, true)?;
            }
            Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                let word = self.words.get(condition)?.clone();
                let test = self.boolean(&word)?;
                for (branch, edge) in [(test, when_true), (!test, when_false)] {
                    if !edge.cleanup_actions.is_empty() {
                        return None;
                    }
                    let guard = self.predicate.both(entered, branch);
                    self.edge(
                        guard,
                        edge.target,
                        &edge.arguments,
                        &edge.structural_arguments,
                        true,
                    )?;
                }
            }
            _ => return None,
        }
        Some(())
    }
}

pub(super) fn derive(
    module: &terminal_psi::TerminalModule,
    machine: &TerminalMachine,
) -> Option<Predicate> {
    let entry = machine.block(&machine.entry)?;
    let header = match &entry.terminator {
        Terminator::Jump {
            target,
            cleanup_actions,
            ..
        } if entry.operations.is_empty() && cleanup_actions.is_empty() => *target,
        _ => machine.entry,
    };
    let mut analysis = Analysis {
        module,
        machine,
        header,
        predicate: Predicate::new(),
        resources: Subjects::at_entry(machine),
        words: Facts::default(),
        reached: Facts::default(),
        observations: 0,
        returned: false,
    };
    if header != machine.entry {
        let Terminator::Jump {
            arguments,
            structural_arguments,
            ..
        } = &entry.terminator
        else {
            return None;
        };
        analysis.edge(ALWAYS, header, arguments, structural_arguments, false)?;
    } else {
        analysis.reached.insert(header, ALWAYS);
    }
    // Count reachable arrivals after cutting only backedges to the original
    // header. Any other cycle or changing resource falls outside this summary.
    let mut incoming = BTreeMap::from([(header, 0usize)]);
    let mut pending = vec![header];
    while let Some(id) = pending.pop() {
        for (_, target) in machine.block(&id)?.terminator.successor_edges() {
            if target == header {
                continue;
            }
            let count = incoming.entry(target).or_default();
            if *count == 0 {
                pending.push(target);
            }
            *count += 1;
        }
    }
    let mut ready = VecDeque::from([header]);
    let mut done = 0;
    while let Some(id) = ready.pop_front() {
        let block = machine.block(&id)?;
        analysis.block(block)?;
        done += 1;
        for (_, target) in block.terminator.successor_edges() {
            if target == header {
                continue;
            }
            let count = incoming.get_mut(&target)?;
            *count -= 1;
            if *count == 0 {
                ready.push_back(target);
            }
        }
    }
    (done == incoming.len()
        && analysis.returned
        && analysis.observations > 0
        && analysis.predicate.retry != NEVER
        && analysis.predicate.retry != ALWAYS)
        .then_some(analysis.predicate)
}
