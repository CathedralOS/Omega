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
use super::{
    AbstractOperation, AbstractOperationPlan, PsiOptimizationFunction, PsiOptimizationUnit,
};
use crate::LegalizationError;
use semantic_vocabulary::{BlockId, OperationId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

/// One outstanding loan: the referent root it suspends, the parent leaf it
/// descends from, the producing operation, and the result row that formed the
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
    /// Immediate parent leaf identity when this loan descends from a carrier.
    pub(in crate::legalization) parent: Option<(PlaceId, Vec<StructuralPathSegment>)>,
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
}

impl Root {
    pub(in crate::legalization) fn place(&self) -> PlaceId {
        match self {
            Self::Place(place) | Self::Ingress(place, _) => *place,
        }
    }
}

/// Live reference loans keyed by current carrier location, exactly mirroring
/// the lowering-time custody map's membership.
#[derive(Debug, Clone, Default)]
pub(in crate::legalization) struct Custody {
    leaves: BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
}

impl Custody {
    pub(in crate::legalization) fn leaf(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Option<&Leaf> {
        self.leaves.get(&(place, path.to_vec()))
    }
}

fn invalid() -> LegalizationError {
    LegalizationError::SourceCustodyMismatch
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
        let Some(declaration) = types.iter().find(|declaration| declaration.id == current) else {
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
            StructuralTypeShape::FixedArray { element, .. } => pending.push(*element),
            StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::ByteSequence(_) => {}
        }
    }
    false
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
        let Some(declaration) = types.iter().find(|declaration| declaration.id == current) else {
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
/// scalar referent; that is the complete established shape this layer replays.
fn referent(
    types: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    let declaration = types
        .iter()
        .find(|declaration| declaration.id == structural_type)?;
    match declaration.shape {
        StructuralTypeShape::Reference {
            referent,
            access: StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow,
        } if matches!(
            types
                .iter()
                .find(|declaration| declaration.id == referent)
                .map(|declaration| &declaration.shape),
            Some(StructuralTypeShape::PrimitiveScalar(_))
        ) =>
        {
            Some(referent)
        }
        _ => None,
    }
}

/// A field-only carrier path ending at a reference leaf, then its referent.
fn leaf_referent(
    types: &[StructuralTypeDeclaration],
    mut current: StructuralTypeId,
    fields: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for segment in fields {
        let StructuralPathSegment::Field(identity) = segment else {
            return None;
        };
        let declaration = types.iter().find(|declaration| declaration.id == current)?;
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

fn primitive_scalar_type(
    types: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    matches!(
        types
            .iter()
            .find(|declaration| declaration.id == structural_type)
            .map(|declaration| &declaration.shape),
        Some(StructuralTypeShape::PrimitiveScalar(_))
    )
    .then_some(structural_type)
}

/// The structural contract a place currently carries: a signature parameter, a
/// block parameter, an operation-produced home, or a live bare carrier.
fn source_contract(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    if let Some(parameter) = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
    {
        return Some(parameter.structural_type);
    }
    if let Some(parameter) = function
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters)
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

/// The primitive referent type an establishment or projection may name. Empty
/// paths name whole primitive places; nonempty paths must end in `Referent`.
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
    if !source.path.is_empty() {
        let Some((StructuralPathSegment::Referent, fields)) = source.path.split_last() else {
            return None;
        };
        let carrier = source_contract(function, custody, source.place)?;
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
                .entry_claim_declarations
                .iter()
                .any(|claim| claim.input == source.place)
        {
            return None;
        }
        return primitive_scalar_type(types, parameter.structural_type);
    }
    let (_, result, _) = super::primitive_locals::producer(function, source.place)?;
    primitive_scalar_type(types, result.structural_type)
}

/// A `.., Referent` source names a suspended child of the located carrier
/// leaf; an empty path names a primitive referent root directly. Any other
/// projection does not describe reference custody this layer admits.
fn normalized_source(
    custody: &Custody,
    source: &StructuralArgument,
) -> Result<(Root, Option<(PlaceId, Vec<StructuralPathSegment>)>), LegalizationError> {
    if let Some((StructuralPathSegment::Referent, carrier_path)) = source.path.split_last() {
        let leaf = custody
            .leaf(source.place, carrier_path)
            .ok_or_else(invalid)?;
        if custody
            .leaves
            .values()
            .any(|child| child.parent.as_ref() == Some(&leaf.identity))
        {
            return Err(invalid());
        }
        Ok((leaf.root.clone(), Some(leaf.identity.clone())))
    } else if source.path.is_empty() {
        if custody
            .leaves
            .values()
            .any(|reference| reference.root == Root::Place(source.place))
        {
            return Err(invalid());
        }
        Ok((Root::Place(source.place), None))
    } else {
        Err(invalid())
    }
}

/// Substitute a callee's formal result-source mapping into the caller's actual
/// argument at the same structural-parameter position.
fn call_source(
    callee: &PsiOptimizationFunction,
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
fn formal_origin(function: &PsiOptimizationFunction, source: &StructuralArgument) -> Option<Root> {
    let parameter = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source.place)?;
    if source.path.is_empty() && parameter.access == StructuralAccess::MutableBorrow {
        Some(Root::Place(source.place))
    } else if let Some((StructuralPathSegment::Referent, path)) = source.path.split_last()
        && parameter.access == StructuralAccess::Owned
    {
        Some(Root::Ingress(source.place, path.to_vec()))
    } else {
        None
    }
}

/// An owned argument that carries at least one reference leaf moves the loans
/// with it; the callee's ingress replay tracks each leaf exactly.
fn argument_owns_references(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    argument: &StructuralArgument,
) -> bool {
    argument.access == StructuralAccess::Owned
        && source_contract(function, custody, argument.place)
            .is_some_and(|contract| contains_reference(types, contract))
}

/// Whether this argument projects through a reference carrier it borrows.
fn is_reference_projection(
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> bool {
    let Some((StructuralPathSegment::Referent, fields)) = source.path.split_last() else {
        return false;
    };
    source.access != StructuralAccess::Owned
        && source_contract(function, custody, source.place)
            .and_then(|contract| leaf_referent(types, contract, fields))
            .is_some()
}

/// Whether `place` is currently suspended as a live loan's referent root.
/// Cleanup cannot discard storage that an outstanding carrier still names.
pub(in crate::legalization) fn is_suspended_root(custody: &Custody, place: PlaceId) -> bool {
    custody
        .leaves
        .values()
        .any(|reference| reference.root == Root::Place(place))
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
    let mut moved = BTreeMap::new();
    for argument in arguments {
        if argument_owns_references(function, custody, types, argument) {
            if !argument.path.is_empty() {
                return Err(invalid());
            }
            let contract =
                source_contract(function, custody, argument.place).ok_or_else(invalid)?;
            let paths = leaf_paths(types, contract, custody.leaves.len()).ok_or_else(invalid)?;
            if paths.len()
                != custody
                    .leaves
                    .keys()
                    .filter(|(carrier, _)| *carrier == argument.place)
                    .count()
            {
                return Err(invalid());
            }
            for path in paths {
                let leaf = custody.leaf(argument.place, &path).ok_or_else(invalid)?;
                // Abstract callee ingress promises independently available
                // leaves. A type cannot describe an unknown suspended parent
                // or a parent/child pair packed into the same incoming record.
                if custody
                    .leaves
                    .values()
                    .any(|child| child.parent.as_ref() == Some(&leaf.identity))
                    || moved
                        .insert((argument.place, path.clone()), leaf.clone())
                        .is_some()
                {
                    return Err(invalid());
                }
                normalized.push((leaf.root.clone(), argument.access, true));
            }
            continue;
        }
        if source_contract(function, custody, argument.place)
            .is_some_and(|contract| contains_reference(types, contract))
            || custody
                .leaves
                .values()
                .any(|reference| reference.root == Root::Place(argument.place))
        {
            if !is_reference_projection(function, custody, types, argument) {
                return Err(invalid());
            }
            let (root, _) = normalized_source(custody, argument)?;
            normalized.push((root, argument.access, true));
        } else {
            normalized.push((Root::Place(argument.place), argument.access, false));
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

/// Establish the declared result reference roster of a `CallStructural` whose
/// result carries reference custody, resolved against the already-committed
/// `moved` argument leaves. Returns the ordered resolved leaf roster the
/// target row must carry.
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
    let callee_result = callee_function.result.structural().ok_or_else(invalid)?;
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
    let mut roots = BTreeSet::new();
    let mut reference_results = Vec::new();
    for mapping in &callee_result.reference_sources {
        let source =
            call_source(callee_function, structural_arguments, mapping).ok_or_else(invalid)?;
        let expected =
            leaf_referent(types, result.structural_type, &mapping.path).ok_or_else(invalid)?;
        if reference_source_type(function, custody, types, &source) != Some(expected) {
            return Err(invalid());
        }
        let leaf = if matches!(
            formal_origin(callee_function, &mapping.source),
            Some(Root::Ingress(_, _))
        ) {
            // An owned-carrier ingress mapping relocates one of the leaves the
            // moved arguments just transferred; the leaf keeps its identity,
            // root and parent, and lands at the declared result path.
            let Some((StructuralPathSegment::Referent, carrier_path)) = source.path.split_last()
            else {
                return Err(invalid());
            };
            moved
                .get(&(source.place, carrier_path.to_vec()))
                .cloned()
                .ok_or_else(invalid)?
        } else {
            let (root, parent) = normalized_source(custody, &source)?;
            Leaf {
                identity: (result.place, mapping.path.clone()),
                root,
                parent,
                operation: Some(psi_operation),
                result: result.clone(),
                referent: expected,
            }
        };
        if !roots.insert(leaf.root.clone()) {
            return Err(invalid());
        }
        reference_results.push(target_operations::TargetReferenceResult {
            path: mapping.path.clone(),
            root: leaf.root.place(),
        });
        established.push(((result.place, mapping.path.clone()), leaf));
    }
    for (key, leaf) in established {
        custody.leaves.insert(key, leaf);
    }
    Ok(reference_results)
}

/// Remove every leaf an owned carrier at `place` still holds, in reverse
/// declaration order. The entire schedule is checked before any loan ends;
/// a live external child keeps its parent suspended.
fn discard_owned(
    function: &PsiOptimizationFunction,
    custody: &mut Custody,
    types: &[StructuralTypeDeclaration],
    source: PlaceId,
) -> Result<bool, LegalizationError> {
    let Some(contract) = source_contract(function, custody, source) else {
        return Ok(false);
    };
    if !contains_reference(types, contract) {
        return Ok(false);
    }
    let paths = leaf_paths(types, contract, custody.leaves.len()).ok_or_else(invalid)?;
    let mut released = BTreeSet::new();
    for path in paths.iter().rev() {
        let leaf = custody.leaf(source, path).ok_or_else(invalid)?;
        if custody.leaves.values().any(|child| {
            child.parent.as_ref() == Some(&leaf.identity) && !released.contains(&child.identity)
        }) {
            return Err(invalid());
        }
        released.insert(leaf.identity.clone());
    }
    if custody
        .leaves
        .iter()
        .any(|((carrier, _), leaf)| *carrier == source && !released.contains(&leaf.identity))
    {
        return Err(invalid());
    }
    custody
        .leaves
        .retain(|_, leaf| !released.contains(&leaf.identity));
    Ok(true)
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
        } => {
            let referent = referent(types, result.structural_type).ok_or_else(invalid)?;
            if result.multiplicity != StructuralMultiplicity::Affine
                || !result.claims.is_empty()
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || custody
                    .leaves
                    .keys()
                    .any(|(carrier, _)| *carrier == result.place)
                || custody
                    .leaves
                    .values()
                    .any(|leaf| leaf.identity.0 == result.place)
                || reference_source_type(function, custody, types, source) != Some(referent)
            {
                return Err(invalid());
            }
            let (root, parent) = normalized_source(custody, source)?;
            let identity = (result.place, Vec::new());
            custody.leaves.insert(
                identity.clone(),
                Leaf {
                    identity,
                    root,
                    parent,
                    operation: Some(*psi_operation),
                    result: result.clone(),
                    referent,
                },
            );
        }
        AbstractOperation::ReleaseReference { source, .. } => {
            let leaf = custody.leaves.remove(&(*source, Vec::new()));
            let Some(leaf) = leaf else {
                return Err(invalid());
            };
            if custody
                .leaves
                .values()
                .any(|child| child.parent.as_ref() == Some(&leaf.identity))
            {
                custody.leaves.insert(leaf.identity.clone(), leaf);
                return Err(invalid());
            }
        }
        AbstractOperation::EstablishRecord { result, fields, .. } => {
            let Some(StructuralTypeDeclaration {
                shape:
                    StructuralTypeShape::Record {
                        fields: declarations,
                    },
                ..
            }) = types
                .iter()
                .find(|declaration| declaration.id == result.structural_type)
            else {
                return Err(invalid());
            };
            let mut relocations = Vec::new();
            let mut moved = BTreeSet::new();
            for (field, declaration) in fields.iter().zip(declarations) {
                let terminal_psi::RecordFieldValue::Structural(argument) = &field.value else {
                    continue;
                };
                let StructuralFieldType::Structural(nested) = declaration.field_type else {
                    continue;
                };
                if !contains_reference(types, nested) {
                    continue;
                }
                let contract =
                    source_contract(function, custody, argument.place).ok_or_else(invalid)?;
                if contract != nested {
                    return Err(invalid());
                }
                let paths =
                    leaf_paths(types, contract, custody.leaves.len()).ok_or_else(invalid)?;
                for path in &paths {
                    let leaf = custody.leaf(argument.place, path).ok_or_else(invalid)?;
                    if !moved.insert(leaf.identity.clone()) {
                        return Err(invalid());
                    }
                    let mut destination =
                        vec![StructuralPathSegment::Field(declaration.identity.clone())];
                    destination.extend(path.iter().cloned());
                    relocations.push(((argument.place, path.clone()), destination));
                }
                if custody
                    .leaves
                    .keys()
                    .any(|(carrier, path)| *carrier == argument.place && !paths.contains(path))
                {
                    return Err(invalid());
                }
            }
            if custody
                .leaves
                .keys()
                .any(|(carrier, _)| *carrier == result.place)
            {
                return Err(invalid());
            }
            for (from, to) in relocations {
                if let Some(leaf) = custody.leaves.remove(&from) {
                    custody.leaves.insert((result.place, to), leaf);
                }
            }
        }
        AbstractOperation::CallStructural {
            psi_operation,
            result,
            callee,
            structural_arguments,
            ..
        } => {
            let moved = call_arguments(function, types, custody, structural_arguments)?;
            let callee_function = unit
                .functions
                .iter()
                .find(|function| function.machine == *callee)
                .ok_or_else(invalid)?;
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
        AbstractOperation::CallUnit {
            structural_arguments,
            ..
        }
        | AbstractOperation::CallStructuralScalar {
            structural_arguments,
            ..
        } => {
            let moved = call_arguments(function, types, custody, structural_arguments)?;
            commit_argument_moves(custody, &moved);
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
    let contract = source_contract(function, custody, source).ok_or_else(invalid)?;
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
        let leaf = custody.leaf(source, &mapping.path).ok_or_else(invalid)?;
        if Some(leaf.root.clone()) != formal_origin(function, &mapping.source)
            || leaf.parent.is_some()
            || custody
                .leaves
                .values()
                .any(|child| child.parent.as_ref() == Some(&leaf.identity))
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
    let mut places: Vec<PlaceId> = Vec::new();
    match operation {
        AbstractOperation::Return {
            cleanup_actions, ..
        }
        | AbstractOperation::ReturnUnit {
            cleanup_actions, ..
        } => {
            for action in cleanup_actions {
                let terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place) = action else {
                    return Err(invalid());
                };
                places.push(*place);
            }
        }
        AbstractOperation::ReturnStructural {
            trivial_affine_discards,
            ..
        }
        | AbstractOperation::Jump {
            trivial_affine_discards,
            ..
        } => places.extend(trivial_affine_discards.iter().copied()),
        AbstractOperation::Conditional {
            when_true,
            when_false,
            ..
        } => {
            places.extend(when_true.trivial_affine_discards.iter().copied());
            places.extend(when_false.trivial_affine_discards.iter().copied());
        }
        AbstractOperation::StructuralCase { cases, .. } => {
            for case in cases {
                places.extend(case.trivial_affine_discards.iter().copied());
            }
        }
        _ => {}
    }
    for place in places {
        // A suspended referent root cannot be discarded while its loan lives.
        if is_suspended_root(custody, place) {
            return Err(invalid());
        }
        discard_owned(function, custody, types, place)?;
    }
    Ok(())
}

/// Compute each block's entry custody snapshot: ingress seeding at the entry
/// block and each dominator's exit state elsewhere, replayed over the exact
/// optimized node order. Reference custody never crosses an edge binding —
/// lowering rejects reference-bearing structural transfers — so a block's
/// entry state is exactly its dominator's exit state.
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
        let terminator = block.nodes.last().ok_or_else(invalid)?;
        for edge in &terminator.successors {
            let target = *positions.get(&edge.target).ok_or_else(invalid)?;
            if !outgoing[position].contains(&target) {
                outgoing[position].push(target);
                incoming[target].push(position);
            }
        }
    }
    let entry_position = *positions.get(&function.entry).ok_or_else(invalid)?;
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
    let mut order = (0..blocks.len()).collect::<Vec<_>>();
    order.sort_by_key(|position| dominators[*position].len());
    let mut states: Vec<Option<Custody>> = vec![None; blocks.len()];
    let mut entries = BTreeMap::new();
    for position in order {
        let mut custody = if position == entry_position {
            entry(function, types)?
        } else {
            let dominator = dominators[position]
                .iter()
                .copied()
                .filter(|dominator| *dominator != position)
                .max_by_key(|dominator| dominators[*dominator].len())
                .ok_or_else(invalid)?;
            states[dominator].clone().ok_or_else(invalid)?
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
        // Only owned affine record carriers may enter with loans; the verified
        // contract rejects every other reference-bearing entry parameter.
        if parameter.access != StructuralAccess::Owned
            || parameter.multiplicity != StructuralMultiplicity::Affine
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || !constructible_record(types, parameter.structural_type)
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
        .ok_or_else(invalid)?;
        for path in paths {
            let identity = (parameter.place, path.clone());
            custody.leaves.insert(
                identity.clone(),
                Leaf {
                    identity,
                    root: Root::Ingress(parameter.place, path.clone()),
                    parent: None,
                    operation: None,
                    result: StructuralOperationResult {
                        place: parameter.place,
                        structural_type: parameter.structural_type,
                        multiplicity: parameter.multiplicity,
                        qualifications: parameter.qualifications.clone(),
                        projected_qualifications: parameter.projected_qualifications.clone(),
                        claims: Vec::new(),
                    },
                    referent: leaf_referent(types, parameter.structural_type, &path)
                        .ok_or_else(invalid)?,
                },
            );
        }
    }
    Ok(custody)
}

/// Every owned-carrier ingress parameter is a constructible record: scalar
/// leaves and bare primitive-reference leaves compose the complete type.
fn constructible_record(types: &[StructuralTypeDeclaration], root: StructuralTypeId) -> bool {
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
        let Some(StructuralTypeDeclaration {
            shape: StructuralTypeShape::Record { fields },
            ..
        }) = types.iter().find(|declaration| declaration.id == current)
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

/// Resolve a `.., Referent` call argument through the replayed custody map to
/// its referent root place. The emitted target argument names the root's ABI
/// placement (or local producer) as transport evidence; custody — which leaf
/// loans the referent — is retained in `path`, never in pointer bits.
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
    let Some((StructuralPathSegment::Referent, carrier_path)) = argument.path.split_last() else {
        return Err(invalid());
    };
    let referent_scalar = super::primitive_locals::scalar(types, declaration.structural_type)
        .and_then(super::scalar_shape)
        .ok_or_else(invalid)?;
    if argument.access == StructuralAccess::Owned
        || argument.access != declaration.access
        || declaration.is_self
        || declaration.multiplicity != StructuralMultiplicity::Unrestricted
        || !declaration.qualifications.is_empty()
        || !declaration.projected_qualifications.is_empty()
        || destination.shape
            != calling_conventions::ValueShape::borrowed_reference(
                referent_scalar.byte_size,
                referent_scalar.alignment,
            )
    {
        return Err(invalid());
    }
    let leaf = custody
        .leaf(argument.place, carrier_path)
        .ok_or_else(invalid)?;
    if leaf.referent != declaration.structural_type {
        return Err(invalid());
    }
    let Root::Place(root) = &leaf.root else {
        // An ingress leaf's referent storage belongs to the caller; no local
        // pointer exists to transport.
        return Err(invalid());
    };
    let (root_type, source) =
        if let Some((producer, result, value)) = super::primitive_locals::producer(caller, *root) {
            if !super::primitive_locals::valid_result(caller, producer, result) {
                return Err(invalid());
            }
            let referent_shape = super::scalar_shape(value.scalar_type).ok_or_else(invalid)?;
            if destination.shape
                != calling_conventions::ValueShape::borrowed_reference(
                    referent_shape.byte_size,
                    referent_shape.alignment,
                )
            {
                return Err(invalid());
            }
            (
                result.structural_type,
                target_operations::TargetStructuralArgumentSource::EstablishedPrimitiveLocal {
                    psi_operation: producer,
                },
            )
        } else {
            let source = super::structural_parameters(target_caller)
                .and_then(|parameters| parameters.iter().find(|parameter| parameter.place == *root))
                .ok_or_else(invalid)?;
            let allowed = match source.access {
                StructuralAccess::MutableBorrow => true,
                StructuralAccess::SharedBorrow => argument.access == StructuralAccess::SharedBorrow,
                StructuralAccess::WriteOnlyBorrow => {
                    argument.access == StructuralAccess::WriteOnlyBorrow
                }
                StructuralAccess::Owned => false,
            };
            if !allowed {
                return Err(invalid());
            }
            (source.structural_type, source.placement.clone().into())
        };
    if root_type != declaration.structural_type {
        return Err(invalid());
    }
    Ok(target_operations::TargetStructuralArgument {
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
        destination: destination.clone(),
    })
}

/// Recompute the ordered reference-result roster a `CallStructural` row must
/// carry: each callee-declared leaf resolved to its caller-visible referent
/// root place through the pre-call custody state. Read-only — `apply` owns
/// the corresponding mutation.
pub(in crate::legalization) fn reference_results(
    caller: &PsiOptimizationFunction,
    callee: &PsiOptimizationFunction,
    arguments: &[StructuralArgument],
    result_type: StructuralTypeId,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
) -> Result<Vec<target_operations::TargetReferenceResult>, LegalizationError> {
    let callee_result = callee.result.structural().ok_or_else(invalid)?;
    let mut roots = BTreeSet::new();
    let mut expected = Vec::new();
    for mapping in &callee_result.reference_sources {
        let source = call_source(callee, arguments, mapping).ok_or_else(invalid)?;
        let expected_referent =
            leaf_referent(types, result_type, &mapping.path).ok_or_else(invalid)?;
        if reference_source_type(caller, custody, types, &source) != Some(expected_referent) {
            return Err(invalid());
        }
        let (root, _) = normalized_source(custody, &source)?;
        if !roots.insert(root.clone()) {
            return Err(invalid());
        }
        expected.push(target_operations::TargetReferenceResult {
            path: mapping.path.clone(),
            root: root.place(),
        });
    }
    Ok(expected)
}
