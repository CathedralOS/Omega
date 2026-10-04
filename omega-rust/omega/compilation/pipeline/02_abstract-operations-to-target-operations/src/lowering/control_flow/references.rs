//! Reference-carrier custody through control-flow lowering. A carrier owns
//! loan permission, never its referent's storage; every origin and parent is
//! reconstructed from operations and signatures, never from pointer bits.
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::{OperationId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{
    TargetStructuralArgument, TargetStructuralParameter, TargetUnitOperation, TerminalPsiProvenance,
};
use terminal_psi::{
    StructuralAccess, StructuralFieldType, StructuralMultiplicity, StructuralPathSegment,
    StructuralTypeShape,
};
use terminal_psi::{StructuralArgument, StructuralOperationResult};

/// One outstanding loan tracked during lowering: which carrier leaf holds it,
/// which referent root it suspends, which parent leaf it descends from, and
/// which result row formed its identity. `root` is compile-time custody — it
/// never names a pointer or descriptor.
#[derive(Debug, Clone)]
pub(super) struct ReferenceCustody {
    /// Stable identity: the place and path where the carrier leaf was formed.
    /// Owned moves relocate the carrier, never this identity.
    pub(super) identity: (PlaceId, Vec<StructuralPathSegment>),
    /// Referent root resolved through custody: a primitive place directly, or
    /// an owned ingress parameter's leaf path when custody entered owned.
    pub(super) root: ReferenceRoot,
    /// Immediate parent leaf identity when this loan descends from a carrier.
    pub(super) parent: Option<(PlaceId, Vec<StructuralPathSegment>)>,
    /// The operation that formed this leaf's carrier identity, when the leaf
    /// descends from a body operation rather than an ingress parameter.
    pub(super) operation: Option<OperationId>,
    /// The structural result row that published this leaf's carrier identity.
    pub(super) result: StructuralOperationResult,
    /// This leaf's primitive referent type.
    pub(super) referent: StructuralTypeId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ReferenceRoot {
    /// A mutable borrowed parameter or activation-local primitive place.
    Place(PlaceId),
    /// One leaf of an owned reference-bearing ingress parameter; the referent
    /// storage belongs to the caller, so no local pointer exists for it.
    Ingress(PlaceId, Vec<StructuralPathSegment>),
    /// One selected permission conservatively pins every possible ingress path.
    Alternatives(PlaceId, Vec<Vec<StructuralPathSegment>>),
}

impl ReferenceRoot {
    fn place(&self) -> PlaceId {
        match self {
            Self::Place(place) | Self::Ingress(place, _) | Self::Alternatives(place, _) => *place,
        }
    }

    fn paths(&self) -> Vec<Vec<StructuralPathSegment>> {
        match self {
            Self::Place(_) => vec![Vec::new()],
            Self::Ingress(_, path) => vec![path.clone()],
            Self::Alternatives(_, paths) => paths.clone(),
        }
    }
}

fn roots_overlap(left: &ReferenceRoot, right: &ReferenceRoot) -> bool {
    if left.place() != right.place() {
        return false;
    }
    let overlaps_path = |path: &[StructuralPathSegment]| match right {
        ReferenceRoot::Place(_) => true,
        ReferenceRoot::Ingress(_, other) => path.starts_with(other) || other.starts_with(path),
        ReferenceRoot::Alternatives(_, paths) => paths
            .iter()
            .any(|other| path.starts_with(other) || other.starts_with(path)),
    };
    match left {
        ReferenceRoot::Place(_) => true,
        ReferenceRoot::Ingress(_, path) => overlaps_path(path),
        ReferenceRoot::Alternatives(_, paths) => paths.iter().any(|path| overlaps_path(path)),
    }
}

/// A live reference's payload producer and geometry after dereferencing it.
/// Custody retains every possible origin; physical addressing loads the one
/// address the callee returned, never a selected entry from that origin set.
pub(in crate::lowering) struct WritableReferentDestination {
    pub(in crate::lowering) producer: OperationId,
    pub(in crate::lowering) result: StructuralOperationResult,
    pub(in crate::lowering) referent: StructuralTypeId,
    /// Relative to the referent, excluding the explicit crossing.
    pub(in crate::lowering) path: Vec<StructuralPathSegment>,
}

fn returned_mutable_record_projection(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    carrier: PlaceId,
    path: &[StructuralPathSegment],
) -> Option<WritableReferentDestination> {
    let (StructuralPathSegment::Referent, tail) = path.split_first()? else {
        return None;
    };
    let leaf = live.references.get(&(carrier, Vec::new()))?;
    let declaration = types.get(&leaf.result.structural_type)?;
    let StructuralTypeShape::Reference {
        referent,
        access: StructuralAccess::MutableBorrow,
    } = &declaration.shape
    else {
        return None;
    };
    // A whole `Referent` path is also the established primitive-reference
    // spelling. Only a record referent may enter this aggregate-origin route.
    if !matches!(
        &types.get(referent)?.shape,
        StructuralTypeShape::Record { .. }
    ) {
        return None;
    }
    let producer = leaf.operation?;
    if *referent != leaf.referent
        || live
            .references
            .values()
            .any(|child| child.parent.as_ref() == Some(&leaf.identity))
        || leaf.result.place != carrier
        || leaf.result.multiplicity != StructuralMultiplicity::Affine
        || !leaf.result.qualification_establishments.is_empty()
        || !leaf.result.qualifications.is_empty()
        || !leaf.result.projected_qualifications.is_empty()
        || !leaf.result.claims.is_empty()
        || !function.operations.iter().any(|operation| matches!(operation,
            AbstractOperation::CallStructural {
                psi_operation,
                result: abstract_operations::AbstractStructuralCallResult::Structural { result, .. },
                ..
            } if *psi_operation == producer && result == &leaf.result))
    {
        return None;
    }
    let root = leaf.root.place();
    // The returned loan may point into an arrival or an activation-local
    // owner. Validate each possible origin through the same storage contract
    // used when lending it; its physical residence does not grant authority.
    let exact_origin = |path: &[StructuralPathSegment]| {
        direct_storage_type(
            function,
            live,
            types,
            &StructuralArgument {
                place: root,
                path: path.to_vec(),
                access: StructuralAccess::WriteOnlyBorrow,
            },
        ) == Some(*referent)
    };
    if !match &leaf.root {
        ReferenceRoot::Place(_) => exact_origin(&[]),
        ReferenceRoot::Ingress(_, path) => exact_origin(path),
        ReferenceRoot::Alternatives(_, paths) => {
            !paths.is_empty() && paths.iter().all(|path| exact_origin(path))
        }
    } || live.structural_homes.get(&carrier)?.operation_result()
        != Some((producer, &leaf.result))
    {
        return None;
    }
    Some(WritableReferentDestination {
        producer,
        result: leaf.result.clone(),
        referent: *referent,
        path: tail.to_vec(),
    })
}

pub(super) fn writable_referent_destination(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    carrier: PlaceId,
    path: &[StructuralPathSegment],
) -> Option<WritableReferentDestination> {
    returned_mutable_record_projection(function, live, types, carrier, path)
}

/// Borrow through the live carrier's address word, regardless of which
/// operation established or relocated it. Custody proves the leaf is live;
/// the typed prefix selects its descriptor and the suffix selects backing.
/// Never substitute one possible origin for the pointer actually stored.
pub(super) fn reference_call_argument(
    argument: &StructuralArgument,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    destination: &TargetStructuralParameter,
    function: &AbstractFunction,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
) -> Result<TargetStructuralArgument, LoweringError> {
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let crossing = argument
        .path
        .iter()
        .position(|segment| *segment == StructuralPathSegment::Referent)
        .ok_or_else(invalid)?;
    let prefix = &argument.path[..crossing];
    let suffix = &argument.path[crossing + 1..];
    if prefix.iter().chain(suffix).any(|segment| {
        !matches!(
            segment,
            StructuralPathSegment::Field(_)
                | StructuralPathSegment::FixedIndex(_)
                | StructuralPathSegment::Referent
        )
    }) {
        return Err(invalid());
    }
    if argument.access == StructuralAccess::Owned
        || argument.access != declaration.access
        || declaration.multiplicity != StructuralMultiplicity::Unrestricted
        || !declaration.qualifications.is_empty()
        || !declaration.projected_qualifications.is_empty()
    {
        return Err(invalid());
    }
    let leaf = live
        .references
        .get(&(argument.place, prefix.to_vec()))
        .ok_or_else(invalid)?;
    if live
        .references
        .values()
        .any(|child| child.parent.as_ref() == Some(&leaf.identity))
    {
        return Err(invalid());
    }
    let (root_type, source) = if let Some(home) = live.structural_homes.get(&argument.place) {
        if home.has_claims()
            || home.multiplicity() == StructuralMultiplicity::Linear
            || !home.qualifications().is_empty()
            || !home.projected_qualifications().is_empty()
            || home
                .operation_result()
                .is_some_and(|(_, result)| !result.qualification_establishments.is_empty())
        {
            return Err(invalid());
        }
        let source = match &home.origin {
            target_operations::TargetStructuralHomeOrigin::OperationResult {
                operation, ..
            } => target_operations::TargetStructuralArgumentSource::StructuralHome {
                psi_operation: *operation,
            },
            target_operations::TargetStructuralHomeOrigin::BlockParameter {
                block,
                declaration,
            } => target_operations::TargetStructuralArgumentSource::BlockParameter {
                block: *block,
                place: declaration.place,
            },
        };
        (home.structural_type(), source)
    } else {
        let parameter = prepared
            .parameters
            .iter()
            .find(|parameter| parameter.place == argument.place)
            .ok_or_else(invalid)?;
        if parameter.access == StructuralAccess::WriteOnlyBorrow {
            return Err(invalid());
        }
        (
            parameter.structural_type,
            parameter.placement.clone().into(),
        )
    };
    let reference_type = leaf_type(types, root_type, prefix).ok_or_else(invalid)?;
    let StructuralTypeShape::Reference { referent, access } =
        types.get(&reference_type).ok_or_else(invalid)?.shape
    else {
        return Err(invalid());
    };
    if referent != leaf.referent
        || !(access == argument.access
            || access == StructuralAccess::MutableBorrow
                && matches!(
                    argument.access,
                    StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
                ))
    {
        return Err(invalid());
    }
    let mut shapes = BTreeMap::new();
    let mut active = BTreeSet::new();
    let mut referent = referent;
    let mut granted_access = access;
    let mut suffix = suffix;
    while let Some(crossing) = suffix
        .iter()
        .position(|segment| *segment == StructuralPathSegment::Referent)
    {
        if granted_access == StructuralAccess::WriteOnlyBorrow {
            return Err(invalid());
        }
        let (carrier, _, _) =
            crate::lowering::structural_layout::resolve_structural_projection_path(
                referent,
                &suffix[..crossing],
                types,
                &mut shapes,
                &mut active,
            )?;
        let StructuralTypeShape::Reference {
            referent: next,
            access,
        } = types.get(&carrier).ok_or_else(invalid)?.shape
        else {
            return Err(invalid());
        };
        if !(access == argument.access
            || access == StructuralAccess::MutableBorrow
                && matches!(
                    argument.access,
                    StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
                ))
        {
            return Err(invalid());
        }
        referent = next;
        granted_access = access;
        suffix = &suffix[crossing + 1..];
    }
    let (selected, shape, offset) =
        crate::lowering::structural_layout::resolve_structural_projection_path(
            referent,
            suffix,
            types,
            &mut shapes,
            &mut active,
        )?;
    if selected != declaration.structural_type
        || crate::lowering::structural_layout::structural_parameter_shape(shape, declaration.access)
            != destination.shape
    {
        return Err(invalid());
    }
    Ok(TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type: root_type,
        structural_type: selected,
        shape: destination.shape,
        source_byte_offset: offset,
        fixed_array_length: None,
        element_stride: None,
        indices: Vec::new(),
        source,
        destination: destination.placement.clone(),
    })
}

/// Inspect owned containment only. A reference's referent is not its payload.
pub(super) fn contains_reference(types: &StructuralTypeLookup<'_>, root: StructuralTypeId) -> bool {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        let Some(declaration) = types.get(&current) else {
            continue;
        };
        let mut push_fields = |fields: &[terminal_psi::StructuralFieldDeclaration]| {
            pending.extend(fields.iter().filter_map(|field| match field.field_type {
                StructuralFieldType::Structural(child) => Some(child),
                _ => None,
            }));
        };
        match &declaration.shape {
            StructuralTypeShape::Reference { .. } => return true,
            StructuralTypeShape::Record { fields } => push_fields(fields),
            StructuralTypeShape::Sum { cases } => {
                for case in cases {
                    push_fields(&case.fields);
                }
            }
            StructuralTypeShape::Mixed { fields, cases } => {
                push_fields(fields);
                for case in cases {
                    push_fields(&case.fields);
                }
            }
            StructuralTypeShape::FixedArray { element, .. }
            | StructuralTypeShape::ElementView { element } => pending.push(*element),
            StructuralTypeShape::FusedServiceBinding
            | StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::ByteSequence(_) => {}
        }
    }
    false
}

