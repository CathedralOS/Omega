//! Independent reference-custody replay over the optimized source graph.
//!
//! `scalar_graph_input` re-derives every loan the target rows must name: which
//! carrier leaf holds it, which referent root it suspends, and which operation
//! formed it. A carrier owns loan permission, never referent storage; roots
//! are reconstructed from operations and signatures, never from pointer bits.
//! Production lowering transports storage metadata without this permission
//! model. This explicit audit independently rejoins `.., Referent` arguments
//! and declared result leaf rosters against the optimized source graph.
//! Selection retains one descriptor with every possible immediate parent.
//! Child access, transfer and disposal test membership in that complete roster;
//! roots describe possible storage, never a runtime address to choose here.
//! Shared descriptor copies retain that roster without consuming the source:
//! each new carrier has its own release identity but the same immediate parents.
//! Owned edges select a receiving parameter occurrence; reconverging stores
//! select the current occurrence of their independently established resident
//! slot. Admitted formations, roots and parents converge before entries return.
//! Ingress slots retain Case/Field guards, not simultaneous grants for inactive
//! alternatives. Construction and transport use resident descriptors. Source
//! dominance scopes storage at joins before comparing surviving custody; this
//! excludes unavailable branch-local homes, never a missing surviving loan.
//! Executed descriptor disposal removes residency but retains parent ancestry
//! while descendants survive. Formation identity distinguishes replacements
//! of the same slot; retired ancestry grants no readable storage or permission.
use super::{
    AbstractOperation, AbstractOperationPlan, PsiOptimizationFunction, PsiOptimizationUnit,
};
use crate::LegalizationError;
use abstract_operations::AbstractStructuralCallResult;
use abstract_operations::call_source;
use optimization_unit_semantics::CallRegion;
use semantic_vocabulary::{BlockId, EdgeId, OperationId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use terminal_psi::{
    ReferenceOriginProjection, StructuralAccess, StructuralArgument, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape, find_structural_type,
};

mod borrowed_arrivals;
mod descriptor_backing;
mod occurrences;
mod resident_joins;
mod retirements;
mod stored_calls;
use occurrences::{
    CarrierContract, Formation, OccurrenceIdentity, merge_arrivals, re_home_bound_leaves,
};

/// One outstanding permission occurrence: its referent roots, immediate
/// parents, actual receiving contract and every transported formation.
/// `root` is compile-time custody — it never names a pointer or descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::legalization) struct Leaf {
    /// Current permission occurrence. Owned edge selection rebinds it to the
    /// receiving parameter; formations retain every admitted source identity.
    pub(in crate::legalization) identity: OccurrenceIdentity,
    /// Referent root resolved through custody: a primitive place directly, or
    /// an owned ingress parameter's leaf path when custody entered owned.
    pub(in crate::legalization) root: Root,
    // Captured lifetime geometry is separate from executable origin evidence.
    backing: Vec<descriptor_backing::Backing>,
    /// All possible immediate parent identities; selection suspends each parent.
    pub(in crate::legalization) parents: Vec<OccurrenceIdentity>,
    /// The actual operation or parameter declaration of the current occurrence.
    contract: CarrierContract,
    /// All admitted formations transported to this occurrence.
    formations: Vec<Formation>,
    /// This leaf's primitive referent type.
    pub(in crate::legalization) referent: StructuralTypeId,
    placed: Option<target_operations::TargetPlacedInterpretation>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::legalization) enum Root {
    /// A mutable borrowed parameter or activation-local primitive place.
    Place(PlaceId),
    /// One leaf of an owned reference-bearing ingress parameter; the referent
    /// storage belongs to the caller, so no local pointer exists for it.
    Ingress(PlaceId, Vec<StructuralPathSegment>),
    /// One permission whose selected static path is carried by its pointer.
    Alternatives(Vec<target_operations::TargetReferenceOrigin>),
}

impl Root {
    /// Compose declaration-relative interpretation without changing the
    /// original storage place or any descriptor/parent residence identity.
    fn projected(&self, suffix: &[StructuralPathSegment]) -> Self {
        self.projected_family(
            &suffix
                .iter()
                .cloned()
                .map(terminal_psi::ReferenceOriginProjection::Exact)
                .collect::<Vec<_>>(),
            semantic_vocabulary::ReferenceOriginExtent::Projected,
        )
    }

    fn projected_family(
        &self,
        suffix: &[terminal_psi::ReferenceOriginProjection],
        extent: semantic_vocabulary::ReferenceOriginExtent,
    ) -> Self {
        if suffix.is_empty() && extent == semantic_vocabulary::ReferenceOriginExtent::Projected {
            return self.clone();
        }
        let mut origins = self.origins();
        for origin in &mut origins {
            if origin.extent == semantic_vocabulary::ReferenceOriginExtent::Projected {
                origin.path.extend_from_slice(suffix);
                origin.extent = extent;
            }
        }
        if origins.len() == 1
            && let Some(path) = origins[0].exact_path()
        {
            Self::Ingress(origins[0].root, path)
        } else {
            origins.sort();
            origins.dedup();
            Self::Alternatives(origins)
        }
    }
    pub(in crate::legalization) fn origins(&self) -> Vec<target_operations::TargetReferenceOrigin> {
        match self {
            Self::Place(root) => vec![target_operations::TargetReferenceOrigin {
                kind: target_operations::TargetReferenceOriginKind::Storage,
                root: *root,
                extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                path: Vec::new(),
            }],
            Self::Ingress(root, path) => vec![target_operations::TargetReferenceOrigin {
                kind: target_operations::TargetReferenceOriginKind::Storage,
                root: *root,
                extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                path: path
                    .iter()
                    .cloned()
                    .map(terminal_psi::ReferenceOriginProjection::Exact)
                    .collect(),
            }],
            Self::Alternatives(origins) => origins.clone(),
        }
    }

    pub(in crate::legalization) fn contains_place(&self, place: PlaceId) -> bool {
        match self {
            Self::Place(root) | Self::Ingress(root, _) => *root == place,
            Self::Alternatives(origins) => origins.iter().any(|origin| origin.root == place),
        }
    }
}

/// Turn captured origins into typed storage cursors. An ingress reference
/// names its incoming descriptor in custody; its pointee is external storage
/// reached by one explicit crossing. This does not materialize any descriptor
/// inside that pointee: a later read still needs its current resident leaf.
fn storage_origins(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    root: &Root,
    referent: StructuralTypeId,
) -> Result<Vec<target_operations::TargetReferenceOrigin>, LegalizationError> {
    root.origins()
        .into_iter()
        .map(|mut origin| {
            let selected = source_contract(function, custody, origin.root)
                .and_then(|contract| {
                    terminal_semantics::captured_reference_origin_type(
                        types.iter(),
                        contract,
                        &origin.path,
                    )
                })
                .ok_or_else(invalid)?;
            if selected != referent {
                if function.structural_parameter(origin.root).is_none()
                    || origin.path.iter().any(|segment| {
                        !matches!(
                            segment,
                            ReferenceOriginProjection::Exact(
                                StructuralPathSegment::Field(_)
                                    | StructuralPathSegment::Case(_)
                                    | StructuralPathSegment::FixedIndex(_)
                            )
                        )
                    })
                    || !find_structural_type(types, selected).is_some_and(|declaration| {
                        matches!(declaration.shape, StructuralTypeShape::Reference {
                        referent: declared, ..
                    } if declared == referent)
                    })
                {
                    return Err(invalid());
                }
                origin.path.push(ReferenceOriginProjection::Exact(
                    StructuralPathSegment::Referent,
                ));
            }
            Ok(origin)
        })
        .collect()
}

fn roots_overlap(left: &Root, right: &Root) -> bool {
    let left = left.origins();
    let right = right.origins();
    left.iter().any(|left| {
        right.iter().any(|right| {
            left.kind != target_operations::TargetReferenceOriginKind::Storage
                || right.kind != target_operations::TargetReferenceOriginKind::Storage
                || (left.root == right.root
                    && terminal_semantics::reference_origin_paths_may_overlap(
                        &left.path,
                        &right.path,
                    ))
        })
    })
}

/// Live reference loans keyed by current carrier location, exactly mirroring
/// the lowering-time custody map's membership.
#[derive(Debug, Clone, Default)]
pub(in crate::legalization) struct Custody {
    leaves: BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
    retired: BTreeMap<OccurrenceIdentity, retirements::RetiredOccurrence>,
    edge_exits: Vec<(
        EdgeId,
        BlockId,
        BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
        BTreeMap<OccurrenceIdentity, retirements::RetiredOccurrence>,
    )>,
    borrowed_sources: std::sync::Arc<BTreeMap<PlaceId, Option<Vec<StructuralArgument>>>>,
    descriptor_backing: std::sync::Arc<descriptor_backing::Observations>,
    stored_calls: std::sync::Arc<stored_calls::Effects>,
    /// Exact module demand rosters, separate from live permission leaves.
    placement_catalog: abstract_operations::StructuralTypeCatalog,
    placed_formals: BTreeMap<PlaceId, terminal_psi::TerminalPlacedViewInput>,
}

impl Custody {
    /// Arrival uses this exact edge's post-transfer, post-cleanup roster.
    /// A block declaration or a dominator's survivors cannot restore a leaf.
    fn edge_custody(&self, edge: EdgeId, target: BlockId) -> Result<Self, LegalizationError> {
        let mut matching = self
            .edge_exits
            .iter()
            .filter(|(candidate, block, _, _)| *candidate == edge && *block == target);
        let (_, _, leaves, retired) = matching.next().ok_or_else(invalid)?;
        if matching.next().is_some() {
            return Err(invalid());
        }
        Ok(Self {
            leaves: leaves.clone(),
            retired: retired.clone(),
            edge_exits: Vec::new(),
            borrowed_sources: self.borrowed_sources.clone(),
            descriptor_backing: self.descriptor_backing.clone(),
            stored_calls: self.stored_calls.clone(),
            placement_catalog: self.placement_catalog.clone(),
            placed_formals: self.placed_formals.clone(),
        })
    }

    pub(in crate::legalization) fn leaf(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Option<&Leaf> {
        self.leaves.get(&(place, path.to_vec()))
    }

    /// Observation needs the carrier's current permission, not merely its
    /// declaration. The caller separately checks the reference's read access.
    pub(in crate::legalization) fn readable_leaf(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Option<&Leaf> {
        let leaf = self.leaf(place, path)?;
        (!self.leaves.values().any(|other| {
            other.root.contains_place(place) || self.descends_from(other, &leaf.identity)
        }))
        .then_some(leaf)
    }
}

/// Live permission and referent-relative geometry behind a materialized carrier.
/// Candidate roots constrain authority; its pointer payload selects the address.
pub(in crate::legalization) struct ReferentDestination {
    pub(in crate::legalization) carrier_type: StructuralTypeId,
    pub(in crate::legalization) referent: StructuralTypeId,
    pub(in crate::legalization) carrier_path: Vec<StructuralPathSegment>,
    pub(in crate::legalization) path: Vec<StructuralPathSegment>,
}

pub(in crate::legalization) fn referent_destination(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    carrier: PlaceId,
    path: &[StructuralPathSegment],
    required_access: StructuralAccess,
) -> Option<ReferentDestination> {
    optimization_unit_semantics::reference_projection(
        function,
        types,
        carrier,
        path,
        required_access,
    )?;
    window_destination(function, custody, types, carrier, path, required_access)
}

/// A window uses the exact descriptor selected inside its current carrier.
/// The original place remains the physical pointer source.
pub(in crate::legalization) fn window_destination(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    carrier: PlaceId,
    path: &[StructuralPathSegment],
    required_access: StructuralAccess,
) -> Option<ReferentDestination> {
    let crossing = path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)?;
    let carrier_path = &path[..crossing];
    let carrier_type = source_contract(function, custody, carrier)?;
    let descriptor = leaf_type(types, carrier_type, carrier_path)?;
    let StructuralTypeShape::Reference { referent, access } =
        find_structural_type(types, descriptor)?.shape
    else {
        return None;
    };
    if !access_covers(access, required_access) {
        return None;
    }
    call_paths::call_reference_custody(
        function,
        custody,
        types,
        &StructuralArgument {
            place: carrier,
            path: path.to_vec(),
            access: required_access,
        },
    )
    .ok()?;
    Some(ReferentDestination {
        carrier_type,
        carrier_path: carrier_path.to_vec(),
        referent,
        path: path[crossing + 1..].to_vec(),
    })
}

/// Authenticate the backing owners of a captured window. A child pointer can
/// retain its enclosing array's permission root while its captured address
/// selects one element. Root authority does not require that coarse root to
/// have the element's type; exact descriptor-slot mutation uses window_regions.
pub(in crate::legalization) fn window_backing_roots(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    place: PlaceId,
    path: &[StructuralPathSegment],
    access: StructuralAccess,
) -> Result<Vec<PlaceId>, LegalizationError> {
    let observed = call_paths::call_reference_custody(
        function,
        custody,
        types,
        &StructuralArgument {
            place,
            path: path.to_vec(),
            access,
        },
    )?;
    Ok(observed
        .origins
        .iter()
        .flat_map(Root::origins)
        .map(|origin| origin.root)
        .collect())
}

/// Select mutation storage through the same current-descriptor observation as
/// calls. One runtime alternative cannot retire every possible backing.
pub(in crate::legalization) fn window_regions(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    place: PlaceId,
    path: &[StructuralPathSegment],
    access: StructuralAccess,
) -> Result<Vec<(PlaceId, Vec<StructuralPathSegment>)>, LegalizationError> {
    let Some(crossing) = path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)
    else {
        return Ok(vec![(place, path.to_vec())]);
    };
    let observed = call_paths::call_reference_custody(
        function,
        custody,
        types,
        &StructuralArgument {
            place,
            path: path.to_vec(),
            access,
        },
    )?;
    let mut regions = Vec::new();
    for root in &observed.origins {
        for origin in storage_origins(function, custody, types, root, observed.referent)? {
            let mut prefix = origin.exact_path().ok_or_else(invalid)?;
            prefix.extend_from_slice(&path[crossing + 1..]);
            let region = (origin.root, prefix);
            if !regions.contains(&region) {
                regions.push(region);
            }
        }
    }
    Ok(regions)
}

