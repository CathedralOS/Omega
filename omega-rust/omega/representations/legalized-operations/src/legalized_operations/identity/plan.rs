use super::calling::*;
use super::shared::*;
use super::structural::*;
use super::structural_types::*;

pub(super) fn encode_structural_unit_function(
    bytes: &mut Vec<u8>,
    function: &LegalizedStructuralUnitFunction,
    retain_call_contract: bool,
) {
    bytes.extend_from_slice(&function.machine.get().to_le_bytes());
    encode_option_id(
        bytes,
        function.attachment.map(|attachment| attachment.get()),
    );
    encode_ids(
        bytes,
        function
            .provenance
            .operations
            .iter()
            .map(|operation| operation.get()),
    );
    encode_ids(
        bytes,
        function.provenance.edges.iter().map(|edge| edge.get()),
    );
    bytes.push(match function.recipe {
        StructuralUnitLegalizationRecipe::ReturnUnitV1 => 0,
        StructuralUnitLegalizationRecipe::AuthoredCallThenReturnUnitV1 => 1,
        StructuralUnitLegalizationRecipe::InstalledProviderCallThenReturnUnitV1 => 2,
        StructuralUnitLegalizationRecipe::ClaimCompletionSettlementsThenReturnUnitV1 => 3,
    });
    encode_len(bytes, function.structural_types.len());
    for declaration in &function.structural_types {
        encode_structural_type(bytes, declaration);
    }
    encode_call_plan(bytes, &function.call_plan);
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
    bytes.extend_from_slice(&function.entry_block.get().to_le_bytes());
    encode_len(bytes, function.boundary_settlements.len());
    for settlement in &function.boundary_settlements {
        encode_boundary_settlement(bytes, settlement);
    }
    match &function.call {
        Some(call) => {
            bytes.push(1);
            encode_call_source(bytes, &call.source);
            bytes.extend_from_slice(&call.operation.get().to_le_bytes());
            bytes.extend_from_slice(&call.callee.get().to_le_bytes());
            encode_len(bytes, call.arguments.len());
            for argument in &call.arguments {
                encode_structural_argument(bytes, &argument.semantic);
                encode_target_structural_argument(bytes, &argument.target);
            }
            encode_len(bytes, call.claim_transfers.len());
            for transfer in &call.claim_transfers {
                bytes.extend_from_slice(&transfer.claim.get().to_le_bytes());
                bytes.extend_from_slice(&transfer.argument_index.to_le_bytes());
            }
            if retain_call_contract {
                encode_ids(
                    bytes,
                    call.requirement_obligations.iter().map(|value| value.get()),
                );
                let crash_routes =
                    terminal_codec::encode_crash_route_buckets(&call.crash_continuations)
                        .expect("verified legalized call crash continuations remain canonical");
                encode_len(bytes, crash_routes.len());
                bytes.extend_from_slice(&crash_routes);
            }
            encode_fuel(bytes, &call.fuel);
            encode_effect(bytes, call.effect);
            encode_ownership_roster(bytes, &call.ownership);
        }
        None => bytes.push(0),
    }
    bytes.extend_from_slice(&function.return_edge.get().to_le_bytes());
    encode_fuel(bytes, &function.return_fuel);
    encode_effect(bytes, function.return_effect);
    encode_ownership_roster(bytes, &function.return_ownership);
}
