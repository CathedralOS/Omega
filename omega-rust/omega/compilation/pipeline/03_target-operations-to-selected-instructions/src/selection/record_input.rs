//! Reconstruct record field geometry from exact declarations.
use calling_conventions::ValueShape;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use terminal_psi::{
    RecordFieldValue, StructuralFieldType, StructuralMultiplicity, StructuralTypeShape,
    find_structural_type,
};

pub(super) fn fields(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
) -> Option<Vec<(u32, ValueShape, Option<semantic_vocabulary::ScalarType>)>> {
    let LegalizedScalarInstructionKind::EstablishRecord {
        result,
        fields,
        shape,
    } = &row.kind
    else {
        return None;
    };
    let declarations = &source.structural.as_ref()?.structural_types;
    // Claims and memberships retain source identities but add no bytes. Geometry
    // follows the complete carrier, including affine records with linear fields.
    if row.result.is_some()
        || crate::structural_inputs::structural_reference_input::shape(
            result.structural_type,
            declarations,
        )? != *shape
    {
        return None;
    }
    let mut selected_declarations = declarations
        .iter()
        .filter(|entry| entry.id == result.structural_type);
    let declaration = selected_declarations.next()?;
    if selected_declarations.next().is_some() {
        return None;
    }
    let StructuralTypeShape::Record {
        fields: expected, ..
    } = &declaration.shape
    else {
        return None;
    };
    let expected = expected.iter().filter(|field| !field.relevance.is_erased());
    if fields.len() != expected.clone().count() {
        return None;
    }
    let mut offset = 0u32;
    let mut placed = Vec::with_capacity(fields.len());
    for (field, expected) in fields.iter().zip(expected) {
        if field.field != expected.id || expected.relevance.is_erased() {
            return None;
        }
        let field_shape = field_shape(source, result, field, expected)?;
        let alignment = u32::from(field_shape.alignment);
        offset = match expected.storage.as_ref() {
            Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(fragments)) => {
                u32::try_from(fragments.byte_envelope()?.0).ok()?
            }
            Some(storage) => u32::try_from(storage.byte_offset()?).ok()?,
            None => offset.checked_add(alignment.checked_sub(1)?)? / alignment * alignment,
        };
        if offset.checked_add(field_shape.byte_size)? > shape.byte_size {
            return None;
        }
        placed.push((offset, field_shape, expected.field_type.scalar_type()));
        offset = offset.checked_add(field_shape.byte_size)?;
    }
    (offset <= shape.byte_size).then_some(placed)
}

/// Rejoin one field's scalar contract or complete owned child independently of
/// whether its enclosing carrier is a record or a selected case.
pub(super) fn field_shape(
    source: &LegalizedScalarFunction,
    parent: &terminal_psi::StructuralOperationResult,
    field: &terminal_psi::RecordFieldInitializer,
    expected: &terminal_psi::StructuralFieldDeclaration,
) -> Option<ValueShape> {
    if field.field != expected.id || expected.relevance.is_erased() {
        return None;
    }
    match &field.value {
        RecordFieldValue::Scalar {
            range_obligation, ..
        } => {
            if terminal_semantics::scalar_field_requires_evidence(&expected.field_type)
                != range_obligation.is_some()
            {
                return None;
            }
            let scalar = expected.field_type.scalar_type()?;
            match expected.storage.as_ref() {
                Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(fragments)) => {
                    if expected.field_type.atomic_scalar_type().is_some() {
                        return None;
                    }
                    Some(ValueShape::integer(
                        u32::try_from(fragments.byte_envelope()?.1).ok()?,
                        1,
                    ))
                }
                Some(storage) => {
                    let codec = runtime_abi::ScalarStorageCodec::from_scalar(
                        storage.stored_scalar().unwrap_or(scalar),
                    )?;
                    Some(ValueShape::integer(u32::from(codec.byte_size()), 1))
                }
                None => super::scalar_call_abi::scalar_shape(scalar),
            }
        }
        RecordFieldValue::Structural(argument) => {
            structural_operand_shape(source, parent, argument, &expected.field_type)
        }
    }
}

