//! A completed aggregate may refer back only to sources unchanged since capture.
//!
//! The ordinary call-frame owner supplies alias-aware effects. Walking siblings
//! backward accumulates writes after each copied child, including enclosing
//! siblings of a nested aggregate. Unknown effects withhold earlier source facts.
//! This governs post-environment metadata, not the independent completed values
//! retained by normal operation evaluation.

use super::super::ordered_values;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::{machine::Machine, state::State};

pub(super) struct SourceObservations<'a> {
    pub(super) program: &'a dyn crate::CheckingInput,
    machine: &'a Machine,
    state: Option<&'a State>,
    frames: Option<crate::CallFrameResolver<'a>>,
}

impl<'a> SourceObservations<'a> {
    /// End-environment reads need suffix effects, while completed payloads
    /// still retire in authored order. Compute the former without reversing
    /// the latter. Unknown effects suppress source identities, not detached
    /// checked snapshots of earlier owned values.
    pub(super) fn following_writes(
        &self,
        expressions: impl DoubleEndedIterator<Item = ExpressionHandle>,
        following: Option<&[String]>,
    ) -> Vec<Option<Vec<String>>> {
        let mut written = following.map(<[String]>::to_vec);
        let mut suffixes = Vec::new();
        for expression in expressions.rev() {
            suffixes.push(written.clone());
            if let Some(paths) = written.as_mut()
                && !self.include_effects(expression, paths)
            {
                written = None;
            }
        }
        suffixes.reverse();
        suffixes
    }

    pub(super) fn effects(&self, expression: ExpressionHandle) -> Option<Vec<String>> {
        let mut written = Vec::new();
        self.include_effects(expression, &mut written)
            .then_some(written)
    }

    pub(super) fn new(
        program: &'a dyn crate::CheckingInput,
        machine: &'a Machine,
        state: Option<&'a State>,
        expression: ExpressionHandle,
    ) -> Self {
        let frames = crate::machine_calls::calls::expression_has_calls(program, expression)
            .then(|| crate::CallFrameResolver::new(program))
            .flatten();
        Self {
            program,
            machine,
            state,
            frames,
        }
    }

    pub(super) fn include_effects(
        &self,
        expression: ExpressionHandle,
        written: &mut Vec<String>,
    ) -> bool {
        if !crate::machine_calls::calls::expression_has_calls(self.program, expression) {
            return true;
        }
        let Some(paths) = self
            .frames
            .as_ref()
            .and_then(|frames| frames.expression_may_write_paths(self.machine, expression))
        else {
            return false;
        };
        written.extend(paths);
        true
    }

    pub(super) fn permits(&self, expression: ExpressionHandle, written: &[String]) -> bool {
        // A nonaggregate child can itself read before an effectful nested
        // operand. Rechecking that whole child after completion is no more an
        // occurrence snapshot than rechecking an earlier aggregate sibling.
        let mut written = written.to_vec();
        if !self.include_effects(expression, &mut written) {
            return false;
        }
        if written.is_empty()
            || matches!(
                self.program.expression_table.expression(expression),
                ExpressionNode::Integer(_)
                    | ExpressionNode::Float(_)
                    | ExpressionNode::UndeterminedNaN(_)
                    | ExpressionNode::Boolean(_)
            )
        {
            return true;
        }
        self.state.is_some_and(|state| {
            ordered_values::operand(
                self.program,
                self.machine,
                state,
                expression,
                self.frames.as_ref(),
            )
            .is_some_and(|operand| operand.survives(&written))
        })
    }
}
