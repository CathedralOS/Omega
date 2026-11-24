//! Copy a record from a byte array only after its decoded contents satisfy the declaration.

use super::{
    OperationFlow, RuntimeRows, ScalarType, StructuralPathSegment, StructuralRuntimePlace,
    StructuralScalarRuntimeField, TerminalExecution, TerminalInterpretError,
    TerminalStructuralValue, malformed,
};
use crate::storage::primitive_storage::encoding::decode;
use crate::storage::scalar_storage::ScalarStorageRows;
use terminal_semantics::scalar_qualifications::field_qualifications;
use terminal_semantics::structural::decoded_record::{
    DecodedRecordError, check_decoded_record_requirements,
};
use terminal_semantics::{byte_carrier_scalar_layout, reference_storage_byte_width};

impl TerminalExecution<'_> {
    pub(super) fn byte_carrier_leaf_copy(
        &mut self,
        leaf: &TerminalStructuralValue,
        result: &terminal_psi::StructuralOperationResult,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let Some((StructuralPathSegment::FixedIndex(index), parent_path)) = leaf.path.split_last()
        else {
            return Err(malformed());
        };
        if !result.is_unrestricted_plain()
            || !result.additional_input_qualifications.is_empty()
            || !result.qualification_establishments.is_empty()
        {
            return Err(malformed());
        }
        let declaration = self.structural_declaration(result.structural_type)?;
        let fields = declaration.shape.record_fields().ok_or_else(malformed)?;
        let array = StructuralRuntimePlace {
            opaque_identity: leaf.opaque_identity,
            path: parent_path.to_vec(),
        };
        let bytes = self.structural_byte_storage.require(&array)?;
        let layout =
            byte_carrier_scalar_layout(self.structural_types().values(), result.structural_type)
                .ok_or(malformed())?;
        if !layout.fits_at(*index, u64::try_from(bytes.len()).map_err(|_| malformed())?) {
            return Err(TerminalInterpretError::StructuralContentsResultInvalid {
                reason: "the byte array does not contain the complete record footprint",
            });
        }
        let index = usize::try_from(*index).map_err(|_| malformed())?;
        let mut fields_to_publish = Vec::with_capacity(fields.len());
        for (declared, member) in fields.iter().zip(&layout.leaves) {
            if !field_qualifications(&self.program.catalogs, declared.id).is_empty() {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "raw bytes do not establish a declared scalar field domain",
                ));
            }
            let offset = index
                .checked_add(usize::try_from(member.offset).map_err(|_| malformed())?)
                .ok_or(malformed())?;
            let size = usize::from(
                reference_storage_byte_width(member.scalar_type).ok_or_else(malformed)?,
            );
            let end = offset.checked_add(size).ok_or(malformed())?;
            let window = bytes.read_window(offset, end)?;
            let mut value = 0u128;
            for position in 0..size {
                value |= u128::from(window.read(position)?) << (8 * position);
            }
            let scalar = decode(member.scalar_type, value)?;
            if !scalar.satisfies_field(&declared.field_type) {
                return Err(TerminalInterpretError::StructuralContentsResultInvalid {
                    reason: "a decoded scalar is outside the field's declared type",
                });
            }
            fields_to_publish.push((member.field, scalar));
        }
        check_decoded_record_requirements(
            &declaration.requires,
            &mut fields_to_publish,
            |(id, _)| *id,
            |(_, scalar)| scalar.literal(),
        )
        .map_err(|error| match error {
            DecodedRecordError::Violated => {
                TerminalInterpretError::StructuralContentsResultInvalid {
                    reason: error.reason(),
                }
            }
            DecodedRecordError::Unsupported(reason) => {
                TerminalInterpretError::UnsupportedSemanticVariant(reason)
            }
        })?;
        let mut cursor = self.local_structural_identities.cursor();
        let copied = TerminalStructuralValue::minted(
            self.local_structural_identities
                .allocate_staged(&mut cursor)?,
            result.structural_type,
            Vec::new(),
        );
        let parent = StructuralRuntimePlace::from(&copied);
        let mut result_row =
            RuntimeRows::from_rows(self.account.as_ref(), [(result.place, copied)])?;
        let mut cells = ScalarStorageRows::new(self.account.as_ref());
        for (field, scalar) in fields_to_publish {
            cells.insert(
                StructuralScalarRuntimeField::member(parent.clone(), field),
                scalar,
            )?;
        }
        self.structural_scalar_fields.append(&mut cells)?;
        self.structural_values.append(&mut result_row)?;
        self.local_structural_identities.commit_cursor(cursor);
        Ok(OperationFlow::Advance)
    }
}