fn window_storage(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    place: PlaceId,
    path: &[StructuralPathSegment],
    access: StructuralAccess,
) -> Result<(PlaceId, Vec<StructuralPathSegment>), LegalizationError> {
    let mut regions = window_regions(function, custody, types, place, path, access)?;
    if regions.len() != 1 {
        return Err(invalid());
    }
    regions.pop().ok_or_else(invalid)
}

#[track_caller]
fn invalid() -> LegalizationError {
    LegalizationError::custody()
}

/// Inspect owned containment only. A reference's referent is not its payload.
pub(in crate::legalization) fn contains_reference(
    types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
) -> bool {
    terminal_psi::structural_type_contains_reference(types, root)
}

/// Copying unrestricted storage may duplicate shared descriptors, never
/// exclusive permissions. Referents are backing, not contained payload.
pub(in crate::legalization) fn shared_copy_type(
    types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
) -> bool {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    while let Some(current) = pending.pop() {
        if visited.contains(&current) {
            continue;
        }
        visited.push(current);
        let Some(declaration) = find_structural_type(types, current) else {
            return false;
        };
        let fields: Vec<_> = match &declaration.shape {
            StructuralTypeShape::Reference { access, .. } => {
                if *access != StructuralAccess::SharedBorrow {
                    return false;
                }
                continue;
            }
            StructuralTypeShape::Record { fields, .. } => fields.iter().collect(),
            StructuralTypeShape::Sum { cases } => {
                cases.iter().flat_map(|case| &case.fields).collect()
            }
            StructuralTypeShape::Mixed { fields, cases } => fields
                .iter()
                .chain(cases.iter().flat_map(|case| &case.fields))
                .collect(),
            StructuralTypeShape::FixedArray { element, length } => {
                if *length != 0 {
                    pending.push(*element);
                }
                continue;
            }
            _ => continue,
        };
        pending.extend(
            fields
                .into_iter()
                .filter(|field| !field.relevance.is_erased())
                .filter_map(|field| field.field_type.as_structural()),
        );
    }
    true
}

/// Push one field's reference-bearing child as a leaf-path continuation:
/// non-structural fields and children without a reference contribute no
/// pending leaf, and exceeding the bounded worklist declines reconstruction.
fn push_field_leaf(
    pending: &mut Vec<(StructuralTypeId, Vec<StructuralPathSegment>)>,
    path: &[StructuralPathSegment],
    leaves: usize,
    maximum_leaves: usize,
    types: &[StructuralTypeDeclaration],
    field: &terminal_psi::StructuralFieldDeclaration,
) -> Option<()> {
    if field.relevance.is_erased() {
        return Some(());
    }
    let Some(child) = field.field_type.as_structural() else {
        return Some(());
    };
    if !contains_reference(types, child) {
        return Some(());
    }
    if pending.len().checked_add(leaves)? >= maximum_leaves {
        return None;
    }
    let mut child_path = path.to_vec();
    child_path.push(StructuralPathSegment::Field(field.identity.clone()));
    pending.push((child, child_path));
    Some(())
}

/// Exact declaration-order reference leaves of the supported owned shape.
/// `maximum_leaves` bounds reconstruction exactly like the verified rule.
fn leaf_paths(
    types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
    maximum_leaves: usize,
) -> Option<Vec<Vec<StructuralPathSegment>>> {
    let mut output = Vec::new();
    let mut pending = vec![(root, Vec::new())];
    while let Some((current, path)) = pending.pop() {
        if !contains_reference(types, current) {
            continue;
        }
        let Some(declaration) = find_structural_type(types, current) else {
            continue;
        };
        match &declaration.shape {
            StructuralTypeShape::Reference { .. } => {
                if output.len() == maximum_leaves {
                    return None;
                }
                output.push(path);
                continue;
            }
            StructuralTypeShape::Record { fields, .. }
            | StructuralTypeShape::Mixed { fields, .. } => {
                for field in fields.iter().rev() {
                    push_field_leaf(
                        &mut pending,
                        &path,
                        output.len(),
                        maximum_leaves,
                        types,
                        field,
                    )?;
                }
            }
            StructuralTypeShape::FixedArray { element, length }
                if contains_reference(types, *element) =>
            {
                let length = usize::try_from(*length).ok()?;
                if pending
                    .len()
                    .checked_add(output.len())?
                    .checked_add(length)?
                    > maximum_leaves
                {
                    return None;
                }
                for index in (0..length).rev() {
                    let mut child_path = path.clone();
                    child_path.push(StructuralPathSegment::FixedIndex(index as u64));
                    pending.push((*element, child_path));
                }
            }
            _ => {}
        }
        if let Some(cases) = declaration.shape.sum_cases() {
            for case in cases.iter().rev() {
                let mut guarded = path.clone();
                guarded.push(StructuralPathSegment::Case(case.id));
                for field in case.fields.iter().rev() {
                    push_field_leaf(
                        &mut pending,
                        &guarded,
                        output.len(),
                        maximum_leaves,
                        types,
                        field,
                    )?;
                }
            }
        }
    }
    output.sort();
    output.dedup();
    Some(output)
}

/// A reference owns one address descriptor, independent of its referent's
/// shape or contained loans. Looking up its declared referent grants neither
/// payload observation nor ownership of nested descriptors. Establishment and
/// each use separately check source compatibility, access and exact paths;
/// writes retain their ordinary validity and displaced-custody obligations.
fn referent(
    types: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    match find_structural_type(types, structural_type)?.shape {
        StructuralTypeShape::Reference {
            referent,
            access:
                StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow,
        } => Some(referent),
        _ => None,
    }
}

/// A field-only carrier path ending at a reference leaf, then its referent;
/// a non-reference leaf names its own storage type.
fn leaf_referent(
    types: &[StructuralTypeDeclaration],
    current: StructuralTypeId,
    fields: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    let leaf = leaf_type(types, current, fields)?;
    match &find_structural_type(types, leaf)?.shape {
        StructuralTypeShape::Reference { .. } => referent(types, leaf),
        _ => Some(leaf),
    }
}

/// Resolve declaration geometry without granting case membership or custody.
/// Case selects its payload namespace; an unqualified field names only a
/// record or Mixed common member, never an inactive alternative's field.
fn leaf_type(
    types: &[StructuralTypeDeclaration],
    current: StructuralTypeId,
    fields: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    terminal_semantics::runtime_structural_path_tip(types.iter(), current, fields)
        .map(|declaration| declaration.id)
        .or_else(|| {
            terminal_semantics::primitive_projection_declaration(types.iter(), current, fields)
        })
}

/// The structural contract a place currently carries: a signature parameter, a
/// block parameter, an operation-produced home, or a live bare carrier.
fn source_contract(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    if let Some(parameter) = function.structural_parameter(place) {
        return Some(parameter.structural_type);
    }
    if let Some(parameter) = function
        .block_structural_parameters()
        .find(|parameter| parameter.place == place)
    {
        return Some(parameter.structural_type);
    }
    if let Ok((_, result)) = super::structural_case::source_result(function, place) {
        return Some(result.structural_type);
    }
    custody
        .leaf(place, &[])
        .map(|leaf| leaf.contract.structural_type())
}

