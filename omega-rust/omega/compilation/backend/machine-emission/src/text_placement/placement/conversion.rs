use super::TextPlacementError;

/// Join the retained invocation owner to its selected instruction provenance.
/// Constructor charges may precede either kind of invocation, but cannot choose
/// the owner: normal calls retain their final operation, completions their edge.
pub(super) fn validate_boundary_invocation_site(
    site: semantic_vocabulary::InvocationSite,
    operations: &[semantic_vocabulary::OperationId],
    edges: &[semantic_vocabulary::EdgeId],
) -> Result<(), TextPlacementError> {
    let matches = match site {
        semantic_vocabulary::InvocationSite::Operation(operation) => {
            operations.last() == Some(&operation) && edges.is_empty()
        }
        semantic_vocabulary::InvocationSite::ExternalCompletion(edge) => edges == [edge],
    };
    if !matches {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    Ok(())
}

pub(super) fn usize_to_u64(value: usize) -> Result<u64, TextPlacementError> {
    u64::try_from(value).map_err(|_| TextPlacementError::OffsetOverflow)
}
