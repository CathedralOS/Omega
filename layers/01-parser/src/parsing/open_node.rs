//! A node the parser has started and not yet finished.

/// A node that has been opened and not yet closed.
///
/// Not `Copy` on purpose: closing a node takes this by value, so a node cannot be closed twice.
pub(crate) struct OpenNode {
    event_index: usize, // Where the node's `Open` event sits in the event list.
}

impl OpenNode {
    pub(crate) fn new(event_index: usize) -> Self {
        Self { event_index }
    }

    pub(crate) fn get_event_index(&self) -> usize {
        self.event_index
    }
}
