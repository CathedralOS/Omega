//! One step of a walk through a syntax tree.

use crate::node_kind::NodeKind;

/// One step of a walk through the tree, in source order. See `SyntaxTree`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    /// A node of this kind starts here.
    Open(NodeKind),

    /// The most recently opened node that is still open ends here.
    Close,

    /// The next meaningful token belongs to the node that is currently open.
    Token,
}
