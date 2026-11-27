//! Evaluate each checked scalar equation once. Named values in an equation
//! refer to its immediate inputs only; reusing a result never copies its tree.
mod helpers;

use super::{Observation, Operand, Scalar, Subject};
use crate::synchronization::progress::ScalarFact;
use semantic_vocabulary::{ScalarTerm, ScalarType, ValueId};
use terminal_psi::Operation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Input {
    Supplied(Operand),
    Previous(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Step {
    Load(Subject, ScalarType),
    UnresolvedCall {
        reads_cell: bool,
    },
    Apply {
        term: ScalarTerm,
        can_trap: bool,
        inputs: Vec<(ValueId, Input)>,
        reads_cell: bool,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Expressions {
    steps: Vec<Step>,
}

type ResultWord = Result<Scalar, Observation>;

impl Expressions {
    pub(super) fn load(&mut self, subject: Subject, ty: ScalarType) -> usize {
        self.steps.push(Step::Load(subject, ty));
        self.steps.len() - 1
    }

    pub(super) fn reads_cell(&self, root: usize) -> bool {
        match &self.steps[root] {
            Step::Load(..) => true,
            Step::Apply { reads_cell, .. } | Step::UnresolvedCall { reads_cell } => *reads_cell,
        }
    }

    pub(super) fn apply(
        &mut self,
        operation: &Operation,
        types: &dyn crate::ValueTypes,
        input: impl Fn(ValueId) -> Option<Input>,
    ) -> Option<usize> {
        let crate::ScalarConstantOrigin::Computed(term) =
            crate::scalar_operation_origin(operation, operation.result.scalar()?.id, types)?
        else {
            return None;
        };
        let mut inputs = Vec::new();
        let mut reads_cell = false;
        if !term.visit_value_ids(|id| {
            let Some(value) = input(id) else { return false };
            if let Input::Previous(index) = value {
                reads_cell |= self.reads_cell(index);
            }
            inputs.push((id, value));
            true
        }) {
            return None;
        }
        self.steps.push(Step::Apply {
            term,
            can_trap: matches!(
                operation.kind,
                terminal_psi::OperationKind::TrappingInteger { .. }
            ),
            inputs,
            reads_cell,
        });
        Some(self.steps.len() - 1)
    }

    fn unresolved_call(&mut self, arguments: Vec<Input>) -> Input {
        let reads_cell = arguments
            .into_iter()
            .any(|input| matches!(input, Input::Previous(i) if self.reads_cell(i)));
        self.steps.push(Step::UnresolvedCall { reads_cell });
        Input::Previous(self.steps.len() - 1)
    }

    pub(super) fn subjects(&mut self) -> impl Iterator<Item = &mut Subject> {
        self.steps.iter_mut().filter_map(|step| match step {
            Step::Load(subject, _) => Some(subject),
            Step::Apply { .. } | Step::UnresolvedCall { .. } => None,
        })
    }

    pub(super) fn substitute(&mut self, map: impl Fn(Operand) -> Option<Operand>) -> Option<()> {
        for step in &mut self.steps {
            if let Step::Apply { inputs, .. } = step {
                for (_, input) in inputs {
                    if let Input::Supplied(value) = input {
                        *value = map(*value)?;
                    }
                }
            }
        }
        Some(())
    }

    pub(super) fn evaluate(
        &self,
        cell: impl Fn(&Subject, ScalarType) -> ResultWord,
        formal: impl Fn(Operand) -> Option<Scalar>,
    ) -> Result<Vec<ResultWord>, Observation> {
        let mut results: Vec<ResultWord> = Vec::with_capacity(self.steps.len());
        for step in &self.steps {
            let answer = match step {
                Step::Load(subject, ty) => cell(subject, *ty),
                Step::UnresolvedCall { .. } => return Err(Observation::Unproved),
                Step::Apply {
                    term,
                    inputs,
                    can_trap,
                    ..
                } => {
                    let mut bindings = Vec::with_capacity(inputs.len());
                    let mut unknown: Option<Observation> = None;
                    for (id, input) in inputs {
                        let value = match input {
                            Input::Previous(index) => results[*index],
                            Input::Supplied(operand) => {
                                formal(*operand).ok_or(Observation::Unproved)
                            }
                        };
                        match value {
                            Ok(literal) => bindings.push((*id, literal)),
                            Err(cause) => {
                                unknown = Some(unknown.map_or(cause, |old| old.and(cause)))
                            }
                        }
                    }
                    let answer = match unknown {
                        Some(cause) => Err(cause),
                        None => evaluate(term, &bindings).ok_or(Observation::Unproved),
                    };
                    if *can_trap && answer.is_err() {
                        return Err(Observation::Unproved);
                    }
                    answer
                }
            };
            results.push(answer);
        }
        Ok(results)
    }
}

fn evaluate(term: &ScalarTerm, bindings: &[(ValueId, Scalar)]) -> Option<Scalar> {
    let (ty, value) = crate::inhabitation::known_scalar_value_with(term, |id| {
        let (_, value) = bindings.iter().find(|(key, _)| *key == id)?;
        ScalarFact::Literal(*value).known_integer()
    })?;
    ScalarFact::from_integer(ty, value)?.literal()
}
