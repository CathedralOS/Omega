//! One place's authored evaluation frontiers, shared by call schedules.
//!
//! A collection is observed before its index. Descriptor crossings therefore
//! complete before the next selector can replace their supplying slot. Inline
//! offsets need no saved address. An owned copy reads its selected contents only
//! after its own selectors, and before the next actual argument begins.

use crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment;
use crate::lowering_error::{LoweringError, unsupported};
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};

/// A value formal observes a descriptor; an outer loan still names its slot.
#[derive(Clone, Copy)]
pub(crate) enum EndpointUse {
    Storage,
    Value(StructuralTypeId),
}

/// Retain either actual storage or descriptor words with their exact window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Capture {
    Address(StructuralTypeId),
    DescriptorValue {
        shape: StructuralTypeShape,
        preferred: Option<StructuralTypeId>,
    },
}

#[derive(Clone)]
pub(crate) enum Frontier {
    Capture { prefix: usize, capture: Capture },
    Selector { position: usize },
    Value { structural_type: StructuralTypeId },
}

pub(crate) fn plan(
    types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
    path: &[PreparedStructuralPathSegment],
    copy_value: bool,
    access: terminal_psi::StructuralAccess,
    endpoint: EndpointUse,
) -> Result<Vec<Frontier>, LoweringError> {
    let mut frontiers = Vec::new();
    let capture_address = access != terminal_psi::StructuralAccess::Owned || copy_value;
    let mut type_path = Vec::new();
    let root_declaration = types
        .iter()
        .find(|declaration| declaration.id == root)
        .ok_or(LoweringError::Unsupported(
            "completed place has no root declaration",
        ))?;
    let mut pending = matches!(
        root_declaration.shape,
        StructuralTypeShape::ElementView { .. }
            | StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
            )
    )
    .then_some((0, Capture::Address(root)));
    for (position, segment) in path.iter().enumerate() {
        let index = matches!(
            segment,
            PreparedStructuralPathSegment::ComputedIndex(_)
                | PreparedStructuralPathSegment::EvaluatedFixedIndex { .. }
                | PreparedStructuralPathSegment::RuntimeIndex(_)
                | PreparedStructuralPathSegment::CapturedIndex(_)
                | PreparedStructuralPathSegment::Static(StructuralPathSegment::FixedIndex(_))
        );
        if index {
            if let Some(shape) = descriptor_shape(types, root, &type_path) {
                // A view's extent belongs to that collection, so capture before
                // its marker as well as before its index expression.
                let prefix = if position > 0
                    && matches!(
                        path[position - 1],
                        PreparedStructuralPathSegment::ViewExtent { .. }
                    ) {
                    position - 1
                } else {
                    position
                };
                pending = Some((
                    prefix,
                    Capture::DescriptorValue {
                        shape,
                        preferred: None,
                    },
                ));
            }
        }
        if matches!(
            segment,
            PreparedStructuralPathSegment::ComputedIndex(_)
                | PreparedStructuralPathSegment::EvaluatedFixedIndex { .. }
                | PreparedStructuralPathSegment::RuntimeIndex(
                    checked_trees::CheckedRuntimeIndex::StorageRead { .. }
                        | checked_trees::CheckedRuntimeIndex::StructuralField(_)
                )
        ) {
            if let Some((prefix, capture)) = pending.take().filter(|_| capture_address) {
                frontiers.push(Frontier::Capture { prefix, capture });
            }
            frontiers.push(Frontier::Selector { position });
        }
        match segment {
            PreparedStructuralPathSegment::Static(segment) => {
                type_path.push(segment.clone());
                if *segment == StructuralPathSegment::Referent {
                    let tip = terminal_semantics::runtime_structural_path_tip(
                        types.iter(),
                        root,
                        &type_path,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "completed place lost its dereferenced carrier",
                    ))?;
                    pending = Some((position + 1, Capture::Address(tip.id)));
                }
            }
            PreparedStructuralPathSegment::EvaluatedFixedIndex { index, .. } => {
                type_path.push(StructuralPathSegment::FixedIndex(*index));
            }
            PreparedStructuralPathSegment::ViewExtent { .. } => {}
            _ => {
                // Coordinates select no storage in this type-only walk. Their
                // actual values and bounds remain with the emitting operation.
                type_path.push(StructuralPathSegment::FixedIndex(0));
            }
        }
    }
    // The selected consumer distinguishes a descriptor value from its slot.
    // Copies observe their final value immediately; other value arguments
    // retain descriptor words before any later actual can change the source.
    if !copy_value
        && let EndpointUse::Value(preferred) = endpoint
        && let Some(shape) = descriptor_shape(types, root, &type_path)
    {
        pending = Some((
            path.len(),
            Capture::DescriptorValue {
                shape,
                preferred: Some(preferred),
            },
        ));
    }
    if let Some((prefix, capture)) = pending.filter(|_| capture_address) {
        frontiers.push(Frontier::Capture { prefix, capture });
    }
    if copy_value {
        let structural_type = match endpoint {
            EndpointUse::Value(identity) => identity,
            EndpointUse::Storage => {
                terminal_semantics::runtime_structural_path_tip(types.iter(), root, &type_path)
                    .ok_or(LoweringError::Unsupported(
                        "completed copy lost its selected carrier",
                    ))?
                    .id
            }
        };
        frontiers.push(Frontier::Value { structural_type });
    }
    if frontiers.windows(2).any(|pair| {
        matches!((&pair[0], &pair[1]),
            (Frontier::Capture { prefix: first, .. }, Frontier::Capture { prefix: second, .. })
                if first > second)
    }) {
        return unsupported("completed place frontiers reverse their authored path");
    }
    Ok(frontiers)
}

