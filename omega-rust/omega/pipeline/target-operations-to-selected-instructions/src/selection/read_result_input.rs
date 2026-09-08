//! Input-only structural roster for owned results of admitted byte input.
use legalized_operations::{LegalizedScalarFunction, LegalizedScalarInstructionKind};
use semantic_vocabulary::StructuralPlaceKind;

pub(super) fn accepts(source: &LegalizedScalarFunction) -> bool {
    let Some(signature) = &source.structural else {
        return false;
    };
    if source.ranked.is_some()
        || !signature.parameters.is_empty()
        || !signature.entry_claims.is_empty()
        || source.parameters.len() != source.call_plan.parameters.len()
        || source
            .parameters
            .iter()
            .zip(&source.call_plan.parameters)
            .any(|(parameter, placement)| parameter.placement != *placement)
    {
        return false;
    }
    let results = source
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|row| match &row.kind {
            LegalizedScalarInstructionKind::HostedReadByte { result, .. } => Some((row, result)),
            _ => None,
        })
        .collect::<Vec<_>>();
    !results.is_empty()
        && results.iter().all(|(row, result)| {
            row.has_valid_hosted_read_byte_shape()
                && signature
                    .structural_places
                    .iter()
                    .filter(|declaration| {
                        declaration.id == result.place
                            && declaration.kind
                                == StructuralPlaceKind::OperationResult {
                                    producer: row.operation,
                                    structural_type: result.structural_type,
                                }
                    })
                    .count()
                    == 1
        })
        && signature
            .structural_places
            .iter()
            .all(|declaration| match declaration.kind {
                StructuralPlaceKind::ProviderAttachment { attachment, .. } => {
                    source.attachment == Some(attachment)
                }
                StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } => results.iter().any(|(row, result)| {
                    row.operation == producer
                        && result.place == declaration.id
                        && result.structural_type == structural_type
                }),
                _ => false,
            })
}
