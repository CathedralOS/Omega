//! Graph instruction checks that do not belong to shared call contracts.

use crate::{LegalizedScalarInstruction, LegalizedScalarInstructionKind};
use calling_conventions::ValueShape;
use terminal_psi::StructuralMultiplicity;

impl LegalizedScalarInstruction {
    pub fn has_valid_hosted_read_byte_shape(&self) -> bool {
        let LegalizedScalarInstructionKind::HostedReadByte { result, layout, .. } = &self.kind
        else {
            return false;
        };
        self.result.is_none()
            && self.ownership
                == [native_vocabulary::provenance::OwnershipEvent::ClaimCompletion(Vec::new())]
            && result.multiplicity == StructuralMultiplicity::Affine
            && result.qualifications.is_empty()
            && result.projected_qualifications.is_empty()
            && result.claims.is_empty()
            && layout.shape == ValueShape::integer(8, 4)
            && layout.tag_byte_offset == 0
            && layout.tag_shape == ValueShape::integer(4, 4)
            && layout.common_fields.is_empty()
            && layout.payload_byte_offset == 4
            && layout.cases.len() == 2
            && layout.cases[0].fields.is_empty()
            && layout.cases[1].fields.as_slice()
                == [calling_conventions::PackedFieldLayout {
                    shape: ValueShape::integer(4, 4),
                    byte_offset: 4,
                }]
    }
}
