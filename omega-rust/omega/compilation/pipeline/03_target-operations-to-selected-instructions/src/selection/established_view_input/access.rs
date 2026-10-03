//! Reconstruct borrowed descriptor access from the legalized source chain.
//! Descriptor geometry and source permission are separate; affine custody is
//! required for a mutable result but never supplies its parent's authority.
use legalized_operations::{LegalizedScalarFunction, LegalizedScalarInstructionKind as I};
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
use terminal_psi::{StructuralAccess as A, StructuralMultiplicity as M, StructuralTypeShape};

fn supplies(actual: A, requested: A) -> bool {
    actual == A::Owned
        || matches!(
            (actual, requested),
            (A::MutableBorrow, A::SharedBorrow | A::MutableBorrow)
                | (A::SharedBorrow, A::SharedBorrow)
        )
}

pub(in crate::selection) fn view_access(
    function: &LegalizedScalarFunction,
    mut place: PlaceId,
) -> Option<A> {
    let signature = function.structural.as_ref()?;
    let mut required = A::SharedBorrow;
    let mut initial = None;
    let mut expected_type = None;
    for _ in 0..signature.structural_places.len() {
        if let Some((_, result)) = super::descriptor_result(function, place) {
            return (required == A::SharedBorrow
                && expected_type.is_none_or(|identity| identity == result.structural_type))
            .then_some(initial.unwrap_or(A::SharedBorrow));
        }
        let mut declarations = signature
            .structural_places
            .iter()
            .filter(|declaration| declaration.id == place);
        let declaration = declarations.next()?;
        if declarations.next().is_some() {
            return None;
        }
        let mut parameters = signature
            .parameters
            .iter()
            .map(|parameter| (None, &parameter.semantic))
            .chain(function.blocks.iter().flat_map(|block| {
                block
                    .structural_parameters
                    .iter()
                    .map(move |parameter| (Some(block.id), parameter))
            }))
            .filter(|(_, parameter)| parameter.place == place);
        if let Some((owner, parameter)) = parameters.next() {
            let expected_kind = owner.map_or(
                StructuralPlaceKind::Parameter {
                    position: parameter.position,
                    is_self: parameter.is_self,
                },
                |block| StructuralPlaceKind::BlockParameter {
                    block,
                    position: parameter.position,
                },
            );
            if parameters.next().is_some()
                || declaration.kind != expected_kind
                || expected_type.is_some_and(|identity| identity != parameter.structural_type)
                || parameter.multiplicity != M::Unrestricted
                || (!parameter.qualifications.is_empty()
                    && !(parameter.access == A::SharedBorrow
                        && signature.structural_types.iter().any(|row| {
                            row.id == parameter.structural_type && row.shape.is_borrowed_byte_view()
                        })))
                || !parameter.projected_qualifications.is_empty()
                || !matches!(parameter.access, A::SharedBorrow | A::MutableBorrow)
                || !supplies(parameter.access, required)
                || !signature.structural_types.iter().any(|row| {
                    row.id == parameter.structural_type
                        && (matches!(row.shape, StructuralTypeShape::ElementView { .. })
                            || row.shape.is_borrowed_byte_view())
                })
            {
                return None;
            }
            return Some(initial.unwrap_or(parameter.access));
        }
        if let StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        } = declaration.kind
        {
            if required != A::SharedBorrow
                || expected_type.is_some_and(|expected| expected != structural_type)
                || !signature.structural_types.iter().any(|row|
                    row.id == structural_type && row.shape.is_borrowed_byte_view())
                || !function.blocks.iter().flat_map(|block| &block.instructions).any(|row|
                    matches!(&row.kind, I::EstablishByteSequenceLiteral { destination, structural_type: actual, .. }
                        if destination == declaration && actual.id == structural_type)) {
                return None;
            }
            return Some(initial.unwrap_or(A::SharedBorrow));
        }
        let StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } = declaration.kind
        else {
            return None;
        };
        if expected_type.is_some_and(|identity| identity != structural_type)
            || !signature.structural_types.iter().any(|row| {
                row.id == structural_type
                    && (matches!(row.shape, StructuralTypeShape::ElementView { .. })
                        || row.shape.is_borrowed_byte_view())
            })
        {
            return None;
        }
        let mut producers = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|row| row.operation == producer);
        let row = producers.next()?;
        if producers.next().is_some() || row.result.is_some() {
            return None;
        }
        let result = match &row.kind {
            I::ByteSequenceSubslice { result, .. }
            | I::ElementViewSubslice { result, .. }
            | I::EstablishElementView { result, .. } => result,
            _ => return None,
        };
        if result.place != place
            || result.structural_type != structural_type
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.qualification_establishments.is_empty()
            || !result.claims.is_empty()
        {
            return None;
        }
        let access = match result.multiplicity {
            M::Unrestricted => A::SharedBorrow,
            M::Affine => A::MutableBorrow,
            _ => return None,
        };
        if !supplies(access, required) {
            return None;
        }
        initial.get_or_insert(access);
        match &row.kind {
            I::ByteSequenceSubslice { source, .. } | I::ElementViewSubslice { source, .. } => {
                required = access;
                expected_type = Some(structural_type);
                place = *source;
            }
            I::EstablishElementView {
                source,
                destination,
                element,
                ..
            } => {
                if *destination != place || source.access != access
                    || source.path.contains(&terminal_psi::StructuralPathSegment::Referent)
                    || !signature.structural_types.iter().any(|row| row.id == structural_type && (
                        matches!(row.shape, StructuralTypeShape::ElementView { element: declared } if declared == *element)
                        || row.shape.is_borrowed_byte_view() && signature.structural_types.iter().any(|leaf|
                            leaf.id == *element && matches!(leaf.shape,
                                StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Integer(integer))
                                if Ok(integer) == semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8))))) {
                    return None;
                }
                let parent_access = signature
                    .parameters
                    .iter()
                    .map(|parameter| &parameter.semantic)
                    .chain(
                        function
                            .blocks
                            .iter()
                            .flat_map(|block| &block.structural_parameters),
                    )
                    .find(|parameter| parameter.place == source.place)
                    .map(|parameter| parameter.access)
                    .or_else(|| {
                        (crate::selection::aggregate_result_input::home(function, source.place)
                            .is_some()
                            || crate::selection::aggregate_result_input::block_home(
                                function,
                                source.place,
                            )
                            .is_some())
                        .then_some(A::Owned)
                    })?;
                return supplies(parent_access, source.access).then_some(initial?);
            }
            _ => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests;
