//! Exact completion-receipt custody replay at final image boundaries.

use std::collections::BTreeSet;

use omega_terminal_target_operations::TerminalCompletionClaimSource;
use psi_core::{ClaimId, ContentPlaceSegment, ContentPlaceVersion};
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
            !source_claims.insert(source.claim()) || !claim_source_is_canonical(source)
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
                (source.input() == argument.place
                    && match &source.entry {
                        Some(source) => argument.path.is_empty() || source.path == argument.path,
                        None => true,
                    })
                .then_some((argument_index, source.claim()))
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

fn claim_source_is_canonical(source: &TerminalCompletionClaimSource) -> bool {
    let entry_is_canonical = source.entry.as_ref().is_none_or(|entry| {
        entry.claim == source.claim
            && entry.path.iter().all(|segment| {
            !matches!(segment, StructuralPathSegment::Field(identity) if identity.is_empty())
        })
    });
    let content_is_canonical = source.content.as_ref().is_none_or(|content| {
        content.claim == source.claim
            && content.input.version == ContentPlaceVersion::Entry
            && !content.projections.is_empty()
            && !content
                .projections
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            && content.input.segments.iter().all(|segment| {
                !matches!(
                    segment,
                    ContentPlaceSegment::Case(identity)
                        | ContentPlaceSegment::Field(identity)
                        if identity.is_empty()
                )
            })
            && content.projections.iter().all(|projection| {
                projection.projection.projection_fingerprint != 0
                    && !projection.algebra.parameter.is_empty()
            })
    });
    let paired_sources_match =
        match (&source.entry, &source.content) {
            (Some(entry), Some(content)) => {
                entry.input == content.input.root
                    && entry.path.len() == content.input.segments.len()
                    && entry.path.iter().zip(&content.input.segments).all(
                        |(entry, content)| match (entry, content) {
                            (
                                StructuralPathSegment::Field(entry),
                                ContentPlaceSegment::Field(content),
                            ) => entry == content,
                            (
                                StructuralPathSegment::FixedIndex(entry),
                                ContentPlaceSegment::FixedIndex(content),
                            ) => entry == content,
                            _ => false,
                        },
                    )
            }
            _ => true,
        };
    (source.entry.is_some() || source.content.is_some())
        && entry_is_canonical
        && content_is_canonical
        && paired_sources_match
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
        let claim = ClaimId::new(claim).expect("claim");
        TerminalCompletionClaimSource {
            claim,
            entry: Some(psi_terminal::EntryClaim {
                claim,
                input: psi_core::PlaceId::new(place).expect("place"),
                path: Vec::new(),
            }),
            content: None,
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
        entry_projection.entry.as_mut().unwrap().path = vec![StructuralPathSegment::FixedIndex(1)];
        assert!(!completion_receipts_have_exact_custody(
            std::slice::from_ref(&projected),
            &[entry_projection],
            &[receipt(1, 0)],
        ));
        let claim = ClaimId::new(1).expect("claim");
        let content_root = TerminalCompletionClaimSource {
            claim,
            entry: None,
            content: Some(psi_terminal::ContentEntryClaim {
                claim,
                input: psi_core::ContentStructuralPlace {
                    version: ContentPlaceVersion::Entry,
                    root: psi_core::PlaceId::new(1).expect("place"),
                    segments: vec![ContentPlaceSegment::FixedIndex(1)],
                },
                projections: vec![psi_terminal::ClaimContentProjection {
                    projection: psi_core::ContentProjectionIdentity {
                        domain: psi_core::ContentDomainId::new(1).expect("domain"),
                        projection_fingerprint: 7,
                    },
                    algebra: psi_core::ContentAlgebra {
                        kind: psi_core::ContentAlgebraKind::CountedQuantity,
                        parameter: "Bytes".into(),
                    },
                }],
            }),
        };
        assert!(completion_receipts_have_exact_custody(
            &[projected],
            std::slice::from_ref(&content_root),
            &[receipt(1, 0)],
        ));

        let mut combined = content_root;
        combined.entry = Some(psi_terminal::EntryClaim {
            claim,
            input: psi_core::PlaceId::new(1).expect("place"),
            path: vec![StructuralPathSegment::FixedIndex(1)],
        });
        assert!(completion_receipts_have_exact_custody(
            &[argument(1)],
            std::slice::from_ref(&combined),
            &[receipt(1, 0)],
        ));
        combined.content.as_mut().unwrap().input.segments[0] = ContentPlaceSegment::FixedIndex(2);
        assert!(!completion_receipts_have_exact_custody(
            &[argument(1)],
            &[combined],
            &[receipt(1, 0)],
        ));
    }
}
