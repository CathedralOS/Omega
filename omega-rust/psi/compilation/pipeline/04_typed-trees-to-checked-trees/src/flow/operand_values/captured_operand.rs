//! A completed row and its recorded arena association travel together.
//! Projection already knows each copied, selected, constructed or stored child
//! handle. Preserve that association instead of searching every arena page at
//! each obligation. An unrecorded borrow remains a distinct, uncached input.

use arena::Handle;
use checked_trees::{FlowControlFacts, FlowOperandFact};
use std::ops::Deref;

#[derive(Clone, Copy)]
pub(crate) struct CapturedOperand<'capture> {
    row: &'capture FlowOperandFact,
    handle: Handle<FlowOperandFact>,
    owner: Option<&'capture FlowControlFacts>,
}

impl<'capture> CapturedOperand<'capture> {
    /// Resolve the row in its declared owner. A stale or absent recorded
    /// identity refuses; it never turns into an unrecorded borrow.
    pub(crate) fn recorded(
        owner: &'capture FlowControlFacts,
        handle: Handle<FlowOperandFact>,
    ) -> Option<Self> {
        owner.operands.is_valid(handle).then(|| Self {
            row: owner.operands.get(handle),
            handle,
            owner: Some(owner),
        })
    }

    pub(crate) fn unrecorded(row: &'capture FlowOperandFact) -> Self {
        Self {
            row,
            handle: Handle::invalid(),
            owner: None,
        }
    }

    pub(crate) fn row(self) -> &'capture FlowOperandFact {
        self.row
    }

    /// A proof consumer needs a recorded association, not the absence-compatible
    /// scratch input accepted by ordinary uncached projection.
    pub(crate) fn recorded_in(self, owner: &FlowControlFacts) -> Option<Handle<FlowOperandFact>> {
        self.owner
            .filter(|recorded| std::ptr::eq(*recorded, owner))?;
        owner.operands.is_valid(self.handle).then_some(self.handle)
    }

    pub(in crate::flow) fn handle(self) -> Handle<FlowOperandFact> {
        self.handle
    }

    pub(in crate::flow) fn belongs_to(self, owner: &FlowControlFacts) -> bool {
        match self.owner {
            // This verifies the arena owning a recorded handle, not language
            // value equality or a pointer-keyed evidence cache.
            Some(recorded) => std::ptr::eq(recorded, owner) && owner.operands.is_valid(self.handle),
            None => !self.handle.is_valid(),
        }
    }
}

impl Deref for CapturedOperand<'_> {
    type Target = FlowOperandFact;

    fn deref(&self) -> &Self::Target {
        self.row
    }
}