/// Exact declaration-order reference leaves of the supported owned shape.
/// `maximum_leaves` bounds reconstruction exactly like the verified rule.
pub(super) fn leaf_paths(
    types: &StructuralTypeLookup<'_>,
    root: StructuralTypeId,
    maximum_leaves: usize,
) -> Option<Vec<Vec<StructuralPathSegment>>> {
    let mut output = Vec::new();
    let mut pending = vec![(root, Vec::new())];
    while let Some((current, path)) = pending.pop() {
        if !contains_reference(types, current) {
            continue;
        }
        let Some(declaration) = types.get(&current) else {
            continue;
        };
        match &declaration.shape {
            StructuralTypeShape::Reference { .. } => {
                if output.len() == maximum_leaves {
                    return None;
                }
                output.push(path);
            }
            StructuralTypeShape::Record { fields } => {
                for field in fields.iter().rev() {
                    if let StructuralFieldType::Structural(child) = field.field_type {
                        if !contains_reference(types, child) {
                            continue;
                        }
                        // Every pending owned subtree owes at least one leaf.
                        // Bound the worklist before a wide DAG can amplify it.
                        if pending.len().checked_add(output.len())? >= maximum_leaves {
                            return None;
                        }
                        let mut child_path = path.clone();
                        child_path.push(StructuralPathSegment::Field(field.identity.clone()));
                        pending.push((child, child_path));
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, length } => {
                if !contains_reference(types, *element) {
                    continue;
                }
                let count = usize::try_from(*length).ok()?;
                if pending
                    .len()
                    .checked_add(output.len())?
                    .checked_add(count)?
                    > maximum_leaves
                {
                    return None;
                }
                for index in (0..*length).rev() {
                    let mut child_path = path.clone();
                    child_path.push(StructuralPathSegment::FixedIndex(index));
                    pending.push((*element, child_path));
                }
            }
            _ => {}
        }
    }
    Some(output)
}

/// Shared references observe their declared referent without acquiring its
/// contents. Exclusive primitive storage uses the same pointer carrier;
/// writable aggregates retain their separate restoration checks below.
pub(super) fn referent(
    types: &StructuralTypeLookup<'_>,
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    let declaration = types.get(&structural_type)?;
    match declaration.shape {
        StructuralTypeShape::Reference {
            referent,
            access: StructuralAccess::SharedBorrow,
        } if types.contains_key(&referent) => Some(referent),
        StructuralTypeShape::Reference {
            referent,
            access:
                StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow,
        } if matches!(
            types.get(&referent).map(|declaration| &declaration.shape),
            Some(StructuralTypeShape::PrimitiveScalar(_))
        ) =>
        {
            Some(referent)
        }
        _ => None,
    }
}

/// The aggregate bare-reference carrier native lowering realizes: a borrow of
/// a plain record whose referent contains storage, not another loan roster.
/// Shared and mutable carriers read the referent through the loan; mutable
/// and write-only carriers store through it. The carrier retains permission
/// metadata and an address payload, never a copied referent record.
fn plain_record_referent(
    types: &StructuralTypeLookup<'_>,
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    let StructuralTypeShape::Reference {
        referent,
        access:
            StructuralAccess::SharedBorrow
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow,
    } = &types.get(&structural_type)?.shape
    else {
        return None;
    };
    matches!(
        types.get(referent).map(|declaration| &declaration.shape),
        Some(StructuralTypeShape::Record { .. })
    )
    .then_some(*referent)
    .filter(|referent| !contains_reference(types, *referent))
}

/// A field-only carrier path ending at a reference leaf, then its referent;
/// a non-reference leaf names its own storage type.
pub(super) fn leaf_referent(
    types: &StructuralTypeLookup<'_>,
    current: StructuralTypeId,
    fields: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    let leaf = leaf_type(types, current, fields)?;
    // A reference leaf over a record names its whole referent the same way a
    // primitive referent does: the mapped source's type is the referent
    // declaration itself, not the `Reference` shell.
    if let Some(referent) = referent(types, leaf) {
        return Some(referent);
    }
    match &types.get(&leaf)?.shape {
        StructuralTypeShape::Reference { .. } => plain_record_referent(types, leaf),
        _ => Some(leaf),
    }
}

/// The structural type the path names beneath `current`: each `Field`
/// segment descends through record or mixed fields, with a plain leaf field
/// resolving to its canonical standalone declaration, and a `FixedIndex`
/// segment descends into a fixed array's element within its declared extent.
pub(super) fn leaf_type(
    types: &StructuralTypeLookup<'_>,
    mut current: StructuralTypeId,
    fields: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for segment in fields {
        if let StructuralPathSegment::FixedIndex(index) = segment {
            let declaration = types.get(&current)?;
            let StructuralTypeShape::FixedArray { element, length } = &declaration.shape else {
                return None;
            };
            if index >= length {
                return None;
            }
            current = *element;
            continue;
        }
        let StructuralPathSegment::Field(identity) = segment else {
            return None;
        };
        let declaration = types.get(&current)?;
        let fields = match &declaration.shape {
            StructuralTypeShape::Record { fields } => fields,
            StructuralTypeShape::Mixed { fields, .. } => fields,
            _ => return None,
        };
        let field = fields.iter().find(|field| &field.identity == identity)?;
        if field.relevance.is_erased() {
            return None;
        }
        current = match &field.field_type {
            StructuralFieldType::Structural(child) => *child,
            leaf => {
                let shape = leaf.canonical_leaf_shape()?;
                *types
                    .iter()
                    .find(|(_, declaration)| declaration.shape == shape)
                    .map(|(id, _)| id)?
            }
        };
    }
    Some(current)
}

/// The structural contract a place currently carries: a signature parameter, a
/// block-entry parameter, an ordinary home, or a live bare carrier result.
fn source_contract(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    place: PlaceId,
) -> Option<(StructuralTypeId, StructuralMultiplicity)> {
    if let Some(parameter) = function.arrival_parameter(place) {
        return Some((parameter.structural_type, parameter.multiplicity));
    }
    if let Some(home) = live.structural_homes.get(&place) {
        return Some((home.structural_type(), home.multiplicity()));
    }
    live.references
        .get(&(place, Vec::new()))
        .map(|leaf| (leaf.result.structural_type, leaf.result.multiplicity))
}

/// Borrow storage through its published contract, regardless of whether it
/// arrived as a parameter or was constructed in this activation. An affine
/// owner may lend storage without copying it; a reference crossing is a
/// different operation and must resolve live carrier custody instead.
fn direct_storage_type(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> Option<StructuralTypeId> {
    if !source.path.iter().all(|segment| {
        matches!(
            segment,
            StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
        )
    }) {
        return None;
    }
    let (structural_type, multiplicity, access, unqualified) = if let Some(parameter) =
        function.arrival_parameter(source.place)
    {
        (
            parameter.structural_type,
            parameter.multiplicity,
            parameter.access,
            parameter.qualifications.is_empty() && parameter.projected_qualifications.is_empty(),
        )
    } else {
        let home = live.structural_homes.get(&source.place)?;
        (
            home.structural_type(),
            home.multiplicity(),
            home.access(),
            home.qualifications().is_empty()
                && home.projected_qualifications().is_empty()
                && !home.has_claims()
                && home
                    .operation_result()
                    .is_none_or(|(_, result)| result.qualification_establishments.is_empty()),
        )
    };
    let access_is_compatible = match source.access {
        StructuralAccess::SharedBorrow => matches!(
            access,
            StructuralAccess::Owned
                | StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
        ),
        StructuralAccess::MutableBorrow => matches!(
            access,
            StructuralAccess::Owned | StructuralAccess::MutableBorrow
        ),
        StructuralAccess::WriteOnlyBorrow => matches!(
            access,
            StructuralAccess::Owned
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        ),
        StructuralAccess::Owned => false,
    };
    if !access_is_compatible
        || multiplicity == StructuralMultiplicity::Linear
        || !unqualified
        || function
            .entry_claims
            .iter()
            .any(|claim| claim.input == source.place)
    {
        return None;
    }
    leaf_type(types, structural_type, &source.path)
}

/// The referent type an establishment or projection may name. An empty path
/// names a whole ingress place, a trailing `Referent` dereferences a live
/// carrier, and a field/fixed-index path names exact borrowed ingress storage.
pub(super) fn reference_source_type(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> Option<StructuralTypeId> {
    if !matches!(
        source.access,
        StructuralAccess::SharedBorrow
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow
    ) {
        return None;
    }
    if let Some((StructuralPathSegment::Referent, fields)) = source.path.split_last() {
        let carrier = source_contract(function, live, source.place)?.0;
        return leaf_referent(types, carrier, fields);
    }
    direct_storage_type(function, live, types, source)
}

// A new shared loan may coexist only with authenticated shared carrier leaves.
// The original roots and parent identities remain live until every loan ends.
fn shared_source_compatible(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    carrier: &PlaceId,
    path: &[StructuralPathSegment],
    access: StructuralAccess,
) -> bool {
    access == StructuralAccess::SharedBorrow
        && source_contract(function, live, *carrier)
            .map(|(root, _)| root)
            .and_then(|root| leaf_type(types, root, path))
            .and_then(|leaf| types.get(&leaf))
            .is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    StructuralTypeShape::Reference {
                        access: StructuralAccess::SharedBorrow,
                        ..
                    }
                )
            })
}

/// A `.., Referent` source names a suspended child of the located carrier
/// leaf; an empty path names the whole ingress referent directly. A static
/// field/fixed-index path retains the exact borrowed ingress address.
fn normalized_source(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> Result<(ReferenceRoot, Option<(PlaceId, Vec<StructuralPathSegment>)>), LoweringError> {
    if let Some((StructuralPathSegment::Referent, carrier_path)) = source.path.split_last() {
        let Some(leaf) = live.references.get(&(source.place, carrier_path.to_vec())) else {
            // The member leaf of a carrier never held as a live reference
            // borrows that carrier's own storage; only a non-reference member
            // may root there — a missing reference carrier is a custody gap.
            let (carrier, _) = source_contract(function, live, source.place)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            return match leaf_type(types, carrier, carrier_path).and_then(|leaf| {
                types
                    .get(&leaf)
                    .and_then(|declaration| match declaration.shape {
                        StructuralTypeShape::Reference { .. } => None,
                        _ => Some(()),
                    })
            }) {
                Some(()) => Ok((
                    ReferenceRoot::Ingress(source.place, carrier_path.to_vec()),
                    None,
                )),
                None => Err(LoweringError::unsupported_control_flow(function.machine)),
            };
        };
        if live.references.iter().any(|((carrier, path), child)| {
            child.parent.as_ref() == Some(&leaf.identity)
                && !shared_source_compatible(function, live, types, carrier, path, source.access)
        }) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        Ok((leaf.root.clone(), Some(leaf.identity.clone())))
    } else if source.path.is_empty() {
        if live.references.iter().any(|((carrier, path), reference)| {
            reference.root.place() == source.place
                && !shared_source_compatible(function, live, types, carrier, path, source.access)
        }) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        Ok((ReferenceRoot::Place(source.place), None))
    } else if direct_storage_type(function, live, types, source).is_some() {
        let root = ReferenceRoot::Ingress(source.place, source.path.clone());
        if live.references.iter().any(|((carrier, path), leaf)| {
            leaf.root.place() == source.place
                && !shared_source_compatible(function, live, types, carrier, path, source.access)
        }) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        Ok((root, None))
    } else {
        Err(LoweringError::unsupported_control_flow(function.machine))
    }
}

/// Substitute a callee's formal result-source mapping into the caller's actual
/// argument at the same structural-parameter position.
fn call_source(
    callee: &AbstractFunction,
    arguments: &[StructuralArgument],
    formal: &StructuralArgument,
) -> Option<StructuralArgument> {
    let position = callee
        .structural_parameters
        .iter()
        .position(|parameter| parameter.place == formal.place)?;
    let mut source = arguments.get(position)?.clone();
    source.path.extend(formal.path.iter().cloned());
    source.access = formal.access;
    Some(source)
}

/// The formal origin a callee result mapping names: a borrowed primitive
/// parameter or an ingress leaf path under an owned or shared carrier.
fn formal_origin(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> Option<ReferenceRoot> {
    let parameter = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source.place)?;
    if source.path.is_empty()
        && matches!(
            parameter.access,
            StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
        )
    {
        Some(ReferenceRoot::Place(source.place))
    } else if let Some((StructuralPathSegment::Referent, path)) = source.path.split_last()
        && matches!(
            parameter.access,
            StructuralAccess::Owned | StructuralAccess::SharedBorrow
        )
    {
        Some(ReferenceRoot::Ingress(source.place, path.to_vec()))
    } else if source.access == StructuralAccess::SharedBorrow
        && matches!(
            parameter.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        )
        || source.access == StructuralAccess::MutableBorrow
            && parameter.access == StructuralAccess::MutableBorrow
        || source.access == StructuralAccess::WriteOnlyBorrow
            && matches!(
                parameter.access,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            )
    {
        if parameter.multiplicity == StructuralMultiplicity::Linear
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || function
                .entry_claims
                .iter()
                .any(|claim| claim.input == source.place)
            || leaf_type(types, parameter.structural_type, &source.path).is_none()
        {
            return None;
        }
        Some(ReferenceRoot::Ingress(source.place, source.path.clone()))
    } else {
        None
    }
}

/// An owned argument that carries at least one reference leaf moves the loans
/// with it; the callee's ingress replay tracks each leaf exactly.
fn argument_owns_references(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    argument: &StructuralArgument,
) -> bool {
    argument.access == StructuralAccess::Owned
        && source_contract(function, live, argument.place)
            .is_some_and(|(contract, _)| contains_reference(types, contract))
}

fn access_covers(authority: StructuralAccess, requested: StructuralAccess) -> bool {
    authority == requested
        || authority == StructuralAccess::Owned
        || authority == StructuralAccess::MutableBorrow
            && matches!(
                requested,
                StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
            )
}

/// Carrier storage and its pointees are distinct aliasing regions.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CallRegion {
    OrdinaryStorage,
    CarrierStorage,
    Referent,
}

mod call_paths;

/// Whether `place` is currently suspended as a live loan's referent root.
/// Cleanup cannot discard storage that an outstanding carrier still names.
pub(super) fn is_suspended_root(live: &LiveDefinitions, place: PlaceId) -> bool {
    live.references
        .values()
        .any(|reference| reference.root.place() == place)
}

/// Reads, writes and establishments cannot touch a suspended referent, and a
/// live carrier is never owned data for primitive or aggregate access.
pub(super) fn check_root_access(live: &LiveDefinitions, place: PlaceId) -> bool {
    check_region_access(live, place, &[])
}

fn check_region_access(
    live: &LiveDefinitions,
    place: PlaceId,
    path: &[StructuralPathSegment],
) -> bool {
    let overlaps = |other: &[StructuralPathSegment]| {
        terminal_semantics::structural_paths_may_overlap(path, other)
    };
    live.references
        .iter()
        .any(|((carrier, carrier_path), reference)| {
            (*carrier == place && overlaps(carrier_path))
                || (reference.identity.0 == place && overlaps(&reference.identity.1))
                || (reference.root.place() == place
                    && match &reference.root {
                        ReferenceRoot::Place(_) => true,
                        ReferenceRoot::Ingress(_, path) => overlaps(path),
                        ReferenceRoot::Alternatives(_, paths) => {
                            paths.iter().any(|path| overlaps(path))
                        }
                    })
        })
}

/// Ordinary field observations and writes touch their exact region. Rejoin the
/// current declaration/home before using shared geometry; a missing contract
/// conservatively covers the entire root and never supplies disjointness.
pub(super) fn check_projected_access(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    place: PlaceId,
    path: &[StructuralPathSegment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
) -> bool {
    let Some((root, _)) = source_contract(function, live, place) else {
        return check_root_access(live, place);
    };
    let region =
        terminal_semantics::projected_storage_region(types.values().copied(), root, path, field);
    check_region_access(live, place, region.direct_path())
}

pub(super) fn check_operation_access(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    operation: &AbstractOperation,
    place: PlaceId,
) -> bool {
    match operation {
        AbstractOperation::StructuralScalarFieldRead { path, field, .. } => {
            let region =
                source_contract(function, live, place).map_or_else(Vec::new, |(root, _)| {
                    terminal_semantics::canonical_field_storage_region(
                        types.values().copied(),
                        root,
                        path,
                        *field,
                    )
                });
            check_region_access(live, place, &region)
        }
        AbstractOperation::WriteOnlyPrimitiveStore { path, .. }
            if path.contains(&StructuralPathSegment::Referent) =>
        {
            call_paths::call_reference_custody(
                function,
                live,
                types,
                &StructuralArgument {
                    place,
                    path: path.clone(),
                    access: StructuralAccess::WriteOnlyBorrow,
                },
            )
            .is_err()
        }
        AbstractOperation::PrimitiveScalarRead { path, .. }
        | AbstractOperation::StructuralCaseMembership { path, .. }
        | AbstractOperation::WriteOnlyPrimitiveStore { path, .. } => {
            check_projected_access(function, live, types, place, path, None)
        }
        AbstractOperation::StructuralScalarFieldStore { path, field, .. } => {
            check_projected_access(function, live, types, place, path, Some(*field))
        }
        _ => check_root_access(live, place),
    }
}

/// Validate every structural call argument under reference custody. Owned
/// carriers move their complete live leaf roster; a suspended root may only be
/// spelled through a `.., Referent` projection; two reference-touching
/// arguments cannot share one root unless both are shared borrows. Returns the
/// moved leaves keyed by their current carrier path; the caller commits the
/// removal after every other call check succeeds.
pub(super) fn call_arguments(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &LiveDefinitions,
    arguments: &[StructuralArgument],
) -> Result<BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceCustody>, LoweringError> {
    let mut normalized = Vec::new();
    let mut dependencies = Vec::new();
    let mut moved = BTreeMap::new();
    for (argument_position, argument) in arguments.iter().enumerate() {
        if argument.path.contains(&StructuralPathSegment::Referent) {
            let resolved = call_paths::call_reference_custody(function, live, types, argument)?;
            dependencies.push((argument_position, resolved.dependencies));
            normalized.push((
                argument_position,
                resolved.origins,
                argument.access,
                CallRegion::Referent,
            ));
            continue;
        }
        if source_contract(function, live, argument.place)
            .is_some_and(|(contract, _)| contains_reference(types, contract))
        {
            let authority = function
                .arrival_parameter(argument.place)
                .map(|parameter| parameter.access)
                .or_else(|| {
                    live.structural_homes
                        .get(&argument.place)
                        .map(|home| home.access())
                })
                .unwrap_or(StructuralAccess::Owned);
            if !access_covers(authority, argument.access) {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            if !argument
                .path
                .iter()
                .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let (contract, _) = source_contract(function, live, argument.place)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            let selected = leaf_type(types, contract, &argument.path)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            let paths = leaf_paths(types, selected, live.references.len())
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            if paths.len()
                != live
                    .references
                    .keys()
                    .filter(|(carrier, path)| {
                        *carrier == argument.place && path.starts_with(&argument.path)
                    })
                    .count()
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let storage = ReferenceRoot::Ingress(argument.place, argument.path.clone());
            if live.references.iter().any(|((carrier, path), leaf)| {
                !(*carrier == argument.place && path.starts_with(&argument.path))
                    && roots_overlap(&storage, &leaf.root)
            }) {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            normalized.push((
                argument_position,
                vec![storage],
                argument.access,
                CallRegion::CarrierStorage,
            ));
            for path in paths {
                let permission = leaf_type(types, selected, &path)
                    .and_then(|leaf| types.get(&leaf).map(|declaration| &declaration.shape))
                    .and_then(|shape| match shape {
                        StructuralTypeShape::Reference { access, .. } => Some(*access),
                        _ => None,
                    })
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                let mut carrier_path = argument.path.clone();
                carrier_path.extend(path);
                let leaf = live
                    .references
                    .get(&(argument.place, carrier_path.clone()))
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                // Abstract callee ingress promises independently available
                // leaves. A type cannot describe an unknown suspended parent
                // or a parent/child pair packed into the same incoming record.
                if live
                    .references
                    .values()
                    .any(|child| child.parent.as_ref() == Some(&leaf.identity))
                    || (argument.access == StructuralAccess::Owned
                        && moved
                            .insert((argument.place, carrier_path), leaf.clone())
                            .is_some())
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                normalized.push((
                    argument_position,
                    vec![leaf.root.clone()],
                    if access_covers(argument.access, permission) {
                        permission
                    } else {
                        argument.access
                    },
                    CallRegion::Referent,
                ));
            }
            continue;
        }
        if source_contract(function, live, argument.place)
            .is_some_and(|(contract, _)| contains_reference(types, contract))
            || live
                .references
                .values()
                .any(|reference| reference.root.place() == argument.place)
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        } else {
            normalized.push((
                argument_position,
                vec![ReferenceRoot::Place(argument.place)],
                argument.access,
                CallRegion::OrdinaryStorage,
            ));
        }
    }
    for (position, (argument_position, roots, access, reference_access)) in
        normalized.iter().enumerate()
    {
        if normalized[position + 1..].iter().any(
            |(other_position, others, other_access, other_reference_access)| {
                (*reference_access != CallRegion::OrdinaryStorage
                    || *other_reference_access != CallRegion::OrdinaryStorage)
                    && (argument_position != other_position
                        || (*reference_access == CallRegion::Referent
                            && *other_reference_access == CallRegion::Referent))
                    && roots
                        .iter()
                        .any(|root| others.iter().any(|other| roots_overlap(root, other)))
                    && (*access != StructuralAccess::SharedBorrow
                        || *other_access != StructuralAccess::SharedBorrow)
            },
        ) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
    }
    // Resolving an inner descriptor borrows all intermediate storage for the
    // duration of the call, even when the final referent is disjoint.
    for (argument_position, roots) in dependencies {
        // Owned carrier arguments normalize to their referents above, but
        // consuming their descriptor storage also conflicts with this read.
        if arguments.iter().enumerate().any(|(other_position, other)| {
            other_position != argument_position
                && other.access != StructuralAccess::SharedBorrow
                && !other.path.contains(&StructuralPathSegment::Referent)
                && roots.iter().any(|root| {
                    roots_overlap(
                        root,
                        &ReferenceRoot::Ingress(other.place, other.path.clone()),
                    )
                })
        }) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        if normalized
            .iter()
            .any(|(other_position, others, access, _)| {
                *other_position != argument_position
                    && *access != StructuralAccess::SharedBorrow
                    && roots
                        .iter()
                        .any(|root| others.iter().any(|other| roots_overlap(root, other)))
            })
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
    }
    Ok(moved)
}

/// Apply the staged argument moves once every other call check succeeded.
/// Moves are keyed by current carrier path, so result leaves re-established
/// under a moved identity survive the commit.
pub(super) fn commit_argument_moves(
    live: &mut LiveDefinitions,
    moved: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceCustody>,
) {
    for key in moved.keys() {
        live.references.remove(key);
    }
}

/// Whether this argument's place touches reference custody: an owned carrier,
/// a `.., Referent` projection, or a suspended primitive root. Call lowering
/// must route such rows through the custody-aware argument path.
pub(super) fn touches(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &LiveDefinitions,
    argument: &StructuralArgument,
) -> bool {
    argument_owns_references(function, live, types, argument)
        || matches!(
            argument.path.split_last(),
            Some((StructuralPathSegment::Referent, _))
        )
        || live
            .references
            .values()
            .any(|reference| reference.root.place() == argument.place)
        || live
            .references
            .keys()
            .any(|(carrier, _)| *carrier == argument.place)
}

/// Establish one verified reference carrier: the result place receives the
/// loan and the referent root is retained as metadata only.
pub(super) fn establish(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::EstablishReference {
        psi_operation,
        result,
        source,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let referent = referent(types, result.structural_type)
        .or_else(|| plain_record_referent(types, result.structural_type))
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    if result.multiplicity != StructuralMultiplicity::Affine
        || !result.claims.is_empty()
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || live.structural_homes.contains_key(&result.place)
        || live
            .references
            .keys()
            .any(|(carrier, _)| *carrier == result.place)
        || live
            .references
            .values()
            .any(|leaf| leaf.identity.0 == result.place)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    if !reference_source_type(function, live, types, source).is_some_and(|actual| {
        terminal_semantics::reference_source_compatible(
            types.values().copied(),
            actual,
            result.structural_type,
        )
    }) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    // An owned ingress record containing exactly this loan moves that loan
    // into the new descriptor. Other establishments reborrow and retain the
    // parent. Physical pointer formation is identical; custody is not.
    let transferred = transferred_ingress_leaf(function, live, types, source);
    let custody = if let Some(key) = transferred {
        live.references
            .remove(&key)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
    } else {
        let (root, parent) = normalized_source(function, live, types, source)?;
        ReferenceCustody {
            identity: (result.place, Vec::new()),
            root,
            parent,
            operation: Some(*psi_operation),
            result: result.clone(),
            referent,
        }
    };
    live.references.insert((result.place, Vec::new()), custody);
    // A reference owns descriptor storage just like every other structural
    // result. Its separate custody row governs the loan, not home discovery.
    let result_home = super::aggregate_results::home(*psi_operation, result, types)?;
    live.structural_homes
        .insert(result.place, result_home.clone());
    operations.push(TargetUnitOperation::EstablishReference {
        psi_operation: *psi_operation,
        result_home,
        source: source.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

/// Retain the exact single-leaf ingress move already checked at the portable
/// boundary. A local or reference-typed carrier still establishes a child loan.
fn transferred_ingress_leaf(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> Option<(PlaceId, Vec<StructuralPathSegment>)> {
    let parameter = function.structural_parameters.iter().find(|parameter| {
        parameter.place == source.place && parameter.access == StructuralAccess::Owned
    })?;
    if matches!(
        types.get(&parameter.structural_type)?.shape,
        StructuralTypeShape::Reference { .. }
    ) {
        return None;
    }
    let (StructuralPathSegment::Referent, path) = source.path.split_last()? else {
        return None;
    };
    let paths = leaf_paths(types, parameter.structural_type, live.references.len())?;
    if paths.as_slice() != [path.to_vec()] {
        return None;
    }
    let key = (source.place, path.to_vec());
    let leaf = live.references.get(&key)?;
    (leaf.parent.is_none()
        && live
            .references
            .keys()
            .filter(|(carrier, _)| *carrier == source.place)
            .count()
            == 1
        && !live
            .references
            .values()
            .any(|child| child.parent.as_ref() == Some(&leaf.identity)))
    .then_some(key)
}

/// Close the loan on the carrier at `source`; referent storage is unaffected.
pub(super) fn release(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::ReleaseReference {
        psi_operation,
        source,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let leaf = live
        .references
        .remove(&(*source, Vec::new()))
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    if live
        .references
        .values()
        .any(|child| child.parent.as_ref() == Some(&leaf.identity))
    {
        live.references.insert(leaf.identity.clone(), leaf);
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    operations.push(TargetUnitOperation::ReleaseReference {
        psi_operation: *psi_operation,
        source: *source,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

/// Whole-owner disposal visits reference leaves in reverse declaration order.
/// The entire schedule is checked before any loan ends; a live external child
/// (or an incorrectly ordered contained child) keeps its parent suspended.
pub(super) fn discard_owned(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    source: PlaceId,
) -> Result<bool, LoweringError> {
    let Some((contract, _)) = source_contract(function, live, source) else {
        return Ok(false);
    };
    if !contains_reference(types, contract) {
        return Ok(false);
    }
    if live.drained_loan_roots.contains(&source) {
        // A sibling edge already released this root's leaves: every edge
        // cleanup roster declares the same places.
        return Ok(true);
    }
    let paths = leaf_paths(types, contract, live.references.len())
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    let mut released = BTreeSet::new();
    for path in paths.iter().rev() {
        let leaf = live
            .references
            .get(&(source, path.clone()))
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        if live.references.values().any(|child| {
            child.parent.as_ref() == Some(&leaf.identity) && !released.contains(&child.identity)
        }) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        released.insert(leaf.identity.clone());
    }
    if live
        .references
        .iter()
        .any(|((carrier, _), leaf)| *carrier == source && !released.contains(&leaf.identity))
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    live.references
        .retain(|_, leaf| !released.contains(&leaf.identity));
    live.drained_loan_roots.insert(source);
    Ok(true)
}

/// Record construction moves carriers, not referents. A `Structural` field
/// argument whose declared type is reference-bearing relocates every live leaf
/// of the argument carrier under the result's field path; the loan identity,
/// root and parent are unchanged.
pub(super) fn record_field_leaves(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &LiveDefinitions,
    destination: StructuralPathSegment,
    nested: StructuralTypeId,
    argument: &StructuralArgument,
    relocations: &mut Vec<(
        (PlaceId, Vec<StructuralPathSegment>),
        Vec<StructuralPathSegment>,
    )>,
    moved: &mut BTreeSet<(PlaceId, Vec<StructuralPathSegment>)>,
) -> Result<(), LoweringError> {
    let (contract, _) = source_contract(function, live, argument.place)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    if contract != nested {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let paths = leaf_paths(types, contract, live.references.len())
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    for path in &paths {
        let leaf = live
            .references
            .get(&(argument.place, path.clone()))
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        if !moved.insert(leaf.identity.clone()) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let mut destination = vec![destination.clone()];
        destination.extend(path.iter().cloned());
        relocations.push(((argument.place, path.clone()), destination));
    }
    // Every leaf the argument carrier owns must relocate; a differing roster
    // cannot describe the operand's declared type.
    if live
        .references
        .keys()
        .any(|(carrier, path)| *carrier == argument.place && !paths.contains(path))
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    Ok(())
}

/// Apply the staged record relocations once every field checks out.
pub(super) fn relocate_record_leaves(
    live: &mut LiveDefinitions,
    result_place: PlaceId,
    relocations: Vec<(
        (PlaceId, Vec<StructuralPathSegment>),
        Vec<StructuralPathSegment>,
    )>,
) {
    for (from, to) in relocations {
        if let Some(leaf) = live.references.remove(&from) {
            live.references.insert((result_place, to), leaf);
        }
    }
}

/// Establish the declared result reference roster of a structural-result
/// `CallStructural` whose result carries reference custody, resolved against
/// the already-committed `moved` argument leaves. Returns the ordered resolved
/// leaf roster retained on the target row.
pub(super) fn call_results(
    function: &AbstractFunction,
    callee_function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    result: &StructuralOperationResult,
    psi_operation: OperationId,
    structural_arguments: &[StructuralArgument],
    live: &mut LiveDefinitions,
    moved: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceCustody>,
) -> Result<Vec<target_operations::TargetReferenceResult>, LoweringError> {
    if !contains_reference(types, result.structural_type) {
        return Ok(Vec::new());
    }
    let callee_result = callee_function
        .result
        .structural()
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    if live
        .references
        .keys()
        .any(|(carrier, _)| *carrier == result.place)
        || live
            .references
            .values()
            .any(|leaf| leaf.identity.0 == result.place)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    // Stage the complete result before publication. Independent leaves cannot
    // duplicate an exclusive origin, even through distinct actual argument
    // spellings. Borrowing a result does not move its parent loan.
    let mut established = Vec::new();
    let mut roots = BTreeSet::new();
    let mut reference_results = Vec::new();
    for mapping in &callee_result.reference_sources {
        if mapping.sources.is_empty() || mapping.sources.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let expected = leaf_referent(types, result.structural_type, &mapping.path)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        if mapping.sources.len() > 1
            && (!mapping.path.is_empty()
                || !matches!(
                    types.get(&expected).map(|declaration| &declaration.shape),
                    Some(StructuralTypeShape::Record { .. })
                )
                || mapping.sources.iter().any(|formal| {
                    formal.place != mapping.sources[0].place
                        || formal.access != StructuralAccess::MutableBorrow
                        || formal.path.is_empty()
                        || formal.path.iter().any(|segment| {
                            !matches!(
                                segment,
                                StructuralPathSegment::Field(_)
                                    | StructuralPathSegment::FixedIndex(_)
                            )
                        })
                }))
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let mut candidates = Vec::with_capacity(mapping.sources.len());
        for formal in &mapping.sources {
            let source = call_source(callee_function, structural_arguments, formal)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            if reference_source_type(function, live, types, &source) != Some(expected) {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let leaf = if matches!(
                formal_origin(callee_function, types, formal),
                Some(ReferenceRoot::Ingress(place, path))
                    if callee_function
                        .structural_parameters
                        .iter()
                        .find(|parameter| parameter.place == place)
                        .and_then(|parameter| {
                            leaf_type(types, parameter.structural_type, &path)
                        })
                        .and_then(|leaf| types.get(&leaf))
                        .is_some_and(|declaration| {
                            matches!(
                                declaration.shape,
                                StructuralTypeShape::Reference { .. }
                            )
                        })
            ) {
                // An owned-carrier ingress mapping relocates one of the leaves the
                // moved arguments just transferred; the leaf keeps its identity,
                // root and parent, and lands at the declared result path.
                let Some((StructuralPathSegment::Referent, carrier_path)) =
                    source.path.split_last()
                else {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                };
                moved
                    .get(&(source.place, carrier_path.to_vec()))
                    .cloned()
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
            } else {
                let (root, parent) = normalized_source(function, live, types, &source)?;
                ReferenceCustody {
                    identity: (result.place, mapping.path.clone()),
                    root,
                    parent,
                    operation: Some(psi_operation),
                    result: result.clone(),
                    referent: expected,
                }
            };
            candidates.push(leaf);
        }
        let leaf = combine_alternatives(function, candidates)?;
        if roots.iter().any(|root| roots_overlap(root, &leaf.root)) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        roots.insert(leaf.root.clone());
        reference_results.push(target_operations::TargetReferenceResult {
            path: mapping.path.clone(),
            root: leaf.root.place(),
            root_paths: leaf.root.paths(),
        });
        established.push(((result.place, mapping.path.clone()), leaf));
    }
    for (key, leaf) in established {
        live.references.insert(key, leaf);
    }
    Ok(reference_results)
}

/// Alternatives describe one newly returned permission, never a union of
/// independently owned leaves or a way to detach a child from its parent.
fn combine_alternatives(
    function: &AbstractFunction,
    candidates: Vec<ReferenceCustody>,
) -> Result<ReferenceCustody, LoweringError> {
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let mut candidates = candidates.into_iter();
    let mut leaf = candidates.next().ok_or_else(invalid)?;
    let mut paths = Vec::new();
    for candidate in candidates {
        if leaf.identity != candidate.identity
            || leaf.result != candidate.result
            || leaf.operation != candidate.operation
            || leaf.referent != candidate.referent
            || leaf.parent.is_some()
            || candidate.parent.is_some()
            || leaf.root.place() != candidate.root.place()
        {
            return Err(invalid());
        }
        let ReferenceRoot::Ingress(_, path) = candidate.root else {
            return Err(invalid());
        };
        paths.push(path);
    }
    if !paths.is_empty() {
        let ReferenceRoot::Ingress(place, path) = &leaf.root else {
            return Err(invalid());
        };
        paths.push(path.clone());
        paths.sort();
        if paths.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid());
        }
        leaf.root = ReferenceRoot::Alternatives(*place, paths);
    }
    Ok(leaf)
}

/// A structural return whose source carries reference leaves must match the
/// declared result roster exactly: every returned leaf still sits at its
/// declared carrier path and descends directly from a checked formal origin.
/// The referent itself is not storage the return transports.
pub(super) fn return_custody(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: PlaceId,
    result: &terminal_psi::StructuralResultDeclaration,
) -> Result<(), LoweringError> {
    if !contains_reference(types, result.structural_type) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let (contract, _) = source_contract(function, live, source)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    if contract != result.structural_type
        || live
            .references
            .keys()
            .filter(|(carrier, _)| *carrier == source)
            .count()
            != result.reference_sources.len()
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    for mapping in &result.reference_sources {
        let leaf = live
            .references
            .get(&(source, mapping.path.clone()))
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        let declared = |root: &ReferenceRoot| {
            mapping
                .sources
                .iter()
                .any(|formal| formal_origin(function, types, formal).as_ref() == Some(root))
        };
        let admitted = match &leaf.root {
            ReferenceRoot::Alternatives(place, paths) => paths
                .iter()
                .all(|path| declared(&ReferenceRoot::Ingress(*place, path.clone()))),
            root => declared(root),
        };
        if !admitted
            || leaf.parent.is_some()
            || live
                .references
                .values()
                .any(|child| child.parent.as_ref() == Some(&leaf.identity))
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
    }
    Ok(())
}

/// Every owned-carrier ingress parameter is a constructible record: scalar
/// leaves and bare primitive-reference leaves compose the complete type.
fn constructible_record(types: &StructuralTypeLookup<'_>, root: StructuralTypeId) -> bool {
    let mut pending = vec![(root, false)];
    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    while let Some((current, exiting)) = pending.pop() {
        if exiting {
            active.remove(&current);
            complete.insert(current);
            continue;
        }
        if complete.contains(&current) {
            continue;
        }
        if !active.insert(current) {
            return false;
        }
        if referent(types, current).is_some() || plain_record_referent(types, current).is_some() {
            active.remove(&current);
            complete.insert(current);
            continue;
        }
        match types
            .get(&current)
            .copied()
            .map(|declaration| &declaration.shape)
        {
            Some(StructuralTypeShape::FixedArray { element, .. }) => {
                pending.push((current, true));
                pending.push((*element, false));
                continue;
            }
            Some(StructuralTypeShape::PrimitiveScalar(_)) => {
                active.remove(&current);
                complete.insert(current);
                continue;
            }
            _ => {}
        }
        let Some(terminal_psi::StructuralTypeDeclaration {
            shape: StructuralTypeShape::Record { fields },
            ..
        }) = types.get(&current).copied()
        else {
            return false;
        };
        pending.push((current, true));
        for field in fields {
            if field.relevance.is_erased() {
                return false;
            }
            match field.field_type {
                StructuralFieldType::Scalar(_)
                | StructuralFieldType::Atomic(_)
                | StructuralFieldType::IeeeFloat(_)
                | StructuralFieldType::RefinedScalar(_)
                | StructuralFieldType::BoundedInteger(_) => {}
                StructuralFieldType::Structural(child)
                    if referent(types, child).is_some()
                        || plain_record_referent(types, child).is_some() => {}
                StructuralFieldType::Structural(child) => pending.push((child, false)),
                _ => return false,
            }
        }
    }
    true
}

/// Seed one ingress leaf per reference leaf of each reference-bearing entry
/// parameter. Owned ingress assumes one existing permission per leaf.
pub(super) fn entry(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
) -> Result<BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceCustody>, LoweringError> {
    let mut references = BTreeMap::new();
    for parameter in &function.structural_parameters {
        if !contains_reference(types, parameter.structural_type) {
            continue;
        }
        // Borrowing lends the carrier roster without transferring its permissions.
        if parameter.multiplicity
            != if parameter.access == StructuralAccess::Owned {
                StructuralMultiplicity::Affine
            } else {
                StructuralMultiplicity::Unrestricted
            }
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || !constructible_record(types, parameter.structural_type)
            || function
                .entry_claims
                .iter()
                .any(|claim| claim.input == parameter.place)
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let paths = leaf_paths(
            types,
            parameter.structural_type,
            4096usize.saturating_sub(references.len()),
        )
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        for path in paths {
            let identity = (parameter.place, path.clone());
            references.insert(
                identity.clone(),
                ReferenceCustody {
                    identity,
                    root: ReferenceRoot::Ingress(parameter.place, path.clone()),
                    parent: None,
                    operation: None,
                    result: StructuralOperationResult {
                        qualification_establishments: Vec::new(),
                        place: parameter.place,
                        structural_type: parameter.structural_type,
                        multiplicity: parameter.multiplicity,
                        qualifications: parameter.qualifications.clone(),
                        projected_qualifications: parameter.projected_qualifications.clone(),
                        claims: Vec::new(),
                    },
                    referent: leaf_referent(types, parameter.structural_type, &path)
                        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?,
                },
            );
        }
    }
    Ok(references)
}
