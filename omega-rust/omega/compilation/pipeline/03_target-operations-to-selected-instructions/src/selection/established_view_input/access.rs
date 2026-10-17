//! Reconstruct borrowed descriptor access from the legalized source chain.
//! Descriptor geometry and source permission are separate; affine custody is
//! required for a mutable result but never supplies its parent's authority.
//! Fixed byte pointers and dynamic byte descriptors borrow the same way;
//! this query never reads a length word or decides their physical layout.
use legalized_operations::{LegalizedScalarFunction, LegalizedScalarInstructionKind as I};
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
use terminal_psi::{StructuralAccess as A, StructuralMultiplicity as M, StructuralTypeShape};

mod copied;

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
    place: PlaceId,
) -> Option<A> {
    let budget = function.structural.as_ref()?.structural_places.len();
    view_access_with_budget(function, place, budget)
}

// A stored descriptor may retain an explicit access ceiling while the incoming
// view uses its canonical carrier. Compare the unchanged physical presentation;
// the source chain below independently supplies the requested backing access.
fn same_descriptor_presentation(
    function: &LegalizedScalarFunction,
    actual: semantic_vocabulary::StructuralTypeId,
    expected: semantic_vocabulary::StructuralTypeId,
) -> bool {
    function.structural.as_ref().is_some_and(|signature| {
        terminal_semantics::borrowed_referent_presentation(
            signature.structural_types.iter(),
            actual,
            expected,
        ) == Some(terminal_semantics::BorrowedReferentPresentation::Unchanged)
    })
}

fn view_access_with_budget(
    function: &LegalizedScalarFunction,
    mut place: PlaceId,
    budget: usize,
) -> Option<A> {
    let signature = function.structural.as_ref()?;
    let mut required = A::SharedBorrow;
    let mut initial = None;
    let mut expected_type = None;
    let mut copied_whole = false;
    for remaining in (0..budget).rev() {
        // Geometry alone grants no backing permission. Calls and declared
        // homes retain their admitted origins; other producers replay their
        // exact source chain below, including every shared descriptor copy.
        if let Some((_, result)) = super::descriptor_result(function, place)
            .or_else(|| super::descriptor_home(function, place))
        {
            let access = terminal_semantics::descriptor_results::descriptor_value_access(
                signature.structural_types.iter(),
                result.structural_type,
                result.multiplicity,
            )?;
            return (supplies(access, required)
                && (!copied_whole || access == A::SharedBorrow)
                && expected_type.is_none_or(|identity| {
                    same_descriptor_presentation(function, result.structural_type, identity)
                }))
            .then_some(initial.unwrap_or(access));
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
                },
                |block| StructuralPlaceKind::BlockParameter {
                    block,
                    position: parameter.position,
                },
            );
            let access = terminal_semantics::descriptor_parameter_access(
                parameter,
                signature.structural_types.iter(),
            )?;
            if parameters.next().is_some()
                || declaration.kind != expected_kind
                || expected_type.is_some_and(|identity| {
                    !same_descriptor_presentation(function, parameter.structural_type, identity)
                })
                || (!parameter.qualifications.is_empty()
                    && !(access == A::SharedBorrow
                        && signature.structural_types.iter().any(|row| {
                            row.id == parameter.structural_type
                                && row.shape.is_borrowed_byte_carrier()
                        })))
                || (copied_whole && access != A::SharedBorrow)
                || !supplies(access, required)
            {
                return None;
            }
            return Some(initial.unwrap_or(access));
        }
        if let StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        } = declaration.kind
        {
            if required != A::SharedBorrow
                || expected_type.is_some_and(|expected| !same_descriptor_presentation(function, structural_type, expected))
                || !signature.structural_types.iter().any(|row|
                    row.id == structural_type && row.shape.is_borrowed_byte_view())
                || !function.instructions().any(|row|
                    matches!(&row.kind, I::EstablishByteSequenceLiteral { destination, structural_type: actual, .. }
                        if destination == declaration && actual.id == structural_type)) {
                return None;
            }
            return Some(initial.unwrap_or(A::SharedBorrow));
        }
        let (producer, structural_type) = declaration.kind.as_operation_result()?;
        if expected_type.is_some_and(|identity| {
            !same_descriptor_presentation(function, structural_type, identity)
        }) || !signature.structural_types.iter().any(|row| {
            row.id == structural_type
                && (matches!(row.shape, StructuralTypeShape::ElementView { .. })
                    || row.shape.is_borrowed_byte_carrier())
        }) {
            return None;
        }
        let mut producers = function
            .instructions()
            .filter(|row| row.operation == producer);
        let row = producers.next()?;
        if producers.next().is_some() || row.result.is_some() {
            return None;
        }
        let result = match &row.kind {
            I::ByteSequenceSubslice { result, .. }
            | I::ElementViewSubslice { result, .. }
            | I::EstablishElementView { result, .. }
            | I::StructuralLeafCopy { result, .. } => result,
            _ => return None,
        };
        if result.place != place
            || result.structural_type != structural_type
            || result
                .projected_qualifications
                .iter()
                .any(|row| !row.is_element_membership())
            || !result.qualification_establishments.is_empty()
            || !result.claims.is_empty()
        {
            return None;
        }
        let access = terminal_semantics::descriptor_results::descriptor_value_access(
            signature.structural_types.iter(),
            result.structural_type,
            result.multiplicity,
        )?;
        if !supplies(access, required) || (copied_whole && access != A::SharedBorrow) {
            return None;
        }
        initial.get_or_insert(access);
        match &row.kind {
            I::StructuralLeafCopy {
                source,
                path,
                byte_offset,
                shape,
                indices,
                ..
            } => {
                if access != A::SharedBorrow {
                    return None;
                }
                if matches!(path, abstract_operations::StructuralCopyPath::Ordinary(path) if path.is_empty())
                {
                    if *byte_offset != 0
                        || !indices.is_empty()
                        || crate::structural_inputs::structural_reference_input::shape(
                            structural_type,
                            &signature.structural_types,
                        )? != *shape
                    {
                        return None;
                    }
                    required = A::SharedBorrow;
                    expected_type = Some(structural_type);
                    copied_whole = true;
                    place = *source;
                } else {
                    copied::shared_projection(function, row)?;
                    return initial;
                }
            }
            I::ByteSequenceSubslice { source, .. } | I::ElementViewSubslice { source, .. } => {
                projected_source_access(function, source, access, remaining)?;
                return initial;
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
                        matches!(row.shape, StructuralTypeShape::ElementView { element: declared, .. } if declared == *element)
                        || row.shape.is_borrowed_byte_carrier() && signature.structural_types.iter().any(|leaf|
                            leaf.id == *element && matches!(leaf.shape,
                                StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Integer(integer))
                                if Ok(integer) == semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8))))) {
                    return None;
                }
                projected_source_access(function, source, access, remaining)?;
                return initial;
            }
            _ => return None,
        }
    }
    None
}

