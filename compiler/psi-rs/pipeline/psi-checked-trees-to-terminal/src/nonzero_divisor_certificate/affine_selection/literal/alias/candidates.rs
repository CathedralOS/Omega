//! Source-ordered one-alias literal candidates for certificate production.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::ProofNode;

mod join;
mod landing_index;
mod root_aliases;

use landing_index::LandingIndex;
use root_aliases::RootAliases;

pub(super) struct LiteralAliasCandidates<'a> {
    root_aliases: RootAliases<'a>,
    landings: LandingIndex<'a>,
}

impl<'a> LiteralAliasCandidates<'a> {
    pub(super) fn new(assumptions: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            root_aliases: RootAliases::new(assumptions, semantic_axioms),
            landings: LandingIndex::new(assumptions, semantic_axioms),
        }
    }

    pub(super) fn find<T>(
        &self,
        mut complete: impl FnMut(
            &'a ScalarTerm,
            &'a ScalarTerm,
            &'a ScalarTerm,
            ProofNode,
            ProofNode,
        ) -> Option<T>,
    ) -> Option<T> {
        self.root_aliases
            .find(|outer_citation, outer_equality, root, alias| {
                for &(inner_citation, inner_equality, literal) in self.landings.candidates(alias) {
                    if !join::eligible(outer_equality, root, inner_equality, literal) {
                        continue;
                    }
                    if let Some(result) = complete(
                        root,
                        alias,
                        literal,
                        outer_citation.proof(outer_equality),
                        inner_citation.proof(inner_equality),
                    ) {
                        return Some(result);
                    }
                }
                None
            })
    }
}
