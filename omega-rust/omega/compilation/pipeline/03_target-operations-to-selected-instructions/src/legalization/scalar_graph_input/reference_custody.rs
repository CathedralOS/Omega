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
use super::{
    AbstractOperation, AbstractOperationPlan, PsiOptimizationFunction, PsiOptimizationUnit,
};
use crate::LegalizationError;
use abstract_operations::AbstractStructuralCallResult;
use abstract_operations::call_source;
use semantic_vocabulary::{BlockId, OperationId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape, find_structural_type, structural_children,
};

/// One outstanding loan: the referent roots and immediate parent leaves it
/// suspends, the producing operation, and the result row that formed the
/// carrier identity. `root` is compile-time custody — it never names a
/// pointer or descriptor.
#[derive(Debug, Clone)]
pub(in crate::legalization) struct Leaf {
    /// Stable identity: the place and path where the carrier leaf was formed.
    /// Owned moves relocate the carrier, never this identity.
    pub(in crate::legalization) identity: (PlaceId, Vec<StructuralPathSegment>),
    /// Referent root resolved through custody: a primitive place directly, or
    /// an owned ingress parameter's leaf path when custody entered owned.
    pub(in crate::legalization) root: Root,
    /// All possible immediate parent identities; selection suspends each parent.
    pub(in crate::legalization) parents: Vec<(PlaceId, Vec<StructuralPathSegment>)>,
    /// The operation that formed this leaf's carrier identity, when the leaf
    /// descends from a body operation rather than an ingress parameter.
    pub(in crate::legalization) operation: Option<OperationId>,
    /// The structural result row that published this leaf's carrier identity.
    pub(in crate::legalization) result: StructuralOperationResult,
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
        BlockId,
        BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
    )>,
    borrowed_sources: std::sync::Arc<BTreeMap<PlaceId, Option<Vec<StructuralArgument>>>>,
}

impl Custody {
    /// Cleanup only removes leaves. Each outgoing edge retains an independent
    /// snapshot; descendants without a direct edge inherit the common survivors.
    fn finish_edges(
        &mut self,
        exits: Vec<(
            BlockId,
            BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
        )>,
    ) {
        self.leaves
            .retain(|key, _| exits.iter().all(|(_, leaves)| leaves.contains_key(key)));
        self.edge_exits = exits;
    }