/// Join a selected descriptor to its real carrier permission. The operand's
/// requested access does not grant backing access: each stored reference and
/// byte-field descriptor attenuates the permission retained by its root.
fn projected_source_access(
    function: &LegalizedScalarFunction,
    source: &terminal_psi::StructuralArgument,
    requested: A,
    budget: usize,
) -> Option<()> {
    use crate::structural_inputs::structural_reference_input as geometry;
    use optimization_unit_semantics::StructuralStorageProjection;
    let signature = function.structural.as_ref()?;
    let types = &signature.structural_types;
    let mut current = crate::selection::reference_input::root_type(function, source.place)?;
    let mut authority = if types.iter().any(|row| {
        row.id == current
            && (matches!(row.shape, StructuralTypeShape::ElementView { .. })
                || row.shape.is_borrowed_byte_carrier())
    }) {
        view_access_with_budget(function, source.place, budget)?
    } else if let Some(parameter) = signature
        .parameters
        .iter()
        .map(|parameter| &parameter.semantic)
        .chain(function.block_structural_parameters())
        .find(|parameter| parameter.place == source.place)
    {
        parameter.access
    } else {
        crate::selection::aggregate_result_input::home(function, source.place)?;
        A::Owned
    };
    if !supplies(source.access, requested) || authority == A::WriteOnlyBorrow {
        return None;
    }
    let mut prefix_start = 0;
    for (position, segment) in source.path.iter().enumerate() {
        if *segment != terminal_psi::StructuralPathSegment::Referent {
            continue;
        }
        let (reference, _, _) =
            geometry::runtime_projection(current, &source.path[prefix_start..position], types)?;
        let StructuralTypeShape::Reference { referent, access } =
            types.iter().find(|row| row.id == reference)?.shape
        else {
            return None;
        };
        authority = if supplies(authority, access) {
            access
        } else if supplies(access, authority) {
            authority
        } else {
            return None;
        };
        if authority == A::WriteOnlyBorrow {
            return None;
        }
        current = referent;
        prefix_start = position + 1;
    }
    let selected = optimization_unit_semantics::structural_borrowed_storage_projection(
        types.iter(),
        current,
        &source.path[prefix_start..],
        requested,
    )?;
    let ceiling = match selected {
        StructuralStorageProjection::ByteField { carrier, .. } => match carrier {
            terminal_psi::ByteSequenceCarrier::BorrowedView { access }
            | terminal_psi::ByteSequenceCarrier::BoundedView { access, .. } => {
                access.unwrap_or(A::SharedBorrow)
            }
            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. } => A::Owned,
        },
        StructuralStorageProjection::Endpoint(identity) => {
            match types.iter().find(|row| row.id == identity)?.shape {
                // Lending an inline collection uses its storage permission;
                // selecting an existing descriptor uses its backing permission.
                // The operation separately checks which carrier it requires.
                StructuralTypeShape::ElementView { access, .. } => access.unwrap_or_else(|| {
                    if source.path.len() == prefix_start {
                        authority
                    } else {
                        A::SharedBorrow
                    }
                }),
                StructuralTypeShape::FixedArray { .. } => authority,
                StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { access }
                    | terminal_psi::ByteSequenceCarrier::BoundedView { access, .. },
                ) => access.unwrap_or(authority),
                StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                ) => A::Owned,
                _ => return None,
            }
        }
        StructuralStorageProjection::ByteWindow { .. } => return None,
    };
    (supplies(authority, requested) && supplies(ceiling, requested)).then_some(())
}
