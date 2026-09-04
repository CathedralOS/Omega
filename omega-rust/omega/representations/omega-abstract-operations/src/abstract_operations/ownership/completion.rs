//! Caller claim sources needed to replay completion after Terminal lowering.

use psi_core::{ClaimId, PlaceId};
use psi_terminal::{ContentEntryClaim, EntryClaim};

/// Exact caller claim source needed to replay boundary-completion custody after
/// the verified module is discarded. Content-bearing sources retain their full
/// entry-version subject and owner-unique projection/algebra catalog rather
/// than collapsing to a generic whole-root claim identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompletionClaimSource {
    pub claim: ClaimId,
    /// Ordinary structural claim source, when this claim participates in the
    /// whole-value frontier.
    pub entry: Option<EntryClaim>,
    /// Exact content subject and projection/algebra catalog, when this claim
    /// also participates in content conservation.
    pub content: Option<ContentEntryClaim>,
}

impl CompletionClaimSource {
    pub const fn claim(&self) -> ClaimId {
        self.claim
    }

    pub fn input(&self) -> PlaceId {
        match &self.entry {
            Some(source) => source.input,
            None => match &self.content {
                Some(source) => source.input.root,
                None => unreachable!(),
            },
        }
    }
}
