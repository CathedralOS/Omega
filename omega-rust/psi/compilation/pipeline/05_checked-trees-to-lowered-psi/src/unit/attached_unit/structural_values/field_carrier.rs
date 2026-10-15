//! Resolve an aggregate field's structural operand carrier for either schedule.
//! Nominal children name a structural declaration; byte fields embed their
//! carrier contract. Resolution supplies storage identity only: the completed
//! operand must still retain its own access, qualifications, and transfer custody.

use crate::LoweringError;
use checked_trees::{CheckedTrees, types::TypeReferenceHandle};
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    ByteSequenceCarrier, StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape,
};

pub(crate) fn resolve(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    declarations: &[StructuralTypeDeclaration],
    field: &StructuralFieldType,
    reference: TypeReferenceHandle,
) -> Result<StructuralTypeId, LoweringError> {
    if let Some(identity) = field.as_structural() {
        return Ok(identity);
    }
    let Some(carrier) = field.byte_sequence_carrier() else {
        return Err(LoweringError::Unsupported(
            "structural operand has a nonstructural field destination",
        ));
    };
    let identity = validation::structural_type_identity(&checked.typed, reference, &[], &[]);
    let declaration = crate::terminal_identities::structural_declaration_by_identity(
        declarations,
        identities,
        checked,
        identity,
        "byte member has no retained carrier",
    )?;
    // The declaration owns the descriptor family and fixed extent. Access is
    // bound by the field and completed operand, never selected by an arbitrary
    // readable-view declaration from the catalog.
    let expected = match carrier {
        ByteSequenceCarrier::BorrowedView { .. } => {
            ByteSequenceCarrier::BorrowedView { access: None }
        }
        ByteSequenceCarrier::BoundedView { capacity, .. } => ByteSequenceCarrier::BoundedView {
            access: None,
            capacity,
        },
        ByteSequenceCarrier::BoundedOwned { .. } => carrier,
    };
    if declaration.shape != StructuralTypeShape::ByteSequence(expected) {
        return Err(LoweringError::Unsupported(
            "byte member carrier differs from its retained declaration",
        ));
    }
    Ok(declaration.id)
}
