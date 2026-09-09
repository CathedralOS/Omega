use super::shared::*;
use super::structural_types::*;

pub(super) fn encode_structural_contract(
    bytes: &mut Vec<u8>,
    function: &LegalizedStructuralContract,
) {
    match &function.result {
        None => bytes.push(0),
        Some(result) => {
            bytes.push(1);
            super::projected_structural_call_return::encode_result(bytes, result);
        }
    }
    encode_len(bytes, function.structural_types.len());
    for declaration in &function.structural_types {
        encode_structural_type(bytes, declaration);
    }
    encode_len(bytes, function.parameters.len());
    for parameter in &function.parameters {
        encode_structural_parameter(bytes, &parameter.semantic);
        encode_target_structural_parameter(bytes, &parameter.target);
    }
    encode_len(bytes, function.structural_places.len());
    for place in &function.structural_places {
        encode_structural_place(bytes, *place);
    }
    encode_len(bytes, function.entry_claims.len());
    for claim in &function.entry_claims {
        encode_entry_claim(bytes, claim);
    }
    encode_ids(
        bytes,
        function
            .published_service_ceiling
            .iter()
            .map(|service| service.get()),
    );
}
