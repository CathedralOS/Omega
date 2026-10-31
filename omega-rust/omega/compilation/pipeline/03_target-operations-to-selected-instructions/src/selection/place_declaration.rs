//! The structural place roster's declaration query: which exact kind the
//! contract recorded for a place, and which block rosters carry it. One owner
//! keeps the equality; a consumer states the producer identity or block
//! position it already holds.
use legalized_operations::{LegalizedScalarFunction, LegalizedStructuralContract};
use semantic_vocabulary::{BlockId, OperationId, PlaceId, StructuralPlaceKind, StructuralTypeId};
use terminal_psi::StructuralParameterDeclaration;

/// Whether the roster declares `place` as the operation result `producer`
/// established with `structural_type`.
pub(crate) fn declares_operation_result(
    contract: &LegalizedStructuralContract,
    place: PlaceId,
    producer: OperationId,
    structural_type: StructuralTypeId,
) -> bool {
    contract
        .structural_place(place)
        .and_then(|declaration| declaration.kind.as_operation_result())
        == Some((producer, structural_type))
}

/// Whether the roster declares `place` as `block`'s parameter at `position`.
pub(crate) fn declares_block_parameter(
    contract: &LegalizedStructuralContract,
    place: PlaceId,
    block: BlockId,
    position: u32,
) -> bool {
    contract.structural_place(place).is_some_and(|declaration| {
        declaration.kind == StructuralPlaceKind::BlockParameter { block, position }
    })
}

/// Every block parameter declaring `place`, with the block that declares it.
pub(crate) fn arrivals<'a>(
    source: &'a LegalizedScalarFunction,
    place: PlaceId,
) -> impl Iterator<Item = (BlockId, &'a StructuralParameterDeclaration)> + 'a {
    source.blocks.iter().flat_map(move |block| {
        block
            .structural_parameters
            .iter()
            .filter(move |parameter| parameter.place == place)
            .map(move |parameter| (block.id, parameter))
    })
}
