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
}

impl ReferenceRoot {
    fn place(&self) -> PlaceId {
        match self {
            Self::Place(place) | Self::Ingress(place, _) => *place,
        }
    }
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
            StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::ByteSequence(_) => {}
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
            _ => {}
        }
    }
    Some(output)
}

/// A whole-carrier result admits an exclusive permission over a primitive
/// scalar referent; that is the complete established shape this layer lowers.
pub(super) fn referent(
    types: &StructuralTypeLookup<'_>,
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    let declaration = types.get(&structural_type)?;
    match declaration.shape {
        StructuralTypeShape::Reference {
            referent,
            access: StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow,
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

/// A field-only carrier path ending at a reference leaf, then its referent.
pub(super) fn leaf_referent(
    types: &StructuralTypeLookup<'_>,
    mut current: StructuralTypeId,
    fields: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for segment in fields {
        let StructuralPathSegment::Field(identity) = segment else {
            return None;
        };
        let declaration = types.get(&current)?;
        let StructuralTypeShape::Record { fields } = &declaration.shape else {
            return None;
        };
        let field = fields.iter().find(|field| &field.identity == identity)?;
        if field.relevance.is_erased() {
            return None;
        }
        let StructuralFieldType::Structural(child) = field.field_type else {
            return None;
        };
        current = child;
    }
    referent(types, current)
}

/// The structural contract a place currently carries: a signature parameter, a
/// block-entry parameter, an ordinary home, or a live bare carrier result.
fn source_contract(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    place: PlaceId,
) -> Option<(StructuralTypeId, StructuralMultiplicity)> {
    if let Some(parameter) = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
    {
        return Some((parameter.structural_type, parameter.multiplicity));
    }
    if let Some(parameter) = function
        .block_entries
        .iter()
        .flat_map(|entry| &entry.structural_parameters)
        .find(|parameter| parameter.place == place)
    {
        return Some((parameter.structural_type, parameter.multiplicity));
    }
    if let Some(home) = live.structural_homes.get(&place) {
        return Some((home.structural_type(), home.multiplicity()));
    }
    live.references
        .get(&(place, Vec::new()))
        .map(|leaf| (leaf.result.structural_type, leaf.result.multiplicity))
}

fn primitive_scalar_type(
    types: &StructuralTypeLookup<'_>,
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    matches!(
        types
            .get(&structural_type)
            .map(|declaration| &declaration.shape),
        Some(StructuralTypeShape::PrimitiveScalar(_))
    )
    .then_some(structural_type)
}

fn primitive_local_result(
    function: &AbstractFunction,
    place: PlaceId,
) -> Option<&terminal_psi::StructuralOperationResult> {
    function
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::EstablishPrimitiveLocal { result, .. } if result.place == place => {
                Some(result)
            }
            _ => None,
        })
}

/// The primitive referent type an establishment or projection may name. Empty
/// paths name whole primitive places; nonempty paths must end in `Referent`.
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
    if !source.path.is_empty() {
        let (StructuralPathSegment::Referent, fields) = source.path.split_last()? else {
            return None;
        };
        let carrier = source_contract(function, live, source.place)?.0;
        return leaf_referent(types, carrier, fields);
    }
    if let Some(parameter) = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source.place)
    {
        if parameter.access != StructuralAccess::MutableBorrow
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || function
                .entry_claims
                .iter()
                .any(|claim| claim.input == source.place)
        {
            return None;
        }
        return primitive_scalar_type(types, parameter.structural_type);
    }
    let result = primitive_local_result(function, source.place)?;
    primitive_scalar_type(types, result.structural_type)
}

/// A `.., Referent` source names a suspended child of the located carrier
/// leaf; an empty path names a primitive referent root directly. Any other
/// projection does not describe reference custody this layer admits.
fn normalized_source(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    source: &StructuralArgument,
) -> Result<(ReferenceRoot, Option<(PlaceId, Vec<StructuralPathSegment>)>), LoweringError> {
    if let Some((StructuralPathSegment::Referent, carrier_path)) = source.path.split_last() {
        let leaf = live
            .references
            .get(&(source.place, carrier_path.to_vec()))
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        if live
            .references
            .values()
            .any(|child| child.parent.as_ref() == Some(&leaf.identity))
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        Ok((leaf.root.clone(), Some(leaf.identity.clone())))
    } else if source.path.is_empty() {
        if live
            .references
            .values()
            .any(|reference| reference.root == ReferenceRoot::Place(source.place))
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        Ok((ReferenceRoot::Place(source.place), None))
    } else {
        Err(LoweringError::unsupported_control_flow(function.machine))
    }
}

