//! Reconstruct a zeroed establishment's admitted home and, for a home that
//! construction stores leaf by leaf, every scalar or live-length cell.
use calling_conventions::ValueShape;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, ScalarType, StructuralCaseId, StructuralFieldId,
};
use terminal_psi::{
    StructuralOperationResult, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape, find_structural_type,
};

/// Admit a zeroed establishment and reconstruct its exact home shape. The
/// home must be zero-constructible: its zero value is exactly all-zero
/// bytes — every scalar zero is the all-zero bit pattern, a sum's zero
/// selects its first case whose tag is 0, and a bounded byte field starts
/// with a zero length. `zeroed_structural_shape` decides that from the
/// declaration graph alone, so admission does not expand array elements and
/// no leaf budget bounds the home's size.
pub(super) fn home<'a>(
    source: &LegalizedScalarFunction,
    row: &'a LegalizedScalarInstruction,
) -> Option<(&'a StructuralOperationResult, ValueShape)> {
    let LegalizedScalarInstructionKind::EstablishZeroedStructural { result, shape } = &row.kind
    else {
        return None;
    };
    // Membership was established by the semantic operation. It does not
    // change the zero image or require another physical layout.
    let declarations = &source.structural.as_ref()?.structural_types;
    if row.result.is_some()
        || !matches!(
            result.multiplicity,
            terminal_psi::nonlinear_multiplicity_pair_pattern!()
        )
        || !result.claims.is_empty()
        || crate::structural_inputs::structural_reference_input::shape(
            result.structural_type,
            declarations,
        )? != *shape
        || !terminal_semantics::zeroed_structural_shape(declarations.iter(), result.structural_type)
    {
        return None;
    }
    Some((result, *shape))
}

/// Each scalar leaf's `(byte offset, width)` of an admitted zeroed home, in
/// canonical leaf order. Leaf enumeration refuses expansions above its
/// resource ceiling, so construction asks only for homes it stores leaf by
/// leaf.
pub(super) fn storage(
    source: &LegalizedScalarFunction,
    result: &StructuralOperationResult,
) -> Option<Vec<(u32, u8)>> {
    let declarations = &source.structural.as_ref()?.structural_types;
    let leaves =
        terminal_semantics::zeroed_structural_leaves(declarations.iter(), result.structural_type)?;
    let mut stores = Vec::with_capacity(leaves.len());
    for leaf in leaves {
        // A zeroed leaf path is fully static — every segment is a declared
        // field or a literal index — so the projection spells no runtime
        // element.
        // Record leaves name their parent and field separately; array leaves
        // end at a primitive structural type. Preserve that distinction or a
        // record field either disappears or aliases another field's offset.
        let (offset, width, elements) = match &leaf {
            terminal_semantics::ZeroedStructuralLeaf::RecordField {
                path,
                field,
                scalar_type,
            } => crate::structural_inputs::structural_reference_input::store(
                result.structural_type,
                path,
                *field,
                *scalar_type,
                declarations,
            )?,
            terminal_semantics::ZeroedStructuralLeaf::ByteSequenceField {
                path,
                field,
                scalar_type,
            } => {
                // Empty storage exposes no payload bytes. Initialize only its
                // exact length cell; capacity is not an initial live extent.
                let (offset, width) =
                    crate::structural_inputs::structural_reference_input::byte_field_length(
                        result.structural_type,
                        path,
                        *field,
                        *scalar_type,
                        declarations,
                    )?;
                (offset, width, Vec::new())
            }
            terminal_semantics::ZeroedStructuralLeaf::CaseField {
                path,
                case,
                field,
                scalar_type,
            } => {
                let (carrier, base, elements) =
                    crate::structural_inputs::structural_reference_input::runtime_projection(
                        result.structural_type,
                        path,
                        declarations,
                    )?;
                let carrier_declaration = find_structural_type(declarations, carrier)?;
                let (field_offset, field_width) = case_payload_geometry(
                    carrier_declaration,
                    *case,
                    *field,
                    *scalar_type,
                    declarations,
                )?;
                (
                    base.checked_add(field_offset)?,
                    u32::from(field_width),
                    elements,
                )
            }
            terminal_semantics::ZeroedStructuralLeaf::ArrayScalar { path, scalar_type } => {
                crate::structural_inputs::structural_reference_input::primitive_geometry(
                    result.structural_type,
                    path,
                    *scalar_type,
                    declarations,
                )?
            }
            terminal_semantics::ZeroedStructuralLeaf::ByteArray { path, length } => {
                // One store per byte, last element first: the canonical
                // order of the run's elements.
                let byte = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).ok()?);
                for index in (0..*length).rev() {
                    let mut element = path.clone();
                    element.push(StructuralPathSegment::FixedIndex(index));
                    let (offset, width, elements) =
                        crate::structural_inputs::structural_reference_input::primitive_geometry(
                            result.structural_type,
                            &element,
                            byte,
                            declarations,
                        )?;
                    if !elements.is_empty() {
                        return None;
                    }
                    stores.push((offset, u8::try_from(width).ok()?));
                }
                continue;
            }
            terminal_semantics::ZeroedStructuralLeaf::SumSelection {
                path,
                structural_type,
                case,
                scalar_type,
            } => {
                // The selected tag cell: the sum's own base plus the layout's
                // tag offset (always byte 0, a signed i32).
                let (carrier, base, elements) =
                    crate::structural_inputs::structural_reference_input::runtime_projection(
                        result.structural_type,
                        path,
                        declarations,
                    )?;
                if carrier != *structural_type {
                    return None;
                }
                let carrier_declaration = find_structural_type(declarations, carrier)?;
                let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
                    &carrier_declaration.shape
                else {
                    return None;
                };
                if cases.first().map(|first| first.id) != Some(*case) {
                    return None;
                }
                let layout = crate::structural_inputs::structural_reference_input::case_layout(
                    carrier,
                    declarations,
                )?;
                if crate::structural_inputs::structural_reference_input::scalar_shape(*scalar_type)?
                    != layout.tag_shape
                {
                    return None;
                }
                (
                    base.checked_add(layout.tag_byte_offset)?,
                    layout.tag_shape.byte_size,
                    elements,
                )
            }
        };
        if !elements.is_empty() {
            return None;
        }
        stores.push((offset, u8::try_from(width).ok()?));
    }
    Some(stores)
}

/// The `(byte offset, width)` of a scalar payload field within a sum's first
/// declared case, relative to the sum's base. Packed field offsets already
/// fold the shared payload base, so the result adds to the carrier's offset
/// directly.
fn case_payload_geometry(
    carrier: &StructuralTypeDeclaration,
    case: StructuralCaseId,
    field: StructuralFieldId,
    scalar_type: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u8)> {
    let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
        &carrier.shape
    else {
        return None;
    };
    let first = cases.first()?;
    if first.id != case {
        return None;
    }
    let (position, declaration) = first
        .fields
        .iter()
        .filter(|candidate| !candidate.relevance.is_erased())
        .enumerate()
        .find(|(_, candidate)| candidate.id == field)?;
    if declaration.field_type.scalar_type() != Some(scalar_type) {
        return None;
    }
    let packed = *crate::structural_inputs::structural_reference_input::case_layout(
        carrier.id,
        declarations,
    )?
    .cases
    .first()?
    .fields
    .get(position)?;
    let shape = crate::structural_inputs::structural_reference_input::scalar_shape(scalar_type)?;
    if packed.shape != shape {
        return None;
    }
    Some((packed.byte_offset, u8::try_from(shape.byte_size).ok()?))
}
