//! Track the context of each written file qualifier while parsing one file.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
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

    pub(crate) fn only_product_operands(self) -> BTreeSet<&'source str> {
        let mut aliases = BTreeMap::new();
        for (alias, product) in self.uses.into_inner().into_values() {
            *aliases.entry(alias).or_insert(true) &= product;
        }
        aliases
            .into_iter()
            .filter_map(|(name, product)| product.then_some(name))
            .collect()
    }
}
