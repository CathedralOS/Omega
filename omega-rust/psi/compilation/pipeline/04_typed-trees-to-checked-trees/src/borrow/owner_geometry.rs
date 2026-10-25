//! Translate nominal loan owner paths to the checked structural namespace.
//! Prepared application fields retain their original member symbols, but their
//! child types include the exact type and region operands. Following the raw
//! generic declaration would lose those operands at the next nested field.

use checked_trees::{BorrowLoanOwnerSegment, CheckedUnitStructuralPathSegment};
use typed_trees::data::DataMember;

/// Declaration selectors of a completed loan relation. The caller has already
/// selected one carrier; these identities translate its prepared namespace,
/// not the identity of unrelated storage or the value of an unknown index.
pub(crate) fn structural_owner_path(
    program: &dyn ::validation::CheckingInput,
    path: &[BorrowLoanOwnerSegment],
) -> Option<Vec<CheckedUnitStructuralPathSegment>> {
    path.iter()
        .map(|segment| match segment {
            BorrowLoanOwnerSegment::Field(symbol) => Some(CheckedUnitStructuralPathSegment::Field(
                program.any_field_owner(*symbol)?.1.member_identity(),
            )),
            BorrowLoanOwnerSegment::Case(symbol) => Some(CheckedUnitStructuralPathSegment::Case(
                program.variant_owner(*symbol)?.1.member_identity(),
            )),
            BorrowLoanOwnerSegment::FixedIndex(index) => Some(
                CheckedUnitStructuralPathSegment::FixedIndex(u64::try_from(*index).ok()?),
            ),
            BorrowLoanOwnerSegment::DynamicIndex => None,
        })
        .collect()
}

/// Translate an occurrence's symbolic owner path through its declarations.
/// Common fields and active payload fields keep distinct nominal namespaces.
/// A stored descriptor can belong to a borrowed containing record or array.
/// Owner selectors name that declared storage, so unwrap its reference shells
/// as `origin_owner_path` does in the reverse direction. This translates an
/// already captured loan path; it neither forms a borrow nor grants access.
pub(crate) fn carrier_path(
    program: &dyn ::validation::CheckingInput,
    mut reference: typed_trees::types::TypeReferenceHandle,
    owner_path: &[BorrowLoanOwnerSegment],
) -> Option<Vec<CheckedUnitStructuralPathSegment>> {
    let mut output = Vec::with_capacity(owner_path.len());
    let mut payload = None;
    for segment in owner_path {
        reference = program
            .type_reference_table
            .peeled_type_reference(reference);
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
                let data = program.closed_data_definition(reference)?;
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
                    checked_trees::CheckedMemberIdentity::of_member(
                        variant.identity,
                        variant.name.clone(),
                    ),
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
                    let data = program.closed_data_definition(reference)?;
                    program
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            DataMember::Field(field) if field.symbol == *symbol => Some(field),
                            _ => None,
                        })?
                };
                output.push(CheckedUnitStructuralPathSegment::Field(
                    checked_trees::CheckedMemberIdentity::of_member(
                        field.identity,
                        field.name.clone(),
                    ),
                ));
                reference = field.type_reference;
            }
            BorrowLoanOwnerSegment::DynamicIndex => return None,
        }
    }
    payload.is_none().then_some(output)
}
