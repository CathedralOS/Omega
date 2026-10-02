//! Rejoin a store's borrowed parameter independently of its physical carrier.
//!
//! An arrival can transport a referent address or a view descriptor. The store
//! keeps its exact semantic declaration in either case; transport chooses how
//! to reach its backing storage only after this identity and permission check.

use legalized_operations::LegalizedScalarFunction;
use semantic_vocabulary::StructuralPlaceKind;
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
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    ) || expected.multiplicity == StructuralMultiplicity::Linear
        || !expected.qualifications.is_empty()
        || !expected.projected_qualifications.is_empty()
        || signature
            .entry_claims
            .iter()
            .any(|claim| claim.input == expected.place)
    {
        return false;
    }
    let mut places = signature
        .structural_places
        .iter()
        .filter(|row| row.id == expected.place);
    let Some(place) = places.next() else {
        return false;
    };
    if places.next().is_some() {
        return false;
    }
    let mut parameters = signature
        .parameters
        .iter()
        .map(|row| &row.semantic)
        .chain(
            source
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .filter(|parameter| parameter.place == expected.place);
    if parameters.next() != Some(expected) || parameters.next().is_some() {
        return false;
    }
    match place.kind {
        StructuralPlaceKind::Parameter { position, is_self } => {
            position == expected.position
                && is_self == expected.is_self
                && signature
                    .parameters
                    .iter()
                    .any(|row| row.semantic == *expected)
        }
        StructuralPlaceKind::BlockParameter { block, position } => {
            block != source.entry_block
                && !expected.is_self
                && position == expected.position
                && expected.access == StructuralAccess::MutableBorrow
                && source
                    .blocks
                    .iter()
                    .find(|row| row.id == block)
                    .and_then(|row| row.structural_parameters.get(position as usize))
                    == Some(expected)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::legalization::owned_block_store::{
        ARRIVAL, ENTRY, arrival, id, legalized_arrival,
    };

    fn borrowed_arrival() -> (LegalizedScalarFunction, StructuralParameterDeclaration) {
        let mut source = legalized_arrival();
        let mut parameter = arrival();
        parameter.access = StructuralAccess::MutableBorrow;
        parameter.multiplicity = StructuralMultiplicity::Unrestricted;
        *source
            .blocks
            .iter_mut()
            .find(|block| block.id == id(ARRIVAL))
            .unwrap()
            .structural_parameters
            .iter_mut()
            .find(|row| row.place == parameter.place)
            .unwrap() = parameter.clone();
        (source, parameter)
    }

    #[test]
    fn borrowed_store_arrival_identity_does_not_depend_on_carrier_shape() {
        let (mut source, parameter) = borrowed_arrival();
        assert!(store_destination(&source, &parameter));
        // The exact arrival is equally writable when its carrier is a descriptor.
        let types = source
            .structural
            .as_mut()
            .unwrap()
            .structural_types
            .make_mut();
        let declaration = types
            .iter_mut()
            .find(|row| row.id == parameter.structural_type)
            .unwrap();
        let mut element = declaration.clone();
        element.id = id(99);
        element.identity = "test::ViewElement".into();
        declaration.shape = terminal_psi::StructuralTypeShape::ElementView {
            element: element.id,
        };
        types.push(element);
        assert!(store_destination(&source, &parameter));
    }

    #[test]
    fn borrowed_store_arrival_rejects_substitution_and_ambiguous_coordinates() {
        let (source, parameter) = borrowed_arrival();
        let mut substituted = parameter.clone();
        substituted.position += 1;
        assert!(!store_destination(&source, &substituted));
        let mut changed = source.clone();
        changed
            .structural
            .as_mut()
            .unwrap()
            .structural_places
            .iter_mut()
            .find(|row| row.id == parameter.place)
            .unwrap()
            .kind = StructuralPlaceKind::BlockParameter {
            block: id(ENTRY),
            position: parameter.position,
        };
        assert!(!store_destination(&changed, &parameter));
        let mut changed = source;
        let signature = changed.structural.as_mut().unwrap();
        let duplicate = signature
            .structural_places
            .iter()
            .find(|row| row.id == parameter.place)
            .unwrap()
            .clone();
        signature.structural_places.push(duplicate);
        assert!(!store_destination(&changed, &parameter));
    }

    #[test]
    fn borrowed_store_arrival_rejects_claims_and_read_only_permission() {
        let (mut source, parameter) = borrowed_arrival();
        source
            .structural
            .as_mut()
            .unwrap()
            .entry_claims
            .push(terminal_psi::EntryClaim {
                claim: id(31),
                input: parameter.place,
                path: Vec::new(),
            });
        assert!(!store_destination(&source, &parameter));
        let (mut source, mut parameter) = borrowed_arrival();
        parameter.access = StructuralAccess::SharedBorrow;
        source
            .blocks
            .iter_mut()
            .find(|block| block.id == id(ARRIVAL))
            .unwrap()
            .structural_parameters[parameter.position as usize] = parameter.clone();
        assert!(!store_destination(&source, &parameter));
    }
}
