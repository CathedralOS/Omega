//! Canonical stack facts of an installed function, and the partial cleanup
//! paths a residual cleanup may name.

pub(crate) fn is_partial_cleanup_path(path: &[terminal_psi::StructuralPathSegment]) -> bool {
    !path.is_empty()
        && path.iter().all(|segment| match segment {
            terminal_psi::StructuralPathSegment::AllElements
            | terminal_psi::StructuralPathSegment::Referent
            | terminal_psi::StructuralPathSegment::RepresentationView { .. }
            | terminal_psi::StructuralPathSegment::Case(_)
            | terminal_psi::StructuralPathSegment::FixedByteRange { .. } => false,
            terminal_psi::StructuralPathSegment::Field(identity) => {
                *identity != terminal_psi::TerminalIdentityIndex::ABSENT
            }
            terminal_psi::StructuralPathSegment::FixedIndex(_)
            | terminal_psi::StructuralPathSegment::RuntimeIndex { .. } => true,
        })
}
