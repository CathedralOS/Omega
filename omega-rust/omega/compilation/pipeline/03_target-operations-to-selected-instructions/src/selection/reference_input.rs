//! Physical reference establishment rejoins exact legalized source geometry.
//!
//! The carrier stores an address, never the referent's bytes. Source legality,
//! live loans and result origins remain independently replayed by legalization;
//! this owner reconstructs which address operation selection must realize.

use calling_conventions::ValueShape;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape,
};

#[cfg(test)]
mod tests;

pub(super) struct ReferenceAddress {
    pub(super) place: PlaceId,
    pub(super) byte_offset: u32,
    /// A trailing Referent on a reference leaf reads that leaf's stored word.
    /// Otherwise the projected storage address itself is the borrowed value.
    pub(super) load_pointer: bool,
    /// Scalar descriptor sources preserve the stored codec on reborrow.
    pub(super) load_codec: bool,
    pub(super) codec: Option<runtime_abi::ScalarStorageCodec>,
    pub(super) storage: Option<terminal_psi::ReferenceStorageView>,
    pub(super) pointer_slots:
        Vec<crate::structural_inputs::structural_reference_input::PointerSlot>,
    pub(super) indices: Vec<legalized_operations::LegalizedRuntimeIndexOperand>,
}

/// The supplying occurrence's permission, separate from pointer geometry and
/// the access requested by a consumer. Descriptor ownership owns its slot,
/// never the backing that slot describes.
pub(super) struct SourceAccess {
    pub structural_type: StructuralTypeId,
    pub multiplicity: StructuralMultiplicity,
    pub access: StructuralAccess,
}

pub(super) fn source_access(
    function: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<SourceAccess> {
    let signature = function.structural.as_ref()?;
    if let Some(parameter) = super::arrival_parameter_input::parameter(function, place) {
        let access = if parameter.access == StructuralAccess::Owned
            && terminal_semantics::descriptor_results::is_descriptor(
                signature.structural_types.iter(),
                parameter.structural_type,
            ) {
            terminal_semantics::descriptor_results::descriptor_value_access(
                signature.structural_types.iter(),
                parameter.structural_type,
                parameter.multiplicity,
            )?
        } else {
            parameter.access
        };
        return Some(SourceAccess {
            structural_type: parameter.structural_type,
            multiplicity: parameter.multiplicity,
            access,
        });
    }
    if let Some((_, result)) = super::aggregate_result_input::operation_result(function, place) {
        let access = if terminal_semantics::descriptor_results::is_descriptor(
            signature.structural_types.iter(),
            result.structural_type,
        ) {
            super::established_view_input::view_access(function, place)?
        } else {
            StructuralAccess::Owned
        };
        return Some(SourceAccess {
            structural_type: result.structural_type,
            multiplicity: result.multiplicity,
            access,
        });
    }
    let semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
        structural_type, ..
    } = signature.structural_place(place)?.kind
    else {
        return None;
    };
    Some(SourceAccess {
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: super::established_view_input::view_access(function, place)?,
    })
}

