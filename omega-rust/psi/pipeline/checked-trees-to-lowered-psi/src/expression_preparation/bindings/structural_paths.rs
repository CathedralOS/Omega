use checked_trees::CheckedUnitStructuralPathSegment;
use terminal_psi::StructuralPathSegment;

pub(crate) fn lower_structural_path(
    path: &[CheckedUnitStructuralPathSegment],
) -> Vec<StructuralPathSegment> {
    path.iter()
        .map(|segment| match segment {
            CheckedUnitStructuralPathSegment::Referent => StructuralPathSegment::Referent,
            CheckedUnitStructuralPathSegment::Field(identity) => {
                StructuralPathSegment::Field(identity.clone())
            }
            CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                StructuralPathSegment::FixedIndex(*index)
            }
            CheckedUnitStructuralPathSegment::RuntimeIndex {
                selector,
                minimum,
                maximum,
            } => StructuralPathSegment::RuntimeIndex {
                selector: *selector,
                minimum: *minimum,
                maximum: *maximum,
            },
            CheckedUnitStructuralPathSegment::FixedByteRange { start, end } => {
                StructuralPathSegment::FixedByteRange {
                    start: *start,
                    end: *end,
                }
            }
        })
        .collect()
}
