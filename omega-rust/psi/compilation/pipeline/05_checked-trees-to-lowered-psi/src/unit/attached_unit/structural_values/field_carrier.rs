//! Resolve an aggregate field's structural operand carrier for either schedule.
//! Nominal children name a structural declaration; byte fields embed their
//! carrier contract. Resolution supplies storage identity only: the completed
//! operand must still retain its own access, qualifications, and transfer custody.

use crate::LoweringError;
use checked_trees::{CheckedTrees, types::TypeReferenceHandle};
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{StructuralFieldType, StructuralTypeDeclaration};

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
    if field.byte_sequence_carrier().is_none() {
        return Err(LoweringError::Unsupported(
            "structural operand has a nonstructural field destination",
        ));
    }
    let identity = validation::structural_type_identity(&checked.typed, reference, &[], &[]);
    let declaration = crate::terminal_identities::structural_declaration_by_identity(
        declarations,
        identities,
        checked,
        identity,
        "byte member has no retained carrier",
    )?;
    Ok(declaration.id)
}
