//! A selected provider can acquire locks, write through a borrowed argument, or
//! run a destructor that releases locks. Translate the proven normal-return
//! effects in execution order before the caller makes its next observation.

mod calls;
mod guards;
mod receive;
mod sequence;
use crate::synchronization::progress::condition::Selector;

use super::activation::Bindings;
use super::storage::{Storage, Stored};
use crate::entry_values::TerminalScalarValue as Scalar;
use crate::synchronization::progress::{ProgressFailure, ProgressScope, ScalarFact};
use crate::synchronization::spin::{CellUpdate, Operand, SpinCondition, Subject};
pub(in crate::synchronization) use calls::normal_effects;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Acquisition {
    subject: Subject,
    held: Scalar,
    blocked: SpinCondition,
    update: CellUpdate<ScalarFact>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AtomicWrite {
    subject: Subject,
    value: Operand,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Effect {
    Acquire(Acquisition),
    Write(AtomicWrite),
}

/// Every retained alternative is a complete normal return. Its conditions
/// refer only to scalar inputs, and its events name the borrowed words as
/// observed in that alternative's execution order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalEffects {
    paths: Vec<NormalReturn>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct NormalReturn {
    when: Vec<Decision>,
    events: Vec<Effect>,
    scalar_result: Option<Operand>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Decision {
    selector: Selector,
    answer: bool,
}

impl Decision {
    fn new(selector: Selector, answer: bool) -> Self {
        let (selector, inverted) = selector.canonical();
        Self {
            selector,
            answer: answer != inverted,
        }
    }

    fn answer_for(&self, test: Selector) -> Option<bool> {
        let (selector, inverted) = test.canonical();
        (selector == self.selector).then_some(self.answer != inverted)
    }
}

impl Default for NormalEffects {
    fn default() -> Self {
        Self {
            paths: vec![NormalReturn::default()],
        }
    }
}

impl NormalReturn {
    fn compact_stores(&mut self) {
        let mut later = std::collections::BTreeSet::new();
        self.events.reverse();
        self.events.retain(|effect| match effect {
            Effect::Write(write) => write
                .subject
                .fixed_path()
                .is_none_or(|path| later.insert((write.subject.parameter, path))),
            Effect::Acquire(_) => {
                later.clear();
                true
            }
        });
        self.events.reverse();
    }

    fn assume(&mut self, condition: Decision) -> bool {
        if let Some(literal) = condition.selector.answer(&|_| None, &Default::default()) {
            return literal == condition.answer;
        }
        if let Some(previous) = self
            .when
            .iter()
            .find(|guard| guard.selector == condition.selector)
        {
            return previous.answer == condition.answer;
        }
        self.when.push(condition);
        true
    }

    fn size(&self) -> usize {
        1 + self.events.len() + self.when.len()
    }
}

impl NormalEffects {
    /// Decide only choices that can change the scalar forwarded by the caller.
    /// The enclosing analysis records these same guards on its returned paths.
    fn returned_scalar(
        &mut self,
        decide: &mut impl FnMut(Selector) -> Option<bool>,
    ) -> Option<Operand> {
        let first = self.paths.first()?.scalar_result?;
        if self
            .paths
            .iter()
            .all(|path| path.scalar_result == Some(first))
        {
            return Some(first);
        }
        let mut feasible = Vec::new();
        for path in std::mem::take(&mut self.paths) {
            let mut selected = true;
            for guard in &path.when {
                let answer = guard
                    .selector
                    .answer(&|_| None, &Default::default())
                    .or_else(|| decide(guard.selector))?;
                if answer != guard.answer {
                    selected = false;
                    break;
                }
            }
            if selected {
                feasible.push(path);
            }
        }
        self.paths = feasible;
        let result = self.paths.first()?.scalar_result?;
        self.paths
            .iter()
            .all(|path| path.scalar_result == Some(result))
            .then_some(result)
    }

    fn none() -> Self {
        Self { paths: Vec::new() }
    }

    fn assume(&mut self, condition: Decision) {
        self.paths.retain_mut(|path| path.assume(condition));
    }

    fn push(&mut self, effect: Effect) {
        for path in &mut self.paths {
            path.events.push(effect.clone());
        }
    }

    fn compact_stores(&mut self) {
        for path in &mut self.paths {
            path.compact_stores();
        }
    }

    fn add_returns(&mut self, mut other: Self, limit: usize) -> Option<()> {
        self.paths.append(&mut other.paths);
        (self.paths.len() <= limit).then_some(())
    }

    fn append_call(&mut self, callee: Self, program_size: usize) -> Option<()> {
        let limit = program_size
            .saturating_add(1)
            .max(callee.paths.iter().map(NormalReturn::size).sum());
        let mut results = Vec::new();
        let mut size = 0;
        for prefix in &self.paths {
            for suffix in &callee.paths {
                let mut path = prefix.clone();
                if !suffix.when.iter().all(|condition| path.assume(*condition)) {
                    continue;
                }
                path.events.extend_from_slice(&suffix.events);
                if size + path.size() > limit {
                    path.compact_stores();
                }
                size += path.size();
                if size > limit {
                    return None;
                }
                results.push(path);
            }
        }
        self.paths = results;
        Some(())
    }

    pub fn requires_progress(&self) -> bool {
        self.paths
            .iter()
            .flat_map(|p| &p.events)
            .any(|e| matches!(e, Effect::Acquire(_)))
    }

    pub(in crate::synchronization) fn map_operands(
        &mut self,
        mut apply: impl FnMut(Operand) -> Option<Operand>,
    ) -> Option<()> {
        for path in &mut self.paths {
            if let Some(value) = &mut path.scalar_result {
                *value = apply(*value)?;
            }
            for condition in &mut path.when {
                condition.selector.map_operands(&mut apply)?;
                *condition = Decision::new(condition.selector, condition.answer);
            }
            for event in &mut path.events {
                if let Effect::Write(write) = event {
                    write.value = apply(write.value)?;
                }
            }
        }
        Some(())
    }

    pub(in crate::synchronization) fn map_subjects(
        &mut self,
        mut apply: impl FnMut(&mut Subject) -> Option<()>,
    ) -> Option<()> {
        for path in &mut self.paths {
            for event in &mut path.events {
                apply(match event {
                    Effect::Acquire(acquisition) => &mut acquisition.subject,
                    Effect::Write(write) => &mut write.subject,
                })?;
            }
        }
        Some(())
    }
}