/// Resolve direct storage from its declared contract, rather than requiring
/// that the owner originated in the machine's input telescope. Reference
/// crossings still require the separately reconstructed live loan.
/// Lending direct storage retains the owner's disposal claims. The source's
/// multiplicity and qualifications do not change a loan's referent geometry.
/// Access still constrains the projection; normalized_source separately
/// retains exact live-loan custody. Source checking owns domain obligations.
fn direct_storage_type(
    function: &PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> Option<StructuralTypeId> {
    if !source.path.iter().all(|segment| {
        matches!(
            segment,
            StructuralPathSegment::Field(_)
                | StructuralPathSegment::FixedIndex(_)
                | StructuralPathSegment::RuntimeIndex { .. }
                | StructuralPathSegment::RepresentationView { .. }
        )
    }) {
        return None;
    }
    let (structural_type, access) =
        if let Some(parameter) = function.arrival_parameter(source.place) {
            (parameter.structural_type, parameter.access)
        } else {
            let (_, result) = super::structural_case::source_result(function, source.place).ok()?;
            (result.structural_type, StructuralAccess::Owned)
        };
    if !source.access.admits(access) {
        return None;
    }
    if source.access == StructuralAccess::SharedBorrow
        && let Some(observed) = terminal_semantics::borrowed_primitive_projection_declaration(
            types.iter(),
            structural_type,
            &source.path,
            None,
        )
    {
        return Some(observed);
    }
    leaf_type(types, structural_type, &source.path)
}

/// The referent type an establishment or projection may name. An empty path
/// names a whole ingress place, a trailing `Referent` dereferences a live
/// carrier, and a field/fixed-index path names exact borrowed ingress storage.
fn reference_source_type(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> Option<StructuralTypeId> {
    if !reference_source_accessible(function, custody, types, source) {
        return None;
    }
    let root = source_contract(function, custody, source.place)?;
    let path = terminal_semantics::reference_source_storage_path(types.iter(), root, &source.path);
    // A shared loan observes a restricted scalar through its read carrier;
    // its owner's predicate remains attached to the original storage. Keep
    // that observation consistent with lowering even for explicit re-views.
    if source.access == StructuralAccess::SharedBorrow
        && let Some(observed) = terminal_semantics::borrowed_primitive_projection_declaration(
            types.iter(),
            root,
            path,
            None,
        )
    {
        return Some(observed);
    }
    leaf_type(types, root, path)
}

/// Declared authority does not require a nominal endpoint. Custody below
/// independently authenticates every live parent and possible backing origin.
fn reference_source_accessible(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> bool {
    let Some(root) = source_contract(function, custody, source.place) else {
        return false;
    };
    let (multiplicity, access) = if let Some(parameter) = function.arrival_parameter(source.place) {
        (parameter.multiplicity, parameter.access)
    } else if let Ok((_, result)) = super::structural_case::source_result(function, source.place) {
        (result.multiplicity, StructuralAccess::Owned)
    } else {
        return false;
    };
    terminal_semantics::reference_source_projection_accessible(
        types.iter(),
        root,
        multiplicity,
        access,
        &source.path,
        source.access,
    )
}

// A new shared loan may coexist only with authenticated shared carrier leaves.
// The original roots and parent identities remain live until every loan ends.
fn shared_source_compatible(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    carrier: &PlaceId,
    path: &[StructuralPathSegment],
    access: StructuralAccess,
) -> bool {
    access == StructuralAccess::SharedBorrow
        && source_contract(function, custody, *carrier)
            .and_then(|root| leaf_type(types, root, path))
            .and_then(|leaf| find_structural_type(types, leaf))
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
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> Option<optimization_unit_semantics::StructuralStorageProjection> {
    let mut carrier = source_contract(function, custody, source.place)?;
    let mut path =
        terminal_semantics::reference_source_storage_path(types.iter(), carrier, &source.path);
    if let Some(crossing) = path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)
    {
        let leaf = custody.leaf(source.place, &path[..crossing])?;
        let reference = leaf_type(types, carrier, &path[..crossing])?;
        let StructuralTypeShape::Reference { referent, access } =
            find_structural_type(types, reference)?.shape
        else {
            return None;
        };
        if referent != leaf.referent || !source.access.admits_referent(access) {
            return None;
        }
        carrier = leaf.referent;
        path = &path[crossing + 1..];
    }
    optimization_unit_semantics::structural_borrowed_storage_projection(
        types.iter(),
        carrier,
        path,
        source.access,
    )
}

/// A `.., Referent` source names a suspended child of the located carrier
/// leaf; an empty path names the whole ingress referent directly. Every
/// selected payload and runtime index retains its exact storage coordinate.
/// Formation compatibility belongs to Psi's admitted borrow judgment, not
/// this coordinate reconstruction. A conservative path-overlap query cannot
/// replay logical disequality of runtime selectors. This audit authenticates
/// existing parents, types and origins; receiving use and disposition still
/// check the live roster rather than treating those origins as permission.
/// Rejoin the actual live source descriptor; the declared placed carrier alone
/// does not provide an established backing interpretation.
fn placed_source(
    custody: &Custody,
    source: &StructuralArgument,
) -> Option<target_operations::TargetPlacedInterpretation> {
    if source.path.is_empty() {
        return custody
            .placed_formals
            .get(&source.place)
            .cloned()
            .map(target_operations::TargetPlacedInterpretation::Formal);
    }
    let (StructuralPathSegment::Referent, path) = source.path.split_last()? else {
        return None;
    };
    custody.leaf(source.place, path)?.placed.clone()
}

/// Normalize the current borrowed source through its admitted storage origins.
fn normalized_source(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> Result<(Root, Vec<OccurrenceIdentity>), LegalizationError> {
    let Some(origins) = custody.borrowed_sources.get(&source.place) else {
        return normalized_direct_source(function, custody, types, source);
    };
    let origins = origins
        .as_ref()
        .filter(|origins| !origins.is_empty())
        .ok_or(invalid())?;
    let expected = storage_projection(function, custody, types, source).ok_or(invalid())?;
    let mut candidates = Vec::with_capacity(origins.len());
    for origin in origins {
        if !source.access.admits(origin.access) {
            return Err(invalid());
        }
        let mut resolved = origin.clone();
        resolved.path.extend_from_slice(&source.path);
        resolved.access = source.access;
        // Rebinding may change the carrier's presentation, never a window's
        // original backing or exact interval. Keep every possible origin.
        if !storage_projection(function, custody, types, &resolved)
            .is_some_and(|actual| actual.matches(types.iter(), expected))
        {
            return Err(invalid());
        }
        candidates.push(normalized_direct_source(
            function, custody, types, &resolved,
        )?);
    }
    combine_candidate_origins(candidates)
}

fn normalized_direct_source(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> Result<(Root, Vec<OccurrenceIdentity>), LegalizationError> {
    let path = source.path.as_slice();
    if let Some(crossing) = source
        .path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)
    {
        let carrier_path = &path[..crossing];
        let suffix = &path[crossing + 1..];
        // A remaining crossing is a real descriptor, never a borrowed-leaf
        // marker. Missing live custody cannot become a direct-storage origin.
        let leaf = custody
            .leaf(source.place, carrier_path)
            .ok_or_else(|| invalid())?;
        if optimization_unit_semantics::structural_borrowed_storage_projection(
            types.iter(),
            leaf.referent,
            suffix,
            source.access,
        )
        .is_none()
        {
            return Err(invalid());
        }
        Ok((leaf.root.projected(suffix), vec![leaf.identity.clone()]))
    } else if source.path.is_empty() {
        Ok((Root::Place(source.place), Vec::new()))
    } else if storage_projection(function, custody, types, source).is_some() {
        let root = Root::Ingress(source.place, path.to_vec());
        Ok((root, Vec::new()))
    } else {
        Err(invalid())
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

mod call_paths;

/// Ingress reference leaves identify external pointees, not descriptor storage.
/// Ordinary origin fields protect their exact storage regions. Shared access
/// may coexist with shared loans; writes, moves and disposal may not. Unknown
/// permissions remain conflicting, never evidence of disjointness or access.
fn storage_access_conflicts(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    place: PlaceId,
    path: &[StructuralPathSegment],
    requested: StructuralAccess,
) -> bool {
    // Captured ingress pointees cannot occupy a home owned by this activation.
    // A borrowed arrival or a projection through a descriptor is not such a
    // home. Resolve this from the typed path, not its flattened overlap region:
    // flattening intentionally marks incoming pointees as indirect and would
    // otherwise make a reference conflict with disposal of its own carrier.
    let activation_local = function.arrival_parameter(place).map_or_else(
        || super::structural_case::source_result(function, place).is_ok(),
        |parameter| parameter.access == StructuralAccess::Owned,
    ) && source_contract(function, custody, place).is_some_and(|root| {
        terminal_semantics::projection_stays_in_inline_storage(types.iter(), root, path)
    });
    let origin_overlaps = |origin_path: &[ReferenceOriginProjection]| {
        let Some(origin) = source_contract(function, custody, place)
            .and_then(|root| {
                terminal_semantics::captured_reference_origin_type(types.iter(), root, origin_path)
            })
            .and_then(|identity| find_structural_type(types, identity))
        else {
            return true;
        };
        !matches!(origin.shape, StructuralTypeShape::Reference { .. })
            && terminal_semantics::reference_origin_paths_may_overlap(
                &path
                    .iter()
                    .cloned()
                    .map(ReferenceOriginProjection::Exact)
                    .collect::<Vec<_>>(),
                origin_path,
            )
    };
    custody.leaves.values().any(|reference| {
        let permission = leaf_type(
            types,
            reference.contract.structural_type(),
            &reference.identity.path,
        )
        .and_then(|identity| find_structural_type(types, identity))
        .and_then(|declaration| match declaration.shape {
            StructuralTypeShape::Reference { access, .. } => Some(access),
            _ => None,
        });
        if requested == StructuralAccess::SharedBorrow
            && permission == Some(StructuralAccess::SharedBorrow)
        {
            return false;
        }
        reference.backing.iter().any(|backing| {
            descriptor_backing::overlaps(
                backing,
                function.machine,
                place,
                path,
                activation_local,
                &origin_overlaps,
            )
        })
    })
}

/// Validate every structural call argument under reference custody. Owned
/// carriers move their complete live leaf roster; a suspended root may only be
/// spelled through a `.., Referent` projection; two reference-touching
/// arguments cannot share one root unless both are shared borrows. Returns the
/// moved leaves keyed by their current carrier path; the caller commits the
/// removal after every other call check succeeds.
fn call_arguments(
    function: &PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
    custody: &Custody,
    arguments: &[StructuralArgument],
) -> Result<BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>, LegalizationError> {
    let mut normalized = Vec::new();
    let mut dependencies = Vec::new();
    let mut moved = BTreeMap::new();
    for (argument_position, argument) in arguments.iter().enumerate() {
        if argument.path.contains(&StructuralPathSegment::Referent) {
            let resolved = call_paths::call_reference_custody(function, custody, types, argument)?;
            dependencies.push((argument_position, resolved.dependencies));
            normalized.push((
                argument_position,
                resolved.origins,
                argument.access,
                CallRegion::Referent,
                None,
            ));
            continue;
        }
        if source_contract(function, custody, argument.place)
            .is_some_and(|contract| contains_reference(types, contract))
        {
            let authority = function
                .arrival_parameter(argument.place)
                .map(|parameter| parameter.access)
                .unwrap_or(StructuralAccess::Owned);
            if !access_covers(authority, argument.access) {
                return Err(invalid());
            }
            let contract =
                source_contract(function, custody, argument.place).ok_or_else(|| invalid())?;
            let selected = leaf_type(types, contract, &argument.path).ok_or_else(|| invalid())?;
            // Scalar/descriptor-free elements retain runtime coordinates.
            // Moving a reference-bearing element still needs a statically
            // selected leaf roster; a dynamic index must not silently omit it.
            if contains_reference(types, selected)
                && argument
                    .path
                    .iter()
                    .any(|segment| matches!(segment, StructuralPathSegment::RuntimeIndex { .. }))
            {
                return Err(invalid());
            }
            if !argument.path.iter().all(|segment| {
                matches!(
                    segment,
                    StructuralPathSegment::Case(_)
                        | StructuralPathSegment::Field(_)
                        | StructuralPathSegment::FixedIndex(_)
                        | StructuralPathSegment::RuntimeIndex { .. }
                )
            }) {
                return Err(invalid());
            }
            let storage = Root::Ingress(argument.place, argument.path.clone());
            // An ingress descriptor names external backing, not the carrier
            // bytes being transferred. Use the same declaration-derived
            // storage judgment as disposition instead of treating a copied
            // descriptor's origin slot as a loan into its former owner.
            if storage_access_conflicts(
                function,
                custody,
                types,
                argument.place,
                &argument.path,
                argument.access,
            ) {
                return Err(invalid());
            }
            normalized.push((
                argument_position,
                vec![storage],
                argument.access,
                CallRegion::CarrierStorage,
                None,
            ));
            // Lending write-only carrier storage does not observe, lend or
            // transfer its resident pointers. Old child permissions remain
            // attached to their captured descriptors across replacement.
            if argument.access == StructuralAccess::WriteOnlyBorrow {
                continue;
            }
            for ((carrier, carrier_path), leaf) in &custody.leaves {
                if *carrier != argument.place {
                    continue;
                }
                let Some(path) = owned_relative_path(carrier_path, &argument.path) else {
                    continue;
                };
                let permission = leaf_type(types, selected, path)
                    .and_then(|leaf| {
                        find_structural_type(types, leaf).map(|declaration| &declaration.shape)
                    })
                    .and_then(|shape| match shape {
                        StructuralTypeShape::Reference { access, .. } => Some(*access),
                        _ => None,
                    })
                    .ok_or_else(|| invalid())?;
                let supplied = if access_covers(argument.access, permission) {
                    permission
                } else {
                    argument.access
                };
                // Shared observations coexist with shared descendants, as on
                // a direct descriptor crossing. Exclusive or owned ingress
                // still cannot abstract away a suspended parent occurrence.
                if custody.leaves.iter().any(|((carrier, path), child)| {
                    custody.descends_from(child, &leaf.identity)
                        && (argument.access == StructuralAccess::Owned
                            || !shared_source_compatible(
                                function, custody, types, carrier, path, supplied,
                            ))
                }) || (argument.access == StructuralAccess::Owned
                    && moved
                        .insert((argument.place, carrier_path.clone()), leaf.clone())
                        .is_some())
                {
                    return Err(invalid());
                }
                normalized.push((
                    argument_position,
                    vec![leaf.root.clone()],
                    supplied,
                    CallRegion::Referent,
                    Some((argument.place, carrier_path.clone())),
                ));
            }
            continue;
        }
        // Descriptor-free storage can still be the parent of a live borrow.
        // Normalizing its origin alone must not restore access while a child
        // survives; shared access remains compatible with shared descendants.
        if storage_access_conflicts(
            function,
            custody,
            types,
            argument.place,
            &argument.path,
            argument.access,
        ) {
            return Err(invalid());
        }
        let (root, _) = normalized_source(function, custody, types, argument)?;
        normalized.push((
            argument_position,
            vec![root],
            argument.access,
            if custody.borrowed_sources.contains_key(&argument.place) {
                CallRegion::Referent
            } else {
                CallRegion::OrdinaryStorage
            },
            None,
        ));
    }
    for (position, (argument_position, roots, access, reference_access, descriptor)) in
        normalized.iter().enumerate()
    {
        if normalized[position + 1..].iter().any(
            |(other_position, others, other_access, other_reference_access, other_descriptor)| {
                // Different payload guards on the same stored sum cannot be
                // active together. This excludes simultaneous permission use;
                // it does not claim the overlaid payload bytes are disjoint.
                let alternatives = descriptor
                    .as_ref()
                    .zip(other_descriptor.as_ref())
                    .is_some_and(|(left, right)| {
                        left.0 == right.0
                            && left
                                .1
                                .iter()
                                .zip(&right.1)
                                .find(|(left, right)| left != right)
                                .is_some_and(|(left, right)| {
                                    matches!(
                                        (left, right),
                                        (
                                            StructuralPathSegment::Case(_),
                                            StructuralPathSegment::Case(_)
                                        )
                                    )
                                })
                    });
                !alternatives
                    && (*reference_access != CallRegion::OrdinaryStorage
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
            return Err(invalid());
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
                    roots_overlap(root, &Root::Ingress(other.place, other.path.clone()))
                })
        }) {
            return Err(invalid());
        }
        if normalized
            .iter()
            .any(|(other_position, others, access, _, _)| {
                *other_position != argument_position
                    && *access != StructuralAccess::SharedBorrow
                    && roots
                        .iter()
                        .any(|root| others.iter().any(|other| roots_overlap(root, other)))
            })
        {
            return Err(invalid());
        }
    }
    Ok(moved)
}

/// Apply the staged argument moves once every other call check succeeded.
/// Moves are keyed by current carrier path, so result leaves re-established
/// under a moved identity survive the commit.
fn commit_argument_moves(
    custody: &mut Custody,
    moved: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
) {
    for key in moved.keys() {
        custody.leaves.remove(key);
    }
}

/// Establish the declared result reference roster of a structural-result
/// `CallStructural` whose result carries reference custody, resolved against
/// the already-committed `moved` argument leaves. Returns the ordered resolved
/// leaf roster the target row must carry.
fn call_results(
    function: &PsiOptimizationFunction,
    contract: terminal_semantics::ReferenceCallContract<'_>,
    types: &[StructuralTypeDeclaration],
    result: &StructuralOperationResult,
    psi_operation: OperationId,
    structural_arguments: &[StructuralArgument],
    custody: &mut Custody,
    moved: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
) -> Result<Vec<target_operations::TargetReferenceResult>, LegalizationError> {
    if !contains_reference(types, result.structural_type) {
        return Ok(Vec::new());
    }
    let mappings = terminal_semantics::bind_reference_call_results(
        types.iter(),
        contract,
        structural_arguments,
    )
    .map_err(|reason| LegalizationError::ReferenceResult { reason })?;
    if custody
        .leaves
        .keys()
        .any(|(carrier, _)| *carrier == result.place)
    {
        return Err(invalid());
    }
    // Reconstruct every declared result path before auditing target rows.
    // Case guards are alternative residences of transported permissions, not
    // simultaneous copies. Semantic custody has checked exclusivity; this
    // independent join retains exact actual origins for each signature slot
    // instead of imposing a second, unguarded identity-overlap policy.
    let mut established = Vec::new();
    let mut reference_results = Vec::new();
    for mapping in mappings {
        let expected =
            leaf_referent(types, result.structural_type, &mapping.path).ok_or_else(|| invalid())?;
        let result_access = leaf_type(types, result.structural_type, &mapping.path)
            .and_then(|leaf| find_structural_type(types, leaf))
            .and_then(|declaration| match declaration.shape {
                StructuralTypeShape::Reference { access, .. } => Some(access),
                _ => None,
            })
            .ok_or_else(invalid)?;
        if mapping.sources.is_empty() {
            return Err(invalid());
        }
        let mut candidates = Vec::with_capacity(mapping.sources.len());
        for binding in &mapping.sources {
            let (source, transfers_existing) = match &binding.origin {
                terminal_semantics::ReferenceCallOrigin::Parameter {
                    source,
                    transfers_existing,
                } => (source, *transfers_existing),
                terminal_semantics::ReferenceCallOrigin::Invocation {
                    carrier_type,
                    path: origin_path,
                    access,
                } => {
                    if !access_covers(*access, result_access)
                        || binding.endpoint != expected
                        || binding.extent != semantic_vocabulary::ReferenceOriginExtent::Projected
                    {
                        return Err(invalid());
                    }
                    let root = Root::Alternatives(vec![target_operations::TargetReferenceOrigin {
                        root: result.place,
                        kind: target_operations::TargetReferenceOriginKind::Invocation {
                            carrier_type: *carrier_type,
                        },
                        path: origin_path.clone(),
                        extent: binding.extent,
                    }]);
                    candidates.push(Leaf::formed(
                        (result.place, mapping.path.clone()),
                        root,
                        Vec::new(),
                        psi_operation,
                        result,
                        expected,
                    ));
                    continue;
                }
            };
            if !access_covers(source.access, result_access) {
                return Err(invalid());
            }
            if reference_source_type(function, custody, types, source)
                .and_then(|root| {
                    terminal_semantics::reference_origin_type(types.iter(), root, &binding.suffix)
                })
                .is_none_or(|endpoint| {
                    binding.extent == semantic_vocabulary::ReferenceOriginExtent::Projected
                        && endpoint != expected
                })
            {
                return Err(invalid());
            }
            let leaf = if transfers_existing {
                // An owned-carrier ingress mapping relocates one of the leaves the
                // moved arguments just transferred; the leaf keeps its identity,
                // root and parent, and lands at the declared result path.
                let Some((StructuralPathSegment::Referent, carrier_path)) =
                    source.path.split_last()
                else {
                    return Err(invalid());
                };
                let Some(leaf) = moved.get(&(source.place, carrier_path.to_vec())) else {
                    // A guarded formal slot need not be resident in this
                    // actual. Only reconstructed input custody contributes
                    // an origin to the returned descriptor relation.
                    if !carrier_path
                        .iter()
                        .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
                    {
                        return Err(invalid());
                    }
                    continue;
                };
                leaf.clone()
            } else {
                let (root, parents) = normalized_source(function, custody, types, source)?;
                let root = root.projected_family(&binding.suffix, binding.extent);
                let (backing, backing_parents) = descriptor_backing::capture(
                    function,
                    custody,
                    types,
                    &source,
                    psi_operation,
                    &root,
                )?;
                let mut leaf = Leaf::formed(
                    (result.place, mapping.path.clone()),
                    root,
                    parents,
                    psi_operation,
                    result,
                    expected,
                );
                leaf.placed = placed_source(custody, &source);
                leaf.backing = backing;
                leaf.parents.extend(backing_parents);
                leaf.parents.sort();
                leaf.parents.dedup();
                leaf
            };
            // A region-relative return proves containment, not the exact
            // whole-view displacement required by a sealed placed association.
            if leaf.placed.is_some()
                && (!binding.suffix.is_empty()
                    || binding.extent != semantic_vocabulary::ReferenceOriginExtent::Projected
                    || leaf.referent != expected)
            {
                return Err(LegalizationError::ReferenceResult {
                    reason: "placed return requires exact whole-view origin",
                });
            }
            candidates.push(leaf);
        }
        if candidates.is_empty() {
            continue;
        }
        let joining = candidates.len() > 1;
        let mut leaf = combine_alternatives(candidates)?;
        if joining {
            // The selected descriptor is one result resource, conservatively
            // pinning the complete union of the consumed input permissions.
            leaf.identity =
                OccurrenceIdentity::operation(&(result.place, mapping.path.clone()), psi_operation);
            leaf.reform(psi_operation, result);
        }
        reference_results.push(target_operations::TargetReferenceResult {
            path: mapping.path.clone(),
            origins: leaf.root.origins(),
        });
        established.push(((result.place, mapping.path.clone()), leaf));
    }
    for (key, leaf) in established {
        custody.leaves.insert(key, leaf);
    }
    Ok(reference_results)
}

/// Alternatives retain one returned permission, not independently live loans.
fn combine_alternatives(candidates: Vec<Leaf>) -> Result<Leaf, LegalizationError> {
    let mut candidates = candidates.into_iter();
    let mut leaf = candidates.next().ok_or(invalid())?;
    let mut origins = vec![(leaf.root.clone(), leaf.parents.clone())];
    for candidate in candidates {
        if leaf.referent != candidate.referent || leaf.placed != candidate.placed {
            return Err(invalid());
        }
        origins.push((candidate.root, candidate.parents));
        leaf.backing.extend(candidate.backing);
        leaf.formations.extend(candidate.formations);
    }
    leaf.backing.sort();
    leaf.backing.dedup();
    occurrences::normalize_formations(&mut leaf.formations)?;
    (leaf.root, leaf.parents) = combine_candidate_origins(origins)?;
    Ok(leaf)
}

fn combine_candidate_origins(
    candidates: Vec<(Root, Vec<OccurrenceIdentity>)>,
) -> Result<(Root, Vec<OccurrenceIdentity>), LegalizationError> {
    let mut candidates = candidates.into_iter();
    let (root, parents) = candidates.next().ok_or(invalid())?;
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
    Ok((Root::Alternatives(origins), immediate_parents))
}

/// Select the inline payload of this storage subtree. A pointer crossing
/// names external backing, which remains anchored to its ingress occurrence
/// when the carrier itself moves, is copied, or is retired.
fn owned_relative_path<'a>(
    path: &'a [StructuralPathSegment],
    prefix: &[StructuralPathSegment],
) -> Option<&'a [StructuralPathSegment]> {
    path.strip_prefix(prefix)
        .filter(|relative| !relative.contains(&StructuralPathSegment::Referent))
}

/// Whole cleanup uses the same declaration-derived subtree release as a
/// partial complement. Storage keys, not shared loan provenance, select rows.
fn discard_owned(
    function: &PsiOptimizationFunction,
    custody: &mut Custody,
    types: &[StructuralTypeDeclaration],
    source: PlaceId,
    event: retirements::DisposalEvent,
) -> Result<bool, LegalizationError> {
    discard_subtree(function, custody, types, source, &[], event)
}

fn discard_subtree(
    function: &PsiOptimizationFunction,
    custody: &mut Custody,
    types: &[StructuralTypeDeclaration],
    source: PlaceId,
    prefix: &[StructuralPathSegment],
    event: retirements::DisposalEvent,
) -> Result<bool, LegalizationError> {
    let Some(contract) = source_contract(function, custody, source) else {
        return Ok(false);
    };
    let selected = leaf_type(types, contract, prefix).ok_or(invalid())?;
    if !contains_reference(types, selected) {
        return Ok(false);
    }
    let released: Vec<_> = custody
        .leaves
        .iter()
        .filter(|((carrier, path), _)| {
            *carrier == source && owned_relative_path(path, prefix).is_some()
        })
        .map(|(location, _)| location.clone())
        .collect();
    for location in released {
        let leaf = custody.leaves.remove(&location).ok_or_else(invalid)?;
        custody.retire(leaf, event)?;
    }
    custody.prune_retired();
    Ok(true)
}

fn discard_residual(
    function: &PsiOptimizationFunction,
    custody: &mut Custody,
    types: &[StructuralTypeDeclaration],
    discard: &terminal_psi::StructuralAffineDiscard,
    event: retirements::DisposalEvent,
) -> Result<(), LegalizationError> {
    let root = source_contract(function, custody, discard.place).ok_or(invalid())?;
    if leaf_type(types, root, &discard.path) != Some(discard.structural_type)
        || storage_access_conflicts(
            function,
            custody,
            types,
            discard.place,
            &discard.path,
            StructuralAccess::Owned,
        )
    {
        return Err(invalid());
    }
    discard_subtree(
        function,
        custody,
        types,
        discard.place,
        &discard.path,
        event,
    )?;
    Ok(())
}

/// Stage every selected reference leaf before committing a storage transfer.
/// Projection prefixes select source storage; destination prefixes relocate
/// its leaves without moving a sibling. Owned unrestricted operands retain
/// their sources and form fresh shared occurrences; moves preserve identity.
fn relocate_subtree_leaves<'a>(
    custody: &mut Custody,
    function: &PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
    destination: PlaceId,
    operation: OperationId,
    operands: impl IntoIterator<
        Item = (
            &'a StructuralArgument,
            StructuralTypeId,
            Vec<StructuralPathSegment>,
        ),
    >,
) -> Result<(), LegalizationError> {
    let mut relocations = Vec::new();
    let mut moved = BTreeSet::new();
    let mut locations = BTreeMap::new();
    for (argument, nested, prefix) in operands {
        if !contains_reference(types, nested) {
            continue;
        }
        if custody.leaves.keys().any(|(carrier, path)| {
            *carrier == destination && owned_relative_path(path, &prefix).is_some()
        }) {
            return Err(invalid());
        }
        let contract = source_contract(function, custody, argument.place).ok_or(invalid())?;
        let parameter = function.structural_parameter(argument.place).or_else(|| {
            function
                .block_structural_parameters()
                .find(|parameter| parameter.place == argument.place)
        });
        let copies_storage = parameter.map_or_else(
            || {
                super::structural_case::source_result(function, argument.place).is_ok_and(
                    |(_, result)| result.multiplicity == StructuralMultiplicity::Unrestricted,
                )
            },
            |parameter| {
                parameter.access == StructuralAccess::Owned
                    && parameter.multiplicity == StructuralMultiplicity::Unrestricted
            },
        );
        if argument.access != StructuralAccess::Owned
            || argument.place == destination
            || leaf_type(types, contract, &argument.path) != Some(nested)
        {
            return Err(invalid());
        }
        for ((carrier, source_path), leaf) in &custody.leaves {
            if *carrier != argument.place {
                continue;
            }
            let Some(path) = owned_relative_path(source_path, &argument.path) else {
                continue;
            };
            if leaf_referent(types, nested, path) != Some(leaf.referent) {
                return Err(invalid());
            }
            let mut target_path = prefix.clone();
            target_path.extend(path.iter().cloned());
            let mut leaf = leaf.clone();
            if copies_storage {
                let reference_type = leaf_type(types, nested, path).ok_or(invalid())?;
                if !find_structural_type(types, reference_type).is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::Reference {
                            access: StructuralAccess::SharedBorrow,
                            ..
                        }
                    )
                }) {
                    return Err(invalid());
                }
                let mut observation = source_path.clone();
                observation.push(StructuralPathSegment::Referent);
                call_paths::call_reference_custody(
                    function,
                    custody,
                    types,
                    &StructuralArgument {
                        place: argument.place,
                        path: observation,
                        access: StructuralAccess::SharedBorrow,
                    },
                )?;
                leaf.identity =
                    OccurrenceIdentity::operation(&(destination, target_path.clone()), operation);
                if let Ok((declaration_operation, result)) =
                    super::structural_case::source_result(function, destination)
                {
                    leaf.contract = CarrierContract::Operation {
                        operation: declaration_operation,
                        result: result.clone(),
                    };
                } else {
                    let parameter = function
                        .structural_parameter(destination)
                        .or_else(|| {
                            function
                                .block_structural_parameters()
                                .find(|parameter| parameter.place == destination)
                        })
                        .ok_or(invalid())?;
                    leaf.contract = CarrierContract::Parameter(parameter.clone());
                }
                leaf.formations = vec![Formation {
                    identity: leaf.identity.clone(),
                    contract: leaf.contract.clone(),
                }];
            } else {
                let source = (argument.place, source_path.clone());
                if !moved.insert(source.clone()) {
                    return Err(invalid());
                }
                locations.insert(source, (destination, target_path.clone()));
            }
            relocations.push((leaf, target_path));
        }
    }
    for source in moved {
        custody.leaves.remove(&source).ok_or(invalid())?;
    }
    for (leaf, to) in relocations {
        custody.leaves.insert((destination, to), leaf);
    }
    borrowed_arrivals::relocate(custody, &locations);
    Ok(())
}

