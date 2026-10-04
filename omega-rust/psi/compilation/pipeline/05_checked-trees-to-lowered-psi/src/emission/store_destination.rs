//! Store destinations retain exact storage identity and operation authority.
//! Local field stores share root admission here; each operation separately
//! checks its replacement and the receiving frontier checks live custody.

use crate::lowering_error::{LoweringError, unsupported};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{StructuralAccess, StructuralMultiplicity};

/// Resolve a local destination once, independently of the value being stored.
/// An owned home and an exclusive loan both permit mutation; neither grants
/// permission to drop predicates or claims. The operation still checks the
/// destination's shape and the current ownership frontier.
pub(crate) fn local_store_root<'a>(
    symbol: symbols::SymbolHandle,
    locals: &'a [crate::expression_preparation::bindings::StructuralLocalBinding],
    operations: &'a crate::emission::operation_emission::buffer::OperationBuffer,
) -> Result<
    (
        &'a terminal_psi::StructuralArgument,
        &'a terminal_psi::StructuralOperationResult,
    ),
    LoweringError,
> {
    let crate::expression_preparation::bindings::StructuralLocalBinding { argument, .. } = locals
        .iter()
        .find(|local| local.root == facts::PlaceRoot::Symbol(symbol))
        .ok_or(LoweringError::Unsupported(
            "store names an unknown structural local",
        ))?;
    if !matches!(
        argument.access,
        StructuralAccess::Owned | StructuralAccess::MutableBorrow
    ) || !argument.path.is_empty()
    {
        return unsupported("store local is not a whole exclusive storage root");
    }
    let produced = operations
        .operations
        .iter()
        .filter_map(|operation| operation.result.structural())
        .find(|result| result.place == argument.place)
        .ok_or(LoweringError::Unsupported(
            "store local has no produced storage root",
        ))?;
    Ok((argument, produced))
}

/// Field replacement and restoration windows share the same root contract.
/// A store cannot preserve opaque root evidence merely by restoring a field.
/// Byte-owner writes instead retain their predicates in the byte operation.
pub(crate) fn unqualified_local_store_root(
    symbol: symbols::SymbolHandle,
    locals: &[crate::expression_preparation::bindings::StructuralLocalBinding],
    operations: &crate::emission::operation_emission::buffer::OperationBuffer,
) -> Result<(PlaceId, StructuralTypeId), LoweringError> {
    let (argument, produced) = local_store_root(symbol, locals, operations)?;
    if !matches!(
        produced.multiplicity,
        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
    ) || !produced.qualifications.is_empty()
        || !produced.projected_qualifications.is_empty()
        || !produced.claims.is_empty()
    {
        return unsupported(
            "store local carries custody or evidence the replacement does not restore",
        );
    }
    Ok((argument.place, produced.structural_type))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StoreDestination {
    Initialize {
        place: PlaceId,
        structural_type: StructuralTypeId,
    },
    Assign {
        place: PlaceId,
    },
}
