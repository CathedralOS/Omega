// We retain the exact caller claim sources needed to replay boundary-completion
// custody after the verified Terminal module is discarded. Content-bearing
// sources keep their full entry-version subject and owner-unique projection and
// algebra catalog rather than collapsing to a generic whole-root claim identity.

use semantic_vocabulary::{ClaimId, PlaceId};
use terminal_psi::{ContentEntryClaim, EntryClaim};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompletionClaimSource {
    pub claim: ClaimId,
    pub entry: Option<EntryClaim>, // Ordinary structural claim source when participating in the whole-value frontier.
    pub content: Option<ContentEntryClaim>, // Exact content subject and projection/algebra catalog when participating in content conservation.
}

impl CompletionClaimSource {
    pub const fn claim(&self) -> ClaimId {
        self.claim
    }

    pub fn input(&self) -> PlaceId {
        match (&self.entry, &self.content) {
            (Some(source), _) => source.input,
            (None, Some(source)) => source.input.root,
            (None, None) => unreachable!(),
        }
    }
}
