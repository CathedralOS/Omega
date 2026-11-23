//! Track the context of each written file qualifier while parsing one file.

use language_core::build_operations::BuildOperation;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use syntax_trees::SyntaxTrees;
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::identifier::Identifier;

#[derive(Default)]
pub(crate) struct FileQualifiers<'source> {
    // Parsing can visit a name more than once. Its opening bar identifies one occurrence.
    uses: RefCell<BTreeMap<usize, (&'source str, bool)>>,
}

impl<'source> FileQualifiers<'source> {
    pub(crate) fn record(&self, start: usize, alias: &'source str) {
        self.uses
            .borrow_mut()
            .entry(start)
            .or_insert((alias, false));
    }

    pub(crate) fn mark_product_operand(&self, path: &[Identifier]) {
        if let Some(head) = path.first()
            && let Some((_, product)) = self
                .uses
                .borrow_mut()
                .get_mut(&head.source_span().span.start)
        {
            *product = true;
        }
    }

    /// A static Build call can select a declaration by filename. Its ordinary
    /// value arguments and any other use of that file still belong to the host.
    pub(crate) fn mark_build_call(&self, trees: &SyntaxTrees, expression: ExpressionHandle) {
        let ExpressionNode::Call(call) = trees.expressions.expression(expression) else {
            return;
        };
        if call.machine_arguments.is_empty() || !call.target_is_static || !call.receiver.is_valid()
        {
            return;
        }
        let ExpressionNode::Name {
            members,
            application: None,
        } = trees.expressions.expression(call.receiver)
        else {
            return;
        };
        let [owner] = trees.expressions.identifier_path_members(*members) else {
            return;
        };
        if owner.as_str(trees) != "Build" {
            return;
        }
        let Some(operation) = BuildOperation::from_call_target(call.target.as_str(trees)) else {
            return;
        };
        if operation.uses_tests_facet() {
            return;
        }
        for (position, argument) in call.machine_arguments.iter().enumerate() {
            if operation.product_operand_role(position).is_some() {
                self.mark_product_operand(&argument.path);
            }
        }
    }

    pub(crate) fn only_product_operands(&self) -> BTreeSet<&'source str> {
        let mut aliases = BTreeMap::new();
        for (alias, product) in self.uses.borrow().values().copied() {
            *aliases.entry(alias).or_insert(true) &= product;
        }
        aliases
            .into_iter()
            .filter_map(|(name, product)| product.then_some(name))
            .collect()
    }
}
