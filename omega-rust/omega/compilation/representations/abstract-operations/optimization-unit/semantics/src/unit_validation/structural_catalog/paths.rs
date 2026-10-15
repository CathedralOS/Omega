//! Projection geometry shared by call admission, origin replay and leaf copies.
//!
//! A selected payload and a runtime element compose in the same walk as record
//! fields. Resolving their endpoint grants no case activity, index bound or loan
//! permission; operation validation and custody replay establish those separately.
//! Byte snapshots retain their distinct leaf-shape rule below.

use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
use std::collections::BTreeMap;

pub(crate) fn structural_qualifications_match(
    carrier: StructuralTypeId,
    qualifications: &[StructuralDomainId],
    domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
) -> bool {
    !qualifications.windows(2).any(|pair| pair[0] >= pair[1])
        && qualifications.iter().all(|domain| {
            domains
                .get(domain)
                .is_some_and(|domain| domain.carrier == carrier)
        })
}

pub(crate) fn structural_projected_qualifications_match(
    root: StructuralTypeId,
    qualifications: &[terminal_psi::StructuralPathQualification],
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &terminal_psi::StructuralDomainDeclaration>,
) -> bool {
    !qualifications.windows(2).any(|pair| pair[0] >= pair[1])
        && qualifications.iter().all(|qualification| {
            !qualification.path.is_empty()
                && !qualification.path.iter().any(|segment| {
                    matches!(
                        segment,
                        terminal_psi::StructuralPathSegment::RepresentationView { .. }
                    )
                })
                // Membership may describe borrowed storage without owning it.
                // Keep owned projection unchanged; this query only checks the
                // declared carrier below an explicit reference crossing.
                && resolve_structural_path(types, root, &qualification.path).or_else(|| {
                    qualification.path.contains(&terminal_psi::StructuralPathSegment::Referent)
                        .then(|| terminal_semantics::runtime_structural_path_tip(
                            types.values().copied(), root, &qualification.path,
                        ).map(|declaration| declaration.id)).flatten()
                }).is_some_and(
                    |projected| {
                        domains
                            .get(&qualification.domain)
                            .is_some_and(|domain| domain.carrier == projected)
                    },
                )
        })
}

pub(crate) fn resolve_structural_path(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
) -> Option<StructuralTypeId> {
    resolve_path(
        types.values().copied(),
        |identity| types.get(&identity).copied(),
        structural_type,
        path,
        terminal_psi::StructuralFieldType::canonical_leaf_type,
    )
}

/// Resolve the referent a call argument's projection presents. Declared
/// structural geometry resolves first; a primitive leaf without a structural
/// child of its own — a byte carrier's runtime-selected u8 element — rejoins
/// its unique primitive declaration, the same endpoint primitive reads and
/// stores name. Bounds, access and live custody remain operation judgments.
pub(crate) fn resolve_runtime_projection(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
) -> Option<StructuralTypeId> {
    resolve_structural_path(types, structural_type, path).or_else(|| {
        terminal_semantics::primitive_projection_declaration(
            types.values().copied(),
            structural_type,
            path,
        )
    })
}

/// An explicit leaf copy can snapshot byte storage without making that field
/// a borrowable structural child or a projected-qualification subject.
pub(crate) fn resolve_leaf_copy_path(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
) -> Option<StructuralTypeId> {
    // Case copies carry their own active-payload evidence in StructuralCopyPath.
    // General endpoint geometry must not admit them as Ordinary copies.
    if path
        .iter()
        .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Case(_)))
    {
        return None;
    }
    resolve_path(
        types.values().copied(),
        |identity| types.get(&identity).copied(),
        structural_type,
        path,
        terminal_psi::StructuralFieldType::leaf_copy_type,
    )
}

/// Resolve storage endpoint geometry in a validated declaration catalog.
/// Case activity, runtime bounds, access, qualifications and live custody remain
/// independent operation judgments. This does not authorize reference formation
/// or cross a Reference carrier; callers must replay that crossing separately.
///
/// Borrowed names rebind onto this geometry, so a field ends at the storage a
/// loan of it addresses (`owner_loans::loaned_field_type`).
pub(crate) fn structural_projection_type<'types>(
    declarations: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
    structural_type: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
) -> Option<StructuralTypeId> {
    resolve_path(
        declarations.clone(),
        |identity| {
            declarations
                .clone()
                .find(|declaration| declaration.id == identity)
        },
        structural_type,
        path,
        abstract_operations::control_flow::owner_loans::loaned_field_type,
    )
    .or_else(|| {
        // An inline byte element has no structural child declaration. Its
        // loan names the same unique scalar endpoint as primitive accesses;
        // type resolution grants neither bounds nor backing authority.
        terminal_semantics::primitive_projection_declaration(declarations, structural_type, path)
    })
}

