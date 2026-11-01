//! One production edge list for recursive subtree queries.
//!
//! Questions such as "does this operand mention a root", "does it contain a
//! call" or "does it read an overlapping place" all walk the same immediate
//! value edges of each expression production. Each query previously restated
//! that list in its own match, so a new production or edge had to be added to
//! every copy. Queries now own only their node judgment and the explicit edge
//! choices in [`SubtreeEdges`]; the production list lives here once.
//!
//! The walk is a recursive preorder in source order. It skips invalid handles
//! (absent receivers and open range endpoints) and does not deduplicate
//! repeated handles, matching the per-query walks it replaces.

use crate::expression::{ExpressionHandle, ExpressionNode};

/// The edge choices on which subtree queries genuinely differ.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubtreeEdges {
    /// A saved subject stands for its producing expression. Without this a
    /// saved subject is a leaf.
    pub saved_sources: bool,
    /// An atomic expression also contributes its structural result edge, not
    /// only its evaluated value.
    pub atomic_results: bool,
    /// Match arm patterns contribute their destructure heads and fields
    /// (`match_pattern_dependencies`), not only value/guard operands
    /// (`match_pattern_operands`).
    pub pattern_dependencies: bool,
}

impl SubtreeEdges {
    /// Every edge of the source value tree; saved subjects stay leaves.
    pub const VALUES: Self = Self {
        saved_sources: false,
        atomic_results: false,
        pattern_dependencies: false,
    };

    /// Every structural edge, including atomic result evidence.
    pub const STRUCTURAL: Self = Self {
        saved_sources: false,
        atomic_results: true,
        pattern_dependencies: false,
    };

    /// Value edges, with saved subjects standing for their producer.
    pub const SAVED_SOURCES: Self = Self {
        saved_sources: true,
        atomic_results: false,
        pattern_dependencies: false,
    };
}

/// A query's judgment of one node before its children are walked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubtreeVisit {
    /// The subtree satisfies the query; stop.
    Found,
    /// Continue into this node's children.
    Descend,
    /// This node's children are outside the query.
    Prune,
    /// Walk only this edge in place of the node's children.
    Instead(ExpressionHandle),
    /// Walk only these edges, in order, in place of the node's children.
    InsteadEach(Vec<ExpressionHandle>),
}

impl crate::TypedTrees {
    /// Whether any node under `root` (inclusive) is judged `Found`.
    pub fn subtree_any(
        &self,
        root: ExpressionHandle,
        edges: SubtreeEdges,
        mut visit: impl FnMut(ExpressionHandle, &ExpressionNode) -> SubtreeVisit,
    ) -> bool {
        self.subtree_any_inner(root, edges, &mut visit)
    }

    /// Visit every node under `root` (inclusive) in preorder.
    pub fn subtree_for_each(
        &self,
        root: ExpressionHandle,
        edges: SubtreeEdges,
        mut visit: impl FnMut(ExpressionHandle, &ExpressionNode),
    ) {
        self.subtree_any(root, edges, |expression, node| {
            visit(expression, node);
            SubtreeVisit::Descend
        });
    }

    fn subtree_any_inner(
        &self,
        expression: ExpressionHandle,
        edges: SubtreeEdges,
        visit: &mut dyn FnMut(ExpressionHandle, &ExpressionNode) -> SubtreeVisit,
    ) -> bool {
        if !expression.is_valid() {
            return false;
        }
        let node = self.expression_table.expression(expression);
        match visit(expression, node) {
            SubtreeVisit::Found => return true,
            SubtreeVisit::Prune => return false,
            SubtreeVisit::Instead(child) => return self.subtree_any_inner(child, edges, visit),
            SubtreeVisit::InsteadEach(children) => {
                return children
                    .into_iter()
                    .any(|child| self.subtree_any_inner(child, edges, visit));
            }
            SubtreeVisit::Descend => {}
        }
        let mut found = false;
        self.subtree_children(node, edges, |child| {
            found = found || self.subtree_any_inner(child, edges, visit);
            found
        });
        found
    }

    /// Immediate value edges of `node` in source order under `edges`. `child`
    /// returns true to stop the enumeration early.
    pub fn subtree_children(
        &self,
        node: &ExpressionNode,
        edges: SubtreeEdges,
        mut child: impl FnMut(ExpressionHandle) -> bool,
    ) {
        let table = &self.expression_table;
        match node {
            ExpressionNode::SavedSubject(saved) => {
                if edges.saved_sources
                    && let Some(source) = self.saved_value_source(saved)
                {
                    child(source);
                }
            }
            ExpressionNode::Match(dispatch) => {
                if child(dispatch.subject) {
                    return;
                }
                for arm in table.match_arms(dispatch.arms) {
                    let stop = if edges.pattern_dependencies {
                        table
                            .match_pattern_dependencies(arm.pattern)
                            .any(&mut child)
                    } else {
                        table.match_pattern_operands(arm.pattern).any(&mut child)
                    };
                    if stop || child(arm.value) {
                        return;
                    }
                }
            }
            ExpressionNode::Atomic(atomic) => {
                let _ = child(atomic.value) || (edges.atomic_results && child(atomic.result));
            }
            ExpressionNode::ArrayLiteral(values) => {
                let _ = table
                    .expression_handles(*values)
                    .iter()
                    .any(|value| child(*value));
            }
            ExpressionNode::Binary(binary) => {
                let _ = child(binary.left) || child(binary.right);
            }
            ExpressionNode::Call(call) => {
                let _ = child(call.receiver)
                    || table
                        .expression_handles(call.arguments)
                        .iter()
                        .any(|argument| child(*argument));
            }
            ExpressionNode::Cast(cast) => {
                child(cast.value);
            }
            ExpressionNode::Indexed(indexed) => {
                let _ = child(indexed.collection) || child(indexed.index);
            }
            ExpressionNode::Range(range) => {
                let _ = child(range.start) || child(range.end);
            }
            ExpressionNode::Member(member) => {
                child(member.receiver);
            }
            ExpressionNode::Borrow(borrow) => {
                child(borrow.target);
            }
            ExpressionNode::Unary(unary) => {
                child(unary.operand);
            }
            ExpressionNode::StructLiteral(literal) => {
                let _ = table
                    .struct_fields(literal.fields)
                    .iter()
                    .any(|field| child(field.value));
            }
            ExpressionNode::TypeExpression(_) | crate::literal_expression_pattern!() => {}
        }
    }
}
