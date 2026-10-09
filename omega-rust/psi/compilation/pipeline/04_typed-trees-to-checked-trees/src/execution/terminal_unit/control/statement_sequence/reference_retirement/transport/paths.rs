//! Translate nominal loan owner paths to the checked structural namespace.

use checked_trees::{BorrowLoanOwnerSegment, CheckedUnitStructuralPathSegment};
use typed_trees::data::DataMember;

/// Translate an occurrence's symbolic owner path through its declarations.
/// Common fields and active payload fields keep distinct nominal namespaces.
pub(super) fn carrier_path(
    program: &dyn ::validation::CheckingInput,
    mut reference: typed_trees::types::TypeReferenceHandle,
    owner_path: &[BorrowLoanOwnerSegment],
) -> Option<Vec<CheckedUnitStructuralPathSegment>> {
    let mut output = Vec::with_capacity(owner_path.len());
    let mut payload = None;
    for segment in owner_path {
        reference = validation::peel_declared_constraints(program, reference);
        match segment {
            BorrowLoanOwnerSegment::FixedIndex(element_index) => {
                let typed_trees::types::TypeReferenceNode::FixedArray {
                    element_type,
                    length,
                } = program.type_reference_table.type_reference(reference)
                else {
                    return None;
                };
                let typed_trees::types::FixedArrayLength::Literal(length) = length else {
                    return None;
                };
                if element_index >= length || payload.is_some() {
                    return None;
                }
                output.push(CheckedUnitStructuralPathSegment::FixedIndex(
                    u64::try_from(*element_index).ok()?,
                ));
                reference = *element_type;
            }
            BorrowLoanOwnerSegment::Case(symbol) => {
                let owner = program
                    .type_reference_table
                    .unparameterized_type_symbol(reference)?;
                let data = program.data_definition_by_symbol(*owner)?;
                if payload.is_some() {
                    return None;
                }
                let variant =
                    program
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Variant(variant) if variant.symbol == *symbol => {
                                Some(variant)
                            }
                            _ => None,
                        })?;
                output.push(CheckedUnitStructuralPathSegment::Case(
                    variant.path_identity(),
                ));
                payload = Some(variant);
            }
            BorrowLoanOwnerSegment::Field(symbol) => {
                let field = if let Some(variant) = payload.take() {
                    program
                        .data_payload_fields(variant)
                        .iter()
                        .find(|field| field.symbol == *symbol)?
                } else {
                    let owner = program
                        .type_reference_table
                        .unparameterized_type_symbol(reference)?;
                    let data = program.data_definition_by_symbol(*owner)?;
                    program
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Field(field) if field.symbol == *symbol => Some(field),
                            _ => None,
                        })?
                };
                output.push(CheckedUnitStructuralPathSegment::Field(
                    field.path_identity(),
                ));
                reference = field.type_reference;
            }
            BorrowLoanOwnerSegment::DynamicIndex => return None,
        }
    }
    payload.is_none().then_some(output)
}