/// Completed structural operands share one type and storage query across record
/// members, selected payloads and fixed-array elements. Psi owns whether the
/// value copies or moves; native transport copies the same stored descriptor
/// words in either case. A shared reference's affine carrier declaration does
/// not prevent its checked use inside unrestricted aggregate storage.
pub(super) fn structural_operand_shape(
    source: &LegalizedScalarFunction,
    parent: &terminal_psi::StructuralOperationResult,
    argument: &terminal_psi::StructuralArgument,
    field_type: &StructuralFieldType,
) -> Option<ValueShape> {
    let declarations = &source.structural.as_ref()?.structural_types;
    if !argument.path.is_empty() {
        let expected_type = field_type.as_structural()?;
        let (selected, _) = projected_operand(source, argument)?;
        if selected != expected_type || argument.place == parent.place {
            return None;
        }
        return crate::structural_inputs::structural_reference_input::shape(selected, declarations);
    }
    let shape = match field_type {
        field_type
            if matches!(
                field_type.byte_sequence_carrier(),
                Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. })
            ) =>
        {
            if argument.access != terminal_psi::StructuralAccess::Owned
                || !argument.path.is_empty()
                || argument.place == parent.place
            {
                return None;
            }
            let signature = source.structural.as_ref()?;
            let mut parameters = signature
                .parameters
                .iter()
                .map(|parameter| &parameter.semantic)
                .chain(
                    source
                        .blocks
                        .iter()
                        .filter(|block| block.id != source.entry_block)
                        .flat_map(|block| &block.structural_parameters),
                )
                .filter(|parameter| parameter.place == argument.place);
            let child_type = if let Some(parameter) = parameters.next() {
                if parameters.next().is_some()
                    || parameter.access != terminal_psi::StructuralAccess::Owned
                    || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    || !parameter.projected_qualifications.is_empty()
                {
                    return None;
                }
                parameter.structural_type
            } else {
                let (_, result) = super::aggregate_result_input::home(source, argument.place)?;
                if result.multiplicity != StructuralMultiplicity::Unrestricted
                    || !result.projected_qualifications.is_empty()
                    || !result.qualification_establishments.is_empty()
                    || !result.claims.is_empty()
                {
                    return None;
                }
                result.structural_type
            };
            // Primitive domain implication was checked before target lowering;
            // selection preserves the exact qualified source home and copies
            // its length-prefixed value, not its capacity as a live extent.
            let declaration = find_structural_type(declarations, child_type)?;
            if declaration.shape
                != StructuralTypeShape::ByteSequence(field_type.byte_sequence_carrier()?)
            {
                return None;
            }
            crate::structural_inputs::structural_reference_input::shape(child_type, declarations)?
        }
        field_type if descriptor_shape(field_type, declarations).is_some() => {
            // A view member stores its two-word descriptor: the
            // initializer moves the descriptor value with owned access, the
            // backing's custody stays with the source's loan. `Structural`
            // members name their descriptor type; direct `&[u8]` members
            // declare the canonical standalone one.
            let expected = descriptor_shape(field_type, declarations)?;
            if argument.access != terminal_psi::StructuralAccess::Owned
                || !argument.path.is_empty()
                || argument.place == parent.place
            {
                return None;
            }
            // Descriptor identity and readable custody have one shared source
            // query. Constructors do not maintain a second producer whitelist.
            let child_type = super::established_view_input::view_type(source, argument.place)
                .or_else(|| {
                    super::established_view_input::element_view_type(source, argument.place)
                })?;
            let readable = match super::established_view_input::view_access(source, argument.place)
            {
                Some(terminal_psi::StructuralAccess::SharedBorrow) => true,
                // The record owns the completed exclusive descriptor once.
                // Transporting its words does not copy or own its referent;
                // unrestricted parent custody cannot retain that resource.
                Some(terminal_psi::StructuralAccess::MutableBorrow) => {
                    parent.multiplicity == StructuralMultiplicity::Affine
                }
                _ => false,
            };
            if (find_structural_type(declarations, child_type)
                .map(|declaration| &declaration.shape)
                != Some(&expected)
                && !descriptor_family(
                    find_structural_type(declarations, child_type)
                        .map(|declaration| &declaration.shape),
                    &expected,
                ))
                || !readable
            {
                return None;
            }
            crate::structural_inputs::structural_reference_input::shape(child_type, declarations)?
        }
        StructuralFieldType::Structural(nested) => {
            // Construction transfers completed storage, including nested loan
            // carriers. The source/result custody checks below govern whether
            // that transfer consumes an affine child; copyable-leaf geometry
            // would incorrectly reject references beneath an array dimension.
            // A bare reference transfers its address, never its referent bytes.
            // A completed primitive home is storage too. Exact source identity
            // and owned access below authorize this operand; layout supplies
            // its bytes without an aggregate-only constructor whitelist.
            if argument.access != terminal_psi::StructuralAccess::Owned
                || !argument.path.is_empty()
                || argument.place == parent.place
            {
                return None;
            }
            let signature = source.structural.as_ref()?;
            let parameters = signature
                .parameters
                .iter()
                .map(|parameter| &parameter.semantic)
                .chain(
                    source
                        .blocks
                        .iter()
                        .filter(|block| block.id != source.entry_block)
                        .flat_map(|block| &block.structural_parameters),
                );
            let mut matches = parameters
                .filter(|parameter| parameter.place == argument.place)
                .map(|parameter| {
                    (
                        parameter.structural_type,
                        parameter.access == terminal_psi::StructuralAccess::Owned,
                    )
                })
                .chain(source.instructions().filter_map(|row| {
                    let result = row.kind.get_structural_result()?;
                    (result.place == argument.place).then_some((result.structural_type, true))
                }));
            let (child_type, plain_owned) = matches.next()?;
            if matches.next().is_some() || child_type != *nested || !plain_owned {
                return None;
            }
            crate::structural_inputs::structural_reference_input::shape(*nested, declarations)?
        }
        _ => return None,
    };
    Some(shape)
}

