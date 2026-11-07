/*
We pair each semantic review row with the exact declaration that produced it
while checked state is projected.

These carriers retain compiler handles, source spans, and the producing
declaration so canonical sorting moves both together and consumers read the
declaration's own tables rather than rediscovering it from reduced identities.
They never enter the stable evidence vocabulary or canonical persistence
boundary.
*/

#[derive(Debug, Clone)]
pub(crate) struct ProjectedReviewRow<'a, Row, Declaration> {
    pub(crate) row: Row,
    pub(crate) declaration: &'a Declaration,
}
