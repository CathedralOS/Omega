//! Exact selected-dispatch edits, separate from checked source semantics.
//!
//! Each settlement owner seals its own batch after applying it. A query checks
//! batches in reverse order, including operand/type graphs, then restores only
//! the nodes that owner replaced. Unrelated source edits remain visible.

mod builder;
mod guard;
mod records;

pub(super) use builder::SourceEditBuilder;
use records::*;

use arena::{Handle, HandleSpan};
use diagnostics::Diagnostic;
use guard::GraphGuard;
use std::borrow::Cow;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::name::Identifier;
use typed_trees::statement::{StatementNode, TableCall};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectedDispatchSourceEdits {
    batches: Vec<Batch>,
}

impl SelectedDispatchSourceEdits {
    /// Preserve the actual settlement order; restoration runs in reverse.
    pub fn append(&mut self, later: Self) {
        self.batches.extend(later.batches);
    }

    /// The pre-selected-dispatch typed source, not pre-specialization syntax.
    /// No edited sites means a borrow; otherwise one scratch clone is shared
    /// by every semantic query performed through the returned view.
    pub fn source_trees<'source>(
        &self,
        settled: &'source TypedTrees,
    ) -> Result<Cow<'source, TypedTrees>, Vec<Diagnostic>> {
        if self.batches.is_empty() {
            return Ok(Cow::Borrowed(settled));
        }
        // Validate the live settled subtree before cloning any of its owned
        // recursive payloads into the scratch tree.
        self.batches
            .last()
            .expect("nonempty batches")
            .validate(settled)?;
        let mut source = settled.clone();
        for (index, batch) in self.batches.iter().rev().enumerate() {
            if index != 0 {
                batch.validate(&source)?;
            }
            for edit in batch.edits.iter().rev() {
                match edit {
                    Edit::Expression {
                        handle, original, ..
                    } => {
                        *source.expression_table.expression_mut(*handle) = original.clone();
                    }
                    Edit::Statement {
                        handle, original, ..
                    } => {
                        *source.statement_table.statement_mut(*handle) =
                            StatementNode::Call(original.clone());
                    }
                }
            }
        }
        Ok(Cow::Owned(source))
    }
}

fn rejected(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "selected-dispatch source view: {message}"
    ))]
}
