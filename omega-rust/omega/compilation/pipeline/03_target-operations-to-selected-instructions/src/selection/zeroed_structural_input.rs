//! Reconstruct every scalar-leaf byte range of a zeroed establishment.
use calling_conventions::{ConventionalSumLayout, ValueShape};
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use semantic_vocabulary::{ScalarType, StructuralFieldId};
use terminal_psi::{StructuralMultiplicity, StructuralTypeDeclaration, StructuralTypeShape};

/// Reconstruct a zeroed establishment's exact home shape and each scalar
/// leaf's `(byte offset, width)` in canonical leaf order. Construction and
/// independent instruction replay each consume this input.
pub(super) fn storage<'a>(
    source: &LegalizedScalarFunction,
    row: &'a LegalizedScalarInstruction,
) -> Option<(
    &'a terminal_psi::StructuralOperationResult,
    ValueShape,
    Vec<(u32, u8)>,
)> {
    let LegalizedScalarInstructionKind::EstablishZeroedStructural { result, shape } = &row.kind
    else {
        return None;
    };
    let declarations = &source.structural.as_ref()?.structural_types;
    if row.result.is_some()
        || !matches!(
            result.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        )
        || !result.claims.is_empty()
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || crate::structural_inputs::structural_reference_input::shape(
            result.structural_type,
            declarations,
        )? != *shape
    {
        return None;
    }
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
            } => {
                let (carrier, base, elements) =
                    crate::structural_inputs::structural_reference_input::runtime_projection(
                        result.structural_type,
                        path,
                        declarations,
                    )?;
                let carrier_declaration = declarations
                    .iter()
                    .find(|declaration| declaration.id == carrier)?;
                if matches!(carrier_declaration.shape, StructuralTypeShape::Sum { .. }) {
                    // A case-payload scalar: the carrier's first-declared
                    // case holds `field`, packed at the shared payload base.
                    let (field_offset, field_width) = case_payload_geometry(
                        carrier_declaration,
                        *field,
                        *scalar_type,
                        declarations,
                    )?;
                    (base.checked_add(field_offset)?, field_width, elements)
                } else {
                    crate::structural_inputs::structural_reference_input::store(
                        result.structural_type,
                        path,
                        *field,
                        *scalar_type,
                        declarations,
                    )?
                }
            }
            terminal_semantics::ZeroedStructuralLeaf::ArrayScalar { path, scalar_type } => {
                crate::structural_inputs::structural_reference_input::primitive_geometry(
                    result.structural_type,
                    path,
                    *scalar_type,
                    declarations,
                )?
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
                let carrier_declaration = declarations
                    .iter()
                    .find(|declaration| declaration.id == carrier)?;
                let StructuralTypeShape::Sum { cases } = &carrier_declaration.shape else {
                    return None;
                };
                if cases.first().map(|first| first.id) != Some(*case) {
                    return None;
                }
                let layout = sum_layout(carrier_declaration, declarations)?;
                if crate::structural_inputs::structural_reference_input::scalar_shape(*scalar_type)?
                    != layout.tag_shape
                {
                    return None;
                }
                (
                    base.checked_add(u32::from(layout.tag_byte_offset))?,
                    u8::try_from(layout.tag_shape.byte_size).ok()?,
                    elements,
                )
            }
        };
        if !elements.is_empty() {
            return None;
        }
        stores.push((offset, width));
    }
    Some((result, *shape, stores))
}

/// The conventional tag-prefixed layout of a sum declaration: every case's
/// non-erased payload field shapes in declaration order. Pure sums carry no
/// common fields.
fn sum_layout(
    declaration: &StructuralTypeDeclaration,
    declarations: &[StructuralTypeDeclaration],
) -> Option<ConventionalSumLayout> {
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return None;
    };
    let payloads = cases
        .iter()
        .map(|case| {
            case.fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .map(|field| {
                    crate::structural_inputs::structural_reference_input::field_shape(
                        &field.field_type,
                        declarations,
                        &mut Vec::new(),
                    )
                })
                .collect::<Option<Vec<_>>>()
        })
        .collect::<Option<Vec<_>>>()?;
    calling_conventions::evaluate_conventional_sum_layout(&[], &payloads).ok()
}

/// The `(byte offset, width)` of a scalar payload field within a sum's first
/// declared case, relative to the sum's base. Packed field offsets already
/// fold the shared payload base, so the result adds to the carrier's offset
/// directly.
fn case_payload_geometry(
    carrier: &StructuralTypeDeclaration,
    field: StructuralFieldId,
    scalar_type: ScalarType,
    declarations: &[StructuralTypeDeclaration],
) -> Option<(u32, u8)> {
    let StructuralTypeShape::Sum { cases } = &carrier.shape else {
        return None;
    };
    let first = cases.first()?;
    let position = first
        .fields
        .iter()
        .filter(|candidate| !candidate.relevance.is_erased())
        .position(|candidate| candidate.id == field)?;
    let packed = *sum_layout(carrier, declarations)?
        .cases
        .first()?
        .fields
        .get(position)?;
    let width =
        crate::structural_inputs::structural_reference_input::scalar_shape(scalar_type)?.byte_size;
    if packed.shape.byte_size != width {
        return None;
    }
    Some((u32::from(packed.byte_offset), u8::try_from(width).ok()?))
}
