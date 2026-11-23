//! A node the parser has finished, kept so that a later rule can wrap it.

/// A node that is complete. Holding one lets a later rule wrap the node in a new parent.
#[derive(Clone, Copy)]
pub(crate) struct ClosedNode {
    event_index: usize, // Where the node's `Open` event sits in the event list.
}

impl ClosedNode {
    pub(crate) fn new(event_index: usize) -> Self {
        Self { event_index }
    }

    pub(crate) fn get_event_index(&self) -> usize {
        self.event_index
    }
}
