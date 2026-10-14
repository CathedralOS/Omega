//! `StructuralLeafCopy` establishes one owned duplicate of an `Unrestricted`
//! leaf projected out of a live readable root. The verifier already proved
//! the read-only observation contract — copying never opens a restoration
//! window, so this leg re-checks only the static shape the verifier's
//! admission committed to: the fresh result row (`Unrestricted`, or an
//! `Affine` shared descriptor disposal occurrence), a readable
//! source signature, and the ordinary or case-qualified path landing on the
//! exact leaf type. Case membership is proved before this lowering runs;
//! selecting a payload's declared type cannot establish its active case.
//! The retained operation carries source place and path unchanged so
//! consumers replay the verifier's own resolution rather than a
//! lowering-side reconstruction.

use abstract_operations::{AbstractOperation, StructuralCopyPath};
use semantic_vocabulary::{
    IntegerSign, IntegerType, PlaceId, ScalarType, StructuralPlaceKind, StructuralTypeId,
};
use terminal_psi::{
    Operation, StructuralAccess, StructuralFieldType, StructuralMultiplicity,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
};

use crate::lowering::LoweringError;

use super::borrowed_windows::{block_parameter, source_signature};

pub(super) fn lower(
    operation: &Operation,
    machine: &TerminalMachine,
    module: &terminal_psi::TerminalModule,
    structural_types: &[StructuralTypeDeclaration],
    source: PlaceId,
    path: StructuralCopyPath,
) -> Result<AbstractOperation, LoweringError> {
    let invalid = || LoweringError::InvalidStructuralLeafCopy(operation.id);
    let result = operation.result.structural().ok_or_else(invalid)?;
    let signature = source_signature(machine, source).ok_or_else(invalid)?;
    // Membership metadata was decided on the exact source occurrence in Psi.
    // It survives every snapshot without changing storage or copy permission;
    // bytes are not a separate qualification-custody mode.
    // A shared descriptor's affine disposal does not make its backing affine
    // payload. Copy its value with independent custody, never a source-home loan.
    let shared_descriptor_copy = result.multiplicity == StructuralMultiplicity::Affine
        && structural_types.iter().any(|declaration| {
            declaration.id == result.structural_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::Reference {
                        access: StructuralAccess::SharedBorrow,
                        ..
                    }
                )
        });
    if (result.multiplicity != StructuralMultiplicity::Unrestricted && !shared_descriptor_copy)
        || !result.qualification_establishments.is_empty()
        || !result.claims.is_empty()
        || !matches!(
            machine.structural_place(result.place)
                .map(|place| place.kind),
            Some(StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            }) if producer == operation.id && structural_type == result.structural_type
        )
    {
        return Err(invalid());
    }
    // Observation requires readable access: a write-only borrow never opens.
    if machine
        .structural_parameters
        .iter()
        .chain(block_parameter(machine, source))
        .find(|parameter| parameter.place == source)
        .is_some_and(|parameter| parameter.access == StructuralAccess::WriteOnlyBorrow)
    {
        return Err(invalid());
    }
    let selected_type = match &path {
        StructuralCopyPath::Ordinary(path) => {
            resolve_copyable_path(structural_types, signature.structural_type, path)
        }
        StructuralCopyPath::Case(path) => terminal_semantics::canonical_case_leaf_type(
            structural_types.iter(),
            signature.structural_type,
            path,
        ),
    }
    .ok_or_else(invalid)?;
    // All copies name the selected value's own type, including descriptors.
    // Reference formation is the separate EstablishReference operation.
    let expected_type = result.structural_type;
    if selected_type != expected_type
        && terminal_semantics::quotients::quotient_copy_adaptation(module, machine, operation)
            .is_none()
        && !byte_carrier_leaf_copy(
            structural_types,
            signature.structural_type,
            &path,
            expected_type,
        )
    {
        return Err(invalid());
    }
    Ok(AbstractOperation::StructuralLeafCopy {
        psi_operation: operation.id,
        result: result.clone(),
        source,
        path,
    })
}

/// Resolve the leaf's declared type beneath `root` through the spelled path,
/// mirroring the verifier's `resolve_structural_path`: field segments select
/// a non-erased record member whose structural or canonical leaf shape joins
/// the declaration roster, fixed indices step into in-bounds array elements,
/// and indexed views select their declared element. A view's extent is its
/// live runtime length, not a static type length; the verifier discharges
/// that bound before this static-shape replay and the path retains its proof.
fn resolve_copyable_path(
    structural_types: &[StructuralTypeDeclaration],
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for segment in path {
        let declaration = terminal_psi::find_structural_type(structural_types, structural_type)?;
        structural_type = match (segment, &declaration.shape) {
            (
                StructuralPathSegment::Referent,
                StructuralTypeShape::Reference { referent, access },
            ) if *access != StructuralAccess::WriteOnlyBorrow => *referent,
            (
                StructuralPathSegment::Field(identity),
                StructuralTypeShape::Record { fields, .. },
            ) => {
                let field = terminal_psi::live_field_by_identity(fields, *identity)?;
                match &field.field_type {
                    StructuralFieldType::Structural(next) => *next,
                    leaf => {
                        let shape = leaf.leaf_copy_shape()?;
                        structural_types
                            .iter()
                            .find(|candidate| candidate.shape == shape)
                            .map(|candidate| candidate.id)?
                    }
                }
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            (
                StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::FixedArray { element, .. },
            ) => *element,
            (
                StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. },
                StructuralTypeShape::ElementView { element },
            ) => *element,
            _ => return None,
        };
    }
    Some(structural_type)
}

/// Whether a leaf copy mints a record out of a byte carrier: `path` ends at a
/// literal index inside a `u8` fixed array, and `expected` is a plain record
/// whose fixed scalar layout fits the elements remaining at that index. This
/// mirrors the verifier's `byte_carrier_leaf_copy` admission — the source
/// recast judgment already proved the representation, so the lowering
/// re-checks the same static window instead of requiring the spelled leaf
/// type. A runtime-selected index cannot prove the wider `index + extent`
/// bound, so it stays outside this arm.
fn byte_carrier_leaf_copy(
    structural_types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
    path: &StructuralCopyPath,
    expected: StructuralTypeId,
) -> bool {
    let StructuralCopyPath::Ordinary(path) = path else {
        return false;
    };
    let Some((StructuralPathSegment::FixedIndex(index), prefix)) = path.split_last() else {
        return false;
    };
    let Some(parent) = resolve_copyable_path(structural_types, root, prefix) else {
        return false;
    };
    let Some(declaration) = terminal_psi::find_structural_type(structural_types, parent) else {
        return false;
    };
    let StructuralTypeShape::FixedArray { element, length } = &declaration.shape else {
        return false;
    };
    let byte_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid"));
    if !structural_types.iter().any(|candidate| {
        candidate.id == *element
            && candidate.shape == StructuralTypeShape::PrimitiveScalar(byte_type)
    }) {
        return false;
    };
    let Some(layout) =
        terminal_semantics::byte_carrier_scalar_layout(structural_types.iter(), expected)
    else {
        return false;
    };
    index
        .checked_add(layout.size)
        .is_some_and(|end| end <= *length)
}
