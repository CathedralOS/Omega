//! Reference-carrier custody through control-flow lowering. A carrier owns
//! loan permission, never its referent's storage; every origin and parent is
//! reconstructed from operations and signatures, never from pointer bits.
//! A selected descriptor is one permission with a finite backing roster and
//! all possible immediate parents. A further reborrow names that descriptor,
//! not its grandparents; movement preserves the roster, and release waits for
//! every child. Choosing one parent here would detach the unselected lineage.
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::call_source;
use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::{OperationId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{
    TargetStructuralArgument, TargetStructuralParameter, TargetUnitOperation, TerminalPsiProvenance,
};
use terminal_psi::{
    StructuralAccess, StructuralFieldType, StructuralMultiplicity, StructuralPathSegment,
    StructuralTypeShape, live_field_by_id,
};
use terminal_psi::{StructuralArgument, StructuralOperationResult};

/// Exact incoming addresses for borrowed state parameters. The module query
/// runs once, then each lowering frontier shares its immutable function roster.
pub(in crate::lowering) type BorrowedSources =
    std::sync::Arc<BTreeMap<PlaceId, Option<Vec<StructuralArgument>>>>;

pub(in crate::lowering) fn borrowed_source_catalog(
    functions: &BTreeMap<semantic_vocabulary::MachineId, &AbstractFunction>,
    boundary_machines: &[terminal_psi::BoundaryMachineDeclaration],
    types: &StructuralTypeLookup<'_>,
) -> BTreeMap<semantic_vocabulary::MachineId, BorrowedSources> {
    use terminal_semantics::descriptor_lifetimes::{DescriptorLocation, descriptor_path};
    use terminal_semantics::descriptor_results::DescriptorAnalysis;
    let mut rosters = BTreeMap::new();
    for function in functions.values() {
        let mut sources = BTreeMap::new();
        for parameter in function.block_structural_parameters() {
            if parameter.access != StructuralAccess::Owned
                && !terminal_semantics::descriptor_results::is_shared_descriptor(
                    types.values().copied(),
                    parameter.structural_type,
                )
            {
                sources.insert(parameter.place, None);
            }
        }
        if !sources.is_empty() {
            rosters.insert(function.machine, sources);
        }
    }
    if rosters.is_empty() {
        return BTreeMap::new();
    }
    if let Some(graph) = optimization_unit_semantics::descriptor_results::abstract_catalog_graph(
        functions.values().copied(),
        boundary_machines,
        types.catalog(),
    ) {
        let mut analysis = DescriptorAnalysis::new(types.catalog(), &graph);
        for function in functions.values() {
            let Some(sources) = rosters.get_mut(&function.machine) else {
                continue;
            };
            let Some(current) = terminal_semantics::descriptor_results::descriptor_machine(
                &graph,
                function.machine,
            ) else {
                continue;
            };
            for entry in &function.block_entries {
                for parameter in &entry.structural_parameters {
                    let Some(slot) = sources.get_mut(&parameter.place) else {
                        continue;
                    };
                    *slot = analysis
                        .storage_origins(
                            function.machine,
                            (entry.block, 0),
                            &DescriptorLocation::whole(parameter.place),
                        )
                        .and_then(|origins| {
                            origins
                                .into_iter()
                                .map(|origin| {
                                    Some(StructuralArgument {
                                        place: origin.root,
                                        path: descriptor_path(
                                            types.catalog(),
                                            *current.place_types.get(&origin.root)?,
                                            &origin.path,
                                        )?,
                                        access: parameter.access,
                                    })
                                })
                                .collect()
                        });
                }
            }
        }
    }
    rosters
        .into_iter()
        .map(|(machine, sources)| (machine, std::sync::Arc::new(sources)))
        .collect()
}

/// One outstanding loan tracked during lowering: which carrier leaf holds it,
/// which referent root it suspends, which parent leaf it descends from, and
/// which result row formed its identity. `root` is compile-time custody — it
/// never names a pointer or descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReferenceCustody {
    /// Stable identity: the place and path where the carrier leaf was formed.
    /// Owned moves relocate the carrier, never this identity.
    pub(super) identity: (PlaceId, Vec<StructuralPathSegment>),
    /// Referent root resolved through custody: a primitive place directly, or
    /// an owned ingress parameter's leaf path when custody entered owned.
    pub(super) root: ReferenceRoot,
    /// All possible immediate parent identities; selection suspends each parent.
    pub(super) parents: Vec<(PlaceId, Vec<StructuralPathSegment>)>,
    /// The actual current carrier declaration, not a selected producer row.
    pub(super) contract: ReferenceCarrierContract,
    /// Every admitted formation that can arrive at this occurrence.
    pub(super) formations: Vec<ReferenceFormation>,
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
    Alternatives(Vec<target_operations::TargetReferenceOrigin>),
}

