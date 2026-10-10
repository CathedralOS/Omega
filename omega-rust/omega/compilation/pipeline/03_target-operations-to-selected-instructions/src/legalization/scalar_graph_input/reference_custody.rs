//! Independent reference-custody replay over the optimized source graph.
//!
//! `scalar_graph_input` re-derives every loan the target rows must name: which
//! carrier leaf holds it, which referent root it suspends, and which operation
//! formed it. A carrier owns loan permission, never referent storage; roots
//! are reconstructed from operations and signatures, never from pointer bits.
//! This is the legalization-side mirror of the lowering custody map — the
//! semantic replay in `optimization-unit-semantics` already proved legality,
//! so this pass only needs enough state to rejoin `.., Referent` arguments and
//! declared result leaf rosters independently.
//! Selection retains one descriptor with every possible immediate parent.
//! Child access, transfer and disposal test membership in that complete roster;
//! roots describe possible storage, never a runtime address to choose here.
//! Shared descriptor copies retain that roster without consuming the source:
//! each new carrier has its own release identity but the same immediate parents.
//! Owned edges select an actual receiving parameter occurrence. Its admitted
//! formations, roots and parents converge before entries are returned; ordinary
//! surviving leaves still agree exactly on every arrival, including backedges.
//! Ingress slots retain Case/Field guards, not simultaneous grants for inactive
//! alternatives. Construction and transport use resident descriptors. Source
//! dominance scopes storage at joins before comparing surviving custody; this
//! excludes unavailable branch-local homes, never a missing surviving loan.
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
    StructuralAccess, StructuralArgument, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape, find_structural_type,
};

mod occurrences;
use occurrences::{CarrierContract, Formation, merge_arrivals, re_home_bound_leaves};

/// One outstanding permission occurrence: its referent roots, immediate
/// parents, actual receiving contract and every transported formation.
/// `root` is compile-time custody — it never names a pointer or descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::legalization) struct Leaf {
    /// Current permission occurrence. Owned edge selection rebinds it to the
    /// receiving parameter; formations retain every admitted source identity.
    pub(in crate::legalization) identity: (PlaceId, Vec<StructuralPathSegment>),
    /// Referent root resolved through custody: a primitive place directly, or
    /// an owned ingress parameter's leaf path when custody entered owned.
    pub(in crate::legalization) root: Root,
    /// All possible immediate parent identities; selection suspends each parent.
    pub(in crate::legalization) parents: Vec<(PlaceId, Vec<StructuralPathSegment>)>,
    /// The actual operation or parameter declaration of the current occurrence.
    contract: CarrierContract,
    /// All admitted formations transported to this occurrence.
    formations: Vec<Formation>,
    /// This leaf's primitive referent type.
    pub(in crate::legalization) referent: StructuralTypeId,
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
    pub(in crate::legalization) fn origins(&self) -> Vec<target_operations::TargetReferenceOrigin> {
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

    pub(in crate::legalization) fn contains_place(&self, place: PlaceId) -> bool {
        match self {
            Self::Place(root) | Self::Ingress(root, _) => *root == place,
            Self::Alternatives(origins) => origins.iter().any(|origin| origin.root == place),
        }
    }
}

fn roots_overlap(left: &Root, right: &Root) -> bool {
    let left = left.origins();
    let right = right.origins();
    left.iter().any(|left| {
        right.iter().any(|right| {
            left.root == right.root
                && terminal_semantics::structural_paths_may_overlap(&left.path, &right.path)
        })
    })
}

/// Live reference loans keyed by current carrier location, exactly mirroring
/// the lowering-time custody map's membership.
#[derive(Debug, Clone, Default)]
pub(in crate::legalization) struct Custody {
    leaves: BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
    edge_exits: Vec<(
        EdgeId,
        BlockId,
        BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
    )>,
    borrowed_sources: std::sync::Arc<BTreeMap<PlaceId, Option<Vec<StructuralArgument>>>>,
}

impl Custody {
    /// Arrival uses this exact edge's post-transfer, post-cleanup roster.
    /// A block declaration or a dominator's survivors cannot restore a leaf.
    fn edge_custody(&self, edge: EdgeId, target: BlockId) -> Result<Self, LegalizationError> {
        let mut matching = self
            .edge_exits
            .iter()
            .filter(|(candidate, block, _)| *candidate == edge && *block == target);
        let (_, _, leaves) = matching.next().ok_or_else(invalid)?;
        if matching.next().is_some() {
            return Err(invalid());
        }
        Ok(Self {
            leaves: leaves.clone(),
            edge_exits: Vec::new(),
            borrowed_sources: self.borrowed_sources.clone(),
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
            other.root.contains_place(place) || other.parents.contains(&leaf.identity)
        }))
        .then_some(leaf)
    }
}

/// Live permission and referent-relative geometry behind a materialized carrier.
/// Candidate roots constrain authority; its pointer payload selects the address.
pub(in crate::legalization) struct WritableReferentDestination {
    pub(in crate::legalization) carrier_type: StructuralTypeId,
    pub(in crate::legalization) referent: StructuralTypeId,
    pub(in crate::legalization) path: Vec<StructuralPathSegment>,
}

