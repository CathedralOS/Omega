//! The parser's output: a flat list of events that describes a tree over the lexer's tokens.

use crate::event::Event;

/*
    A syntax tree, stored as the list of events you would see walking it.

    We do not build nodes that point at their children. The parser appends one
    byte per event to one list, and the list *is* the tree: an `Open` starts a
    node, the events up to its matching `Close` are its contents, and each
    `Token` stands for the next token the lexer produced, skipping whitespace
    and comments. A node therefore costs two bytes and no allocation of its
    own, where a tree of boxed nodes would cost an allocation each. (An event
    is one byte because `NodeKind` has fewer than 254 kinds, which leaves the
    compiler two spare values of that byte for `Close` and `Token`. A test at
    the bottom of this file fails if that ever stops being true.)

    Every meaningful token appears exactly once, in order. That is the check
    this layer's tests lean on: count the `Token` events of a tree and you must
    get the number of meaningful tokens the lexer produced, none dropped and
    none repeated.

    `meaningful_token_indices` maps the n-th `Token` event to its position in
    the lexer's token list, so a reader can get from the tree back to spellings
    and byte ranges.
*/
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxTree {
    events: Vec<Event>,
    meaningful_token_indices: Vec<u32>,
}

impl SyntaxTree {
    pub(crate) fn new(
        events: Vec<Event>,
        meaningful_token_indices: Vec<u32>,
    ) -> Self {
        Self {
            events,
            meaningful_token_indices,
        }
    }

    /// The walk over the tree, in source order.
    pub fn get_events(&self) -> &[Event] {
        &self.events
    }

    /// For the n-th `Token` event, the index of that token in the lexer's output.
    pub fn get_meaningful_token_indices(&self) -> &[u32] {
        &self.meaningful_token_indices
    }
}

#[cfg(test)]
mod tests {
    use crate::event::Event;

    #[test]
    fn an_event_is_one_byte() {
        assert_eq!(size_of::<Event>(), 1);
    }
}