impl ReferenceRoot {
    pub(super) fn origins(&self) -> Vec<target_operations::TargetReferenceOrigin> {
        match self {
            Self::Place(root) => vec![target_operations::TargetReferenceOrigin {
                root: *root,
                path: Vec::new(),
            }],
            Self::Ingress(root, path) => vec![target_operations::TargetReferenceOrigin {
                root: *root,
                path: path.clone(),
            }],
            Self::Alternatives(origins) => origins.clone(),
        }
    }

    pub(super) fn contains_place(&self, place: PlaceId) -> bool {
        match self {
            Self::Place(root) | Self::Ingress(root, _) => *root == place,
            Self::Alternatives(origins) => origins.iter().any(|origin| origin.root == place),
        }
    }
}

fn roots_overlap(left: &ReferenceRoot, right: &ReferenceRoot) -> bool {
    let left = left.origins();
    let right = right.origins();
    left.iter().any(|left| {
        right.iter().any(|right| {
            left.root == right.root
                && terminal_semantics::structural_paths_may_overlap(&left.path, &right.path)
        })
    })
}

/// A live reference's payload producer and geometry after dereferencing it.
/// Custody retains every possible origin; physical addressing loads the one
/// address stored in the carrier, never a selected entry from that origin set.
pub(in crate::lowering) struct WritableReferentDestination {
    pub(in crate::lowering) carrier_type: StructuralTypeId,
    pub(in crate::lowering) referent: StructuralTypeId,
    /// Relative to the referent, excluding the explicit crossing.
    pub(in crate::lowering) path: Vec<StructuralPathSegment>,
}

fn materialized_mutable_record_projection(
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    carrier: PlaceId,
    path: &[StructuralPathSegment],
) -> Option<WritableReferentDestination> {
    let (StructuralPathSegment::Referent, tail) = path.split_first()? else {
        return None;
    };
    let leaf = live.references.get(&(carrier, Vec::new()))?;
    let home = live.structural_homes.get(&carrier)?;
    let declaration = types.get(&home.structural_type())?;
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
    if *referent != leaf.referent
        || live
            .references
            .values()
            .any(|child| child.parents.contains(&leaf.identity))
    {
        return None;
    }
    // Formation authenticated backing and view interpretation. A store uses
    // this live carrier's authority; backing need not have its semantic type.
    if home.access() != StructuralAccess::Owned
        || home.multiplicity() != StructuralMultiplicity::Affine
        || !home.qualifications().is_empty()
        || !home.projected_qualifications().is_empty()
        || home.operation_result().is_some_and(|(_, result)| {
            !result.qualification_establishments.is_empty() || !result.claims.is_empty()
        })
    {
        return None;
    }
    Some(WritableReferentDestination {
        carrier_type: home.structural_type(),
        referent: *referent,
        path: tail.to_vec(),
    })
}