fn materialized_mutable_record_projection(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    carrier: PlaceId,
    path: &[StructuralPathSegment],
) -> Option<WritableReferentDestination> {
    let (StructuralPathSegment::Referent, tail) = path.split_first()? else {
        return None;
    };
    let leaf = custody.leaf(carrier, &[])?;
    // Formation provenance survives transport; the current declaration
    // supplies carrier custody, not authority over the loan's backing.
    let carrier_type =
        optimization_unit_semantics::unqualified_owned_reference_carrier(function, carrier)?;
    let StructuralTypeShape::Reference {
        referent,
        access: StructuralAccess::MutableBorrow,
    } = &find_structural_type(types, carrier_type)?.shape
    else {
        return None;
    };
    // A whole `Referent` path is also the established primitive-reference
    // spelling. Only a record referent may enter this aggregate-origin route.
    if !matches!(
        &find_structural_type(types, *referent)?.shape,
        StructuralTypeShape::Record { .. }
    ) {
        return None;
    }
    if *referent != leaf.referent
        || custody
            .leaves
            .values()
            .any(|child| child.parents.contains(&leaf.identity))
    {
        return None;
    }
    // The retained formation has already authenticated backing and view
    // interpretation. Origin type equality would erase byte-backed views.
    Some(WritableReferentDestination {
        carrier_type,
        referent: *referent,
        path: tail.to_vec(),
    })
}

