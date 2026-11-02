//! Ordered de-duplication of provenance and warning rosters.

/// Append each addition `target` does not already hold, in first-seen order.
pub(crate) fn extend_unique<T: PartialEq>(
    target: &mut Vec<T>,
    additions: impl IntoIterator<Item = T>,
) {
    for addition in additions {
        push_unique(target, addition);
    }
}

/// Append `addition` unless `target` already holds it.
pub(crate) fn push_unique<T: PartialEq>(target: &mut Vec<T>, addition: T) {
    if !target.contains(&addition) {
        target.push(addition);
    }
}
