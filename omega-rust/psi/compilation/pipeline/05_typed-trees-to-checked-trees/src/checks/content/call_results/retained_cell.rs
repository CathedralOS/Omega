//! Extraction returns the qualified value already owned by a resident or run.
//! Identify its exact payload slot after the caller authenticates the core
//! operation and its complete applied result type. Shape alone is not authority.

use facts::PlaceSegment;
use typed_trees::data::DataMember;
use typed_trees::types::TypeReferenceHandle;

pub(super) fn is_extracted_value_path(
    program: &dyn validation::CheckingInput,
    result: TypeReferenceHandle,
    element: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> bool {
    let Some(outcome) = program
        .data_application_definition(result)
        .or_else(|| program.closed_data_definition(result))
    else {
        return false;
    };
    let [DataMember::Variant(moved)] = program.data_members(outcome) else {
        return false;
    };
    let [_storage, value] = program.data_payload_fields(moved) else {
        return false;
    };
    let [
        PlaceSegment::Case { variant },
        PlaceSegment::Field { symbol },
        ..,
    ] = path
    else {
        return false;
    };
    *variant == moved.symbol
        && *symbol == value.symbol
        && program.normalized_type_identity(value.type_reference)
            == program.normalized_type_identity(element)
}