pub(in crate::legalization) fn writable_referent_destination(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    carrier: PlaceId,
    path: &[StructuralPathSegment],
) -> Option<WritableReferentDestination> {
    materialized_mutable_record_projection(function, custody, types, carrier, path)
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

/// Reconstruct shared referents by declared identity, not their owned contents.
/// Writable aggregates still need their separate restoration contract below.
fn referent(
    types: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    let declaration = find_structural_type(types, structural_type)?;
    match declaration.shape {
        StructuralTypeShape::Reference {
            referent,
            access: StructuralAccess::SharedBorrow,
        } => Some(referent),
        StructuralTypeShape::Reference {
            referent,
            access: StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow,
        } if matches!(
            find_structural_type(types, referent).map(|declaration| &declaration.shape),
            Some(StructuralTypeShape::PrimitiveScalar(_))
        ) =>
        {
            Some(referent)
        }
        _ => None,
    }
}

/// The aggregate bare-reference carrier native lowering realizes: a shared,
/// mutable or write-only borrow of a plain record. Its referent contains
/// storage, not another loan roster, and the carrier owns no record home.
fn plain_record_referent(
    types: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    let StructuralTypeShape::Reference {
        referent,
        access:
            StructuralAccess::SharedBorrow
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow,
    } = &find_structural_type(types, structural_type)?.shape
    else {
        return None;
    };
    matches!(
        find_structural_type(types, *referent).map(|declaration| &declaration.shape),
        Some(StructuralTypeShape::Record { .. })
    )
    .then_some(*referent)
    .filter(|referent| !contains_reference(types, *referent))
}

/// A field-only carrier path ending at a reference leaf, then its referent;
/// a non-reference leaf names its own storage type.
fn leaf_referent(
    types: &[StructuralTypeDeclaration],
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
    match &find_structural_type(types, leaf)?.shape {
        StructuralTypeShape::Reference { .. } => plain_record_referent(types, leaf),
        _ => Some(leaf),
    }
}

/// Resolve declaration geometry without granting case membership or custody.
/// Case selects its payload namespace; an unqualified field names only a
/// record or Mixed common member, never an inactive alternative's field.
fn leaf_type(
    types: &[StructuralTypeDeclaration],
    mut current: StructuralTypeId,
    fields: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    let mut case_fields = None;
    for segment in fields {
        if case_fields.is_some() && !matches!(segment, StructuralPathSegment::Field(_)) {
            return None;
        }
        if let StructuralPathSegment::Case(case) = segment {
            let declaration = find_structural_type(types, current)?;
            let cases = declaration.shape.sum_cases()?;
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
            find_structural_type(types, current)?.shape,
            StructuralTypeShape::ByteSequence(_)
        ) {
            current = terminal_semantics::primitive_projection_declaration(
                types.iter(),
                current,
                std::slice::from_ref(segment),
            )?;
            continue;
        }
        if matches!(segment, StructuralPathSegment::RuntimeIndex { .. }) {
            current = match &find_structural_type(types, current)?.shape {
                StructuralTypeShape::FixedArray { element, .. }
                | StructuralTypeShape::ElementView { element } => *element,
                _ => return None,
            };
            continue;
        }
        if *segment == StructuralPathSegment::Referent {
            let StructuralTypeShape::Reference { referent, .. } =
                &find_structural_type(types, current)?.shape
            else {
                return None;
            };
            current = *referent;
            continue;
        }
        if let StructuralPathSegment::FixedIndex(index) = segment {
            let declaration = find_structural_type(types, current)?;
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
        let declaration = find_structural_type(types, current)?;
        let selected_fields = match case_fields.take() {
            Some(fields) => fields,
            None => declaration.shape.direct_fields()?,
        };
        let field = terminal_psi::live_field_by_identity(selected_fields, identity)?;
        current = match &field.field_type {
            StructuralFieldType::Structural(child) => *child,
            leaf => {
                let shape = leaf.canonical_leaf_shape()?;
                types
                    .iter()
                    .find(|declaration| declaration.shape == shape)?
                    .id
            }
        };
    }
    case_fields.is_none().then_some(current)
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
/// multiplicity does not constrain a loan's referent type; qualifications and
/// access do. normalized_source separately retains exact live-loan custody.
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
    } else {
        let (_, result) = super::structural_case::source_result(function, source.place).ok()?;
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
fn reference_source_type(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
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
        let mut current = source_contract(function, custody, source.place)?;
        let mut start = 0;
        for (position, segment) in source.path.iter().enumerate() {
            if *segment != StructuralPathSegment::Referent {
                continue;
            }
            current = leaf_type(types, current, &source.path[start..position])?;
            let StructuralTypeShape::Reference { referent, access } =
                &find_structural_type(types, current)?.shape
            else {
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
    direct_storage_type(function, types, source)
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
    let carrier = source_contract(function, custody, source.place)?;
    optimization_unit_semantics::structural_borrowed_storage_projection(
        types.iter(),
        carrier,
        &source.path,
        source.access,
    )
}

/// A `.., Referent` source names a suspended child of the located carrier
/// leaf; an empty path names the whole ingress referent directly. Every
/// selected payload and runtime index retains its exact storage coordinate.
fn normalized_source(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> Result<(Root, Vec<(PlaceId, Vec<StructuralPathSegment>)>), LegalizationError> {
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
) -> Result<(Root, Vec<(PlaceId, Vec<StructuralPathSegment>)>), LegalizationError> {
    if let Some(crossing) = source
        .path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)
    {
        let carrier_path = &source.path[..crossing];
        let suffix = &source.path[crossing + 1..];
        let Some(leaf) = custody.leaf(source.place, carrier_path) else {
            // Member storage under a carrier never held as a live reference
            // borrows the carrier's own storage; only a non-reference member
            // may root there — a missing reference carrier is a custody gap.
            let carrier =
                source_contract(function, custody, source.place).ok_or_else(|| invalid())?;
            return match leaf_type(types, carrier, carrier_path).and_then(|leaf| {
                find_structural_type(types, leaf).and_then(|declaration| match declaration.shape {
                    StructuralTypeShape::Reference { .. } => None,
                    _ if suffix.is_empty() => Some(()),
                    _ => None,
                })
            }) {
                Some(()) => {
                    let root = Root::Ingress(source.place, carrier_path.to_vec());
                    if custody.leaves.iter().any(|((carrier, path), reference)| {
                        roots_overlap(&reference.root, &root)
                            && !shared_source_compatible(
                                function,
                                custody,
                                types,
                                carrier,
                                path,
                                source.access,
                            )
                    }) {
                        return Err(invalid());
                    }
                    Ok((root, Vec::new()))
                }
                None => Err(invalid()),
            };
        };
        if leaf_type(types, leaf.referent, suffix).is_none() {
            return Err(invalid());
        }
        if custody.leaves.iter().any(|((carrier, path), child)| {
            child.parents.contains(&leaf.identity)
                && !shared_source_compatible(function, custody, types, carrier, path, source.access)
        }) {
            return Err(invalid());
        }
        Ok((leaf.root.clone(), vec![leaf.identity.clone()]))
    } else if source.path.is_empty() {
        if custody.leaves.iter().any(|((carrier, path), reference)| {
            reference.root.contains_place(source.place)
                && !shared_source_compatible(function, custody, types, carrier, path, source.access)
        }) {
            return Err(invalid());
        }
        Ok((Root::Place(source.place), Vec::new()))
    } else if storage_projection(function, custody, types, source).is_some() {
        let root = Root::Ingress(source.place, source.path.clone());
        if custody.leaves.iter().any(|((carrier, path), leaf)| {
            roots_overlap(&leaf.root, &root)
                && !shared_source_compatible(function, custody, types, carrier, path, source.access)
        }) {
            return Err(invalid());
        }
        Ok((root, Vec::new()))
    } else {
        Err(invalid())
    }
}

/// Substitute a callee's formal result-source mapping into the caller's actual
/// argument at the same structural-parameter position.

/// The formal origin a callee result mapping names: a borrowed primitive
/// parameter or an ingress leaf path under an owned or shared carrier.
fn formal_origin(
    function: &PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> Option<Root> {
    let parameter = function.structural_parameter(source.place)?;
    if source.path.is_empty()
        && matches!(
            parameter.access,
            terminal_psi::reading_borrow_pair_pattern!()
        )
    {
        Some(Root::Place(source.place))
    } else if let Some((StructuralPathSegment::Referent, path)) = source.path.split_last()
        && matches!(
            parameter.access,
            StructuralAccess::Owned | StructuralAccess::SharedBorrow
        )
    {
        Some(Root::Ingress(source.place, path.to_vec()))
    } else if direct_storage_type(function, types, source).is_some() {
        Some(Root::Ingress(source.place, source.path.clone()))
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

mod call_paths;

/// Ingress reference leaves identify external pointees, not descriptor storage.
/// Ordinary origin fields still suspend their exact storage regions; unresolved
/// declarations cannot establish disjointness. This matches lowering's custody
/// geometry without changing the identities retained across owned transfers.
fn is_suspended_storage_region(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    place: PlaceId,
    path: &[StructuralPathSegment],
) -> bool {
    let origin_overlaps = |origin_path: &[StructuralPathSegment]| {
        let Some(origin) = source_contract(function, custody, place)
            .and_then(|root| leaf_type(types, root, origin_path))
            .and_then(|identity| find_structural_type(types, identity))
        else {
            return true;
        };
        !matches!(origin.shape, StructuralTypeShape::Reference { .. })
            && terminal_semantics::structural_paths_may_overlap(path, origin_path)
    };
    custody
        .leaves
        .values()
        .any(|reference| match &reference.root {
            Root::Place(root) => *root == place,
            Root::Ingress(root, origin) => *root == place && origin_overlaps(origin),
            Root::Alternatives(origins) => origins
                .iter()
                .any(|origin| origin.root == place && origin_overlaps(&origin.path)),
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
            if !argument.path.iter().all(|segment| {
                matches!(
                    segment,
                    StructuralPathSegment::Case(_)
                        | StructuralPathSegment::Field(_)
                        | StructuralPathSegment::FixedIndex(_)
                )
            }) {
                return Err(invalid());
            }
            let contract =
                source_contract(function, custody, argument.place).ok_or_else(|| invalid())?;
            let selected = leaf_type(types, contract, &argument.path).ok_or_else(|| invalid())?;
            let storage = Root::Ingress(argument.place, argument.path.clone());
            // An ingress descriptor names external backing, not the carrier
            // bytes being transferred. Use the same declaration-derived
            // storage judgment as disposition instead of treating a copied
            // descriptor's origin slot as a loan into its former owner.
            if is_suspended_storage_region(function, custody, types, argument.place, &argument.path)
            {
                return Err(invalid());
            }
            normalized.push((
                argument_position,
                vec![storage],
                argument.access,
                CallRegion::CarrierStorage,
                None,
            ));
            for ((carrier, carrier_path), leaf) in &custody.leaves {
                if *carrier != argument.place {
                    continue;
                }
                let Some(path) = carrier_path.strip_prefix(argument.path.as_slice()) else {
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
                // Abstract callee ingress promises independently available
                // leaves. A type cannot describe an unknown suspended parent
                // or a parent/child pair packed into the same incoming record.
                if custody
                    .leaves
                    .values()
                    .any(|child| child.parents.contains(&leaf.identity))
                    || (argument.access == StructuralAccess::Owned
                        && moved
                            .insert((argument.place, carrier_path.clone()), leaf.clone())
                            .is_some())
                {
                    return Err(invalid());
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
                    Some((argument.place, carrier_path.clone())),
                ));
            }
            continue;
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
    callee_function: &PsiOptimizationFunction,
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
    let callee_result = callee_function
        .result
        .structural()
        .ok_or_else(|| invalid())?;
    if custody
        .leaves
        .keys()
        .any(|(carrier, _)| *carrier == result.place)
        || custody
            .leaves
            .values()
            .any(|leaf| leaf.identity.0 == result.place)
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
    for mapping in &callee_result.reference_sources {
        let expected =
            leaf_referent(types, result.structural_type, &mapping.path).ok_or_else(|| invalid())?;
        validate_result_sources(mapping, result.structural_type, types)?;
        let mut candidates = Vec::with_capacity(mapping.sources.len());
        for formal in &mapping.sources {
            let source = call_source(
                &callee_function.structural_parameters,
                structural_arguments,
                formal,
            )
            .ok_or_else(|| invalid())?;
            if reference_source_type(function, custody, types, &source) != Some(expected) {
                return Err(invalid());
            }
            let leaf = if matches!(
                formal_origin(callee_function, types, formal),
                Some(Root::Ingress(place, path))
                    if callee_function.structural_parameter(place)
                        .and_then(|parameter| {
                            leaf_type(types, parameter.structural_type, &path)
                        })
                        .and_then(|leaf| {
                            find_structural_type(types, leaf)
                        })
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
                let (root, parents) = normalized_source(function, custody, types, &source)?;
                Leaf::formed(
                    (result.place, mapping.path.clone()),
                    root,
                    parents,
                    psi_operation,
                    result,
                    expected,
                )
            };
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
            leaf.identity = (result.place, mapping.path.clone());
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

fn validate_result_sources(
    mapping: &terminal_psi::StructuralReferenceResultSource,
    result_type: StructuralTypeId,
    types: &[StructuralTypeDeclaration],
) -> Result<StructuralAccess, LegalizationError> {
    if mapping.sources.is_empty() {
        return Err(invalid());
    }
    let result_access = leaf_type(types, result_type, &mapping.path)
        .and_then(|leaf| find_structural_type(types, leaf))
        .and_then(|declaration| match declaration.shape {
            StructuralTypeShape::Reference { access, .. } => Some(access),
            _ => None,
        })
        .ok_or(invalid())?;
    if mapping
        .sources
        .iter()
        .any(|formal| !access_covers(formal.access, result_access))
        || mapping.sources.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(invalid());
    }
    Ok(result_access)
}

/// Alternatives retain one returned permission, not independently live loans.
fn combine_alternatives(candidates: Vec<Leaf>) -> Result<Leaf, LegalizationError> {
    let mut candidates = candidates.into_iter();
    let mut leaf = candidates.next().ok_or(invalid())?;
    let mut origins = vec![(leaf.root.clone(), leaf.parents.clone())];
    for candidate in candidates {
        if leaf.referent != candidate.referent {
            return Err(invalid());
        }
        origins.push((candidate.root, candidate.parents));
        leaf.formations.extend(candidate.formations);
    }
    occurrences::normalize_formations(&mut leaf.formations)?;
    (leaf.root, leaf.parents) = combine_candidate_origins(origins)?;
    Ok(leaf)
}

fn combine_candidate_origins(
    candidates: Vec<(Root, Vec<(PlaceId, Vec<StructuralPathSegment>)>)>,
) -> Result<(Root, Vec<(PlaceId, Vec<StructuralPathSegment>)>), LegalizationError> {
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

/// Whole cleanup uses the same declaration-derived subtree release as a
/// partial complement. Storage keys, not shared loan provenance, select rows.
fn discard_owned(
    function: &PsiOptimizationFunction,
    custody: &mut Custody,
    types: &[StructuralTypeDeclaration],
    source: PlaceId,
) -> Result<bool, LegalizationError> {
    discard_subtree(function, custody, types, source, &[])
}

fn discard_subtree(
    function: &PsiOptimizationFunction,
    custody: &mut Custody,
    types: &[StructuralTypeDeclaration],
    source: PlaceId,
    prefix: &[StructuralPathSegment],
) -> Result<bool, LegalizationError> {
    let Some(contract) = source_contract(function, custody, source) else {
        return Ok(false);
    };
    let selected = leaf_type(types, contract, prefix).ok_or(invalid())?;
    if !contains_reference(types, selected) {
        return Ok(false);
    }
    let released: BTreeSet<_> = custody
        .leaves
        .iter()
        .filter(|((carrier, path), _)| *carrier == source && path.starts_with(prefix))
        .map(|(_, leaf)| leaf.identity.clone())
        .collect();
    if custody.leaves.values().any(|child| {
        !released.contains(&child.identity)
            && child.parents.iter().any(|parent| released.contains(parent))
    }) {
        return Err(invalid());
    }
    custody
        .leaves
        .retain(|(carrier, path), _| *carrier != source || !path.starts_with(prefix));
    Ok(true)
}

fn discard_residual(
    function: &PsiOptimizationFunction,
    custody: &mut Custody,
    types: &[StructuralTypeDeclaration],
    discard: &terminal_psi::StructuralAffineDiscard,
) -> Result<(), LegalizationError> {
    let root = source_contract(function, custody, discard.place).ok_or(invalid())?;
    if leaf_type(types, root, &discard.path) != Some(discard.structural_type)
        || is_suspended_storage_region(function, custody, types, discard.place, &discard.path)
    {
        return Err(invalid());
    }
    discard_subtree(function, custody, types, discard.place, &discard.path)?;
    Ok(())
}

/// Stage every selected reference leaf before committing a storage transfer.
/// Projection prefixes select source storage; destination prefixes relocate
/// its leaves without moving a sibling or changing the loan's stable identity.
fn relocate_subtree_leaves<'a>(
    custody: &mut Custody,
    function: &PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
    destination: PlaceId,
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
    for (argument, nested, prefix) in operands {
        if !contains_reference(types, nested) {
            continue;
        }
        if custody
            .leaves
            .keys()
            .any(|(carrier, path)| *carrier == destination && path.starts_with(&prefix))
        {
            return Err(invalid());
        }
        let contract = source_contract(function, custody, argument.place).ok_or(invalid())?;
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
            let Some(path) = source_path.strip_prefix(argument.path.as_slice()) else {
                continue;
            };
            if leaf_referent(types, nested, path) != Some(leaf.referent) {
                return Err(invalid());
            }
            if !moved.insert(leaf.identity.clone()) {
                return Err(invalid());
            }
            let mut target_path = prefix.clone();
            target_path.extend(path.iter().cloned());
            relocations.push(((argument.place, source_path.clone()), target_path));
        }
    }
    for (from, to) in relocations {
        let leaf = custody.leaves.remove(&from).ok_or(invalid())?;
        custody.leaves.insert((destination, to), leaf);
    }
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
            .any(|child| child.parents.contains(&leaf.identity)))
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
    let installed = super::call_origin::installed_operation(node, function, native, plan)?;
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
            let shared_descriptor = find_structural_type(types, result.structural_type)
                .is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::Reference {
                            access: StructuralAccess::SharedBorrow,
                            ..
                        }
                    )
                });
            if !shared_copy_type(types, result.structural_type)
                || (result.multiplicity != StructuralMultiplicity::Unrestricted
                    && !(result.multiplicity == StructuralMultiplicity::Affine
                        && shared_descriptor))
                || !result.claims.is_empty()
                || !result.qualification_establishments.is_empty()
                || source_contract(function, custody, *source)
                    .and_then(|root| leaf_type(types, root, &path))
                    != Some(result.structural_type)
                || custody.leaves.iter().any(|((carrier, _), leaf)| {
                    *carrier == result.place || leaf.identity.0 == result.place
                })
            {
                return Err(invalid());
            }
            let paths = leaf_paths(types, result.structural_type, custody.leaves.len())
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
                call_paths::call_reference_custody(
                    function,
                    custody,
                    types,
                    &StructuralArgument {
                        place: *source,
                        path: observation,
                        access: StructuralAccess::SharedBorrow,
                    },
                )?;
                let source_leaf = custody.leaves.get(&(*source, selected)).ok_or(invalid())?;
                let expected =
                    leaf_referent(types, result.structural_type, relative).ok_or(invalid())?;
                if source_leaf.referent != expected {
                    return Err(invalid());
                }
                let mut leaf = source_leaf.clone();
                leaf.identity = (result.place, relative.clone());
                leaf.reform(*psi_operation, result);
                copied.push(((result.place, relative.clone()), leaf));
            }
            if custody.leaves.keys().any(|(carrier, selected)| {
                *carrier == *source
                    && selected
                        .strip_prefix(path.as_ref())
                        .is_some_and(|relative| !paths.iter().any(|expected| expected == relative))
            }) {
                return Err(invalid());
            }
            custody.leaves.extend(copied);
        }
        AbstractOperation::EstablishReference {
            psi_operation,
            result,
            source,
            storage,
        } => {
            let referent = referent(types, result.structural_type)
                .or_else(|| plain_record_referent(types, result.structural_type))
                .ok_or_else(|| invalid())?;
            if !result.is_affine_plain()
                || custody
                    .leaves
                    .keys()
                    .any(|(carrier, _)| *carrier == result.place)
                || custody
                    .leaves
                    .values()
                    .any(|leaf| leaf.identity.0 == result.place)
                || !reference_source_type(function, custody, types, source).is_some_and(|actual| {
                    match storage {
                        Some(storage) => terminal_semantics::reference_storage_source_compatible(
                            types.iter(),
                            actual,
                            result.structural_type,
                            storage.stored_scalar,
                        ),
                        None => terminal_semantics::reference_source_compatible(
                            types.iter(),
                            actual,
                            result.structural_type,
                        ),
                    }
                })
            {
                return Err(invalid());
            }
            let transferred = transferred_ingress_leaf(function, custody, types, source);
            let leaf = if let Some(key) = transferred {
                custody.leaves.remove(&key).ok_or_else(|| invalid())?
            } else {
                let (root, parents) = normalized_source(function, custody, types, source)?;
                Leaf::formed(
                    (result.place, Vec::new()),
                    root,
                    parents,
                    *psi_operation,
                    result,
                    referent,
                )
            };
            custody.leaves.insert((result.place, Vec::new()), leaf);
        }
        AbstractOperation::ReleaseReference { source, path, .. } => {
            if custody.leaves.values().any(|enclosing| {
                enclosing.root.origins().iter().any(|origin| {
                    let origin_path = &origin.path;
                    origin.root == *source
                        && source_contract(function, custody, *source)
                            .and_then(|contract| leaf_type(types, contract, origin_path))
                            == Some(enclosing.referent)
                        && terminal_semantics::structural_paths_may_overlap(origin_path, path)
                })
            }) {
                return Err(invalid());
            }
            let key = (*source, path.clone());
            let leaf = custody.leaves.remove(&key);
            let Some(leaf) = leaf else {
                return Err(invalid());
            };
            if custody
                .leaves
                .values()
                .any(|child| child.parents.contains(&leaf.identity))
            {
                custody.leaves.insert(key, leaf);
                return Err(invalid());
            }
        }
        AbstractOperation::EstablishRecord { result, fields, .. }
        | AbstractOperation::EstablishCase { result, fields, .. } => {
            if custody
                .leaves
                .keys()
                .any(|(carrier, _)| *carrier == result.place)
            {
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
            relocate_subtree_leaves(custody, function, types, result.place, operands)?;
        }
        AbstractOperation::EstablishStructuralArray {
            result, elements, ..
        } => {
            if custody
                .leaves
                .keys()
                .any(|(carrier, _)| *carrier == result.place)
            {
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
            let argument = StructuralArgument {
                place: source.place(),
                path: extent.path,
                access: StructuralAccess::Owned,
            };
            relocate_subtree_leaves(
                custody,
                function,
                types,
                result.place,
                [(&argument, result.structural_type, Vec::new())],
            )?;
        }
        AbstractOperation::StoreStructuralField {
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
            )?;
            relocate_subtree_leaves(
                custody,
                function,
                types,
                destination.place(),
                [(value, extent.field_type, extent.path)],
            )?;
        }
        AbstractOperation::CallStructural {
            psi_operation,
            result: AbstractStructuralCallResult::Structural { result, .. },
            callee,
            structural_arguments,
            ..
        } => {
            let moved = call_arguments(function, types, custody, structural_arguments)?;
            let callee_function = unit
                .functions
                .iter()
                .find(|function| Some(function.machine) == callee.machine())
                .ok_or_else(|| invalid())?;
            commit_argument_moves(custody, &moved);
            call_results(
                function,
                callee_function,
                types,
                result,
                *psi_operation,
                structural_arguments,
                custody,
                &moved,
            )?;
        }
        AbstractOperation::CallStructural {
            structural_arguments,
            result: AbstractStructuralCallResult::Unit | AbstractStructuralCallResult::Scalar(_),
            ..
        } => {
            let moved = call_arguments(function, types, custody, structural_arguments)?;
            commit_argument_moves(custody, &moved);
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
    Ok(())
}

/// A structural return whose source carries reference leaves must match the
/// declared result paths: every resident leaf descends directly from a checked
/// formal origin. The declaration lists possible case slots, not simultaneous
/// permissions; inactive payloads owe no descriptor. Establishment and transfer
/// reconstruct the resident roster before this independent return audit.
pub(in crate::legalization) fn return_custody(
    custody: &Custody,
    function: &PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
    source: PlaceId,
    result: &terminal_psi::StructuralResultDeclaration,
) -> Result<(), LegalizationError> {
    if !contains_reference(types, result.structural_type) {
        return Err(invalid());
    }
    let contract = source_contract(function, custody, source).ok_or_else(|| invalid())?;
    if contract != result.structural_type {
        return Err(invalid());
    }
    for ((carrier, path), leaf) in &custody.leaves {
        if *carrier != source {
            continue;
        }
        let mapping = result
            .reference_sources
            .iter()
            .find(|mapping| mapping.path == *path)
            .ok_or_else(|| invalid())?;
        let declared = |root: &Root| {
            mapping
                .sources
                .iter()
                .any(|formal| formal_origin(function, types, formal).as_ref() == Some(root))
        };
        let admitted = match &leaf.root {
            Root::Alternatives(origins) => origins.iter().all(|origin| {
                let root = if origin.path.is_empty() {
                    Root::Place(origin.root)
                } else {
                    Root::Ingress(origin.root, origin.path.clone())
                };
                declared(&root)
            }),
            root => declared(root),
        };
        if !admitted
            || !leaf.parents.is_empty()
            || custody
                .leaves
                .values()
                .any(|child| child.parents.contains(&leaf.identity))
        {
            return Err(invalid());
        }
        // Receiving parameters are actual declarations, not synthetic result
        // rows. Every possible transported formation retains its real owner.
        if !leaf.contract.matches_declaration(function, leaf.identity.0)
            || leaf.formations.is_empty()
            || leaf.formations.iter().any(|formation| {
                !formation
                    .contract
                    .matches_declaration(function, formation.identity.0)
            })
        {
            return Err(invalid());
        }
    }
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
            apply_cleanup_actions(&mut exit, actions.iter().cloned(), function, types)?;
            exits.push((edge, target, exit.leaves));
        } else {
            apply_cleanup_actions(custody, actions.iter().cloned(), function, types)?;
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
                // only this carrier's metadata, never origins at other homes.
                custody.leaves.retain(|(carrier, _), _| *carrier != place);
                continue;
            }
            TerminalAffineCleanupAction::DiscardRoot(place) => place,
            TerminalAffineCleanupAction::DiscardResidual(discard) => {
                discard_residual(function, custody, types, &discard)?;
                continue;
            }
            TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                if !cleanup.requirement_obligations.is_empty()
                    || source_contract(function, custody, cleanup.place)
                        .and_then(|root| leaf_type(types, root, &cleanup.path))
                        != Some(cleanup.structural_type)
                    || is_suspended_storage_region(
                        function,
                        custody,
                        types,
                        cleanup.place,
                        &cleanup.path,
                    )
                {
                    return Err(invalid());
                }
                // The borrowed call retains the selected original storage;
                // completing it retires only this subject's contained loans.
                discard_subtree(function, custody, types, cleanup.place, &cleanup.path)?;
                continue;
            }
        };
        // A suspended referent root cannot be discarded while its loan lives.
        if is_suspended_storage_region(function, custody, types, place, &[]) {
            return Err(invalid());
        }
        discard_owned(function, custody, types, place)?;
    }
    Ok(())
}

/// Reconstruct borrowed state addresses from exact incoming edge bindings.
/// Missing or unsupported origins remain explicit; they never become local roots.
fn borrowed_sources(
    function: &PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
    unit: &PsiOptimizationUnit,
) -> std::sync::Arc<BTreeMap<PlaceId, Option<Vec<StructuralArgument>>>> {
    use terminal_semantics::descriptor_lifetimes::{DescriptorLocation, descriptor_path};
    use terminal_semantics::descriptor_results::DescriptorAnalysis;
    let mut sources = BTreeMap::new();
    for parameter in function.block_structural_parameters() {
        if parameter.access != StructuralAccess::Owned
            && !terminal_semantics::descriptor_results::is_shared_descriptor(
                types.iter(),
                parameter.structural_type,
            )
        {
            sources.insert(parameter.place, None);
        }
    }
    if sources.is_empty() {
        return std::sync::Arc::new(sources);
    }
    if let Some(graph) = optimization_unit_semantics::descriptor_results::optimized_catalog_graph(
        unit.functions.iter(),
        &unit.boundary_machines,
        types,
    ) {
        if let Some(current) =
            terminal_semantics::descriptor_results::descriptor_machine(&graph, function.machine)
        {
            let mut analysis = DescriptorAnalysis::new(types, &graph);
            for block in &function.blocks {
                for parameter in &block.structural_parameters {
                    let Some(slot) = sources.get_mut(&parameter.place) else {
                        continue;
                    };
                    *slot = analysis
                        .storage_origins(
                            function.machine,
                            (block.id, 0),
                            &DescriptorLocation::whole(parameter.place),
                        )
                        .and_then(|origins| {
                            origins
                                .into_iter()
                                .map(|origin| {
                                    Some(StructuralArgument {
                                        place: origin.root,
                                        path: descriptor_path(
                                            types,
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
    std::sync::Arc::new(sources)
}

/// Compute entries from actual incoming edges, never dominator storage alone.
/// Each changed selected arrival replays its dependent block. Only owned
/// parameter occurrences may accumulate formations; all other leaves agree
/// exactly. Final entries equal the complete merge of every incoming edge.
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
    let borrowed_sources = borrowed_sources(function, types, unit);
    let mut initial = entry(function, types)?;
    initial.borrowed_sources = borrowed_sources;
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
            let scope = dominators.get(&edge.target).ok_or(invalid())?;
            arrival.leaves.retain(|(carrier, _), _| {
                storage_definitions
                    .get(carrier)
                    .is_none_or(|(definition, parameter)| {
                        scope.contains(definition) && (*parameter || *definition != edge.target)
                    })
            });
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
            let mut incoming = final_arrivals
                .iter()
                .flatten()
                .filter(|(_, candidate, _)| *candidate == target);
            let (_, _, first) = incoming.next().ok_or_else(invalid)?;
            let mut complete = first.clone();
            let mut consistent = true;
            for (_, _, arrival) in incoming {
                if merge_arrivals(function, target, &mut complete.leaves, &arrival.leaves)?
                    .is_none()
                {
                    consistent = false;
                    break;
                }
            }
            let target_position = *positions.get(&target).ok_or_else(invalid)?;
            if !consistent {
                // Keep the previous state provisional while other predecessors
                // propagate. Never replay a queued stale, nonselected roster.
                deferred.insert(target);
                pending.retain(|position| *position != target_position);
                queued[target_position] = false;
                continue;
            }
            let was_deferred = deferred.remove(&target);
            let changed = entries
                .get(&target)
                .is_none_or(|entry| entry.leaves != complete.leaves);
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
        let mut arrivals = final_arrivals
            .iter()
            .flatten()
            .filter(|(_, target, _)| *target == block.id);
        let (_, _, first) = arrivals.next().ok_or_else(invalid)?;
        let mut complete = first.leaves.clone();
        for (_, _, arrival) in arrivals {
            if merge_arrivals(function, block.id, &mut complete, &arrival.leaves)?.is_none() {
                return Err(invalid());
            }
        }
        if entries
            .get(&block.id)
            .is_none_or(|entry| entry.leaves != complete)
        {
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
                    identity: identity.clone(),
                    root: Root::Ingress(parameter.place, path.clone()),
                    parents: Vec::new(),
                    contract: CarrierContract::Parameter(parameter.clone()),
                    formations: vec![Formation {
                        identity,
                        contract: CarrierContract::Parameter(parameter.clone()),
                    }],
                    referent: leaf_referent(types, parameter.structural_type, &path)
                        .ok_or_else(|| invalid())?,
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
    target_caller: &super::TargetFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
) -> Result<target_operations::TargetStructuralArgument, LegalizationError> {
    let crossing = argument
        .path
        .iter()
        .position(|segment| matches!(segment, StructuralPathSegment::Referent))
        .ok_or(invalid())?;
    if argument.path.iter().enumerate().any(|(position, segment)| {
        position != crossing
            && !matches!(
                segment,
                StructuralPathSegment::Case(_)
                    | StructuralPathSegment::Field(_)
                    | StructuralPathSegment::FixedIndex(_)
                    | StructuralPathSegment::Referent
            )
    }) || argument.access == StructuralAccess::Owned
        || argument.access != declaration.access
        || declaration.multiplicity != StructuralMultiplicity::Unrestricted
        || !declaration.qualifications.is_empty()
        || !declaration.projected_qualifications.is_empty()
    {
        return Err(invalid());
    }
    let carrier_path = &argument.path[..crossing];
    let leaf = custody
        .leaf(argument.place, carrier_path)
        .ok_or(invalid())?;
    if custody
        .leaves
        .values()
        .any(|child| child.parents.contains(&leaf.identity))
    {
        return Err(invalid());
    }
    let root_type = source_contract(caller, custody, argument.place).ok_or(invalid())?;
    let (reference_type, _) = crate::structural_inputs::structural_reference_input::project(
        root_type,
        carrier_path,
        types,
    )
    .ok_or(invalid())?;
    let StructuralTypeShape::Reference { referent, access } =
        find_structural_type(types, reference_type)
            .ok_or(invalid())?
            .shape
    else {
        return Err(invalid());
    };
    // Transport borrows the captured referent without changing its authority.
    // Write-only descriptors can supply write-only access, never observations.
    let allowed = access != StructuralAccess::Owned && access_covers(access, argument.access);
    let projection = crate::structural_inputs::structural_reference_input::reference_projection(
        root_type,
        &argument.path,
        argument.access,
        types,
    )
    .ok_or(invalid())?;
    let selected = projection.selected;
    let source_byte_offset = projection.referent_offset;
    if !allowed
        || leaf.referent != referent
        || selected != declaration.structural_type
        || Some(destination.shape)
            != crate::structural_inputs::structural_reference_input::parameter_shape(
                declaration,
                types,
            )
    {
        return Err(invalid());
    }
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
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
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
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
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
        indices: Vec::new(),
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
    callee: &PsiOptimizationFunction,
    arguments: &[StructuralArgument],
    result: &StructuralOperationResult,
    psi_operation: OperationId,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
) -> Result<Vec<target_operations::TargetReferenceResult>, LegalizationError> {
    let moved = call_arguments(caller, types, custody, arguments)?;
    let mut returned = custody.clone();
    commit_argument_moves(&mut returned, &moved);
    call_results(
        caller,
        callee,
        types,
        result,
        psi_operation,
        arguments,
        &mut returned,
        &moved,
    )
}