/// Reconstruct the sole-leaf move from owned ingress independently of target
/// rows. Other establishments retain their parent as ordinary reborrows.
fn transferred_ingress_leaf(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> Option<(PlaceId, Vec<StructuralPathSegment>)> {
    let parameter = function
        .structural_parameter(source.place)
        .filter(|parameter| parameter.access == StructuralAccess::Owned)?;
    if matches!(
        find_structural_type(types, parameter.structural_type)?.shape,
        StructuralTypeShape::Reference { .. }
    ) {
        return None;
    }
    let (StructuralPathSegment::Referent, path) = source.path.split_last()? else {
        return None;
    };
    let paths = leaf_paths(types, parameter.structural_type, custody.leaves.len())?;
    if paths.len() != 1 || paths[0].as_slice() != path {
        return None;
    }
    let key = (source.place, path.to_vec());
    let leaf = custody.leaves.get(&key)?;
    (leaf.parents.is_empty()
        && custody
            .leaves
            .keys()
            .filter(|(carrier, _)| *carrier == source.place)
            .count()
            == 1
        && !custody
            .leaves
            .values()
            .any(|child| custody.descends_from(child, &leaf.identity)))
    .then_some(key)
}

/// Apply one operation's custody effect after its target row validated.
/// Admission was proven upstream; this replay keeps only enough state to
/// resolve later `.., Referent` spellings and result rosters independently.
pub(in crate::legalization) fn apply(
    custody: &mut Custody,
    node: &optimization_unit::OptimizationNode,
    function: &PsiOptimizationFunction,
    native: &target_operations::TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    // A selected Omega provider has ordinary call custody, including owned
    // reference carriers. Reuse the exact retained-origin join used by row
    // validation; a boundary declaration or candidate roster alone cannot
    // turn a foreign ABI call into an in-module ownership transfer.
    let installed = super::call_origin::selected_boundary_operation(node, function, native, plan)?;
    let operation = installed
        .as_ref()
        .map(|(operation, _)| operation)
        .unwrap_or(&node.operation);
    let types = &plan.structural_types[..];
    match operation {
        AbstractOperation::StructuralLeafCopy {
            psi_operation,
            result,
            source,
            path,
        } if contains_reference(types, result.structural_type) => {
            let source_type = source_contract(function, custody, *source).ok_or(invalid())?;
            let path: std::borrow::Cow<'_, [StructuralPathSegment]> = match path {
                abstract_operations::StructuralCopyPath::Ordinary(path) => {
                    std::borrow::Cow::Borrowed(path)
                }
                abstract_operations::StructuralCopyPath::Case(path) => std::borrow::Cow::Owned(
                    terminal_semantics::canonical_structural_storage_path(
                        types.iter(),
                        source_type,
                        path,
                    )
                    .ok_or(invalid())?,
                ),
            };
            let shared_descriptor = terminal_semantics::is_shared_reference_carrier(
                types.iter(),
                result.structural_type,
            );
            if !shared_copy_type(types, result.structural_type)
                || (result.multiplicity != StructuralMultiplicity::Unrestricted
                    && !(result.multiplicity == StructuralMultiplicity::Affine
                        && shared_descriptor))
                || !result.claims.is_empty()
                || !result.qualification_establishments.is_empty()
                || source_contract(function, custody, *source)
                    .and_then(|root| leaf_type(types, root, &path))
                    != Some(result.structural_type)
                || custody
                    .leaves
                    .keys()
                    .any(|(carrier, _)| *carrier == result.place)
            {
                return Err(invalid());
            }
            // Bound declared slots as at ingress, not by resident descriptors:
            // an empty case may legitimately have no live payload leaves.
            let paths = leaf_paths(types, result.structural_type, 4096)
                .filter(|paths| !paths.is_empty())
                .ok_or(invalid())?;
            let mut copied = Vec::with_capacity(paths.len());
            for relative in &paths {
                let mut selected = path.to_vec();
                selected.extend_from_slice(relative);
                let mut observation = selected.clone();
                observation.push(StructuralPathSegment::Referent);
                // Observation authenticates current permission and descriptor
                // storage without minting a reborrow of the source identity.
                let observed = call_paths::observe_reference_custody(
                    function,
                    custody,
                    types,
                    &StructuralArgument {
                        place: *source,
                        path: observation,
                        access: StructuralAccess::SharedBorrow,
                    },
                )?;
                if observed.descriptors.is_empty() {
                    // Only a declaration's conditional case leaves can be
                    // absent; the observation rejects every missing ordinary
                    // slot and every permission mismatch independently.
                    continue;
                }
                let expected =
                    leaf_referent(types, result.structural_type, relative).ok_or(invalid())?;
                let mut alternatives = Vec::with_capacity(observed.descriptors.len());
                for descriptor in observed.descriptors {
                    let source_leaf = custody.leaves.get(&descriptor).ok_or(invalid())?;
                    if source_leaf.referent != expected {
                        return Err(invalid());
                    }
                    let mut leaf = source_leaf.clone();
                    leaf.identity = OccurrenceIdentity::operation(
                        &(result.place, relative.clone()),
                        *psi_operation,
                    );
                    leaf.reform(*psi_operation, result);
                    alternatives.push(leaf);
                }
                copied.push((
                    (result.place, relative.clone()),
                    combine_alternatives(alternatives)?,
                ));
            }
            if custody.leaves.keys().any(|(carrier, selected)| {
                *carrier == *source
                    && owned_relative_path(selected, path.as_ref())
                        .is_some_and(|relative| !paths.iter().any(|expected| expected == relative))
            }) {
                return Err(invalid());
            }
            custody.leaves.extend(copied);
        }
        AbstractOperation::EstablishPlacedReference {
            psi_operation,
            result,
            source,
            correspondence,
        } => {
            let referent = referent(types, result.structural_type).ok_or_else(invalid)?;
            let mut selected = plan
                .structural_types
                .placed_views()
                .iter()
                .filter(|plan| plan.carrier == referent);
            let (Some(selected), None) = (selected.next(), selected.next()) else {
                return Err(invalid());
            };
            let actual =
                reference_source_type(function, custody, types, source).ok_or_else(invalid)?;
            let expected = terminal_semantics::placed_ram_plan_correspondence(
                types,
                plan.structural_types.placed_views(),
                selected,
                actual,
            )
            .map_err(|_| invalid())?;
            if &expected != correspondence
                || !result.is_affine_plain()
                || !matches!(
                    source.access,
                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                )
                || !find_structural_type(types, result.structural_type).is_some_and(|row| {
                    matches!(row.shape, StructuralTypeShape::Reference { access, .. }
                        if access.admits(source.access))
                })
                || custody.leaves.keys().any(|(carrier, path)| {
                    *carrier == result.place && owned_relative_path(path, &[]).is_some()
                })
                || custody
                    .leaves
                    .values()
                    .any(|leaf| leaf.identity.place == result.place)
            {
                return Err(invalid());
            }
            let (root, parents) = normalized_source(function, custody, types, source)?;
            let (backing, backing_parents) = descriptor_backing::capture(
                function,
                custody,
                types,
                source,
                *psi_operation,
                &root,
            )?;
            let mut leaf = Leaf::formed(
                (result.place, Vec::new()),
                root,
                parents,
                *psi_operation,
                result,
                referent,
            );
            leaf.backing = backing;
            leaf.parents.extend(backing_parents);
            leaf.parents.sort();
            leaf.parents.dedup();
            leaf.placed = Some(target_operations::TargetPlacedInterpretation::Captured(
                correspondence.clone(),
            ));
            custody.leaves.insert((result.place, Vec::new()), leaf);
        }
        AbstractOperation::EstablishReference {
            psi_operation,
            result,
            source,
            storage,
        } => {
            let referent = referent(types, result.structural_type).ok_or_else(|| invalid())?;
            if result.multiplicity != StructuralMultiplicity::Affine
                || !result.qualifications.is_empty()
                || !result.claims.is_empty()
                || !result.qualification_establishments.is_empty()
                || custody
                    .leaves
                    .keys()
                    .any(|(carrier, _)| *carrier == result.place)
                || !reference_source_accessible(function, custody, types, source)
                || !match storage {
                    Some(storage) => reference_source_type(function, custody, types, source)
                        .is_some_and(|actual| {
                            terminal_semantics::reference_storage_source_compatible(
                                types.iter(),
                                actual,
                                result.structural_type,
                                storage.stored_scalar,
                            )
                        }),
                    None => source_contract(function, custody, source.place).is_some_and(|root| {
                        terminal_semantics::reference_source_projection_compatible(
                            types.iter(),
                            root,
                            &source.path,
                            result.structural_type,
                        )
                    }),
                }
            {
                return Err(invalid());
            }
            let transferred = transferred_ingress_leaf(function, custody, types, source);
            let leaf =
                if let Some(key) = transferred {
                    custody.leaves.remove(&key).ok_or_else(|| invalid())?
                } else {
                    let (mut root, parents) = normalized_source(function, custody, types, source)?;
                    if let Some(storage) = storage {
                        // The source operation, not its signature, issues the
                        // interpretation step. A recipe with no relative offset
                        // can describe only an actual zero-offset formation.
                        let mut offsets =
                            function
                                .block_nodes()
                                .filter_map(|node| match node.operation {
                                    AbstractOperation::IntegerConstant {
                                        result, value, ..
                                    } if result == storage.byte_offset => Some(value),
                                    _ => None,
                                });
                        if offsets.next() != Some(semantic_vocabulary::IntegerValue::Unsigned(0))
                            || offsets.next().is_some()
                        {
                            return Err(invalid());
                        }
                        terminal_semantics::representation_view_declaration(
                            types.iter(),
                            referent,
                            storage.stored_scalar,
                        )
                        .ok_or_else(invalid)?;
                        root = root.projected(&[StructuralPathSegment::RepresentationView {
                            structural_type: referent,
                            stored_scalar: storage.stored_scalar,
                        }]);
                    }
                    let (backing, backing_parents) = descriptor_backing::capture(
                        function,
                        custody,
                        types,
                        source,
                        *psi_operation,
                        &root,
                    )?;
                    let mut leaf = Leaf::formed(
                        (result.place, Vec::new()),
                        root,
                        parents,
                        *psi_operation,
                        result,
                        referent,
                    );
                    leaf.placed = placed_source(custody, source);
                    leaf.backing = backing;
                    leaf.parents.extend(backing_parents);
                    leaf.parents.sort();
                    leaf.parents.dedup();
                    leaf
                };
            custody.leaves.insert((result.place, Vec::new()), leaf);
        }
        AbstractOperation::ReleaseReference {
            psi_operation,
            source,
            path,
            ..
        } => {
            if custody.leaves.values().any(|enclosing| {
                enclosing.root.origins().iter().any(|origin| {
                    let origin_path = &origin.path;
                    origin.root == *source
                        && source_contract(function, custody, *source).and_then(|contract| {
                            terminal_semantics::captured_reference_origin_type(
                                types.iter(),
                                contract,
                                origin_path,
                            )
                        }) == Some(enclosing.referent)
                        && terminal_semantics::reference_origin_paths_may_overlap(
                            origin_path,
                            &path
                                .iter()
                                .cloned()
                                .map(ReferenceOriginProjection::Exact)
                                .collect::<Vec<_>>(),
                        )
                })
            }) {
                return Err(invalid());
            }
            let key = (*source, path.clone());
            let leaf = custody.leaves.remove(&key);
            let Some(leaf) = leaf else {
                return Err(invalid());
            };
            custody.retire(leaf, retirements::DisposalEvent::Operation(*psi_operation))?;
        }
        AbstractOperation::EstablishRecord { result, fields, .. }
        | AbstractOperation::EstablishCase { result, fields, .. } => {
            if custody.leaves.keys().any(|(carrier, path)| {
                *carrier == result.place && owned_relative_path(path, &[]).is_some()
            }) {
                return Err(invalid());
            }
            let shape = &find_structural_type(types, result.structural_type)
                .ok_or(invalid())?
                .shape;
            let selected_case = match operation {
                AbstractOperation::EstablishCase { result_case, .. } => Some(*result_case),
                _ => None,
            };
            let declarations = shape
                .direct_fields()
                .into_iter()
                .flatten()
                .map(|field| (None, field))
                .chain(
                    shape
                        .sum_cases()
                        .into_iter()
                        .flatten()
                        .filter(|case| Some(case.id) == selected_case)
                        .flat_map(|case| {
                            case.fields.iter().map(move |field| (Some(case.id), field))
                        }),
                )
                .filter(|(_, field)| !field.relevance.is_erased());
            if fields.len() != declarations.clone().count() {
                return Err(invalid());
            }
            let mut operands = Vec::new();
            for (field, (case, declaration)) in fields.iter().zip(declarations) {
                if field.field != declaration.id {
                    return Err(invalid());
                }
                if let (
                    terminal_psi::RecordFieldValue::Structural(argument),
                    StructuralFieldType::Structural(nested),
                ) = (&field.value, &declaration.field_type)
                {
                    let mut prefix = Vec::new();
                    if let Some(case) = case {
                        prefix.push(StructuralPathSegment::Case(case));
                    }
                    prefix.push(StructuralPathSegment::Field(declaration.identity.clone()));
                    operands.push((argument, *nested, prefix));
                }
            }
            relocate_subtree_leaves(
                custody,
                function,
                types,
                result.place,
                operation.psi_operation().ok_or_else(|| invalid())?,
                operands,
            )?;
        }
        AbstractOperation::EstablishStructuralArray {
            result, elements, ..
        } => {
            if custody.leaves.keys().any(|(carrier, path)| {
                *carrier == result.place && owned_relative_path(path, &[]).is_some()
            }) {
                return Err(invalid());
            }
            let Some(StructuralTypeDeclaration {
                shape: StructuralTypeShape::FixedArray { element, length },
                ..
            }) = find_structural_type(types, result.structural_type)
            else {
                return Err(invalid());
            };
            if u64::try_from(elements.len()).ok() != Some(*length) {
                return Err(invalid());
            }
            relocate_subtree_leaves(
                custody,
                function,
                types,
                result.place,
                operation.psi_operation().ok_or_else(|| invalid())?,
                elements.iter().enumerate().map(|(index, argument)| {
                    (
                        argument,
                        *element,
                        vec![StructuralPathSegment::FixedIndex(index as u64)],
                    )
                }),
            )?;
        }
        AbstractOperation::MoveStructuralField {
            result,
            source,
            path,
            field,
            ..
        } => {
            let extent = super::borrowed_windows::moved(
                function, source, path, *field, result, plan, custody,
            )?;
            if !contains_reference(types, result.structural_type) {
                return Ok(());
            }
            let (place, path) = window_storage(
                function,
                custody,
                types,
                source.place(),
                &extent.path,
                StructuralAccess::MutableBorrow,
            )?;
            let argument = StructuralArgument {
                place,
                path,
                access: StructuralAccess::Owned,
            };
            relocate_subtree_leaves(
                custody,
                function,
                types,
                result.place,
                operation.psi_operation().ok_or_else(|| invalid())?,
                [(&argument, result.structural_type, Vec::new())],
            )?;
        }
        AbstractOperation::StoreStructuralField {
            disposition,
            destination,
            path,
            field,
            value,
            ..
        } => {
            let extent = super::borrowed_windows::extent(
                function,
                destination,
                path,
                *field,
                plan,
                custody,
                disposition.required_access(),
            )?;
            if !contains_reference(types, extent.field_type) {
                return Ok(());
            }
            let (place, path) = window_storage(
                function,
                custody,
                types,
                destination.place(),
                &extent.path,
                disposition.required_access(),
            )?;
            if *disposition == terminal_psi::StructuralStoreDisposition::ReplaceDiscardable {
                // End this descriptor's residency; children retain the exact
                // old occurrence, never the replacement in the same slot.
                discard_subtree(
                    function,
                    custody,
                    types,
                    place,
                    &path,
                    retirements::DisposalEvent::Operation(
                        operation.psi_operation().ok_or_else(|| invalid())?,
                    ),
                )?;
            }
            relocate_subtree_leaves(
                custody,
                function,
                types,
                place,
                operation.psi_operation().ok_or_else(|| invalid())?,
                [(value, extent.field_type, path)],
            )?;
        }
        AbstractOperation::CallStructural {
            psi_operation,
            result: AbstractStructuralCallResult::Structural { result, .. },
            callee,
            structural_arguments,
            ..
        } => {
            let (callee_function, result_contract) = match callee {
                abstract_operations::AbstractCallTarget::Machine(machine) => {
                    let callee = unit
                        .functions
                        .iter()
                        .find(|function| function.machine == *machine)
                        .ok_or_else(invalid)?;
                    let result = callee.result.structural().ok_or_else(invalid)?;
                    (
                        Some(callee),
                        terminal_semantics::ReferenceCallContract::Machine {
                            parameters: &callee.structural_parameters,
                            sources: &result.reference_sources,
                        },
                    )
                }
                abstract_operations::AbstractCallTarget::Dynamic { .. } => {
                    let abstract_operations::AbstractCallSignature::Requirement(requirement) =
                        super::call_signature::signature(*callee, plan)?
                    else {
                        return Err(invalid());
                    };
                    if !requirement.matches_operation_result(
                        &terminal_psi::OperationResult::Structural(result.clone()),
                    ) {
                        return Err(invalid());
                    }
                    (
                        None,
                        terminal_semantics::ReferenceCallContract::Requirement(requirement),
                    )
                }
            };
            call_transition(
                function,
                callee_function,
                Some(result_contract),
                types,
                Some(result),
                *psi_operation,
                structural_arguments,
                custody,
            )?;
        }
        AbstractOperation::CallStructural {
            psi_operation,
            callee,
            structural_arguments,
            result: AbstractStructuralCallResult::Unit | AbstractStructuralCallResult::Scalar(_),
            ..
        } => {
            let callee_function = callee.machine().and_then(|machine| {
                unit.functions
                    .iter()
                    .find(|function| function.machine == machine)
            });
            call_transition(
                function,
                callee_function,
                None,
                types,
                None,
                *psi_operation,
                structural_arguments,
                custody,
            )?;
        }
        AbstractOperation::BoundaryCall {
            structural_arguments,
            ..
        } => {
            // A non-retaining foreign borrow has ordinary live-loan custody.
            // The opaque counterparty cannot receive Omega reference carriers
            // by ownership; its separately evaluated ABI owns pointer geometry.
            if !call_arguments(function, types, custody, structural_arguments)?.is_empty() {
                return Err(invalid());
            }
        }
        _ => {}
    }
    custody.prune_retired();
    custody.validate_occurrences()?;
    Ok(())
}

