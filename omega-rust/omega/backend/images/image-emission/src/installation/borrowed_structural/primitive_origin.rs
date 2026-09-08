//! Primitive local origin and referent extent in installation call records.
use crate::installation::{InstallationRecord, InstalledFunction};
use calling_conventions::ValueShape;
use machine_code::{InternalUnitCallArgumentRecord, SemanticCodeSite, StructuralSourceLocation};
use semantic_vocabulary::OperationId;
use terminal_psi::StructuralAccess;

pub(super) fn source_is_exact(
    record: &InstallationRecord,
    function: &InstalledFunction,
    argument: &InternalUnitCallArgumentRecord,
    producer: OperationId,
    frame_bytes: u32,
) -> bool {
    // Interleaved preservation instructions can split a semantic span.
    // Exact producer identity and initialization are checked against the retained image.
    record.semantic_code_attribution.iter().any(|row| {
        row.machine == function.machine
            && row.attribution.site == SemanticCodeSite::Operation(producer)
            && row.attribution.byte_count > 0
    }) && local_primitive_source_is_exact(argument, frame_bytes)
}

pub(super) fn local_primitive_source_is_exact(
    argument: &machine_code::InternalUnitCallArgumentRecord,
    frame_bytes: u32,
) -> bool {
    let StructuralSourceLocation::Stack { byte_offset } = argument.source_location else {
        return false;
    };
    matches!(
        argument.access,
        StructuralAccess::SharedBorrow
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow
    ) && argument.path.is_empty()
        && argument.source_byte_offset == 0
        && argument.root_structural_type == argument.structural_type
        && matches!(argument.shape.byte_size, 1 | 2 | 4 | 8)
        && argument.shape
            == ValueShape::borrowed_reference(argument.shape.byte_size, argument.shape.byte_size)
        && byte_offset.is_multiple_of(u32::from(argument.shape.alignment))
        && byte_offset
            .checked_add(u32::from(argument.shape.byte_size))
            .is_some_and(|end| end <= frame_bytes)
}
