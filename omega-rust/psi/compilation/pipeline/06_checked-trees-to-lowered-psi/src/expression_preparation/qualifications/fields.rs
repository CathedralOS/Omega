//! Join stored scalar membership to exact declared field identities.
//! The storage predicate is only the numeric component. Construction and writes
//! establish nominal membership from actual operands, and reads capture it on
//! immutable SSA results; the receiving annotation itself establishes nothing.

use super::PreparedScalarQualifications;
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedTrees;
use checked_trees::types::TypeReferenceHandle;
use semantic_vocabulary::StructuralFieldId;

pub(crate) type FieldType = (StructuralFieldId, TypeReferenceHandle);

impl PreparedScalarQualifications {
    pub(super) fn retain_fields(
        &mut self,
        checked: &CheckedTrees,
        fields: &[FieldType],
    ) -> Result<(), LoweringError> {
        for (field, reference) in fields {
            let value_type = self.value_type(checked, *reference)?;
            if !value_type.qualifications.is_empty() {
                self.catalog
                    .fields
                    .push(terminal_psi::ScalarFieldQualification {
                        field: *field,
                        value_type,
                    });
            }
        }
        self.catalog.fields.sort_by_key(|row| row.field);
        if self
            .catalog
            .fields
            .windows(2)
            .any(|pair| pair[0].field == pair[1].field)
        {
            return unsupported("scalar field membership has duplicate declarations");
        }
        Ok(())
    }
}
