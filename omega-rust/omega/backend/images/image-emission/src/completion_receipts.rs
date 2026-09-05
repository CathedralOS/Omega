//! Exact completion-receipt custody replay at final image boundaries.

use std::collections::BTreeSet;

use machine_code::{BoundarySettlementRecord, derive_completion_provider_custody};
use semantic_vocabulary::{ClaimId, ContentPlaceSegment, ContentPlaceVersion};
use target_operations::CompletionClaimSource;
use terminal_psi::{CompletionReceipt, StructuralArgument, StructuralPathSegment};

/// Closed failure classes shared by object and installation settlement replay.
/// Callers translate these private classes to their existing public errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CompletionCustodyError {
    ArgumentPath,
    ReceiptArgumentIndex,
    ReceiptCustody,
    ProviderCustody,
}

/// Replay the complete completion-custody responsibility after the verified
/// module has been discarded.
///
/// Check order is load-bearing because object and installation validation
/// expose distinct public error enums with established precedence.
pub(super) fn validate_completion_custody(
    settlement: &BoundarySettlementRecord,
) -> Result<(), CompletionCustodyError> {
    if settlement.arguments.iter().any(|argument| {
        argument.path.iter().any(
            |segment| matches!(segment, StructuralPathSegment::Field(identity) if identity.is_empty()),
        )
    }) {
        return Err(CompletionCustodyError::ArgumentPath);
    }
    if settlement.completion_receipts.iter().any(|receipt| {
        usize::try_from(receipt.argument_index)
            .map_or(true, |index| index >= settlement.arguments.len())
    }) {
        return Err(CompletionCustodyError::ReceiptArgumentIndex);
    }
    if !completion_receipts_have_exact_custody(
        &settlement.arguments,
        &settlement.completion_claim_sources,
        &settlement.completion_receipts,
    ) {
        return Err(CompletionCustodyError::ReceiptCustody);
    }
    let compiler_builtin_pair_is_exact = match (settlement.execution, settlement.realization) {
        (
            machine_code::BoundaryExecutionRecord::CompilerBuiltin(
                target_operations::CompilerBuiltinExecution::LinuxExitGroupI32,
            ),
            target_operations::BoundaryRealization::LinuxExitGroupI32(_),
        )
        | (
            machine_code::BoundaryExecutionRecord::CompilerBuiltin(
                target_operations::CompilerBuiltinExecution::LinuxWriteByteI32,
            ),
            target_operations::BoundaryRealization::LinuxWriteByteI32(_),
        )
        | (
            machine_code::BoundaryExecutionRecord::CompilerBuiltin(
                target_operations::CompilerBuiltinExecution::LinuxReadByte,
            ),
            target_operations::BoundaryRealization::LinuxReadByte(_),
        ) => true,
        (machine_code::BoundaryExecutionRecord::CompilerBuiltin(_), _)
        | (
            _,
            target_operations::BoundaryRealization::LinuxExitGroupI32(_)
            | target_operations::BoundaryRealization::LinuxWriteByteI32(_)
            | target_operations::BoundaryRealization::LinuxReadByte(_),
        ) => false,
        _ => true,
    };
    if !compiler_builtin_pair_is_exact {
        return Err(CompletionCustodyError::ProviderCustody);
    }
    if derive_completion_provider_custody(
        settlement.execution,
        &settlement.completion_claim_sources,
        &settlement.completion_receipts,
    )
    .is_none_or(|expected| expected != settlement.completion_provider_custody)
    {
        return Err(CompletionCustodyError::ProviderCustody);
    }
    Ok(())
}