/// A structural return whose source carries reference leaves must match the
/// declared result paths: every resident leaf descends directly from a checked
/// formal origin. The declaration lists possible case slots, not simultaneous
/// permissions; inactive payloads owe no descriptor. Establishment and transfer
/// reconstruct the resident roster before this independent return audit.
fn transfer_return_custody(
    custody: &mut Custody,
    function: &PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
    source: PlaceId,
    result: &terminal_psi::StructuralResultDeclaration,
    cleanup_actions: &[terminal_psi::TerminalAffineCleanupAction],
) -> Result<(), LegalizationError> {
    if !contains_reference(types, result.structural_type) {
        return Err(invalid());
    }
    let contract = source_contract(function, custody, source).ok_or_else(|| invalid())?;
    if contract != result.structural_type {
        return Err(invalid());
    }
    for ((carrier, path), leaf) in &custody.leaves {
        if *carrier != source || owned_relative_path(path, &[]).is_none() {
            continue;
        }
        let mapping = result
            .reference_sources
            .iter()
            .find(|mapping| mapping.path == *path)
            .ok_or_else(|| invalid())?;
        let access = leaf_type(types, leaf.contract.structural_type(), &leaf.identity.path)
            .and_then(|identity| find_structural_type(types, identity))
            .and_then(|declaration| match declaration.shape {
                StructuralTypeShape::Reference { access, .. } => Some(access),
                _ => None,
            })
            .ok_or_else(invalid)?;
        let declared = |root: &Root| {
            root.origins().iter().all(|origin| {
                if let target_operations::TargetReferenceOriginKind::Invocation { carrier_type } = origin.kind {
                    return mapping.sources.contains(&terminal_psi::StructuralReferenceOrigin::Invocation {
                        carrier_type, path: origin.path.clone(), extent: origin.extent, access,
                    });
                }
                let Some(parameter) = function.structural_parameter(origin.root) else {
                    return false;
                };
                // Summarize a captured selection without replaying its index or
                // expanding an input collection into a finite candidate roster.
                let mut path = origin.path.iter().map(|projection| match projection {
                    ReferenceOriginProjection::Exact(StructuralPathSegment::RuntimeIndex { .. }) =>
                        ReferenceOriginProjection::Element,
                    _ => projection.clone(),
                }).collect::<Vec<_>>();
                let Some(endpoint) = terminal_semantics::reference_origin_type(
                    types.iter(), parameter.structural_type, &path,
                ) else { return false; };
                if origin.extent == semantic_vocabulary::ReferenceOriginExtent::Projected && endpoint != leaf.referent {
                    if !find_structural_type(types, endpoint).is_some_and(|declaration|
                        matches!(declaration.shape, StructuralTypeShape::Reference { referent, access: supplied }
                            if referent == leaf.referent && access_covers(supplied, access)))
                    { return false; }
                    path.push(ReferenceOriginProjection::Exact(StructuralPathSegment::Referent));
                }
                let actual = terminal_psi::ReferenceOriginSource {
                    place: origin.root, path, access,
                    extent: origin.extent,
                };
                // Supplying permission is attenuated to the requested view;
                // the complete declared roster still covers its geometry.
                let supplying = mapping.sources.iter()
                    .filter_map(terminal_psi::StructuralReferenceOrigin::parameter)
                    .filter(|allowed| access_covers(allowed.access, actual.access))
                    .map(|allowed| terminal_psi::ReferenceOriginSource {
                        access: actual.access,
                        ..allowed.clone()
                    })
                    .collect::<Vec<_>>();
                terminal_semantics::reference_origin_is_covered(
                    types.iter(), parameter.structural_type, &actual, &supplying,
                )
            })
        };
        let admitted = declared(&leaf.root);
        if !admitted && !descriptor_backing::matches_return(&leaf.backing, &declared) {
            return Err(invalid());
        }
        if custody
            .leaves
            .values()
            .any(|child| custody.descends_from(child, &leaf.identity))
        {
            return Err(invalid());
        }
        // Receiving parameters are actual declarations, not synthetic result
        // rows. Every possible transported formation retains its real owner.
        if !leaf
            .contract
            .matches_declaration(function, leaf.identity.place)
            || leaf.formations.is_empty()
            || leaf.formations.iter().any(|formation| {
                !formation
                    .contract
                    .matches_declaration(function, formation.identity.place)
            })
        {
            return Err(invalid());
        }
    }
    // Output is already formed. Its original storage and admitted view chain
    // were matched above; the signature never supplies an origin. Close only
    // local ancestry with actual disposal on this path, including earlier
    // replacements. Residency selects return/cleanup; identity selects ancestry.
    // Case alternatives of one carrier are one residency (validated with the
    // custody): it is returned by any alternative and disposed only when every
    // alternative is disposed.
    let mut occurrences = Vec::<terminal_semantics::ReferenceCompletionOccurrence<'_, _>>::new();
    for ((carrier, path), leaf) in &custody.leaves {
        let returned = *carrier == source && owned_relative_path(path, &[]).is_some();
        let disposed = function
            .structural_parameter(*carrier)
            .is_none_or(|parameter| parameter.access == StructuralAccess::Owned)
            && terminal_semantics::cleanup_disposes_reference(cleanup_actions, *carrier, path);
        if let Some(alternative) = occurrences
            .iter_mut()
            .find(|occurrence| *occurrence.identity == leaf.identity)
        {
            alternative.returned |= returned;
            alternative.disposed &= disposed;
            continue;
        }
        occurrences.push(terminal_semantics::ReferenceCompletionOccurrence {
            identity: &leaf.identity,
            parents: leaf.parents.iter().collect(),
            returned,
            disposed,
        });
    }
    occurrences.extend(custody.retired.values().map(|retired| {
        terminal_semantics::ReferenceCompletionOccurrence {
            identity: &retired.leaf.identity,
            parents: retired.leaf.parents.iter().collect(),
            returned: false,
            // These are replayed disposal operations on this exact path, not
            // missing residents inferred to have been disposed at some point.
            disposed: !retired.disposals.is_empty(),
        }
    }));
    terminal_semantics::validate_reference_result_completion(&occurrences)
        .map_err(|reason| LegalizationError::ReferenceCompletion { reason })?;
    custody.leaves.retain(|(carrier, path), _| {
        *carrier != source || owned_relative_path(path, &[]).is_none()
    });
    custody.prune_retired();
    Ok(())
}