pub(super) fn writable_referent_destination(
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    carrier: PlaceId,
    path: &[StructuralPathSegment],
) -> Option<WritableReferentDestination> {
    materialized_mutable_record_projection(live, types, carrier, path)
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
    let crossing = argument
        .path
        .iter()
        .position(|segment| *segment == StructuralPathSegment::Referent)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
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
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    if argument.access == StructuralAccess::Owned
        || argument.access != declaration.access
        || declaration.multiplicity != StructuralMultiplicity::Unrestricted
        || !declaration.qualifications.is_empty()
        || !declaration.projected_qualifications.is_empty()
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let leaf = live
        .references
        .get(&(argument.place, prefix.to_vec()))
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    if live
        .references
        .values()
        .any(|child| child.parents.contains(&leaf.identity))
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
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
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let source = home.argument_source();
        (home.structural_type(), source)
    } else {
        let parameter = prepared
            .parameter(argument.place)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        if parameter.access == StructuralAccess::WriteOnlyBorrow {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        (
            parameter.structural_type,
            parameter.placement.clone().into(),
        )
    };
    let reference_type = leaf_type(types, root_type, prefix)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let StructuralTypeShape::Reference { referent, .. } = types
        .get(&reference_type)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?
        .shape
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    if referent != leaf.referent {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let (selected, shape, offset) =
        crate::lowering::structural_layout::reference_referent_projection(
            root_type,
            &argument.path,
            argument.access,
            types,
        )?;
    if selected != declaration.structural_type
        || crate::lowering::structural_layout::internal_parameter_shape(
            selected,
            shape,
            declaration.access,
            types,
        ) != destination.shape
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
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
    terminal_psi::structural_type_contains_reference_by(|id| types.get(&id).copied(), root)
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
            StructuralTypeShape::Record { fields, .. } => {
                for field in fields.iter().rev() {
                    if let Some(child) = field.field_type.as_structural() {
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
    let mut case_fields = None;
    for segment in fields {
        if case_fields.is_some() && !matches!(segment, StructuralPathSegment::Field(_)) {
            return None;
        }
        if let StructuralPathSegment::Case(case) = segment {
            let declaration = types.get(&current)?;
            let cases = declaration.shape.sum_cases()?;
            // Case selects a namespace, not a structural child. Disposal of a
            // selected residual must resolve the payload's declaration before
            // asking whether it carries descriptors; shape alone grants no
            // active-case or ownership permission.
            case_fields = Some(
                cases
                    .iter()
                    .find(|candidate| candidate.id == *case)?
                    .fields
                    .as_slice(),
            );
            continue;
        }
        if matches!(
            segment,
            StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. }
        ) && matches!(
            types.get(&current)?.shape,
            StructuralTypeShape::ByteSequence(_)
        ) {
            current = terminal_semantics::primitive_projection_declaration(
                types.values().copied(),
                current,
                std::slice::from_ref(segment),
            )?;
            continue;
        }
        if matches!(segment, StructuralPathSegment::RuntimeIndex { .. }) {
            current = match &types.get(&current)?.shape {
                StructuralTypeShape::FixedArray { element, .. }
                | StructuralTypeShape::ElementView { element } => *element,
                _ => return None,
            };
            continue;
        }
        if *segment == StructuralPathSegment::Referent {
            let StructuralTypeShape::Reference { referent, .. } = &types.get(&current)?.shape
            else {
                return None;
            };
            current = *referent;
            continue;
        }
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
        let fields = match case_fields.take() {
            Some(fields) => fields,
            None => declaration.shape.direct_fields()?,
        };
        let field = terminal_psi::live_field_by_identity(fields, identity)?;
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
    case_fields.is_none().then_some(current)
}

/// The structural contract a place currently carries: a signature parameter, a
/// block-entry parameter, an ordinary home, a live established view, or a live
/// bare carrier result. A shared view has no home of its own; its descriptor
/// still names the element or byte geometry a loan through it selects.
fn source_contract(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    if let Some(parameter) = function.arrival_parameter(place) {
        return Some(parameter.structural_type);
    }
    if let Some(home) = live.structural_homes.get(&place) {
        return Some(home.structural_type());
    }
    if let Some((_, view)) = live.views.get(&place) {
        return Some(*view);
    }
    live.references
        .get(&(place, Vec::new()))
        .map(|leaf| leaf.contract.structural_type())
}

/// Borrow storage through its published contract, regardless of whether it
/// arrived as a parameter or was constructed in this activation. Lending
/// storage does not move it or discharge the owner's claims: multiplicity
/// constrains disposal, not this type lookup. Qualifications and access still
/// constrain formation; normalized_source retains exact live-loan custody.
/// A reference crossing must resolve live carrier custody instead.
fn direct_storage_type(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> Option<StructuralTypeId> {
    if !source.path.iter().all(|segment| {
        matches!(
            segment,
            StructuralPathSegment::Field(_)
                | StructuralPathSegment::FixedIndex(_)
                | StructuralPathSegment::RuntimeIndex { .. }
        )
    }) {
        return None;
    }
    let (structural_type, access, unqualified) = if let Some(parameter) =
        function.arrival_parameter(source.place)
    {
        (
            parameter.structural_type,
            parameter.access,
            parameter.qualifications.is_empty() && parameter.projected_qualifications.is_empty(),
        )
    } else if let Some(home) = live.structural_homes.get(&source.place) {
        (
            home.structural_type(),
            home.access(),
            home.qualifications().is_empty()
                && home.projected_qualifications().is_empty()
                && home
                    .operation_result()
                    .is_none_or(|(_, result)| result.qualification_establishments.is_empty()),
        )
    } else {
        // A shared view keeps no home, but its producer published the same
        // operation-result contract a home retains, with the same authority.
        let result = live.view_result(function, source.place)?;
        (
            result.structural_type,
            StructuralAccess::Owned,
            result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.qualification_establishments.is_empty(),
        )
    };
    if !source.access.admits(access) || !unqualified {
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
    if source.path.contains(&StructuralPathSegment::Referent) {
        let mut current = source_contract(function, live, source.place)?;
        let mut start = 0;
        for (position, segment) in source.path.iter().enumerate() {
            if *segment != StructuralPathSegment::Referent {
                continue;
            }
            current = leaf_type(types, current, &source.path[start..position])?;
            let StructuralTypeShape::Reference { referent, access } = &types.get(&current)?.shape
            else {
                // Preserve the existing terminal spelling for a direct leaf.
                return (position + 1 == source.path.len()).then_some(current);
            };
            let compatible = source.access.admits_referent(*access);
            if !compatible {
                return None;
            }
            current = *referent;
            start = position + 1;
        }
        return leaf_type(types, current, &source.path[start..]);
    }
    direct_storage_type(function, live, types, source)
}

/// An edge loan follows the same authenticated carrier dependencies as a call.
pub(super) fn reference_loan_source_live(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> bool {
    access_paths::call_reference_custody(function, live, types, source).is_ok()
}

/// A case observation reads through the same live descriptor chain as other
/// nonescaping accesses, without imposing a call argument's endpoint grammar.
pub(super) fn case_reference_source_live(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    place: PlaceId,
    path: &[StructuralPathSegment],
) -> bool {
    access_paths::access_reference_custody(
        function,
        live,
        types,
        &StructuralArgument {
            place,
            path: path.to_vec(),
            access: StructuralAccess::SharedBorrow,
        },
    )
    .is_ok()
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

/// Rebind ordinary borrowed storage without granting a new reference. Its
/// operation owns bounds, case and access checks; formation and result mapping
/// separately retain reference_source_type's stricter eligibility rules.
fn storage_projection(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> Option<optimization_unit_semantics::StructuralStorageProjection> {
    let carrier = source_contract(function, live, source.place)?;
    optimization_unit_semantics::structural_borrowed_storage_projection(
        types.values().copied(),
        carrier,
        &source.path,
        source.access,
    )
}

/// A `.., Referent` source names a suspended child of the located carrier
/// leaf; an empty path names the whole ingress referent directly. Every
/// selected payload and runtime index retains its exact storage coordinate.
fn normalized_source(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> Result<(ReferenceRoot, Vec<(PlaceId, Vec<StructuralPathSegment>)>), LoweringError> {
    let Some(origins) = live.borrowed_reference_sources.get(&source.place) else {
        return normalized_direct_source(function, live, types, source);
    };
    let origins = origins
        .as_ref()
        .filter(|origins| !origins.is_empty())
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let expected = storage_projection(function, live, types, source)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let mut candidates = Vec::with_capacity(origins.len());
    for origin in origins {
        let mut resolved = origin.clone();
        resolved.path.extend_from_slice(&source.path);
        resolved.access = source.access;
        // Rebinding may change the carrier's presentation, never a window's
        // original backing or exact interval. Keep every possible origin.
        if !storage_projection(function, live, types, &resolved)
            .is_some_and(|actual| actual.matches(types.values().copied(), expected))
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        candidates.push(normalized_direct_source(function, live, types, &resolved)?);
    }
    combine_candidate_origins(function, candidates)
}

fn normalized_direct_source(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> Result<(ReferenceRoot, Vec<(PlaceId, Vec<StructuralPathSegment>)>), LoweringError> {
    if let Some(crossing) = source
        .path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)
    {
        let carrier_path = &source.path[..crossing];
        let suffix = &source.path[crossing + 1..];
        let Some(leaf) = live.references.get(&(source.place, carrier_path.to_vec())) else {
            // The member leaf of a carrier never held as a live reference
            // borrows that carrier's own storage; only a non-reference member
            // may root there — a missing reference carrier is a custody gap.
            let carrier = source_contract(function, live, source.place)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            return match leaf_type(types, carrier, carrier_path).and_then(|leaf| {
                types
                    .get(&leaf)
                    .and_then(|declaration| match declaration.shape {
                        StructuralTypeShape::Reference { .. } => None,
                        _ if suffix.is_empty() => Some(()),
                        _ => None,
                    })
            }) {
                Some(()) => {
                    let root = ReferenceRoot::Ingress(source.place, carrier_path.to_vec());
                    if live.references.iter().any(|((carrier, path), reference)| {
                        roots_overlap(&reference.root, &root)
                            && !shared_source_compatible(
                                function,
                                live,
                                types,
                                carrier,
                                path,
                                source.access,
                            )
                    }) {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    Ok((root, Vec::new()))
                }
                None => Err(LoweringError::unsupported_control_flow(function.machine)),
            };
        };
        // The runtime source stays the retained carrier. Custody is inherited
        // from that carrier, while typed descendants select its live backing;
        // an initializer or a guessed origin never supplies the address.
        if leaf_type(types, leaf.referent, suffix).is_none() {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        if live.references.iter().any(|((carrier, path), child)| {
            child.parents.contains(&leaf.identity)
                && !shared_source_compatible(function, live, types, carrier, path, source.access)
        }) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        Ok((leaf.root.clone(), vec![leaf.identity.clone()]))
    } else if source.path.is_empty() {
        if live.references.iter().any(|((carrier, path), reference)| {
            reference.root.contains_place(source.place)
                && !shared_source_compatible(function, live, types, carrier, path, source.access)
        }) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        Ok((ReferenceRoot::Place(source.place), Vec::new()))
    } else if storage_projection(function, live, types, source).is_some() {
        let root = ReferenceRoot::Ingress(source.place, source.path.clone());
        if live.references.iter().any(|((carrier, path), leaf)| {
            roots_overlap(&leaf.root, &root)
                && !shared_source_compatible(function, live, types, carrier, path, source.access)
        }) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        Ok((root, Vec::new()))
    } else {
        Err(LoweringError::unsupported_control_flow(function.machine))
    }
}

/// Substitute a callee's formal result-source mapping into the caller's actual
/// argument at the same structural-parameter position.

/// The formal origin a callee result mapping names: a borrowed primitive
/// parameter or an ingress leaf path under an owned or shared carrier.
fn formal_origin(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> Option<ReferenceRoot> {
    let parameter = function.structural_parameter(source.place)?;
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

mod access_paths;
mod copies;
pub(super) use copies::copied_leaves;
mod occurrences;
pub(super) use occurrences::{ReferenceCarrierContract, ReferenceFormation};
mod arrivals;
pub(super) use arrivals::merge_arrivals;

/// Whether `place` is currently suspended as a live loan's referent root.
/// Cleanup cannot discard storage that an outstanding carrier still names.
pub(super) fn is_suspended_root(live: &LiveDefinitions, place: PlaceId) -> bool {
    live.references
        .values()
        .any(|reference| reference.root.contains_place(place))
}

/// Test the selected storage, not an ingress descriptor's external pointee.
/// Ingress origins at reference leaves denote caller-backed referents; origins
/// at ordinary fields denote that field's storage. Missing declarations cannot
/// establish disjointness. Carrier relocation never changes these origins.
pub(super) fn is_suspended_storage_region(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    place: PlaceId,
    path: &[StructuralPathSegment],
) -> bool {
    let origin_overlaps = |origin_path: &[StructuralPathSegment]| {
        let Some(origin) = source_contract(function, live, place)
            .and_then(|root| leaf_type(types, root, origin_path))
            .and_then(|identity| types.get(&identity))
        else {
            return true;
        };
        !matches!(origin.shape, StructuralTypeShape::Reference { .. })
            && terminal_semantics::structural_paths_may_overlap(path, origin_path)
    };
    live.references
        .values()
        .any(|reference| match &reference.root {
            ReferenceRoot::Place(root) => *root == place,
            ReferenceRoot::Ingress(root, origin) => *root == place && origin_overlaps(origin),
            ReferenceRoot::Alternatives(origins) => origins
                .iter()
                .any(|origin| origin.root == place && origin_overlaps(&origin.path)),
        })
}

/// Moving a carrier preserves its loans, but cannot detach a suspended parent
/// from a child that remains outside the selected subtree.
pub(super) fn owned_transfer_is_suspended(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    argument: &StructuralArgument,
) -> bool {
    let selected = |carrier: PlaceId, path: &[StructuralPathSegment]| {
        carrier == argument.place && path.starts_with(&argument.path)
    };
    is_suspended_storage_region(function, live, types, argument.place, &argument.path)
        || live.references.iter().any(|((carrier, path), leaf)| {
            selected(*carrier, path)
                && live
                    .references
                    .iter()
                    .any(|((child_carrier, child_path), child)| {
                        child.parents.contains(&leaf.identity)
                            && !selected(*child_carrier, child_path)
                    })
        })
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
    check_region_access_with(live, place, path, |_, _| false)
}

fn check_region_access_with(
    live: &LiveDefinitions,
    place: PlaceId,
    path: &[StructuralPathSegment],
    compatible_origin: impl Fn(&PlaceId, &[StructuralPathSegment]) -> bool,
) -> bool {
    let overlaps = |other: &[StructuralPathSegment]| {
        terminal_semantics::structural_paths_may_overlap(path, other)
    };
    live.references
        .iter()
        .any(|((carrier, carrier_path), reference)| {
            (*carrier == place && overlaps(carrier_path))
                || (reference.identity.0 == place && overlaps(&reference.identity.1))
                || (reference.root.contains_place(place)
                    && !compatible_origin(carrier, carrier_path)
                    && match &reference.root {
                        ReferenceRoot::Place(_) => true,
                        ReferenceRoot::Ingress(_, path) => overlaps(path),
                        ReferenceRoot::Alternatives(origins) => origins
                            .iter()
                            .any(|origin| origin.root == place && overlaps(&origin.path)),
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
    let Some(root) = source_contract(function, live, place) else {
        return check_root_access(live, place);
    };
    let region =
        terminal_semantics::projected_storage_region(types.values().copied(), root, path, field);
    check_region_access(live, place, region.direct_path())
}

/// Nonescaping reads and sanctioned atomic events need shared access to their
/// exact field, not ordinary write access to the whole owner. Exclusive loans
/// still suspend overlapping owner access, and explicit reference crossings
/// must use the live carrier's permission rather than the owner's. The atomic
/// operation separately owns its cell type, ordering and mutation authority.
pub(super) fn check_projected_shared_access(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    place: PlaceId,
    path: &[StructuralPathSegment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
) -> bool {
    let Some(root) = source_contract(function, live, place) else {
        return check_root_access(live, place);
    };
    if path.contains(&StructuralPathSegment::Referent) {
        let mut projection = path.to_vec();
        if let Some(field) = field {
            let selected = leaf_type(types, root, path)
                .and_then(|parent| types.get(&parent))
                .and_then(|declaration| declaration.shape.record_fields())
                .and_then(|fields| live_field_by_id(fields, field));
            let Some(selected) = selected else {
                return true;
            };
            projection.push(StructuralPathSegment::Field(selected.identity.clone()));
        }
        return access_paths::access_reference_custody(
            function,
            live,
            types,
            &StructuralArgument {
                place,
                path: projection,
                access: StructuralAccess::SharedBorrow,
            },
        )
        .is_err();
    }
    let region =
        terminal_semantics::projected_storage_region(types.values().copied(), root, path, field);
    check_region_access_with(
        live,
        place,
        region.direct_path(),
        |carrier, carrier_path| {
            shared_source_compatible(
                function,
                live,
                types,
                carrier,
                carrier_path,
                StructuralAccess::SharedBorrow,
            )
        },
    )
}

pub(super) fn check_operation_access(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    operation: &AbstractOperation,
    place: PlaceId,
) -> bool {
    match operation {
        AbstractOperation::StructuralLeafCopy {
            psi_operation,
            result,
            source,
            path,
        } => {
            // Descriptor copies are observations of carrier storage, not owned
            // accesses to its suspended referent. The copy owner authenticates
            // every selected shared leaf before publishing fresh occurrences.
            if contains_reference(types, result.structural_type) {
                return copied_leaves(function, types, live, result, *source, path, *psi_operation)
                    .is_err();
            }
            match path {
                abstract_operations::StructuralCopyPath::Ordinary(path) => {
                    check_projected_shared_access(function, live, types, place, path, None)
                }
                abstract_operations::StructuralCopyPath::Case(_) => check_root_access(live, place),
            }
        }
        // Atomic mutation is sanctioned shared access to the original cell,
        // not replacement of its enclosing record or stored descriptors.
        AbstractOperation::AtomicEvent { event, .. } => event.location().is_some_and(|location| {
            check_projected_shared_access(
                function,
                live,
                types,
                location.root,
                &location.path,
                Some(location.field),
            )
        }),
        AbstractOperation::WriteOnlyPrimitiveStore { path, .. }
            if path.contains(&StructuralPathSegment::Referent) =>
        {
            access_paths::access_reference_custody(
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
            if path.contains(&StructuralPathSegment::Referent) =>
        {
            access_paths::access_reference_custody(
                function,
                live,
                types,
                &StructuralArgument {
                    place,
                    path: path.clone(),
                    access: StructuralAccess::SharedBorrow,
                },
            )
            .is_err()
        }
        // An admitted nonescaping observation produces a scalar, not another
        // loan. Psi owns its compatibility judgment; applying shared-loan
        // formation rules here would invent a stricter source requirement.
        // Storage owners still join the live root and exact physical path,
        // and explicit reference crossings retain the custody walk above.
        AbstractOperation::PrimitiveScalarRead { .. }
        | AbstractOperation::StructuralScalarFieldRead { .. }
        | AbstractOperation::StructuralCaseMembership { .. } => false,
        AbstractOperation::WriteOnlyPrimitiveStore { path, .. } => {
            check_projected_access(function, live, types, place, path, None)
        }
        AbstractOperation::StructuralScalarFieldStore { path, field, .. } => {
            check_projected_access(function, live, types, place, path, Some(*field))
        }
        AbstractOperation::MoveStructuralField { path, field, .. }
        | AbstractOperation::StoreStructuralField { path, field, .. } => {
            // A storage transfer carries its descriptor leaves with it. It
            // must not detach a suspended parent or touch borrowed backing,
            // but a live descriptor in a disjoint sibling is not a whole-root
            // loan. Use the same regional transfer judgment as owned operands.
            let Some(contract) = source_contract(function, live, place) else {
                return true;
            };
            let Some(projection) = terminal_semantics::structural_window_projection(
                types.values().copied(),
                contract,
                path,
                *field,
            ) else {
                return true;
            };
            owned_transfer_is_suspended(
                function,
                live,
                types,
                &StructuralArgument {
                    place,
                    path: projection.path,
                    access: StructuralAccess::Owned,
                },
            )
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
            let resolved = access_paths::call_reference_custody(function, live, types, argument)?;
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
            .is_some_and(|contract| contains_reference(types, contract))
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
            let contract = source_contract(function, live, argument.place)
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
                    .any(|child| child.parents.contains(&leaf.identity))
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
        // Direct storage arguments use the same exact projection as reference
        // formation. A loan of one field does not suspend its disjoint sibling;
        // collapsing both to the containing place loses that distinction and
        // also makes pairwise call-argument checks compare the wrong regions.
        let (root, _) = normalized_source(function, live, types, argument)?;
        normalized.push((
            argument_position,
            vec![root],
            argument.access,
            if live
                .borrowed_reference_sources
                .contains_key(&argument.place)
            {
                CallRegion::Referent
            } else {
                CallRegion::OrdinaryStorage
            },
        ));
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
        storage,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let referent = referent(types, result.structural_type)
        .or_else(|| plain_record_referent(types, result.structural_type))
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    if !result.is_affine_plain()
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
    let actual = reference_source_type(function, live, types, source);
    if !actual.is_some_and(|actual| match storage {
        Some(storage) => terminal_semantics::reference_storage_source_compatible(
            types.values().copied(),
            actual,
            result.structural_type,
            storage.stored_scalar,
        ),
        None => terminal_semantics::reference_source_compatible(
            types.values().copied(),
            actual,
            result.structural_type,
        ),
    }) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    // EstablishReference always borrows, including a projection through an
    // owned record. Moving a stored descriptor uses the checked owned-value
    // projection/transfer operation, not an ingress-shape reinterpretation.
    let (root, parents) = normalized_source(function, live, types, source)?;
    let custody = ReferenceCustody {
        identity: (result.place, Vec::new()),
        root,
        parents,
        contract: ReferenceCarrierContract::Operation {
            operation: *psi_operation,
            result: result.clone(),
        },
        formations: vec![ReferenceFormation {
            identity: (result.place, Vec::new()),
            contract: ReferenceCarrierContract::Operation {
                operation: *psi_operation,
                result: result.clone(),
            },
        }],
        referent,
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
        storage: *storage,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

/// Close the exact stored carrier; path and stable loan identity survive
/// relocation. The operation affects custody only, never referent storage.
pub(super) fn release(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::ReleaseReference {
        psi_operation,
        source,
        path,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    // Releasing stored pointer bytes cannot bypass a loan over their owner.
    if live.references.values().any(|enclosing| {
        enclosing.root.origins().iter().any(|origin| {
            let origin_path = &origin.path;
            origin.root == *source
                && source_contract(function, live, *source)
                    .and_then(|contract| leaf_type(types, contract, origin_path))
                    == Some(enclosing.referent)
                && terminal_semantics::structural_paths_may_overlap(origin_path, path)
        })
    }) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let key = (*source, path.clone());
    let leaf = live
        .references
        .remove(&key)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    if live
        .references
        .values()
        .any(|child| child.parents.contains(&leaf.identity))
    {
        live.references.insert(key, leaf);
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    operations.push(TargetUnitOperation::ReleaseReference {
        psi_operation: *psi_operation,
        source: *source,
        path: path.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

/// Whole cleanup is the empty projection of exact subtree disposal.
pub(super) fn discard_owned(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    source: PlaceId,
) -> Result<bool, LoweringError> {
    if live.drained_loan_roots.contains(&source) {
        return Ok(true);
    }
    let discarded = discard_subtree(function, types, live, source, &[])?;
    if discarded {
        live.drained_loan_roots.insert(source);
    }
    Ok(discarded)
}

/// Dispose only the live descriptor slots selected by an authenticated cleanup
/// subtree. Identity follows loan provenance; removal follows current storage.
pub(super) fn discard_subtree(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    source: PlaceId,
    prefix: &[StructuralPathSegment],
) -> Result<bool, LoweringError> {
    let Some(contract) = source_contract(function, live, source) else {
        return Ok(false);
    };
    let selected = leaf_type(types, contract, prefix)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    if !contains_reference(types, selected) {
        return Ok(false);
    }
    let paths = leaf_paths(types, selected, live.references.len())
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let mut released = BTreeSet::new();
    let mut released_paths = BTreeSet::new();
    for path in paths.iter().rev() {
        let mut selected_path = prefix.to_vec();
        selected_path.extend_from_slice(path);
        let leaf = live
            .references
            .get(&(source, selected_path.clone()))
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        if live.references.values().any(|child| {
            child.parents.contains(&leaf.identity) && !released.contains(&child.identity)
        }) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        released.insert(leaf.identity.clone());
        released_paths.insert(selected_path);
    }
    if live.references.keys().any(|(carrier, path)| {
        *carrier == source && path.starts_with(prefix) && !released_paths.contains(path)
    }) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    live.references
        .retain(|(carrier, path), _| *carrier != source || !released_paths.contains(path));
    Ok(true)
}

/// Structural storage transfer moves carriers, not referents. Stage the exact
/// selected roster under its destination prefix before changing either home.
/// Construction and window replacement preserve the same identity, backing and
/// parents; neither a byte copy nor a matching type can create missing custody.
pub(super) fn subtree_leaves(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &LiveDefinitions,
    destination: &[StructuralPathSegment],
    nested: StructuralTypeId,
    argument: &StructuralArgument,
    relocations: &mut Vec<(
        (PlaceId, Vec<StructuralPathSegment>),
        Vec<StructuralPathSegment>,
    )>,
    moved: &mut BTreeSet<(PlaceId, Vec<StructuralPathSegment>)>,
) -> Result<(), LoweringError> {
    if !contains_reference(types, nested) {
        return Ok(());
    }
    let contract = source_contract(function, live, argument.place)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    if leaf_type(types, contract, &argument.path) != Some(nested) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let paths = leaf_paths(types, nested, live.references.len())
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    for path in &paths {
        let mut source_path = argument.path.clone();
        source_path.extend_from_slice(path);
        let leaf = live
            .references
            .get(&(argument.place, source_path.clone()))
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        if !moved.insert(leaf.identity.clone()) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let mut destination = destination.to_vec();
        destination.extend(path.iter().cloned());
        relocations.push(((argument.place, source_path), destination));
    }
    // Only the selected subtree moves; disjoint siblings keep their custody.
    if live.references.keys().any(|(carrier, path)| {
        *carrier == argument.place
            && path
                .strip_prefix(argument.path.as_slice())
                .is_some_and(|relative| !paths.iter().any(|selected| selected == relative))
    }) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    Ok(())
}

/// Apply staged relocations only after the complete operation checks out.
pub(super) fn relocate_leaves(
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
    let mut roots = Vec::new();
    let mut identities = BTreeSet::new();
    let mut reference_results = Vec::new();
    for mapping in &callee_result.reference_sources {
        if mapping.sources.is_empty() || mapping.sources.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let expected = leaf_referent(types, result.structural_type, &mapping.path)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        let result_access = leaf_type(types, result.structural_type, &mapping.path)
            .and_then(|leaf| types.get(&leaf))
            .and_then(|declaration| match declaration.shape {
                StructuralTypeShape::Reference { access, .. } => Some(access),
                _ => None,
            })
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        let mut candidates = Vec::with_capacity(mapping.sources.len());
        for formal in &mapping.sources {
            if !access_covers(formal.access, result_access) {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let source = call_source(
                &callee_function.structural_parameters,
                structural_arguments,
                formal,
            )
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            if reference_source_type(function, live, types, &source) != Some(expected) {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let leaf = if matches!(
                formal_origin(callee_function, types, formal),
                Some(ReferenceRoot::Ingress(place, path))
                    if callee_function.structural_parameter(place)
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
                let (root, parents) = normalized_source(function, live, types, &source)?;
                ReferenceCustody {
                    identity: (result.place, mapping.path.clone()),
                    root,
                    parents,
                    contract: ReferenceCarrierContract::Operation {
                        operation: psi_operation,
                        result: result.clone(),
                    },
                    formations: vec![ReferenceFormation {
                        identity: (result.place, mapping.path.clone()),
                        contract: ReferenceCarrierContract::Operation {
                            operation: psi_operation,
                            result: result.clone(),
                        },
                    }],
                    referent: expected,
                }
            };
            candidates.push(leaf);
        }
        let joining = candidates.len() > 1;
        let mut leaf = combine_alternatives(function, candidates)?;
        if joining {
            // The selected descriptor is one result resource, conservatively
            // pinning the complete union of the consumed input permissions.
            leaf.identity = (result.place, mapping.path.clone());
            leaf.reform(psi_operation, result);
        }
        if !identities.insert(leaf.identity.clone())
            || roots.iter().any(|(root, prior_access)| {
                roots_overlap(root, &leaf.root)
                    && (result_access != StructuralAccess::SharedBorrow
                        || *prior_access != StructuralAccess::SharedBorrow)
            })
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        roots.push((leaf.root.clone(), result_access));
        reference_results.push(target_operations::TargetReferenceResult {
            path: mapping.path.clone(),
            origins: leaf.root.origins(),
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
    let mut candidates = candidates.into_iter();
    let mut leaf = candidates
        .next()
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let mut origins = vec![(leaf.root.clone(), leaf.parents.clone())];
    for candidate in candidates {
        if leaf.referent != candidate.referent {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        origins.push((candidate.root, candidate.parents));
    }
    (leaf.root, leaf.parents) = combine_candidate_origins(function, origins)?;
    Ok(leaf)
}

fn combine_candidate_origins(
    function: &AbstractFunction,
    candidates: Vec<(ReferenceRoot, Vec<(PlaceId, Vec<StructuralPathSegment>)>)>,
) -> Result<(ReferenceRoot, Vec<(PlaceId, Vec<StructuralPathSegment>)>), LoweringError> {
    let mut candidates = candidates.into_iter();
    let (root, parents) = candidates
        .next()
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    if candidates.len() == 0 {
        return Ok((root, parents));
    }
    let mut origins = Vec::new();
    let mut immediate_parents = Vec::new();
    for (root, parents) in std::iter::once((root, parents)).chain(candidates) {
        // Selection produces one permission, but every possible immediate
        // parent remains suspended until that permission and its children end.
        origins.extend(root.origins());
        immediate_parents.extend(parents);
    }
    origins.sort();
    origins.dedup();
    immediate_parents.sort();
    immediate_parents.dedup();
    Ok((ReferenceRoot::Alternatives(origins), immediate_parents))
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
    let contract = source_contract(function, live, source)
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
            ReferenceRoot::Alternatives(origins) => origins.iter().all(|origin| {
                let root = if origin.path.is_empty() {
                    ReferenceRoot::Place(origin.root)
                } else {
                    ReferenceRoot::Ingress(origin.root, origin.path.clone())
                };
                declared(&root)
            }),
            root => declared(root),
        };
        if !admitted
            || !leaf.parents.is_empty()
            || live
                .references
                .values()
                .any(|child| child.parents.contains(&leaf.identity))
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
            shape: StructuralTypeShape::Record { fields, .. },
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
                    parents: Vec::new(),
                    contract: ReferenceCarrierContract::Parameter(parameter.clone()),
                    formations: vec![ReferenceFormation {
                        identity: (parameter.place, path.clone()),
                        contract: ReferenceCarrierContract::Parameter(parameter.clone()),
                    }],
                    referent: leaf_referent(types, parameter.structural_type, &path)
                        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?,
                },
            );
        }
    }
    Ok(references)
}