pub(in crate::selection) fn borrowed_scalar_source(
    function: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<semantic_vocabulary::ScalarType> {
    let signature = function.structural.as_ref()?;
    let parameter = signature
        .parameters
        .iter()
        .find(|parameter| parameter.semantic.place == place)
        .map(|parameter| &parameter.semantic)
        .or_else(|| {
            function
                .block_structural_parameters()
                .find(|parameter| parameter.place == place)
        })?;
    (parameter.access != StructuralAccess::Owned).then_some(())?;
    crate::structural_inputs::structural_reference_input::scalar_storage_type(
        parameter.structural_type,
        &signature.structural_types,
    )
}

/// Thin foreign transport omits the descriptor codec. Require its footprint
/// from the loan contract or from every retained physical backing alternative.
pub(in crate::selection) fn validate_scalar_foreign_export(
    function: &LegalizedScalarFunction,
    argument: &terminal_psi::StructuralArgument,
    scalar: semantic_vocabulary::ScalarType,
    width: u32,
) -> Result<(), crate::SelectedInstructionError> {
    if terminal_semantics::reference_storage_export_byte_width(scalar, argument.access)
        .map(u32::from)
        == Some(width)
        || super::storage_origin_input::scalar_backing_width_matches(function, argument, width)?
    {
        Ok(())
    } else {
        Err(crate::SelectedInstructionError::custody_because(
            "thin scalar foreign pointer requires the same byte footprint in every backing",
        ))
    }
}

pub(super) fn establishment(
    function: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
) -> Option<ReferenceAddress> {
    let (result, source, storage, indices, shape, placed) = match &row.kind {
        LegalizedScalarInstructionKind::EstablishReference {
            result,
            source,
            storage,
            indices,
            shape,
        } => (result, source, *storage, indices, shape, None),
        LegalizedScalarInstructionKind::EstablishPlacedReference {
            result,
            source,
            correspondence,
            indices,
            shape,
        } => (result, source, None, indices, shape, Some(correspondence)),
        _ => return None,
    };
    let signature = function.structural.as_ref()?;
    let declarations = &signature.structural_types;
    if row.result.is_some()
        || Some(*shape)
            != crate::structural_inputs::structural_reference_input::shape(
                result.structural_type,
                declarations,
            )
        || result.multiplicity != StructuralMultiplicity::Affine
        || !result.qualification_establishments.is_empty()
        || !result.claims.is_empty()
        || super::aggregate_result_input::home(function, result.place)
            != Some((row.operation, result))
    {
        return None;
    }
    let StructuralTypeShape::Reference { access, referent } = declarations
        .iter()
        .find(|declaration| declaration.id == result.structural_type)?
        .shape
    else {
        return None;
    };
    // Formation captures an address; it does not read the lent bytes. A
    // write-only loan therefore forms the same descriptor while retaining
    // its exact access. Only mutable access can attenuate to another kind.
    if source.access == StructuralAccess::Owned
        || !(access == source.access
            || source.access == StructuralAccess::MutableBorrow
                && matches!(
                    access,
                    StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
                ))
    {
        return None;
    }
    let root = root_type(function, source.place)?;
    let owner =
        terminal_semantics::runtime_structural_path_tip(declarations.iter(), root, &source.path)
            .map(|declaration| declaration.id);
    let address_source = match placed {
        Some(backing) => backing.address_source(source, declarations.as_slice())?,
        None => source.clone(),
    };
    let source = &address_source;
    let semantic_scalar = crate::structural_inputs::structural_reference_input::scalar_storage_type(
        referent,
        declarations,
    );
    let (path, through_referent) = match source.path.split_last() {
        Some((StructuralPathSegment::Referent, prefix)) => (prefix, true),
        _ => (source.path.as_slice(), false),
    };
    let mut projection_root = root;
    let mut segment_start = 0;
    let geometry =
        crate::structural_inputs::structural_reference_input::reference_formation_geometry(
            root,
            path,
            declarations,
        )?;
    if !crate::structural_inputs::structural_reference_input::runtime_indices_match(
        indices,
        &geometry.elements,
    ) {
        return None;
    }
    for (position, segment) in path.iter().enumerate() {
        if *segment != StructuralPathSegment::Referent {
            continue;
        }
        let (carrier, _, _) =
            crate::structural_inputs::structural_reference_input::runtime_projection(
                projection_root,
                &path[segment_start..position],
                declarations,
            )?;
        let StructuralTypeShape::Reference { referent, .. } = declarations
            .iter()
            .find(|declaration| declaration.id == carrier)?
            .shape
        else {
            return None;
        };
        projection_root = referent;
        segment_start = position + 1;
    }
    let projection_path = &path[segment_start..];
    let byte_offset = geometry.byte_offset;
    // An inline owner keeps its declaration-local field contract; it must not
    // acquire a nominal identity from a same-shaped catalog entry. Only scalar
    // or structural endpoints need a declaration for codec/schema handling.
    let selected = match geometry.selected.as_ref() {
        terminal_psi::StructuralFieldType::Structural(identity) => Some(*identity),
        field => {
            let contract = if source.access == StructuralAccess::SharedBorrow {
                field
                    .shared_observation_type()
                    .filter(|observed| observed.scalar_type().is_some())
                    .or_else(|| field.canonical_leaf_type())
            } else {
                field.canonical_leaf_type()
            };
            contract.and_then(|contract| {
                contract
                    .leaf_declaration(declarations.iter())
                    .map(|row| row.id)
            })
        }
    };
    let selected_shape = selected.and_then(|identity| {
        declarations
            .iter()
            .find(|row| row.id == identity)
            .map(|row| &row.shape)
    });
    let (actual, mut load_pointer) = match selected_shape {
        Some(StructuralTypeShape::Reference { referent, .. }) if through_referent => {
            (Some(*referent), true)
        }
        _ => (selected, false),
    };
    // Borrowed scalar ingress carries an address-and-codec descriptor. A new
    // interpretation replaces the codec, not the descriptor's pointer crossing.
    load_pointer |= borrowed_scalar_source(function, source.place).is_some();
    load_pointer |= placed.is_some_and(|backing| backing.resident_source().is_some());
    let load_codec = storage.is_none() && semantic_scalar.is_some() && load_pointer;
    let codec = match storage {
        Some(storage) => storage
            .stored_scalar
            .and_then(runtime_abi::ScalarStorageCodec::from_scalar),
        None if !load_codec && semantic_scalar.is_some() => {
            crate::structural_inputs::structural_reference_input::projected_scalar_codec(
                projection_root,
                projection_path,
                declarations,
            )
        }
        None => None,
    };
    (if let Some(correspondence) = placed {
        let mut plans = declarations
            .placed_views()
            .iter()
            .filter(|plan| plan.carrier == referent);
        match (plans.next(), plans.next()) {
            (Some(plan), None) => {
                owner == Some(correspondence.source_type())
                    && (correspondence.resident_source().is_some()
                        || actual == Some(correspondence.schema()))
                    && optimization_unit_semantics::placed_references::validate_backing(
                        declarations.as_slice(),
                        declarations.placed_views(),
                        plan,
                        correspondence.source_type(),
                        correspondence,
                    )
                    .is_ok()
            }
            _ => false,
        }
    } else {
        match storage {
            Some(storage) => actual.is_some_and(|actual| {
                terminal_semantics::reference_storage_source_compatible(
                    declarations.iter(),
                    actual,
                    result.structural_type,
                    storage.stored_scalar,
                )
            }),
            None => terminal_semantics::reference_source_projection_compatible(
                declarations.iter(),
                root,
                &source.path,
                result.structural_type,
            ),
        }
    })
    .then_some(ReferenceAddress {
        place: source.place,
        byte_offset,
        load_pointer,
        load_codec,
        codec,
        storage,
        pointer_slots: geometry.pointer_slots,
        indices: indices.clone(),
    })
}

pub(super) fn root_type(
    function: &LegalizedScalarFunction,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    let signature = function.structural.as_ref()?;
    let mut parameters = signature
        .parameters
        .iter()
        .map(|parameter| &parameter.semantic)
        .chain(
            function
                .blocks
                .iter()
                .filter(|block| block.id != function.entry_block)
                .flat_map(|block| &block.structural_parameters),
        )
        .filter(|parameter| parameter.place == place);
    if let Some(parameter) = parameters.next() {
        return parameters
            .next()
            .is_none()
            .then_some(parameter.structural_type);
    }
    if let Some((_, result, _, _)) = super::primitive_local_input::local(function, place) {
        return Some(result.structural_type);
    }
    super::aggregate_result_input::home(function, place)
        .map(|(_, result)| result.structural_type)
        // Literal descriptors use their authenticated immutable establishment,
        // rather than an operation-result allocation declaration.
        .or_else(|| {
            let semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                structural_type,
                ..
            } = signature.structural_place(place)?.kind
            else {
                return None;
            };
            super::established_view_input::view_access(function, place)?;
            (super::established_view_input::view_type(function, place) == Some(structural_type))
                .then_some(structural_type)
        })
}