/// Apply the custody effects of a terminator's edge-local discards, in the
/// same cumulative edge order the lowering's cleanup pass uses.
pub(in crate::legalization) fn apply_terminator(
    custody: &mut Custody,
    operation: &AbstractOperation,
    function: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
) -> Result<(), LegalizationError> {
    let types = &plan.structural_types[..];
    if let AbstractOperation::ReturnStructural {
        source,
        cleanup_actions,
        ..
    } = operation
        && let Some(result) = function.result.structural()
        && contains_reference(types, result.structural_type)
    {
        transfer_return_custody(custody, function, types, *source, result, cleanup_actions)?;
    }
    let mut exits = Vec::new();
    for (edge, actions) in operation.cleanup_rosters() {
        let owned_transfer = operation.owned_transfer(edge);
        let target = operation.successor_target(edge);
        if let Some(target) = target {
            let mut exit = custody.clone();
            if let Some((target, bindings)) = owned_transfer {
                let destination = function.block(target).ok_or_else(invalid)?;
                for binding in bindings {
                    if binding
                        .argument
                        .path
                        .contains(&StructuralPathSegment::Referent)
                    {
                        // Type geometry cannot authenticate a pointer crossing.
                        // Rejoin the live carrier dependencies before cleanup
                        // or arrival storage changes their locations.
                        call_paths::call_reference_custody(
                            function,
                            &exit,
                            types,
                            &binding.argument,
                        )?;
                    }
                }
                re_home_bound_leaves(
                    &mut exit,
                    bindings,
                    &destination.structural_parameters,
                    types,
                )?;
            }
            apply_cleanup_actions(
                &mut exit,
                actions.iter().cloned(),
                function,
                types,
                retirements::DisposalEvent::Edge(edge),
            )?;
            exits.push((edge, target, exit.leaves, exit.retired));
        } else {
            apply_cleanup_actions(
                custody,
                actions.iter().cloned(),
                function,
                types,
                retirements::DisposalEvent::Edge(edge),
            )?;
        }
    }
    if !exits.is_empty() {
        custody.edge_exits = exits;
    }
    Ok(())
}