/// Cross the selected checked formal's normalized identity into the existing
/// Terminal identity table. Source spelling and layout equality are not identity.
pub(crate) fn endpoint_use(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &checked_trees::CheckedTrees,
    types: &[StructuralTypeDeclaration],
    parameter: &checked_trees::CheckedUnitStructuralParameterPlan,
) -> Result<EndpointUse, LoweringError> {
    let identity =
        checked
            .type_identity_text(parameter.type_identity)
            .ok_or(LoweringError::Unsupported(
                "completed formal has no checked type identity",
            ))?;
    let identity = identities.intern(&identity);
    let mut matches = types.iter().filter(|row| row.identity == identity);
    let Some(declaration) = matches.next() else {
        // Independently prepared primitive storage has no view-value role.
        return Ok(EndpointUse::Storage);
    };
    if matches.next().is_some() {
        return unsupported("completed formal repeats its structural type identity");
    }
    Ok(
        if parameter.access == checked_trees::CheckedStructuralAccess::Owned
            || declaration.shape.is_canonical_view_descriptor()
        {
            EndpointUse::Value(declaration.id)
        } else {
            EndpointUse::Storage
        },
    )
}

fn descriptor_shape(
    types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeShape> {
    // Inline byte leaves need not have one standalone declaration: real field
    // identity and Case/Field namespace supply their exact carrier directly.
    if let Some(field) = terminal_semantics::runtime_structural_field(types.iter(), root, path)
        && let Some(carrier) = field.field_type.byte_sequence_carrier()
        && matches!(
            carrier,
            terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
        )
    {
        return Some(StructuralTypeShape::ByteSequence(carrier));
    }
    let shape = &terminal_semantics::runtime_structural_path_tip(types.iter(), root, path)?.shape;
    matches!(
        shape,
        StructuralTypeShape::ElementView { .. }
            | StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
            )
    )
    .then(|| shape.clone())
}

/// Select the descriptor presentation from actual source memberships. The
/// exact formal wins; intermediate carriers are deterministic among candidates
/// that retain every identical theory. Geometry grants neither image storage,
/// access, initialized bytes, liveness nor qualification evidence.
pub(crate) fn present_capture(
    capture: &Capture,
    access: terminal_psi::StructuralAccess,
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    whole: Vec<semantic_vocabulary::StructuralDomainId>,
    projected: Vec<terminal_psi::StructuralPathQualification>,
) -> Result<
    (
        StructuralTypeId,
        Vec<semantic_vocabulary::StructuralDomainId>,
        Vec<terminal_psi::StructuralPathQualification>,
    ),
    LoweringError,
> {
    let (shape, preferred) = match capture {
        Capture::Address(identity) => return Ok((*identity, whole, projected)),
        Capture::DescriptorValue { shape, preferred } => (shape, preferred),
    };
    if !matches!(access, terminal_psi::reading_borrow_pair_pattern!()) {
        return unsupported("descriptor completion requires readable occurrence access");
    }
    let eligible = |row: &&StructuralTypeDeclaration| {
        row.shape.is_canonical_view_descriptor()
            && terminal_semantics::descriptor_reborrow_shapes_match(
                shape,
                access,
                Some(access),
                &row.shape,
                access,
            )
    };
    let present = |identity, mut whole: Vec<_>, mut projected: Vec<_>| {
        terminal_semantics::present_structural_memberships(
            types.iter(),
            domains.iter(),
            identity,
            &mut whole,
            &mut projected,
        )
        .map_err(LoweringError::Unsupported)?;
        Ok((identity, whole, projected))
    };
    if let Some(identity) = preferred {
        let mut matches = types.iter().filter(|row| row.id == *identity);
        let row = matches.next().ok_or(LoweringError::Unsupported(
            "descriptor completion lost its exact formal carrier",
        ))?;
        if matches.next().is_some() || !eligible(&row) {
            return unsupported("descriptor completion changes its formal geometry or access");
        }
        return present(*identity, whole, projected);
    }
    let mut candidates = types.iter().filter(eligible).collect::<Vec<_>>();
    candidates.sort_by_key(|row| row.id);
    if candidates.windows(2).any(|pair| pair[0].id == pair[1].id) {
        return unsupported("descriptor completion repeats a canonical carrier identity");
    }
    let mut failure = None;
    for candidate in candidates {
        match present(candidate.id, whole.clone(), projected.clone()) {
            Ok(presentation) => return Ok(presentation),
            Err(error) => failure = Some(error),
        }
    }
    Err(failure.unwrap_or(LoweringError::Unsupported(
        "descriptor completion has no canonical membership-preserving carrier",
    )))
}