/// Cross only the explicit referent boundary of an exact completed carrier
/// with the operation's required access. The retained path stays on the operation; this borrowed suffix is
/// used solely to reconstruct geometry within the selected referent.
pub(super) fn referent_projection<'path>(
    function: &LegalizedScalarFunction,
    place: PlaceId,
    path: &'path [StructuralPathSegment],
    required_access: StructuralAccess,
) -> Option<(StructuralTypeId, &'path [StructuralPathSegment])> {
    let crossing = path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)?;
    let (structural_type, multiplicity) =
        if let Some((_, result)) = super::aggregate_result_input::home(function, place) {
            if !result.qualification_establishments.is_empty() || !result.claims.is_empty() {
                return None;
            }
            (result.structural_type, result.multiplicity)
        } else {
            let parameter = super::arrival_parameter_input::parameter(function, place)?;
            // The exact arrival declaration retains qualifications just as
            // an operation result does. Membership metadata neither grants
            // referent access nor changes its address; rejecting its presence
            // only here breaks otherwise identical references at joins.
            if function
                .structural
                .as_ref()?
                .entry_claims
                .iter()
                .any(|claim| claim.input == place)
            {
                return None;
            }
            (parameter.structural_type, parameter.multiplicity)
        };
    let source = source_access(function, place)?;
    if source.structural_type != structural_type || source.multiplicity != multiplicity {
        return None;
    }
    let declarations = &function.structural.as_ref()?.structural_types;
    // Keep the incoming carrier's authority distinct from the referent's.
    // Fragmented writes request mutable access because they read sibling bits.
    if multiplicity == StructuralMultiplicity::Linear
        || !terminal_semantics::reference_source_projection_accessible(
            declarations.iter(),
            structural_type,
            multiplicity,
            source.access,
            path,
            required_access,
        )
    {
        return None;
    }
    let projection = crate::structural_inputs::structural_reference_input::reference_projection(
        structural_type,
        &path[..=crossing],
        required_access,
        None,
        declarations,
    )?;
    Some((projection.selected, &path[crossing + 1..]))
}