fn apply_cleanup_actions(
    custody: &mut Custody,
    actions: impl IntoIterator<Item = terminal_psi::TerminalAffineCleanupAction>,
    function: &PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
    event: retirements::DisposalEvent,
) -> Result<(), LegalizationError> {
    use terminal_psi::TerminalAffineCleanupAction;
    for action in actions {
        let place = match action {
            TerminalAffineCleanupAction::RetireHome(place) => {
                if source_contract(function, custody, place).is_none() {
                    return Err(invalid());
                }
                // The checked partition has disposed resident obligations.
                // Signature slots for inactive cases can remain in this
                // conservative roster after the active payload moved. Retire
                // only inline payload. External storage remains anchored to
                // ingress even after its pointer carrier is no longer resident.
                custody.leaves.retain(|(carrier, path), _| {
                    *carrier != place || owned_relative_path(path, &[]).is_none()
                });
                continue;
            }
            TerminalAffineCleanupAction::DiscardRoot(place) => place,
            TerminalAffineCleanupAction::DiscardResidual(discard) => {
                discard_residual(function, custody, types, &discard, event)?;
                continue;
            }
            TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                if !cleanup.requirement_obligations.is_empty()
                    || source_contract(function, custody, cleanup.place)
                        .and_then(|root| leaf_type(types, root, &cleanup.path))
                        != Some(cleanup.structural_type)
                    || storage_access_conflicts(
                        function,
                        custody,
                        types,
                        cleanup.place,
                        &cleanup.path,
                        StructuralAccess::Owned,
                    )
                {
                    return Err(invalid());
                }
                // The borrowed call retains the selected original storage;
                // completing it retires only this subject's contained loans.
                discard_subtree(
                    function,
                    custody,
                    types,
                    cleanup.place,
                    &cleanup.path,
                    event,
                )?;
                continue;
            }
        };
        // A suspended referent root cannot be discarded while its loan lives.
        if storage_access_conflicts(
            function,
            custody,
            types,
            place,
            &[],
            StructuralAccess::Owned,
        ) {
            return Err(LegalizationError::ReferenceDisposalConflict {
                machine: function.machine,
                place,
            });
        }
        discard_owned(function, custody, types, place, event)?;
    }
    custody.prune_retired();
    custody.validate_occurrences()?;
    Ok(())
}

/// Reconstruct borrowed state addresses from exact incoming edge bindings.
/// Missing or unsupported origins remain explicit; they never become local roots.
fn borrowed_sources(
    function: &PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
) -> std::sync::Arc<BTreeMap<PlaceId, Option<Vec<StructuralArgument>>>> {
    let sources = function
        .block_structural_parameters()
        .filter(|parameter| {
            parameter.access != StructuralAccess::Owned
                && !terminal_semantics::descriptor_results::is_descriptor(
                    types.iter(),
                    parameter.structural_type,
                )
        })
        .map(|parameter| (parameter.place, None))
        .collect();
    std::sync::Arc::new(sources)
}

