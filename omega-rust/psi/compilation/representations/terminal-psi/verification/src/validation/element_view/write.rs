//! Runtime-indexed scalar stores through an exact mutable element view.
//!
//! A shared-view establishment (`EstablishElementView` / `ElementViewSubslice`)
//! mints immutable custody, so the only destinations a write may take are a
//! `&mut`-borrowed parameter place or an operation-result place some other
//! producer published. Whether that other establishment actually granted a
//! mutable loan is proven upstream where the op was emitted; here the write
//! re-derives the structural contract: exact `ElementView` type, an in-bounds
//! obligation over the dominating length observation, and a value typed as
//! the view's scalar leaf.

use crate::validation::{
    IntegerSign, IntegerType, ModuleError, OperationKind, OperationResult, PlaceId, ScalarType,
    StructuralAccess, StructuralMultiplicity, StructuralPlaceKind, StructuralTypeShape,
    TerminalMachine, TerminalModule, ValueId,
};

/// The element view an operation-result place carries when its producer is
/// not a shared-view establishment. `EstablishElementView` and
/// `ElementViewSubslice` publish immutable custody; any other producer of an
/// `ElementView`-typed result place publishes the mutability its own
/// establishment checked. Returns that result.
pub(in crate::validation) fn mutable_result<'a>(
    module: &TerminalModule,
    machine: &'a TerminalMachine,
    place: PlaceId,
) -> Option<&'a terminal_psi::StructuralOperationResult> {
    if let Some((result, access)) = super::view_loan_result_in_module(module, machine, place) {
        return (access == StructuralAccess::MutableBorrow).then_some(result);
    }
    let declaration = machine
        .structural_places
        .iter()
        .find(|row| row.id == place)?;
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = declaration.kind
    else {
        return None;
    };
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            if operation.id != producer
                || matches!(
                    operation.kind,
                    OperationKind::EstablishElementView { .. }
                        | OperationKind::EstablishMutableElementView { .. }
                        | OperationKind::ElementViewSubslice { .. }
                        | OperationKind::ByteSequenceSubslice { .. }
                )
            {
                return None;
            }
            operation.result.structural().filter(|result| {
                result.place == place
                    && result.structural_type == structural_type
                    && matches!(
                        result.multiplicity,
                        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                    )
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
            })
        })
}

/// The element view a write mutates: a `&mut`-borrowed parameter or block
/// parameter, or an operation-result place no shared-view establishment
/// produced. Returns the place's declared structural type.
fn mutable_view_type(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    destination: PlaceId,
) -> Result<semantic_vocabulary::StructuralTypeId, ModuleError> {
    let invalid = || ModuleError::InvalidElementViewWrite(operation.id);
    if machine
        .entry_claims
        .iter()
        .any(|claim| claim.input == destination)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == destination)
    {
        return Err(invalid());
    }
    let place = machine
        .structural_places
        .iter()
        .find(|place| place.id == destination)
        .ok_or_else(invalid)?;
    match place.kind {
        StructuralPlaceKind::Parameter { position, is_self } => {
            let parameter = machine
                .structural_parameters
                .iter()
                .find(|parameter| {
                    parameter.place == destination
                        && parameter.position == position
                        && parameter.is_self == is_self
                })
                .ok_or_else(invalid)?;
            if parameter.access != StructuralAccess::MutableBorrow
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return Err(invalid());
            }
            Ok(parameter.structural_type)
        }
        StructuralPlaceKind::BlockParameter { .. } => {
            let parameter = crate::validation::block_views::parameter(machine, destination)
                .ok_or_else(invalid)?;
            if parameter.access != StructuralAccess::MutableBorrow
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return Err(invalid());
            }
            Ok(parameter.structural_type)
        }
        StructuralPlaceKind::OperationResult { .. } => {
            Ok(mutable_result(module, machine, destination)
                .ok_or_else(invalid)?
                .structural_type)
        }
        _ => Err(invalid()),
    }
    .and_then(|structural_type| {
        module
            .structural_types
            .iter()
            .any(|declaration| {
                declaration.id == structural_type
                    && matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
            })
            .then_some(structural_type)
            .ok_or_else(invalid)
    })
}

/// The declared scalar leaf `value` must carry: the unique producing
/// operation's result scalar type, equal to the view's element scalar.
fn value_leaf_scalar(
    module: &TerminalModule,
    machine: &TerminalMachine,
    structural_type: semantic_vocabulary::StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    value: ValueId,
) -> Option<ScalarType> {
    let declared = module
        .structural_types
        .iter()
        .find_map(|declaration| match declaration.shape {
            StructuralTypeShape::ElementView { element } if declaration.id == structural_type => {
                terminal_semantics::element_view_leaf_scalar(
                    &module.structural_types,
                    element,
                    path,
                )
            }
            _ => None,
        });
    let actual = machine
        .parameters
        .iter()
        .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
        .find(|declaration| declaration.id == value)
        .map(|declaration| declaration.scalar_type)
        .or_else(|| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|operation| {
                    operation
                        .result
                        .scalar()
                        .filter(|result| result.id == value)
                        .map(|result| result.scalar_type)
                })
        });
    match (declared, actual) {
        (Some(declared), Some(actual)) if declared == actual => Some(actual),
        _ => None,
    }
}

pub(in crate::validation) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidElementViewWrite(operation.id);
    let OperationKind::ElementViewWrite {
        destination,
        path,
        index,
        value,
        length,
        ..
    } = &operation.kind
    else {
        return Err(invalid());
    };
    let (destination, index, value, length) = (*destination, *index, *value, *length);
    let structural_type = mutable_view_type(module, machine, operation, destination)?;
    value_leaf_scalar(module, machine, structural_type, path, value).ok_or_else(invalid)?;
    // The index is an element count like the length it is bounded by; the
    // typed-operand pass pins both to `u64`, and the write must name the
    // dominating length observation of this same source.
    let u64_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"));
    let index_type = machine
        .parameters
        .iter()
        .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
        .find(|declaration| declaration.id == index)
        .map(|declaration| declaration.scalar_type)
        .or_else(|| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find_map(|candidate| {
                    candidate
                        .result
                        .scalar()
                        .filter(|result| result.id == index)
                        .map(|result| result.scalar_type)
                })
        })
        .ok_or_else(invalid)?;
    if index_type != u64_type {
        return Err(invalid());
    }
    if operation.result != OperationResult::Unit
        || !crate::validation::element_view::length::is_exact_length(machine, destination, length)
    {
        return Err(invalid());
    }
    let producer = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|candidate| {
            candidate.result.scalar().is_some_and(|result| result.id == length)
                && matches!(candidate.kind, OperationKind::ElementViewLength { source } if source == destination)
        })
        .ok_or_else(invalid)?;
    if !crate::validation::structural::byte_sequence_fields::view_length_is_current(
        module,
        machine,
        producer.id,
        operation.id,
        destination,
    ) {
        return Err(invalid());
    }
    Ok(())
}