/// Substitute a callee's formal result-source mapping into the caller's actual
/// argument at the same structural-parameter position.
fn call_source(
    callee: &AbstractFunction,
    arguments: &[StructuralArgument],
    mapping: &terminal_psi::StructuralReferenceResultSource,
) -> Option<StructuralArgument> {
    let position = callee
        .structural_parameters
        .iter()
        .position(|parameter| parameter.place == mapping.source.place)?;
    let mut source = arguments.get(position)?.clone();
    source.path.extend(mapping.source.path.iter().cloned());
    source.access = mapping.source.access;
    Some(source)
}

/// The formal origin a callee result mapping names: a mutable primitive
/// parameter or an owned-carrier ingress leaf path.
fn formal_origin(
    function: &AbstractFunction,
    source: &StructuralArgument,
) -> Option<ReferenceRoot> {
    let parameter = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source.place)?;
    if source.path.is_empty() && parameter.access == StructuralAccess::MutableBorrow {
        Some(ReferenceRoot::Place(source.place))
    } else if let Some((StructuralPathSegment::Referent, path)) = source.path.split_last()
        && parameter.access == StructuralAccess::Owned
    {
        Some(ReferenceRoot::Ingress(source.place, path.to_vec()))
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

/// Whether this argument projects through a reference carrier it borrows.
fn is_reference_projection(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> bool {
    let Some((StructuralPathSegment::Referent, fields)) = source.path.split_last() else {
        return false;
    };
    source.access != StructuralAccess::Owned
        && source_contract(function, live, source.place)
            .and_then(|(contract, _)| leaf_referent(types, contract, fields))
            .is_some()
}

/// Whether `place` is currently suspended as a live loan's referent root.
/// Cleanup cannot discard storage that an outstanding carrier still names.
pub(super) fn is_suspended_root(live: &LiveDefinitions, place: PlaceId) -> bool {
    live.references
        .values()
        .any(|reference| reference.root == ReferenceRoot::Place(place))
}

/// Reads, writes and establishments cannot touch a suspended referent, and a
/// live carrier is never owned data for primitive or aggregate access.
pub(super) fn check_root_access(live: &LiveDefinitions, place: PlaceId) -> bool {
    live.references.values().any(|reference| {
        reference.root == ReferenceRoot::Place(place) || reference.identity.0 == place
    }) || live.references.keys().any(|(carrier, _)| *carrier == place)
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
    let mut moved = BTreeMap::new();
    for argument in arguments {
        if argument_owns_references(function, live, types, argument) {
            if !argument.path.is_empty() {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let (contract, _) = source_contract(function, live, argument.place)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            let paths = leaf_paths(types, contract, live.references.len())
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            if paths.len()
                != live
                    .references
                    .keys()
                    .filter(|(carrier, _)| *carrier == argument.place)
                    .count()
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            for path in paths {
                let leaf = live
                    .references
                    .get(&(argument.place, path.clone()))
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                // Abstract callee ingress promises independently available
                // leaves. A type cannot describe an unknown suspended parent
                // or a parent/child pair packed into the same incoming record.
                if live
                    .references
                    .values()
                    .any(|child| child.parent.as_ref() == Some(&leaf.identity))
                    || moved
                        .insert((argument.place, path.clone()), leaf.clone())
                        .is_some()
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                normalized.push((leaf.root.clone(), argument.access, true));
            }
            continue;
        }
        if source_contract(function, live, argument.place)
            .is_some_and(|(contract, _)| contains_reference(types, contract))
            || live
                .references
                .values()
                .any(|reference| reference.root == ReferenceRoot::Place(argument.place))
        {
            if !is_reference_projection(function, live, types, argument) {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            let (root, _) = normalized_source(function, live, argument)?;
            normalized.push((root, argument.access, true));
        } else {
            normalized.push((ReferenceRoot::Place(argument.place), argument.access, false));
        }
    }
    for (position, (root, access, reference_access)) in normalized.iter().enumerate() {
        if normalized[position + 1..]
            .iter()
            .any(|(other, other_access, other_reference_access)| {
                (*reference_access || *other_reference_access)
                    && root == other
                    && (*access != StructuralAccess::SharedBorrow
                        || *other_access != StructuralAccess::SharedBorrow)
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
            .any(|reference| reference.root == ReferenceRoot::Place(argument.place))
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
    if reference_source_type(function, live, types, source) != Some(referent) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let (root, parent) = normalized_source(function, live, source)?;
    let identity = (result.place, Vec::new());
    live.references.insert(
        identity.clone(),
        ReferenceCustody {
            identity,
            root,
            parent,
            operation: Some(*psi_operation),
            result: result.clone(),
            referent,
        },
    );
    operations.push(TargetUnitOperation::EstablishReference {
        psi_operation: *psi_operation,
        result_home: super::aggregate_results::home(*psi_operation, result, types)?,
        source: source.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
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
    field_identity: &str,
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
        let mut destination = vec![StructuralPathSegment::Field(field_identity.to_owned())];
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

/// Establish the declared result reference roster of a `CallStructural` whose
/// result carries reference custody, resolved against the already-committed
/// `moved` argument leaves. Returns the ordered resolved leaf roster retained
/// on the target row.
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
        let source = call_source(callee_function, structural_arguments, mapping)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        let expected = leaf_referent(types, result.structural_type, &mapping.path)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        if reference_source_type(function, live, types, &source) != Some(expected) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let leaf = if matches!(
            formal_origin(callee_function, &mapping.source),
            Some(ReferenceRoot::Ingress(_, _))
        ) {
            // An owned-carrier ingress mapping relocates one of the leaves the
            // moved arguments just transferred; the leaf keeps its identity,
            // root and parent, and lands at the declared result path.
            let Some((StructuralPathSegment::Referent, carrier_path)) = source.path.split_last()
            else {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            };
            moved
                .get(&(source.place, carrier_path.to_vec()))
                .cloned()
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
        } else {
            let (root, parent) = normalized_source(function, live, &source)?;
            ReferenceCustody {
                identity: (result.place, mapping.path.clone()),
                root,
                parent,
                operation: Some(psi_operation),
                result: result.clone(),
                referent: expected,
            }
        };
        if !roots.insert(leaf.root.clone()) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        reference_results.push(target_operations::TargetReferenceResult {
            path: mapping.path.clone(),
            root: leaf.root.place(),
        });
        established.push(((result.place, mapping.path.clone()), leaf));
    }
    for (key, leaf) in established {
        live.references.insert(key, leaf);
    }
    Ok(reference_results)
}

/// Resolve a `.., Referent` call argument through the live custody map to its
/// referent root place. The emitted target argument names the root's ABI
/// placement (or local producer) as transport evidence; custody — which leaf
/// loans the referent — is retained in `path`, never in pointer bits.
#[allow(clippy::too_many_arguments)]
pub(super) fn referent_argument(
    argument: &StructuralArgument,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    destination: &TargetStructuralParameter,
    function: &AbstractFunction,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
) -> Result<Option<TargetStructuralArgument>, LoweringError> {
    let Some((StructuralPathSegment::Referent, carrier_path)) = argument.path.split_last() else {
        return Ok(None);
    };
    if argument.access == StructuralAccess::Owned
        || argument.access != declaration.access
        || !super::primitive_storage::is_primitive_reference(declaration, types)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let leaf = live
        .references
        .get(&(argument.place, carrier_path.to_vec()))
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    if leaf.referent != declaration.structural_type {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let ReferenceRoot::Place(root) = &leaf.root else {
        // An ingress leaf's referent storage belongs to the caller; no local
        // pointer exists to transport.
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let (root_type, source) = if let Some(home) = live.structural_homes.get(root) {
        let (defining_operation, home_result) = home
            .operation_result()
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        if !function.operations.iter().any(|operation| {
            matches!(operation,
            AbstractOperation::EstablishPrimitiveLocal { psi_operation, result, .. }
            if *psi_operation == defining_operation && result == home_result)
        }) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        (
            home.structural_type(),
            target_operations::TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
                psi_operation: defining_operation,
            },
        )
    } else {
        let source = prepared
            .parameters
            .iter()
            .find(|source| source.place == *root)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        let allowed = match source.access {
            StructuralAccess::MutableBorrow => true,
            StructuralAccess::SharedBorrow => argument.access == StructuralAccess::SharedBorrow,
            StructuralAccess::WriteOnlyBorrow => {
                argument.access == StructuralAccess::WriteOnlyBorrow
            }
            StructuralAccess::Owned => false,
        };
        if !allowed {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        (source.structural_type, source.placement.clone().into())
    };
    if root_type != declaration.structural_type {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    Ok(Some(TargetStructuralArgument {
        place: *root,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type: root_type,
        structural_type: root_type,
        shape: destination.shape,
        source_byte_offset: 0,
        fixed_array_length: None,
        element_stride: None,
        source,
        destination: destination.placement.clone(),
    }))
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
        if Some(leaf.root.clone()) != formal_origin(function, &mapping.source)
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
                | StructuralFieldType::IeeeFloat(_)
                | StructuralFieldType::BoundedInteger(_) => {}
                StructuralFieldType::Structural(child) if referent(types, child).is_some() => {}
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
        // Only owned affine record carriers may enter with loans; the verified
        // contract rejects every other reference-bearing entry parameter.
        if parameter.access != StructuralAccess::Owned
            || parameter.multiplicity != StructuralMultiplicity::Affine
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