/// Compute entries from actual incoming edges, never dominator storage alone.
/// Each changed selected arrival replays its dependent block. Owned parameter
/// bindings and independently established resident slots select their current
/// occurrence without forgetting predecessor formations. Final entries equal
/// the complete merge of every incoming edge.
pub(in crate::legalization) fn block_entry_states(
    function: &PsiOptimizationFunction,
    native: &target_operations::TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<BTreeMap<BlockId, Custody>, LegalizationError> {
    let types = &plan.structural_types[..];
    let blocks = &function.blocks;
    let positions: BTreeMap<BlockId, usize> = blocks
        .iter()
        .enumerate()
        .map(|(position, block)| (block.id, position))
        .collect();
    let entry_position = *positions.get(&function.entry).ok_or_else(|| invalid())?;
    if positions.len() != blocks.len() {
        return Err(invalid());
    }
    // Independently reconstruct which source storage is available on entry.
    // A branch-local descriptor cannot require an unrelated arm to recreate
    // it. Receiving parameters are available at their own block's entry;
    // operation results require a strict dominating definition.
    let mut predecessors: BTreeMap<_, BTreeSet<_>> = blocks
        .iter()
        .map(|block| (block.id, BTreeSet::new()))
        .collect();
    let mut storage_definitions = BTreeMap::new();
    for block in blocks {
        for parameter in &block.structural_parameters {
            storage_definitions.insert(parameter.place, (block.id, true));
        }
        for node in &block.nodes {
            if let Some((_, result)) = node.operation.structural_result() {
                storage_definitions.insert(result.place, (block.id, false));
            }
            for edge in &node.successors {
                predecessors
                    .get_mut(&edge.target)
                    .ok_or(invalid())?
                    .insert(block.id);
            }
        }
    }
    let dominators = optimization_unit::dominators(
        function.entry,
        blocks.iter().map(|block| block.id),
        &predecessors,
    );
    let borrowed_sources = borrowed_sources(function, types);
    let mut initial = entry(function, types)?;
    initial.borrowed_sources = borrowed_sources;
    initial.placement_catalog = plan.structural_types.clone();
    for input in plan
        .structural_types
        .placed_view_inputs()
        .iter()
        .filter(|input| input.machine == function.machine)
    {
        let parameter = function
            .structural_parameters
            .get(input.runtime_position as usize)
            .ok_or_else(invalid)?;
        let (matched, _) = plan
            .structural_types
            .placed_parameter_demand(function.machine, input.runtime_position as usize, parameter)
            .ok_or_else(invalid)?;
        if matched != input
            || initial
                .placed_formals
                .insert(parameter.place, input.clone())
                .is_some()
        {
            return Err(invalid());
        }
    }
    initial.descriptor_backing =
        std::sync::Arc::new(descriptor_backing::prepare(function, native, plan, unit)?);
    initial.stored_calls = std::sync::Arc::new(stored_calls::prepare(native, plan, unit));
    let mut entries = BTreeMap::from([(function.entry, initial)]);
    let mut pending = VecDeque::from([entry_position]);
    let mut queued = vec![false; blocks.len()];
    queued[entry_position] = true;
    let mut final_arrivals = vec![Vec::new(); blocks.len()];
    let mut deferred = BTreeSet::new();
    while let Some(position) = pending.pop_front() {
        queued[position] = false;
        let block = &blocks[position];
        let mut custody = entries.get(&block.id).cloned().ok_or_else(invalid)?;
        for node in &blocks[position].nodes {
            if matches!(
                node.operation,
                abstract_operations::return_or_return_unit_pattern!(..)
                    | AbstractOperation::ReturnStructural { .. }
                    | AbstractOperation::Jump { .. }
                    | AbstractOperation::Conditional { .. }
                    | AbstractOperation::StructuralCase { .. }
                    | AbstractOperation::ExternalCompletion { .. }
            ) {
                apply_terminator(&mut custody, &node.operation, function, plan)?;
            } else {
                apply(&mut custody, node, function, native, plan, unit).map_err(|error| {
                    error.attributed(function.machine, node.operation.psi_operation())
                })?;
            }
        }
        let terminator = block.nodes.last().ok_or_else(invalid)?;
        let mut arrivals = Vec::with_capacity(terminator.successors.len());
        for edge in &terminator.successors {
            let target_position = *positions.get(&edge.target).ok_or_else(invalid)?;
            if target_position == entry_position {
                return Err(invalid());
            }
            let mut arrival = custody.edge_custody(edge.psi_edge, edge.target)?;
            // Case payload establishment has its own transport; ordinary owned
            // bindings were already captured before their edge cleanup.
            if matches!(
                terminator.operation,
                AbstractOperation::StructuralCase { .. }
            ) {
                re_home_bound_leaves(
                    &mut arrival,
                    &edge.structural_bindings,
                    &blocks[target_position].structural_parameters,
                    types,
                )?;
            }
            borrowed_arrivals::bind(
                function,
                &custody,
                &mut arrival,
                &edge.structural_bindings,
                &blocks[target_position].structural_parameters,
                types,
            )?;
            let scope = dominators.get(&edge.target).ok_or(invalid())?;
            arrival.leaves.retain(|(carrier, path), _| {
                owned_relative_path(path, &[]).is_none()
                    || storage_definitions
                        .get(carrier)
                        .is_none_or(|(definition, parameter)| {
                            scope.contains(definition) && (*parameter || *definition != edge.target)
                        })
            });
            arrival.prune_retired();
            arrival.validate_occurrences()?;
            arrivals.push((edge.psi_edge, edge.target, arrival));
        }
        final_arrivals[position] = arrivals;
        let targets: BTreeSet<_> = final_arrivals[position]
            .iter()
            .map(|(_, target, _)| *target)
            .collect();
        for target in targets {
            // Compare current predecessor outputs with one another, not with
            // an older provisional entry. Upstream selection may enlarge the
            // roster even when this block has just one predecessor.
            let incoming = final_arrivals
                .iter()
                .flatten()
                .filter(|(_, candidate, _)| *candidate == target)
                .map(|(_, _, arrival)| arrival.clone());
            let target_position = *positions.get(&target).ok_or_else(invalid)?;
            let Some(complete) = resident_joins::merge(function, target, types, incoming)? else {
                // Keep the previous state provisional while other predecessors
                // propagate. Never replay a queued stale, nonselected roster.
                deferred.insert(target);
                pending.retain(|position| *position != target_position);
                queued[target_position] = false;
                continue;
            };
            let was_deferred = deferred.remove(&target);
            let changed = entries.get(&target).is_none_or(|entry| {
                entry.leaves != complete.leaves
                    || entry.retired != complete.retired
                    || entry.borrowed_sources != complete.borrowed_sources
            });
            if changed {
                entries.insert(target, complete);
            }
            if (changed || was_deferred) && !queued[target_position] {
                pending.push_back(target_position);
                queued[target_position] = true;
            }
        }
    }
    if entries.len() != blocks.len() || !deferred.is_empty() {
        return Err(invalid());
    }
    for block in blocks.iter().filter(|block| block.id != function.entry) {
        let arrivals = final_arrivals
            .iter()
            .flatten()
            .filter(|(_, target, _)| *target == block.id)
            .map(|(_, _, arrival)| arrival.clone());
        let complete =
            resident_joins::merge(function, block.id, types, arrivals)?.ok_or_else(invalid)?;
        if entries.get(&block.id).is_none_or(|entry| {
            entry.leaves != complete.leaves
                || entry.retired != complete.retired
                || entry.borrowed_sources != complete.borrowed_sources
        }) {
            return Err(invalid());
        }
    }
    Ok(entries)
}

/// Owned-carrier ingress parameters seed one leaf per declared reference leaf.
/// Owned ingress assumes one existing permission per leaf.
fn entry(
    function: &PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
) -> Result<Custody, LegalizationError> {
    let mut custody = Custody::default();
    for parameter in &function.structural_parameters {
        if !contains_reference(types, parameter.structural_type) {
            continue;
        }
        // Reconstruct descriptor permissions independently of the carrier's
        // owned claims and refinements. Borrowing lends this roster without
        // transferring its permissions; each leaf keeps its exact ingress.
        let paths = leaf_paths(
            types,
            parameter.structural_type,
            4096usize.saturating_sub(custody.leaves.len()),
        )
        .ok_or_else(|| invalid())?;
        for path in paths {
            let identity = (parameter.place, path.clone());
            custody.leaves.insert(
                identity.clone(),
                Leaf {
                    identity: OccurrenceIdentity::parameter(&identity),
                    root: Root::Ingress(parameter.place, path.clone()),
                    backing: vec![descriptor_backing::Backing::Source(Root::Ingress(
                        parameter.place,
                        path.clone(),
                    ))],
                    parents: Vec::new(),
                    contract: CarrierContract::Parameter(parameter.clone()),
                    formations: vec![Formation {
                        identity: OccurrenceIdentity::parameter(&identity),
                        contract: CarrierContract::Parameter(parameter.clone()),
                    }],
                    referent: leaf_referent(types, parameter.structural_type, &path)
                        .ok_or_else(|| invalid())?,
                    placed: None,
                },
            );
        }
    }
    Ok(custody)
}

/// Transport the live reference leaf's address word from its original carrier.
/// Custody proves the loan; its possible origins never select runtime pointer bits.
#[allow(clippy::too_many_arguments)]
pub(in crate::legalization) fn referent_argument(
    argument: &StructuralArgument,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    destination: &calling_conventions::ValuePlacement,
    caller: &PsiOptimizationFunction,
    native: &target_operations::TargetOperationPlan,
    call_operation: semantic_vocabulary::OperationId,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
) -> Result<target_operations::TargetStructuralArgument, LegalizationError> {
    if argument.access == StructuralAccess::Owned
        || argument.access != declaration.access
        || declaration.multiplicity != StructuralMultiplicity::Unrestricted
    {
        return Err(invalid());
    }
    call_paths::call_reference_custody(caller, custody, types, argument)?;
    let root_type = source_contract(caller, custody, argument.place).ok_or(invalid())?;
    let projection = crate::structural_inputs::structural_reference_input::reference_projection(
        root_type,
        &argument.path,
        argument.access,
        Some(declaration.structural_type),
        types,
    )
    .ok_or(invalid())?;
    let selected = projection.selected;
    let source_byte_offset = projection.referent_offset;
    if selected != declaration.structural_type
        || Some(destination.shape)
            != crate::structural_inputs::structural_reference_input::parameter_shape(
                declaration,
                types,
            )
    {
        return Err(invalid());
    }
    let target_caller = native.function(caller.machine).ok_or(invalid())?;
    let source = if let Some(parameter) = caller.structural_parameter(argument.place) {
        let retained = super::structural_parameters(target_caller)
            .and_then(|parameters| {
                parameters
                    .iter()
                    .find(|parameter| parameter.place == argument.place)
            })
            .ok_or(invalid())?;
        if parameter.access == StructuralAccess::WriteOnlyBorrow
            || retained.structural_type != root_type
        {
            return Err(invalid());
        }
        retained.placement.clone().into()
    } else if let Some((block, parameter)) = caller.blocks.iter().find_map(|block| {
        block
            .structural_parameter(argument.place)
            .map(|parameter| (block.id, parameter))
    }) {
        if parameter.access != StructuralAccess::Owned
            || parameter.multiplicity == StructuralMultiplicity::Linear
        {
            return Err(invalid());
        }
        target_operations::TargetStructuralArgumentSource::BlockParameter {
            block,
            place: argument.place,
        }
    } else {
        let (producer, result) = super::structural_case::source_result(caller, argument.place)?;
        if result.structural_type != root_type
            || result.multiplicity == StructuralMultiplicity::Linear
            || !result.qualification_establishments.is_empty()
            || !result.claims.is_empty()
        {
            return Err(invalid());
        }
        target_operations::TargetStructuralArgumentSource::StructuralHome {
            psi_operation: producer,
        }
    };
    Ok(target_operations::TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type: root_type,
        structural_type: selected,
        shape: destination.shape,
        source_byte_offset,
        fixed_array_length: None,
        element_stride: None,
        indices: super::target::control_flow::sources::call_indices(
            native,
            caller,
            target_operations::CallSiteOwner::Operation(call_operation),
            &projection.elements,
        )?,
        source,
        destination: destination.clone(),
    })
}

/// Compare a retained call row against the audit's independently reconstructed
/// custody transition. The state update and row comparison use the same join:
/// a second origin-only algorithm lost inactive input slots and could disagree
/// with the exact transfer it was supposed to audit. Caller storage stays
/// unchanged; the scratch transition never uses producer-owned target rows.
pub(in crate::legalization) fn reference_results(
    caller: &PsiOptimizationFunction,
    callee: Option<&PsiOptimizationFunction>,
    contract: terminal_semantics::ReferenceCallContract<'_>,
    arguments: &[StructuralArgument],
    result: &StructuralOperationResult,
    psi_operation: OperationId,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
) -> Result<Vec<target_operations::TargetReferenceResult>, LegalizationError> {
    let mut returned = custody.clone();
    call_transition(
        caller,
        callee,
        Some(contract),
        types,
        Some(result),
        psi_operation,
        arguments,
        &mut returned,
    )
}

/// Independently join the exact formal demand to the current live descriptor.
/// The ordinary observation validates access, active descendants and enclosing
/// loans first; a sealed plan never supplies an absent permission occurrence.
pub(in crate::legalization) fn placed_call_custody(
    caller: &PsiOptimizationFunction,
    callee: &PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
    arguments: &[StructuralArgument],
    custody: &Custody,
) -> Result<Vec<target_operations::TargetPlacedCallBinding>, LegalizationError> {
    let catalog = &custody.placement_catalog;
    let mut bindings = Vec::new();
    for (runtime_position, parameter) in callee.structural_parameters.iter().enumerate() {
        if !catalog.placed_view_inputs().iter().any(|input| {
            input.machine == callee.machine
                && usize::try_from(input.runtime_position).ok() == Some(runtime_position)
        }) {
            continue;
        }
        let (input, plan) = catalog
            .placed_parameter_demand(callee.machine, runtime_position, parameter)
            .ok_or_else(invalid)?;
        let argument = arguments
            .get(runtime_position)
            .filter(|argument| argument.access == parameter.access)
            .ok_or_else(invalid)?;
        let association = match placed_source(custody, argument) {
            Some(target_operations::TargetPlacedInterpretation::Captured(captured)) => {
                let observed =
                    call_paths::call_reference_custody(caller, custody, types, argument)?;
                if observed.referent != plan.carrier {
                    return Err(invalid());
                }
                let expected = terminal_semantics::placed_ram_plan_correspondence(
                    types,
                    catalog.placed_views(),
                    plan,
                    captured.schema,
                )
                .map_err(|_| invalid())?;
                if captured != expected {
                    return Err(invalid());
                }
                target_operations::TargetPlacedCallAssociation::Captured {
                    schema: captured.schema,
                    carrier: captured.carrier,
                }
            }
            Some(target_operations::TargetPlacedInterpretation::Formal(formal)) => {
                if !parameter.access.admits(formal.access)
                    || !custody
                        .placed_formals
                        .values()
                        .any(|input| input == &formal)
                {
                    return Err(invalid());
                }
                if argument.path.contains(&StructuralPathSegment::Referent) {
                    let observed =
                        call_paths::call_reference_custody(caller, custody, types, argument)?;
                    if observed.referent != plan.carrier {
                        return Err(invalid());
                    }
                }
                let source_parameter = caller
                    .structural_parameters
                    .get(formal.runtime_position as usize)
                    .ok_or_else(invalid)?;
                let (_, source_plan) = catalog
                    .placed_parameter_demand(
                        caller.machine,
                        formal.runtime_position as usize,
                        source_parameter,
                    )
                    .ok_or_else(invalid)?;
                if source_plan != plan {
                    return Err(invalid());
                }
                // Ordinary call replay below checks this parameter's live storage,
                // enclosing children and simultaneous operand interference.
                target_operations::TargetPlacedCallAssociation::CallerFormal(formal)
            }
            None => return Err(invalid()),
        };
        bindings.push(target_operations::TargetPlacedCallBinding {
            input: input.clone(),
            argument_index: input.runtime_position,
            association,
        });
    }
    Ok(bindings)
}

/// Both state replay and retained-row comparison use one complete call
/// transaction. Result mappings capture pre-call permissions; stored effects
/// then replace carrier bindings, with duplicate transfers checked together.
fn call_transition(
    caller: &PsiOptimizationFunction,
    callee: Option<&PsiOptimizationFunction>,
    result_contract: Option<terminal_semantics::ReferenceCallContract<'_>>,
    types: &[StructuralTypeDeclaration],
    result: Option<&StructuralOperationResult>,
    operation: OperationId,
    arguments: &[StructuralArgument],
    custody: &mut Custody,
) -> Result<Vec<target_operations::TargetReferenceResult>, LegalizationError> {
    if let Some(callee) = callee {
        placed_call_custody(caller, callee, types, arguments, custody)?;
    }
    let moved = call_arguments(caller, types, custody, arguments)?;
    let stored = stored_calls::stage(caller, callee, types, custody, arguments, operation, &moved)?;
    commit_argument_moves(custody, &moved);
    let results = match (result_contract, result) {
        (Some(contract), Some(result)) => call_results(
            caller, contract, types, result, operation, arguments, custody, &moved,
        )?,
        (None, None) => Vec::new(),
        _ => return Err(invalid()),
    };
    stored_calls::commit(
        caller,
        types,
        result.map(|result| result.place),
        custody,
        stored,
    )?;
    Ok(results)
}

/// A scalar accessor uses the current whole placed parent, independently of its field metadata.
pub(in crate::legalization) fn placed_scalar_custody(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> Result<target_operations::TargetPlacedInterpretation, LegalizationError> {
    let interpretation = placed_source(custody, source).ok_or(invalid())?;
    normalized_source(function, custody, types, source)?;
    Ok(interpretation)
}
