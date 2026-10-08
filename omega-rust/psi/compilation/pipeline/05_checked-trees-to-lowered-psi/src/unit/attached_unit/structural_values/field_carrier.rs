//! Resolve an aggregate field's structural operand carrier for either schedule.
//! Nominal children name a structural declaration; byte fields embed their
//! carrier contract. Resolution supplies storage identity only: the completed
//! operand must still retain its own access, qualifications, and transfer custody.

use crate::LoweringError;
use checked_trees::{CheckedTrees, types::TypeReferenceHandle};
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{ByteSequenceCarrier, StructuralFieldType, StructuralTypeDeclaration};

pub(crate) fn resolve(
    checked: &CheckedTrees,
    declarations: &[StructuralTypeDeclaration],
    field: &StructuralFieldType,
    reference: TypeReferenceHandle,
) -> Result<StructuralTypeId, LoweringError> {
    if let Some(identity) = field.as_structural() {
        return Ok(identity);
    }
    match field.byte_sequence_carrier() {
        Some(ByteSequenceCarrier::BoundedOwned { .. }) => {
            let identity =
                validation::structural_type_identity(&checked.typed, reference, &[], &[]);
            terminal_psi::structural_type_by_identity(declarations, &identity)
                .map(|declaration| declaration.id)
                .ok_or(LoweringError::Unsupported(
                    "byte member has no retained carrier",
                ))
        }
        Some(ByteSequenceCarrier::BorrowedView { .. }) => {
            // The field embeds a shared descriptor contract, not a nominal
            // source alias. Its source loan and declared predicates remain
            // obligations on the completed structural operand.
            declarations
                .iter()
                .find_map(|declaration| {
                    declaration
                        .shape
                        .is_shared_readable_byte_view()
                        .then_some(declaration.id)
                })
                .ok_or(LoweringError::Unsupported(
                    "shared byte member has no descriptor carrier",
                ))
        }
        _ => Err(LoweringError::Unsupported(
            "structural operand has a nonstructural field destination",
        )),
    }
}
