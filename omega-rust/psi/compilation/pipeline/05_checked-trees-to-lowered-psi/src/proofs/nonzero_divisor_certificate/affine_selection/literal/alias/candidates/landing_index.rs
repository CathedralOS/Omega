//! Source-ordered alias-to-literal landing index for certificate production.

use std::cell::RefCell;
use std::rc::Rc;

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, ScalarTerm};

use super::super::super::super::super::integer_evidence::Citation;
use super::super::super::super::super::scope_identity::{self, ScopeDerived};
use super::super::super::super::{equalities, fact_identity, value_index::ValueIndex};

pub(super) struct LandingIndex<'a> {
    /// Exact value bindings by their value root, as `(citation, reversed)`
    /// orientations: a pure function of the scope, derived once per scope.
    by_alias: Rc<ValueIndex<(Citation, bool)>>,
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
}

impl<'a> LandingIndex<'a> {
    pub(super) fn new(assumptions: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        thread_local! {
            static INDEXES: RefCell<ScopeDerived<ValueIndex<(Citation, bool)>>> =
                const { RefCell::new(ScopeDerived::new()) };
        }
        let by_alias = scope_identity::derived(&INDEXES, assumptions, semantic_axioms, || {
            let mut by_alias = ValueIndex::new();
            for &(citation, reversed) in
                equalities::exact_value_binding_orientations(assumptions, semantic_axioms).iter()
            {
                let (_, _, alias, _) =
                    equalities::oriented(citation, reversed, assumptions, semantic_axioms);
                by_alias.push(alias, (citation, reversed));
            }
            by_alias
        });
        Self {
            by_alias,
            assumptions,
            semantic_axioms,
        }
    }

    pub(super) fn find<T>(
        &self,
        root: &ScalarTerm,
        alias: &ScalarTerm,
        outer_citation: Citation,
        outer_equality: &Proposition,
        mut complete: impl FnMut(&'a ScalarTerm, ProofNode, ProofNode) -> Option<T>,
    ) -> Option<T> {
        if root.scalar_type() != alias.scalar_type() {
            return None;
        }
        self.by_alias
            .candidates(alias)
            .iter()
            .find_map(|&(citation, reversed)| {
                let (citation, inner_equality, _, literal) = equalities::oriented(
                    citation,
                    reversed,
                    self.assumptions,
                    self.semantic_axioms,
                );
                if !fact_identity::distinct(outer_equality, inner_equality) {
                    return None;
                }
                complete(
                    literal,
                    outer_citation.proof(outer_equality),
                    citation.proof(inner_equality),
                )
            })
    }
}