pub(super) fn store_destination<'path>(
    function: &LegalizedScalarFunction,
    destination: &abstract_operations::StructuralScalarStoreDestination,
    path: &'path [StructuralPathSegment],
    required_access: StructuralAccess,
) -> Option<(StructuralTypeId, &'path [StructuralPathSegment])> {
    let admitted = match destination {
        abstract_operations::StructuralScalarStoreDestination::OwnedResult { producer, result } => {
            super::aggregate_result_input::home(function, result.place) == Some((*producer, result))
        }
        abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
            block,
            parameter,
        } => {
            super::aggregate_result_input::block_home(function, parameter.place)
                == Some((*block, parameter))
        }
        abstract_operations::StructuralScalarStoreDestination::Parameter(parameter) => {
            super::arrival_parameter_input::store_destination(function, parameter)
        }
    };
    if !admitted {
        return None;
    }
    referent_projection(function, destination.place(), path, required_access)
}

/// An outgoing borrow transports the pointer word, not the carrier home's
/// address. Exact target metadata must retain the authored carrier and path.
/// Reconstruct the address independently of the destination ABI. Internal
/// calls validate their descriptor placement; normalized foreign calls validate
/// their evaluated thin-pointer placement and prove the exported footprint
/// before discarding a scalar codec descriptor.
pub(super) fn borrowed_argument(
    function: &LegalizedScalarFunction,
    argument: &terminal_psi::StructuralArgument,
    target: &target_operations::TargetStructuralArgument,
    indices: &[legalized_operations::LegalizedRuntimeIndexOperand],
) -> Option<BorrowedReferenceAddress> {
    let root = root_type(function, argument.place)?;
    let signature = function.structural.as_ref()?;
    let declarations = &signature.structural_types;
    match &target.source {
        target_operations::TargetStructuralArgumentSource::StructuralHome { psi_operation } => {
            let (producer, result) = super::aggregate_result_input::home(function, argument.place)?;
            if producer != *psi_operation || result.structural_type != root {
                return None;
            }
        }
        target_operations::TargetStructuralArgumentSource::Placement(placement) => {
            let parameter = signature
                .parameters
                .iter()
                .find(|parameter| parameter.semantic.place == argument.place)?;
            if parameter.target.placement != *placement
                || parameter.semantic.structural_type != root
                || parameter.semantic.access == StructuralAccess::WriteOnlyBorrow
            {
                return None;
            }
        }
        target_operations::TargetStructuralArgumentSource::BlockParameter { block, place } => {
            let (owner, parameter) =
                super::aggregate_result_input::block_home(function, argument.place)?;
            if owner != *block || *place != argument.place || parameter.structural_type != root {
                return None;
            }
        }
        _ => return None,
    }
    let projection = crate::structural_inputs::structural_reference_input::reference_projection(
        root,
        &argument.path,
        argument.access,
        Some(target.structural_type),
        declarations,
    )?;
    let selected = projection.selected;
    let offset = projection.referent_offset;
    let shape =
        crate::structural_inputs::structural_reference_input::shape(selected, declarations)?;
    (target.place == argument.place
        && target.path == argument.path
        && target.access == argument.access
        && target.root_structural_type == root
        && target.structural_type == selected
        && target.source_byte_offset == offset
        && target.shape
            == if crate::structural_inputs::structural_reference_input::scalar_storage_type(
                selected,
                declarations,
            )
            .is_some()
                && target.destination.shape.class
                    == calling_conventions::ValueClass::BorrowedScalarReference
            {
                ValueShape::borrowed_scalar_reference()
            } else {
                ValueShape::borrowed_reference(shape.byte_size, shape.alignment)
            }
        && crate::structural_inputs::structural_reference_input::runtime_indices_match(
            indices,
            &projection.elements,
        )
        && target.fixed_array_length.is_none()
        && target.element_stride.is_none())
    .then_some(BorrowedReferenceAddress {
        pointer_slots: projection.pointer_slots,
        indices: indices.to_vec(),
        referent_offset: offset,
        scalar_descriptor: projection.scalar_descriptor,
    })
}

pub(super) struct BorrowedReferenceAddress {
    pub(super) pointer_slots:
        Vec<crate::structural_inputs::structural_reference_input::PointerSlot>,
    pub(super) indices: Vec<legalized_operations::LegalizedRuntimeIndexOperand>,
    pub(super) referent_offset: u32,
    pub(super) scalar_descriptor: bool,
}
