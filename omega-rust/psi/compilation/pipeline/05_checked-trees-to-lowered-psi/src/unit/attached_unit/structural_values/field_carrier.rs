//! Resolve an aggregate field's structural operand carrier for either schedule.
//! Nominal children name a structural declaration; byte fields embed their
//! carrier contract. Resolution supplies storage identity only: the completed
//! operand must still retain its own access, qualifications, and transfer custody.

use crate::LoweringError;
use checked_trees::{CheckedTrees, types::TypeReferenceHandle};
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralFieldType, StructuralTypeDeclaration,
    StructuralTypeShape,
};

pub(crate) fn resolve(
    checked: &CheckedTrees,
    declarations: &[StructuralTypeDeclaration],
    field: &StructuralFieldType,
    reference: TypeReferenceHandle,
) -> Result<StructuralTypeId, LoweringError> {
    if let StructuralFieldType::Structural(identity) = field {
        return Ok(*identity);
    }
    match field.byte_sequence_carrier() {
        Some(carrier @ ByteSequenceCarrier::BoundedOwned { .. }) => {
            let identity =
                validation::structural_type_identity(&checked.typed, reference, &[], &[]);
            let declaration = declarations
                .iter()
                .find(|declaration| declaration.identity == identity)
                .ok_or(LoweringError::Unsupported(
                    "byte member has no retained carrier",
                ))?;
            if declaration.shape != StructuralTypeShape::ByteSequence(carrier) {
                return Err(LoweringError::Unsupported(
                    "byte member changed its declared carrier",
                ));
            }
            Ok(declaration.id)
        }
        Some(ByteSequenceCarrier::BorrowedView { .. }) => {
            // The field embeds a shared descriptor contract, not a nominal
            // source alias. Its source loan and declared predicates remain
            // obligations on the completed structural operand.
            declarations
                .iter()
                .find_map(|declaration| {
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                            access: None | Some(StructuralAccess::SharedBorrow),
                        })
                    )
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