    fn enter_edge(&mut self, target: BlockId) {
        let mut matching = self.edge_exits.iter().filter(|(block, _)| *block == target);
        if let Some((_, leaves)) = matching.next() {
            self.leaves = leaves.clone();
            for (_, leaves) in matching {
                self.leaves.retain(|key, _| leaves.contains_key(key));
            }
        }
        self.edge_exits.clear();
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
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        let Some(declaration) = find_structural_type(types, current) else {
            continue;
        };
        let mut push_fields = |fields: &[terminal_psi::StructuralFieldDeclaration]| {
            pending.extend(structural_children(fields));
        };
        match &declaration.shape {
            StructuralTypeShape::Reference { .. } => return true,
            StructuralTypeShape::Record { fields, .. } => push_fields(fields),
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
    let StructuralFieldType::Structural(child) = field.field_type else {
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
            }
            StructuralTypeShape::Record { fields, .. } => {
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
            // Case payloads hold leaves under field-identity paths as well.
            // Identical segments across cases collapse into the single leaf
            // the authored path names.
            StructuralTypeShape::Sum { cases } => {
                for field in cases.iter().flat_map(|case| &case.fields).rev() {
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
            StructuralTypeShape::Mixed { fields, cases } => {
                for field in cases
                    .iter()
                    .flat_map(|case| &case.fields)
                    .chain(fields)
                    .rev()
                {
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
            _ => {}
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
        } if types.iter().any(|declaration| declaration.id == referent) => Some(referent),
        StructuralTypeShape::Reference {
            referent,
            access:
                StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow,
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
        let field = selected_fields
            .iter()
            .find(|field| &field.identity == identity)?;
        if field.relevance.is_erased() {
            return None;
        }
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
        .map(|leaf| leaf.result.structural_type)
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
            let compatible = match source.access {
                StructuralAccess::SharedBorrow => matches!(
                    access,
                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                ),
                StructuralAccess::MutableBorrow => *access == StructuralAccess::MutableBorrow,
                StructuralAccess::WriteOnlyBorrow => matches!(
                    access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                ),
                StructuralAccess::Owned => false,
            };
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
    optimization_unit_semantics::structural_storage_projection(types.iter(), carrier, &source.path)
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
            StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
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
pub(in crate::legalization) fn is_suspended_root(custody: &Custody, place: PlaceId) -> bool {
    custody
        .leaves
        .values()
        .any(|reference| reference.root.contains_place(place))
}

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
            if !argument
                .path
                .iter()
                .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
            {
                return Err(invalid());
            }
            let contract =
                source_contract(function, custody, argument.place).ok_or_else(|| invalid())?;
            let selected = leaf_type(types, contract, &argument.path).ok_or_else(|| invalid())?;
            let paths =
                leaf_paths(types, selected, custody.leaves.len()).ok_or_else(|| invalid())?;
            if paths.len()
                != custody
                    .leaves
                    .keys()
                    .filter(|(carrier, path)| {
                        *carrier == argument.place && path.starts_with(&argument.path)
                    })
                    .count()
            {
                return Err(invalid());
            }
            let storage = Root::Ingress(argument.place, argument.path.clone());
            if custody.leaves.iter().any(|((carrier, path), leaf)| {
                !(*carrier == argument.place && path.starts_with(&argument.path))
                    && roots_overlap(&storage, &leaf.root)
            }) {
                return Err(invalid());
            }
            normalized.push((
                argument_position,
                vec![storage],
                argument.access,
                CallRegion::CarrierStorage,
            ));
            for path in paths {
                let permission = leaf_type(types, selected, &path)
                    .and_then(|leaf| {
                        find_structural_type(types, leaf).map(|declaration| &declaration.shape)
                    })
                    .and_then(|shape| match shape {
                        StructuralTypeShape::Reference { access, .. } => Some(*access),
                        _ => None,
                    })
                    .ok_or_else(|| invalid())?;
                let mut carrier_path = argument.path.clone();
                carrier_path.extend(path);
                let leaf = custody
                    .leaf(argument.place, &carrier_path)
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
                            .insert((argument.place, carrier_path), leaf.clone())
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
            .any(|(other_position, others, access, _)| {
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
    // Stage the complete result before publication. Independent leaves cannot
    // duplicate an exclusive origin, even through distinct actual argument
    // spellings. Borrowing a result does not move its parent loan.
    let mut established = Vec::new();
    let mut roots = Vec::new();
    let mut identities = BTreeSet::new();
    let mut reference_results = Vec::new();
    for mapping in &callee_result.reference_sources {
        let expected =
            leaf_referent(types, result.structural_type, &mapping.path).ok_or_else(|| invalid())?;
        let result_access = validate_result_sources(mapping, result.structural_type, types)?;
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
                moved
                    .get(&(source.place, carrier_path.to_vec()))
                    .cloned()
                    .ok_or_else(|| invalid())?
            } else {
                let (root, parents) = normalized_source(function, custody, types, &source)?;
                Leaf {
                    identity: (result.place, mapping.path.clone()),
                    root,
                    parents,
                    operation: Some(psi_operation),
                    result: result.clone(),
                    referent: expected,
                }
            };
            candidates.push(leaf);
        }
        let joining = candidates.len() > 1;
        let mut leaf = combine_alternatives(candidates)?;
        if joining {
            // The selected descriptor is one result resource, conservatively
            // pinning the complete union of the consumed input permissions.
            leaf.identity = (result.place, mapping.path.clone());
            leaf.result = result.clone();
            leaf.operation = Some(psi_operation);
        }
        if !identities.insert(leaf.identity.clone())
            || roots.iter().any(|(root, prior_access)| {
                roots_overlap(root, &leaf.root)
                    && (result_access != StructuralAccess::SharedBorrow
                        || *prior_access != StructuralAccess::SharedBorrow)
            })
        {
            return Err(invalid());
        }
        roots.push((leaf.root.clone(), result_access));
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
    }
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
    let paths = leaf_paths(types, selected, custody.leaves.len()).ok_or(invalid())?;
    let mut released = BTreeSet::new();
    let mut released_paths = BTreeSet::new();
    for path in paths.iter().rev() {
        let mut selected_path = prefix.to_vec();
        selected_path.extend_from_slice(path);
        let leaf = custody.leaf(source, &selected_path).ok_or(invalid())?;
        if custody.leaves.values().any(|child| {
            child.parents.contains(&leaf.identity) && !released.contains(&child.identity)
        }) {
            return Err(invalid());
        }
        released.insert(leaf.identity.clone());
        released_paths.insert(selected_path);
    }
    if custody.leaves.keys().any(|(carrier, path)| {
        *carrier == source && path.starts_with(prefix) && !released_paths.contains(path)
    }) {
        return Err(invalid());
    }
    custody
        .leaves
        .retain(|(carrier, path), _| *carrier != source || !released_paths.contains(path));
    Ok(true)
}

fn discard_residual(
    function: &PsiOptimizationFunction,
    custody: &mut Custody,
    types: &[StructuralTypeDeclaration],
    discard: &terminal_psi::StructuralAffineDiscard,
) -> Result<(), LegalizationError> {
    let root = source_contract(function, custody, discard.place).ok_or(invalid())?;
    if discard.path.is_empty()
        || leaf_type(types, root, &discard.path) != Some(discard.structural_type)
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
        let paths = leaf_paths(types, nested, custody.leaves.len()).ok_or(invalid())?;
        for path in &paths {
            let mut source_path = argument.path.clone();
            source_path.extend(path.iter().cloned());
            let leaf = custody
                .leaf(argument.place, &source_path)
                .ok_or(invalid())?;
            if !moved.insert(leaf.identity.clone()) {
                return Err(invalid());
            }
            let mut target_path = prefix.clone();
            target_path.extend(path.iter().cloned());
            relocations.push(((argument.place, source_path), target_path));
        }
        if custody.leaves.keys().any(|(carrier, path)| {
            *carrier == argument.place
                && path.starts_with(&argument.path)
                && !paths
                    .iter()
                    .any(|relative| path[argument.path.len()..] == relative[..])
        }) {
            return Err(invalid());
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
    operation: &AbstractOperation,
    function: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    let types = &plan.structural_types[..];
    match operation {
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
                Leaf {
                    identity: (result.place, Vec::new()),
                    root,
                    parents,
                    operation: Some(*psi_operation),
                    result: result.clone(),
                    referent,
                }
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
        AbstractOperation::EstablishRecord { result, fields, .. } => {
            if custody
                .leaves
                .keys()
                .any(|(carrier, _)| *carrier == result.place)
            {
                return Err(invalid());
            }
            let Some(StructuralTypeDeclaration {
                shape:
                    StructuralTypeShape::Record {
                        fields: declarations,
                        ..
                    },
                ..
            }) = find_structural_type(types, result.structural_type)
            else {
                return Err(invalid());
            };
            let declarations = declarations
                .iter()
                .filter(|field| !field.relevance.is_erased());
            if fields.len() != declarations.clone().count() {
                return Err(invalid());
            }
            let mut operands = Vec::new();
            for (field, declaration) in fields.iter().zip(declarations) {
                if field.field != declaration.id {
                    return Err(invalid());
                }
                if let (
                    terminal_psi::RecordFieldValue::Structural(argument),
                    StructuralFieldType::Structural(nested),
                ) = (&field.value, &declaration.field_type)
                {
                    operands.push((
                        argument,
                        *nested,
                        vec![StructuralPathSegment::Field(declaration.identity.clone())],
                    ));
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
/// declared result roster exactly: every returned leaf still sits at its
/// declared carrier path and descends directly from a checked formal origin.
/// The referent itself is not storage the return transports.
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
    if contract != result.structural_type
        || custody
            .leaves
            .keys()
            .filter(|(carrier, _)| *carrier == source)
            .count()
            != result.reference_sources.len()
    {
        return Err(invalid());
    }
    for mapping in &result.reference_sources {
        let leaf = custody
            .leaf(source, &mapping.path)
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
        // A body-formed leaf's declared place names its forming operation.
        if let Some(producer) = leaf.operation
            && !function.structural_places.iter().any(|declaration| {
                declaration.id == leaf.result.place
                    && declaration.kind
                        == semantic_vocabulary::StructuralPlaceKind::OperationResult {
                            producer,
                            structural_type: leaf.result.structural_type,
                        }
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
        let owned_transfer = match operation {
            AbstractOperation::Jump {
                target,
                structural_bindings,
                ..
            } => Some((*target, structural_bindings.as_slice())),
            AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => [when_true, when_false]
                .into_iter()
                .find(|successor| successor.psi_edge == edge)
                .map(|successor| (successor.target, successor.structural_bindings.as_slice())),
            _ => None,
        };
        let target = match operation {
            AbstractOperation::Jump { target, .. } => Some(*target),
            AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => [when_true, when_false]
                .into_iter()
                .find(|successor| successor.psi_edge == edge)
                .map(|successor| successor.target),
            AbstractOperation::StructuralCase { cases, .. } => cases
                .iter()
                .find(|successor| successor.psi_edge == edge)
                .map(|successor| successor.target),
            _ => None,
        };
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
                re_home_bound_leaves(&mut exit, bindings, &destination.structural_parameters);
            }
            apply_cleanup_actions(&mut exit, actions.iter().cloned(), function, types)?;
            exits.push((target, exit.leaves));
        } else {
            apply_cleanup_actions(custody, actions.iter().cloned(), function, types)?;
        }
    }
    if !exits.is_empty() {
        custody.finish_edges(exits);
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
                if source_contract(function, custody, place).is_none()
                    || custody.leaves.keys().any(|(carrier, _)| *carrier == place)
                {
                    return Err(invalid());
                }
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
        if is_suspended_root(custody, place) {
            return Err(invalid());
        }
        discard_owned(function, custody, types, place)?;
    }
    Ok(())
}

/// An owned edge move re-homes the moved subtree's loan leaves onto the
/// bound name: the custody key is the current carrier location and follows
/// the value across the edge. Formation identity, result row, root, parent
/// and referent are provenance and stay put.
fn re_home_bound_leaves(
    custody: &mut Custody,
    bindings: &[abstract_operations::AbstractStructuralBinding],
    parameters: &[terminal_psi::StructuralParameterDeclaration],
) {
    let mut moved = Vec::new();
    for (binding, parameter) in bindings.iter().zip(parameters) {
        if parameter.access != StructuralAccess::Owned
            || binding.argument.access != StructuralAccess::Owned
        {
            continue;
        }
        for ((carrier, path), leaf) in &custody.leaves {
            if *carrier == binding.argument.place
                && let Some(relative) = path.strip_prefix(binding.argument.path.as_slice())
            {
                moved.push((
                    (*carrier, path.clone()),
                    (parameter.place, relative.to_vec()),
                    leaf.clone(),
                ));
            }
        }
    }
    // Edge bindings are simultaneous, including swaps of existing carriers.
    for (source, _, _) in &moved {
        custody.leaves.remove(source);
    }
    for (_, destination, leaf) in moved {
        custody.leaves.insert(destination, leaf);
    }
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

/// Compute each block's entry custody snapshot: ingress seeding at the entry
/// block and each dominator's exit state elsewhere, replayed over the exact
/// optimized node order. Whole owned edge bindings may re-home a carrier —
/// every stored loan travels with its record — so a block's entry state is
/// its dominator's exit state with the edge's bound leaves re-keyed.
pub(in crate::legalization) fn block_entry_states(
    function: &PsiOptimizationFunction,
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
    let mut incoming = vec![Vec::new(); blocks.len()];
    let mut outgoing = vec![Vec::new(); blocks.len()];
    for (position, block) in blocks.iter().enumerate() {
        let terminator = block.nodes.last().ok_or_else(|| invalid())?;
        for edge in &terminator.successors {
            let target = *positions.get(&edge.target).ok_or_else(|| invalid())?;
            if !outgoing[position].contains(&target) {
                outgoing[position].push(target);
                incoming[target].push(position);
            }
        }
    }
    let entry_position = *positions.get(&function.entry).ok_or_else(|| invalid())?;
    if !incoming[entry_position].is_empty() {
        return Err(invalid());
    }
    // Strict-dominator chain schedule, mirroring the lowering's dominance walk.
    let mut reachable = BTreeSet::new();
    let mut pending = vec![entry_position];
    while let Some(position) = pending.pop() {
        if reachable.insert(position) {
            pending.extend(outgoing[position].iter().copied());
        }
    }
    if reachable.len() != blocks.len() {
        return Err(invalid());
    }
    let mut dominators = vec![reachable.clone(); blocks.len()];
    dominators[entry_position] = BTreeSet::from([entry_position]);
    loop {
        let mut changed = false;
        for (position, predecessors) in incoming.iter().enumerate() {
            if position == entry_position {
                continue;
            }
            let Some((first, others)) = predecessors.split_first() else {
                return Err(invalid());
            };
            let mut current = dominators[*first].clone();
            for predecessor in others {
                current.retain(|candidate| dominators[*predecessor].contains(candidate));
            }
            current.insert(position);
            if current != dominators[position] {
                dominators[position] = current;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let borrowed_sources = borrowed_sources(function, types, unit);
    let mut order = (0..blocks.len()).collect::<Vec<_>>();
    order.sort_by_key(|position| dominators[*position].len());
    let mut states: Vec<Option<Custody>> = vec![None; blocks.len()];
    let mut entries = BTreeMap::new();
    for position in order {
        let mut custody = if position == entry_position {
            let mut custody = entry(function, types)?;
            custody.borrowed_sources = borrowed_sources.clone();
            custody
        } else {
            let dominator = dominators[position]
                .iter()
                .copied()
                .filter(|dominator| *dominator != position)
                .max_by_key(|dominator| dominators[*dominator].len())
                .ok_or_else(|| invalid())?;
            let mut custody = states[dominator].clone().ok_or_else(|| invalid())?;
            custody.enter_edge(blocks[position].id);
            // A dominating edge that binds this block's parameters carries
            // each whole owned move's loans onto the bound places. When the
            // dominator reaches this block through an intermediate block —
            // a merge point — no binding happens on its terminator and the
            // entry state stays its exit state unchanged.
            if let Some(edge) = blocks[dominator].nodes.last().and_then(|terminator| {
                // Ordinary edge transfers were captured before cleanup.
                // Case payload establishment keeps its separate transport.
                if !matches!(
                    terminator.operation,
                    AbstractOperation::StructuralCase { .. }
                ) {
                    return None;
                }
                terminator
                    .successors
                    .iter()
                    .find(|edge| edge.target == blocks[position].id)
            }) {
                re_home_bound_leaves(
                    &mut custody,
                    &edge.structural_bindings,
                    &blocks[position].structural_parameters,
                );
            }
            custody
        };
        entries.insert(blocks[position].id, custody.clone());
        for node in &blocks[position].nodes {
            if matches!(
                node.operation,
                AbstractOperation::Return { .. }
                    | AbstractOperation::ReturnUnit { .. }
                    | AbstractOperation::ReturnStructural { .. }
                    | AbstractOperation::Jump { .. }
                    | AbstractOperation::Conditional { .. }
                    | AbstractOperation::StructuralCase { .. }
                    | AbstractOperation::ExternalCompletion { .. }
            ) {
                apply_terminator(&mut custody, &node.operation, function, plan)?;
            } else {
                apply(&mut custody, &node.operation, function, plan, unit)?;
            }
        }
        states[position] = Some(custody);
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
        // Borrowing lends the carrier roster without transferring its permissions.
        if parameter.multiplicity
            != if parameter.access == StructuralAccess::Owned {
                StructuralMultiplicity::Affine
            } else {
                StructuralMultiplicity::Unrestricted
            }
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || !constructible(types, parameter.structural_type)
            || function
                .entry_claim_declarations
                .iter()
                .any(|claim| claim.input == parameter.place)
        {
            return Err(invalid());
        }
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
                    identity,
                    root: Root::Ingress(parameter.place, path.clone()),
                    parents: Vec::new(),
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
                        .ok_or_else(|| invalid())?,
                },
            );
        }
    }
    Ok(custody)
}

/// Every owned-carrier ingress parameter is a constructible carrier: scalar
/// leaves and bare primitive-reference leaves compose the complete type, in
/// record fields and case payloads alike.
fn constructible(types: &[StructuralTypeDeclaration], root: StructuralTypeId) -> bool {
    let mut pending = vec![root];
    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if complete.contains(&current) {
            continue;
        }
        if !active.insert(current) {
            return false;
        }
        let Some(declaration) = find_structural_type(types, current) else {
            return false;
        };
        if let StructuralTypeShape::FixedArray { element, .. } = &declaration.shape {
            if referent(types, *element).is_none()
                && plain_record_referent(types, *element).is_none()
            {
                pending.push(*element);
            }
            active.remove(&current);
            complete.insert(current);
            continue;
        }
        let matching: Vec<&terminal_psi::StructuralFieldDeclaration> = match &declaration.shape {
            StructuralTypeShape::Record { fields, .. } => fields.iter().collect(),
            StructuralTypeShape::Sum { cases } => {
                cases.iter().flat_map(|case| case.fields.iter()).collect()
            }
            StructuralTypeShape::Mixed { fields, cases } => fields
                .iter()
                .chain(cases.iter().flat_map(|case| case.fields.iter()))
                .collect(),
            _ => return false,
        };
        for field in matching {
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
                StructuralFieldType::Structural(child) => pending.push(child),
                _ => return false,
            }
        }
        active.remove(&current);
        complete.insert(current);
    }
    true
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
                StructuralPathSegment::Field(_)
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
    let allowed = match access {
        StructuralAccess::MutableBorrow => matches!(
            argument.access,
            StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        ),
        StructuralAccess::SharedBorrow => argument.access == StructuralAccess::SharedBorrow,
        _ => false,
    };
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

/// Recompute the ordered reference-result roster a structural-result
/// `CallStructural` row must carry: each callee-declared leaf resolved to its
/// caller-visible referent root place through the pre-call custody state.
/// Read-only — `apply` owns the corresponding mutation.
pub(in crate::legalization) fn reference_results(
    caller: &PsiOptimizationFunction,
    callee: &PsiOptimizationFunction,
    arguments: &[StructuralArgument],
    result_type: StructuralTypeId,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
) -> Result<Vec<target_operations::TargetReferenceResult>, LegalizationError> {
    let callee_result = callee.result.structural().ok_or_else(|| invalid())?;
    let mut roots = Vec::new();
    let mut expected = Vec::new();
    for mapping in &callee_result.reference_sources {
        let expected_referent =
            leaf_referent(types, result_type, &mapping.path).ok_or_else(|| invalid())?;
        let result_access = validate_result_sources(mapping, result_type, types)?;
        let mut candidates = Vec::with_capacity(mapping.sources.len());
        for formal in &mapping.sources {
            let source =
                call_source(&callee.structural_parameters, arguments, formal).ok_or(invalid())?;
            if reference_source_type(caller, custody, types, &source) != Some(expected_referent) {
                return Err(invalid());
            }
            let (root, _) = normalized_source(caller, custody, types, &source)?;
            candidates.push(root);
        }
        // This read-only roster describes storage alternatives, not a new
        // child loan. The cursor's parent identifies how to find each source;
        // apply/call_results separately transfers or retains actual lineage.
        let mut origins = candidates
            .into_iter()
            .flat_map(|root| root.origins())
            .collect::<Vec<_>>();
        origins.sort();
        origins.dedup();
        let root = Root::Alternatives(origins);
        if roots.iter().any(|(other, prior_access)| {
            roots_overlap(other, &root)
                && (result_access != StructuralAccess::SharedBorrow
                    || *prior_access != StructuralAccess::SharedBorrow)
        }) {
            return Err(invalid());
        }
        roots.push((root.clone(), result_access));
        expected.push(target_operations::TargetReferenceResult {
            path: mapping.path.clone(),
            origins: root.origins(),
        });
    }
    Ok(expected)
}
