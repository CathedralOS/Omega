//! Ordered de-duplication of provenance and warning rosters.

/// Append each addition `target` does not already hold, in first-seen order.
pub(crate) fn extend_unique<T: PartialEq>(
    target: &mut Vec<T>,
    additions: impl IntoIterator<Item = T>,
) {
    for addition in additions {
        if !target.contains(&addition) {
            target.push(addition);
        }
    }
}
