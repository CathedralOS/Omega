//! Owned block arrivals in legalized input, rejoined for selection.
//!
//! An owned structural block parameter is a whole value that every incoming
//! edge moved into the block's own home. `block_entry` gives that home one
//! frame slot (`LocalStorageSlotId::StructuralBlockParameter`) and publishes
//! its address as the place's transport pointer, so a field read and a field
//! store reach the same storage. Construction and validation both rejoin the
//! store's retained destination here from the legalized function; neither
//! trusts the other's choice.

use legalized_operations::LegalizedScalarFunction;
use semantic_vocabulary::{BlockId, StructuralPlaceKind};
use terminal_psi::StructuralParameterDeclaration;

/// Whether `expected` is the owned arrival whose block home a field store
/// writes: the unique block parameter at that place, declared in `block`, not
/// the entry and not an address join, carrying an owned aggregate home, and
/// declared as that block's parameter in the structural catalog. A loan, a
/// join, or another block's arrival at the same place is not this destination.
pub(super) fn store_destination(
    source: &LegalizedScalarFunction,
    block: BlockId,
    expected: &StructuralParameterDeclaration,
) -> bool {
    let Some(signature) = source.structural.as_ref() else {
        return false;
    };
    let mut matching = source.blocks.iter().flat_map(|candidate| {
        candidate
            .structural_parameters
            .iter()
            .filter(|parameter| parameter.place == expected.place)
            .map(move |parameter| (candidate.id, parameter))
    });
    matching.next() == Some((block, expected))
        && matching.next().is_none()
        && block != source.entry_block
        && super::address_join_input::join(source, expected.place).is_none()
        && super::aggregate_result_input::block_parameter_shape(source, expected).is_some()
        && signature.structural_places.iter().any(|declaration| {
            declaration.id == expected.place
                && declaration.kind
                    == (StructuralPlaceKind::BlockParameter {
                        block,
                        position: expected.position,
                    })
        })
}

#[cfg(test)]
mod tests {
    use super::store_destination;
    use crate::tests::legalization::owned_block_store::{
        ARRIVAL, ENTRY, HOME, arrival, id, legalized_arrival,
    };
    use semantic_vocabulary::StructuralPlaceKind;
    use terminal_psi::StructuralAccess;

    #[test]
    fn store_destination_is_the_exact_arrival_of_its_own_block() {
        let function = legalized_arrival();
        assert!(store_destination(&function, id(ARRIVAL), &arrival()));
        // Another block's identity for the same declaration.
        assert!(!store_destination(&function, id(ENTRY), &arrival()));
        // A substituted place, or a shared or unrestricted declaration.
        let mut substituted = arrival();
        substituted.place = id(HOME);
        assert!(!store_destination(&function, id(ARRIVAL), &substituted));
        let mut shared = arrival();
        shared.access = StructuralAccess::SharedBorrow;
        assert!(!store_destination(&function, id(ARRIVAL), &shared));
        let mut copied = arrival();
        copied.multiplicity = terminal_psi::StructuralMultiplicity::Unrestricted;
        assert!(!store_destination(&function, id(ARRIVAL), &copied));
        // The catalog must declare the place as this block's parameter.
        let mut misdeclared = function.clone();
        let signature = misdeclared.structural.as_mut().unwrap();
        let place = signature
            .structural_places
            .iter_mut()
            .find(|declaration| declaration.id == arrival().place)
            .unwrap();
        place.kind = StructuralPlaceKind::BlockParameter {
            block: id(ARRIVAL),
            position: 1,
        };
        assert!(!store_destination(&misdeclared, id(ARRIVAL), &arrival()));
    }
}