/// Replay the verifier's exact claim-source matching, claim uniqueness, and
/// canonical receipt ordering after the verified module has been discarded.
fn completion_receipts_have_exact_custody(
    arguments: &[StructuralArgument],
    sources: &[CompletionClaimSource],
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

fn claim_source_is_canonical(source: &CompletionClaimSource) -> bool {
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
                projection.projection.projection_report_fingerprint != 0
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

    fn settlement(
        arguments: Vec<StructuralArgument>,
        completion_claim_sources: Vec<CompletionClaimSource>,
        completion_receipts: Vec<CompletionReceipt>,
    ) -> BoundarySettlementRecord {
        let provider_execution =
            machine_code::ProviderExecutionRecord::new(1, 2, 3, 4, 5).expect("provider execution");
        let completion_provider_custody = derive_completion_provider_custody(
            machine_code::BoundaryExecutionRecord::AdmittedProvider(provider_execution),
            &completion_claim_sources,
            &completion_receipts,
        )
        .unwrap_or_default();
        BoundarySettlementRecord {
            psi_operation: semantic_vocabulary::OperationId::new(1).expect("operation"),
            boundary: semantic_vocabulary::BoundaryMachineId::new(1).expect("boundary"),
            execution: machine_code::BoundaryExecutionRecord::AdmittedProvider(provider_execution),
            realization: target_operations::BoundaryRealization::MetadataOnlyPort(
                target_operations::MetadataOnlyPortRealization {
                    effect_operation: semantic_vocabulary::OperationId::new(2)
                        .expect("effect operation"),
                    service: semantic_vocabulary::ServiceId::new(1).expect("service"),
                    port: 0x20,
                    value: 0x20,
                },
            ),
            scalar_arguments: Vec::new(),
            runtime_scalar_arguments: Vec::new(),
            arguments,
            byte_sequence_arguments: Vec::new(),
            completion_claim_sources,
            completion_receipts,
            completion_provider_custody,
            native_result: machine_code::BoundaryResultRecord::Unit,
            operation_ordinal: 1,
            code_offset: 0,
            byte_count: 0,
        }
    }

    fn receipt(claim: u64, argument_index: u32) -> CompletionReceipt {
        CompletionReceipt {
            claim: ClaimId::new(claim).expect("claim"),
            argument_index,
        }
    }

    fn argument(place: u64) -> StructuralArgument {
        StructuralArgument {
            access: terminal_psi::StructuralAccess::Owned,
            place: semantic_vocabulary::PlaceId::new(place).expect("place"),
            path: Vec::new(),
        }
    }

    fn source(claim: u64, place: u64) -> CompletionClaimSource {
        let claim = ClaimId::new(claim).expect("claim");
        CompletionClaimSource {
            claim,
            entry: Some(terminal_psi::EntryClaim {
                claim,
                input: semantic_vocabulary::PlaceId::new(place).expect("place"),
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
            access: terminal_psi::StructuralAccess::Owned,
            place: semantic_vocabulary::PlaceId::new(1).expect("place"),
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
        let content_root = CompletionClaimSource {
            claim,
            entry: None,
            content: Some(terminal_psi::ContentEntryClaim {
                claim,
                input: semantic_vocabulary::ContentStructuralPlace {
                    version: ContentPlaceVersion::Entry,
                    root: semantic_vocabulary::PlaceId::new(1).expect("place"),
                    segments: vec![ContentPlaceSegment::FixedIndex(1)],
                },
                projections: vec![terminal_psi::ClaimContentProjection {
                    projection: semantic_vocabulary::ContentProjectionIdentity {
                        domain: semantic_vocabulary::ContentDomainId::new(1).expect("domain"),
                        projection_report_fingerprint: 7,
                    },
                    algebra: semantic_vocabulary::ContentAlgebra {
                        kind: semantic_vocabulary::ContentAlgebraKind::CountedQuantity,
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
        combined.entry = Some(terminal_psi::EntryClaim {
            claim,
            input: semantic_vocabulary::PlaceId::new(1).expect("place"),
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

    #[test]
    fn complete_custody_replay_preserves_validation_precedence() {
        let valid = settlement(vec![argument(1)], vec![source(1, 1)], vec![receipt(1, 0)]);
        assert_eq!(validate_completion_custody(&valid), Ok(()));

        let mut invalid_argument = valid.clone();
        invalid_argument.arguments[0].path = vec![StructuralPathSegment::Field(String::new())];
        invalid_argument.completion_receipts[0].argument_index = 2;
        assert_eq!(
            validate_completion_custody(&invalid_argument),
            Err(CompletionCustodyError::ArgumentPath)
        );

        let mut invalid_index = valid.clone();
        invalid_index.completion_receipts[0].argument_index = 2;
        assert_eq!(
            validate_completion_custody(&invalid_index),
            Err(CompletionCustodyError::ReceiptArgumentIndex)
        );

        let mut invalid_receipt = valid.clone();
        invalid_receipt.completion_receipts[0].claim = ClaimId::new(2).expect("other claim");
        assert_eq!(
            validate_completion_custody(&invalid_receipt),
            Err(CompletionCustodyError::ReceiptCustody)
        );

        let mut invalid_provider = valid;
        invalid_provider.completion_provider_custody[0]
            .provider_execution
            .provider_plan_report_identity = 9;
        assert_eq!(
            validate_completion_custody(&invalid_provider),
            Err(CompletionCustodyError::ProviderCustody)
        );

        let mut role_substitution = settlement(Vec::new(), Vec::new(), Vec::new());
        role_substitution.execution = machine_code::BoundaryExecutionRecord::CompilerBuiltin(
            target_operations::CompilerBuiltinExecution::LinuxExitGroupI32,
        );
        assert_eq!(
            validate_completion_custody(&role_substitution),
            Err(CompletionCustodyError::ProviderCustody)
        );

        let mut reverse_role_substitution = settlement(Vec::new(), Vec::new(), Vec::new());
        reverse_role_substitution.realization =
            target_operations::BoundaryRealization::LinuxWriteByteI32(Default::default());
        assert_eq!(
            validate_completion_custody(&reverse_role_substitution),
            Err(CompletionCustodyError::ProviderCustody)
        );
    }
}
