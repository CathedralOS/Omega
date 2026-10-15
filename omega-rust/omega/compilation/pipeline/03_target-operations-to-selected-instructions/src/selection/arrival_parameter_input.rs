//! Rejoin a store's owned or borrowed parameter independently of its carrier.
//!
//! An arrival can transport a referent address or a view descriptor. The store
//! keeps its exact semantic declaration in either case; transport chooses how
//! to reach its backing storage only after this identity and permission check.

use legalized_operations::LegalizedScalarFunction;
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration};

pub(super) fn store_destination(
    source: &LegalizedScalarFunction,
    expected: &StructuralParameterDeclaration,
) -> bool {
    let Some(signature) = source.structural.as_ref() else {
        return false;
    };
    if !matches!(
        expected.access,
        StructuralAccess::Owned | terminal_psi::exclusive_borrow_pair_pattern!()
    ) || expected.multiplicity == StructuralMultiplicity::Linear
        || signature
            .entry_claims
            .iter()
            .any(|claim| claim.input == expected.place)
    {
        return false;
    }
    // The store's receiving and membership obligations were replayed against
    // the current graph. Rejoin the complete declaration here; qualification
    // neither changes the arrival's address nor grants access or claims.
    parameter(source, expected.place) == Some(expected)
        && match signature
            .structural_place(expected.place)
            .map(|row| row.kind)
        {
            Some(StructuralPlaceKind::BlockParameter { block, .. }) => match expected.access {
                StructuralAccess::Owned => {
                    super::owned_block_home_input::store_destination(source, block, expected)
                }
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow => true,
                _ => false,
            },
            _ => true,
        }
}

/// Rejoin the declared arrival independently of its physical carrier. Atomic
/// access and ordinary stores apply their own permissions after this identity
/// check, so a block-local spelling cannot borrow the invocation's authority.
pub(super) fn parameter(
    source: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<&StructuralParameterDeclaration> {
    let signature = source.structural.as_ref()?;
    let mut places = signature
        .structural_places
        .iter()
        .filter(|row| row.id == place);
    let declaration = places.next()?;
    if places.next().is_some() {
        return None;
    }
    let mut parameters = signature
        .parameters
        .iter()
        .map(|row| &row.semantic)
        .chain(source.block_structural_parameters())
        .filter(|parameter| parameter.place == place);
    let parameter = parameters.next()?;
    if parameters.next().is_some() {
        return None;
    }
    let matches = match declaration.kind {
        StructuralPlaceKind::Parameter { position } => {
            position == parameter.position
                && signature
                    .parameters
                    .get(position as usize)
                    .is_some_and(|row| row.semantic == *parameter)
        }
        StructuralPlaceKind::BlockParameter { block, position } => {
            block != source.entry_block
                && position == parameter.position
                && source
                    .block(block)
                    .and_then(|row| row.structural_parameters.get(position as usize))
                    == Some(parameter)
        }
        _ => false,
    };
    matches.then_some(parameter)
}
