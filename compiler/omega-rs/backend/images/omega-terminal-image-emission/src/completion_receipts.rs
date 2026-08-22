//! Exact completion-receipt custody replay at final image boundaries.

use std::collections::BTreeSet;

use omega_terminal_target_operations::TerminalCompletionClaimSource;
use psi_core::ClaimId;
use psi_terminal::{CompletionReceipt, StructuralArgument, StructuralPathSegment};

/// Replay the verifier's exact claim-source matching, claim uniqueness, and
/// canonical receipt ordering after the verified module has been discarded.
pub(super) fn completion_receipts_have_exact_custody(
    arguments: &[StructuralArgument],
    sources: &[TerminalCompletionClaimSource],
    receipts: &[CompletionReceipt],
) -> bool {
    let mut source_claims = BTreeSet::<ClaimId>::new();
    if sources.windows(2).any(|pair| pair[0] >= pair[1])
        || sources.iter().any(|source| {
            !source_claims.insert(source.claim)
                || source.path.as_ref().is_some_and(|path| {
                    path.iter().any(|segment| {
                        matches!(segment, StructuralPathSegment::Field(identity) if identity.is_empty())
                    })
                })
        })
    {
        return false;
    }

    let expected = arguments
        .iter()
        .enumerate()
        .flat_map(|(index, argument)| {
            sources.iter().filter_map(move |source| {
                let argument_index = u32::try_from(index).ok()?;
                (source.input == argument.place
                    && match &source.path {
                        Some(path) => argument.path.is_empty() || *path == argument.path,
                        None => true,
                    })
                .then_some((argument_index, source.claim))
            })
        })
        .collect::<BTreeSet<_>>();
    let actual = receipts
        .iter()
        .map(|receipt| (receipt.argument_index, receipt.claim))
        .collect::<BTreeSet<_>>();
    let mut receipt_claims = BTreeSet::<ClaimId>::new();
    receipts.windows(2).all(|pair| pair[0] < pair[1])
        && receipts
            .iter()
            .all(|receipt| receipt_claims.insert(receipt.claim))
        && actual == expected
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(claim: u64, argument_index: u32) -> CompletionReceipt {
        CompletionReceipt {
            claim: ClaimId::new(claim).expect("claim"),
            argument_index,
        }
    }

    fn argument(place: u64) -> StructuralArgument {
        StructuralArgument {
            place: psi_core::PlaceId::new(place).expect("place"),
            path: Vec::new(),
        }
    }

    fn source(claim: u64, place: u64) -> TerminalCompletionClaimSource {
        TerminalCompletionClaimSource {
            claim: ClaimId::new(claim).expect("claim"),
            input: psi_core::PlaceId::new(place).expect("place"),
            path: Some(Vec::new()),
        }
    }

    #[test]
    fn exact_completion_receipt_custody_replays_the_claim_catalog() {
        assert!(completion_receipts_have_exact_custody(&[], &[], &[]));
        assert!(completion_receipts_have_exact_custody(
            &[argument(1)],
            &[source(1, 1)],
            &[receipt(1, 0)],
        ));
        assert!(completion_receipts_have_exact_custody(
            &[argument(1), argument(2),],
            &[source(1, 2), source(2, 1),],
            &[receipt(1, 1), receipt(2, 0)]
        ));

        assert!(!completion_receipts_have_exact_custody(
            &[argument(1), argument(2),],
            &[source(1, 2), source(2, 1)],
            &[receipt(2, 0), receipt(1, 1)]
        ));
        assert!(!completion_receipts_have_exact_custody(
            &[argument(1), argument(2)],
            &[source(1, 1)],
            &[receipt(1, 0), receipt(1, 1)],
        ));
        assert!(!completion_receipts_have_exact_custody(
            &[argument(1)],
            &[source(1, 1)],
            &[receipt(1, 0), receipt(1, 0)],
        ));
        assert!(!completion_receipts_have_exact_custody(
            &[argument(1)],
            &[source(1, 1)],
            &[],
        ));

        let projected = StructuralArgument {
            place: psi_core::PlaceId::new(1).expect("place"),
            path: vec![StructuralPathSegment::FixedIndex(2)],
        };
        let mut entry_projection = source(1, 1);
        entry_projection.path = Some(vec![StructuralPathSegment::FixedIndex(1)]);
        assert!(!completion_receipts_have_exact_custody(
            std::slice::from_ref(&projected),
            &[entry_projection],
            &[receipt(1, 0)],
        ));
        let mut content_root = source(1, 1);
        content_root.path = None;
        assert!(completion_receipts_have_exact_custody(
            &[projected],
            &[content_root],
            &[receipt(1, 0)],
        ));
    }
}