/// The descriptor shape a view member stores: an `ElementView` or
/// borrowed byte view through its `Structural` child type, or the canonical
/// `&[u8]` spelling directly on the field.
fn descriptor_shape(
    field_type: &StructuralFieldType,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
) -> Option<StructuralTypeShape> {
    match field_type {
        StructuralFieldType::Structural(child) => find_structural_type(declarations, *child)
            .map(|declaration| declaration.shape.clone())
            .filter(|shape| {
                matches!(
                    shape,
                    StructuralTypeShape::ElementView { .. }
                        | StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                        )
                )
            }),
        StructuralFieldType::ByteSequence(
            carrier @ terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
        )
        | StructuralFieldType::PredicateByteSequence {
            carrier: carrier @ terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
            ..
        } => Some(StructuralTypeShape::ByteSequence(*carrier)),
        _ => None,
    }
}

/// Two descriptor shapes match when they carry the same view family: a
/// member's `BorrowedView` spells its loan access while the standalone
/// descriptor declaration keeps `access: None` — same layout, same family.
fn descriptor_family(actual: Option<&StructuralTypeShape>, expected: &StructuralTypeShape) -> bool {
    match (actual, expected) {
        (
            Some(StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
            )),
            StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
                ..
            }),
        ) => true,
        _ => false,
    }
}

/// Exact static constructor selection over the original owned home.
/// Return the source displacement as well as its type so construction and replay
/// cannot accidentally copy the root's leading bytes for a later field.
pub(super) fn projected_operand(
    source: &LegalizedScalarFunction,
    argument: &terminal_psi::StructuralArgument,
) -> Option<(semantic_vocabulary::StructuralTypeId, u32)> {
    if argument.access != terminal_psi::StructuralAccess::Owned || argument.path.is_empty() {
        return None;
    }
    let signature = source.structural.as_ref()?;
    let mut parameters = signature
        .parameters
        .iter()
        .map(|parameter| &parameter.semantic)
        .chain(
            source
                .blocks
                .iter()
                .filter(|block| block.id != source.entry_block)
                .flat_map(|block| &block.structural_parameters),
        )
        .filter(|parameter| parameter.place == argument.place);
    // Claims and root/projected memberships stay on the exact checked operand
    // and result. This query resolves its physical type and displacement; none
    // of that semantic custody adds bytes or grants another transfer.
    let root = if let Some(parameter) = parameters.next() {
        if parameters.next().is_some() || parameter.access != terminal_psi::StructuralAccess::Owned
        {
            return None;
        }
        parameter.structural_type
    } else {
        let (_, result) = super::aggregate_result_input::home(source, argument.place)?;
        result.structural_type
    };
    let (_, selected) = terminal_semantics::canonical_static_projection(
        signature.structural_types.iter(),
        root,
        &argument.path,
    )?;
    let (physical_type, offset) = crate::structural_inputs::structural_reference_input::project(
        root,
        &argument.path,
        &signature.structural_types,
    )?;
    (physical_type == selected.id).then_some((physical_type, offset))
}

pub(super) fn operand_offset(
    source: &LegalizedScalarFunction,
    argument: &terminal_psi::StructuralArgument,
) -> Option<u32> {
    if argument.path.is_empty() {
        Some(0)
    } else {
        projected_operand(source, argument).map(|(_, offset)| offset)
    }
}
