//! Input-only custody for silent boundary invocations under provider custody.
//! Every structural place is a provider attachment of this function's own
//! attachment or a byte-sequence literal with its exact establishment row; the
//! call itself projects through the ordinary call/installed lanes.
use legalized_operations::{LegalizedScalarFunction, LegalizedScalarInstructionKind};
use semantic_vocabulary::StructuralPlaceKind;

pub(super) fn accepts(source: &LegalizedScalarFunction) -> bool {
    let Some(signature) = &source.structural else {
        return false;
    };
    if !signature.parameters.is_empty()
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
    let literals = signature
        .structural_places
        .iter()
        .filter(|declaration| {
            matches!(
                declaration.kind,
                StructuralPlaceKind::ByteSequenceLiteral { .. }
            )
        })
        .count();
    literals > 0
        && signature
            .structural_places
            .iter()
            .all(|declaration| match declaration.kind {
                StructuralPlaceKind::ProviderAttachment { attachment, .. } => {
                    source.attachment == Some(attachment)
                }
                StructuralPlaceKind::ByteSequenceLiteral { .. } => source
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .any(|row| {
                        matches!(
                            &row.kind,
                            LegalizedScalarInstructionKind::EstablishByteSequenceLiteral {
                                destination,
                                ..
                            } if destination == declaration
                        )
                    }),
                _ => false,
            })
}