fn resolve_path<'types>(
    declarations: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
    declaration: impl Fn(StructuralTypeId) -> Option<&'types terminal_psi::StructuralTypeDeclaration>,
    mut structural_type: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    field_type: fn(&terminal_psi::StructuralFieldType) -> Option<terminal_psi::StructuralFieldType>,
) -> Option<StructuralTypeId> {
    declaration(structural_type)?;
    let mut payload_fields = None;
    for segment in path {
        let declaration = declaration(structural_type)?;
        if let terminal_psi::StructuralPathSegment::Case(case) = segment {
            if payload_fields.is_some() {
                return None;
            }
            let (terminal_psi::StructuralTypeShape::Sum { cases }
            | terminal_psi::StructuralTypeShape::Mixed { cases, .. }) = &declaration.shape
            else {
                return None;
            };
            payload_fields = Some(
                cases
                    .iter()
                    .find(|candidate| candidate.id == *case)?
                    .fields
                    .as_slice(),
            );
            continue;
        }
        if payload_fields.is_some()
            && !matches!(segment, terminal_psi::StructuralPathSegment::Field(_))
        {
            return None;
        }
        structural_type = match (segment, &declaration.shape) {
            (
                terminal_psi::StructuralPathSegment::RepresentationView {
                    structural_type: view,
                    stored_scalar,
                },
                shape,
            ) => {
                if matches!(shape, terminal_psi::StructuralTypeShape::Reference { .. }) {
                    return None;
                }
                terminal_semantics::representation_view_declaration(
                    declarations.clone(),
                    *view,
                    *stored_scalar,
                )?
                .id
            }
            (terminal_psi::StructuralPathSegment::Field(identity), shape) => {
                let fields = match payload_fields.take() {
                    Some(fields) => fields,
                    None => match shape {
                        terminal_psi::record_or_mixed_shape_pattern!(fields) => fields.as_slice(),
                        _ => return None,
                    },
                };
                let field = terminal_psi::live_field_by_identity(fields, *identity)?;
                match &field.field_type {
                    terminal_psi::StructuralFieldType::Structural(next) => *next,
                    leaf => field_type(leaf)?.leaf_declaration(declarations.clone())?.id,
                }
            }
            (
                terminal_psi::StructuralPathSegment::FixedIndex(index),
                terminal_psi::StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            (
                terminal_psi::StructuralPathSegment::RuntimeIndex { .. },
                terminal_psi::StructuralTypeShape::FixedArray { element, .. },
            ) => *element,
            // A view's extent is its stored runtime length: the index step
            // selects an element whose bound the owning operation carries
            // as its verified obligation.
            (
                terminal_psi::StructuralPathSegment::FixedIndex(_),
                terminal_psi::StructuralTypeShape::ElementView { element },
            ) => *element,
            (
                terminal_psi::StructuralPathSegment::RuntimeIndex { .. },
                terminal_psi::StructuralTypeShape::ElementView { element },
            ) => *element,
            _ => return None,
        };
    }
    payload_fields.is_none().then_some(structural_type)
}

/// Whether a leaf copy mints a record out of a byte carrier: `path` ends at
/// a literal index inside a `u8` fixed array whose remaining elements host
/// `expected`'s fixed scalar layout — the same window the verifier's
/// `byte_carrier_leaf_copy` admission proved. A runtime-selected index
/// cannot prove the wider `index + extent` bound, so it stays outside this
/// arm.
pub(crate) fn byte_carrier_leaf_window(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    expected: StructuralTypeId,
) -> bool {
    // This is the byte-window alternative of an Ordinary copy, so it has the
    // same payload-evidence boundary as resolve_leaf_copy_path.
    if path
        .iter()
        .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Case(_)))
    {
        return false;
    }
    let Some((terminal_psi::StructuralPathSegment::FixedIndex(index), prefix)) = path.split_last()
    else {
        return false;
    };
    let Some(parent) = resolve_structural_path(types, root, prefix) else {
        return false;
    };
    let Some(declaration) = types.get(&parent) else {
        return false;
    };
    let terminal_psi::StructuralTypeShape::FixedArray { element, length } = &declaration.shape
    else {
        return false;
    };
    let byte_type = semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
            .expect("u8 is valid"),
    );
    if !types.values().any(|candidate| {
        candidate.id == *element
            && candidate.shape == terminal_psi::StructuralTypeShape::PrimitiveScalar(byte_type)
    }) {
        return false;
    };
    let Some(layout) =
        terminal_semantics::byte_carrier_scalar_layout(types.values().copied(), expected)
    else {
        return false;
    };
    index
        .checked_add(layout.size)
        .is_some_and(|end| end <= *length)
}
