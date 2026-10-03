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
use abstract_operations::AbstractStructuralCallResult;
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
    /// One permission whose selected static path is carried by its pointer.
    Alternatives(PlaceId, Vec<Vec<StructuralPathSegment>>),
}

impl Root {
    pub(in crate::legalization) fn place(&self) -> PlaceId {
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

fn roots_overlap(left: &Root, right: &Root) -> bool {
    if left.place() != right.place() {
        return false;
    }
    left.paths().iter().any(|left| {
        right
            .paths()
            .iter()
            .any(|right| left.starts_with(right) || right.starts_with(left))
    })
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

    /// Observation needs the carrier's current permission, not merely its
    /// declaration. The caller separately checks the reference's read access.
    pub(in crate::legalization) fn readable_leaf(
        &self,
        place: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Option<&Leaf> {
        let leaf = self.leaf(place, path)?;
        (!self.leaves.values().any(|other| {
            other.root.place() == place || other.parent.as_ref() == Some(&leaf.identity)
        }))
        .then_some(leaf)
    }
}

#[cfg(test)]
mod read_custody_tests {
    use super::*;

    #[test]
    fn reference_read_requires_live_unsuspended_leaf() {
        let carrier = PlaceId::new(1).unwrap();
        let child_carrier = PlaceId::new(2).unwrap();
        let root = PlaceId::new(3).unwrap();
        let structural_type = StructuralTypeId::new(1).unwrap();
        let result = StructuralOperationResult {
            place: carrier,
            structural_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        };
        let leaf = Leaf {
            identity: (carrier, Vec::new()),
            root: Root::Place(root),
            parent: None,
            operation: OperationId::new(1),
            result,
            referent: structural_type,
        };
        let mut custody = Custody::default();
        assert!(custody.readable_leaf(carrier, &[]).is_none());
        custody.leaves.insert(leaf.identity.clone(), leaf.clone());
        assert!(custody.readable_leaf(carrier, &[]).is_some());

        let child_identity = (child_carrier, Vec::new());
        let mut child = leaf.clone();
        child.identity = child_identity.clone();
        child.result.place = child_carrier;
        child.parent = Some(leaf.identity.clone());
        custody.leaves.insert(child_identity.clone(), child.clone());
        assert!(custody.readable_leaf(carrier, &[]).is_none());
        custody.leaves.remove(&child_identity);
        assert!(custody.readable_leaf(carrier, &[]).is_some());

        child.parent = None;
        child.root = Root::Place(carrier);
        custody.leaves.insert(child_identity.clone(), child);
        assert!(custody.readable_leaf(carrier, &[]).is_none());
        custody.leaves.remove(&child_identity);
        assert!(custody.readable_leaf(carrier, &[]).is_some());
        custody.leaves.remove(&leaf.identity);
        assert!(custody.readable_leaf(carrier, &[]).is_none());
    }
}

/// Live permission and referent-relative geometry behind a returned carrier.
/// Candidate roots constrain authority; its pointer payload selects the address.
pub(in crate::legalization) struct WritableReferentDestination {
    pub(in crate::legalization) producer: OperationId,
    pub(in crate::legalization) result: StructuralOperationResult,
    pub(in crate::legalization) referent: StructuralTypeId,
    pub(in crate::legalization) path: Vec<StructuralPathSegment>,
    root: Root,
}

fn returned_mutable_record_projection(
    custody: &Custody,
    function: &PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
    carrier: PlaceId,
    path: &[StructuralPathSegment],
) -> Option<WritableReferentDestination> {
    let (StructuralPathSegment::Referent, tail) = path.split_first()? else {
        return None;
    };
    let leaf = custody.leaf(carrier, &[])?;
    let StructuralTypeShape::Reference {
        referent,
        access: StructuralAccess::MutableBorrow,
    } = &types
        .iter()
        .find(|declaration| declaration.id == leaf.result.structural_type)?
        .shape
    else {
        return None;
    };
    // A whole `Referent` path is also the established primitive-reference
    // spelling. Only a record referent may enter this aggregate-origin route.
    if !matches!(
        &types
            .iter()
            .find(|declaration| declaration.id == *referent)?
            .shape,
        StructuralTypeShape::Record { .. }
    ) {
        return None;
    }
    let producer = leaf.operation?;
    if *referent != leaf.referent
        || custody
            .leaves
            .values()
            .any(|child| child.parent.as_ref() == Some(&leaf.identity))
        || leaf.result.place != carrier
        || leaf.result.multiplicity != StructuralMultiplicity::Affine
        || !leaf.result.qualification_establishments.is_empty()
        || !leaf.result.qualifications.is_empty()
        || !leaf.result.projected_qualifications.is_empty()
        || !leaf.result.claims.is_empty()
        || !function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .any(|node| {
                matches!(&node.operation,
            AbstractOperation::CallStructural {
                psi_operation,
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            } if *psi_operation == producer && result == &leaf.result)
            })
    {
        return None;
    }
    let root = leaf.root.place();
    let parameter = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == root)?;
    if !matches!(
        parameter.access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    ) || parameter.multiplicity == StructuralMultiplicity::Linear
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || function
            .entry_claim_declarations
            .iter()
            .any(|claim| claim.input == root)
        || function
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == root)
        || leaf
            .root
            .paths()
            .iter()
            .any(|path| leaf_type(types, parameter.structural_type, path) != Some(*referent))
        || leaf.root.paths().is_empty()
    {
        return None;
    }
    Some(WritableReferentDestination {
        producer,
        result: leaf.result.clone(),
        referent: *referent,
        path: tail.to_vec(),
        root: leaf.root.clone(),
    })
}

pub(in crate::legalization) fn writable_referent_destination(
    custody: &Custody,
    function: &PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
    carrier: PlaceId,
    path: &[StructuralPathSegment],
) -> Option<WritableReferentDestination> {
    returned_mutable_record_projection(custody, function, types, carrier, path)
}

/// Rejoin one call-only projection through a returned mutable record. The
/// target row retains its actual pointer home and explicit referent crossing.
pub(in crate::legalization) fn returned_record_call_argument(
    argument: &StructuralArgument,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    destination: &calling_conventions::ValuePlacement,
    caller: &PsiOptimizationFunction,
    target_caller: &super::TargetFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
) -> Result<Option<target_operations::TargetStructuralArgument>, LegalizationError> {
    let Some(resolved) =
        returned_mutable_record_projection(custody, caller, types, argument.place, &argument.path)
    else {
        return Ok(None);
    };
    let Some((StructuralPathSegment::Referent, _)) = argument.path.split_first() else {
        return Ok(None);
    };
    if argument.access == StructuralAccess::Owned
        || argument.access != declaration.access
        || declaration.multiplicity != StructuralMultiplicity::Unrestricted
        || !declaration.qualifications.is_empty()
        || !declaration.projected_qualifications.is_empty()
    {
        return Err(invalid());
    }
    let has_exact_home = target_caller.graph.blocks.iter().flat_map(|block| &block.operations).any(|operation| {
        matches!(operation, target_operations::TargetUnitOperation::Call {
            psi_operation,
            result: target_operations::TargetCallResult::Structural { result, result_home: Some(_), .. },
            ..
        } if *psi_operation == resolved.producer && result == &resolved.result)
    });
    let access_is_compatible = matches!(
        argument.access,
        StructuralAccess::SharedBorrow
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow
    );
    let (selected, source_byte_offset) =
        crate::structural_inputs::structural_reference_input::project(
            resolved.referent,
            &resolved.path,
            types,
        )
        .ok_or_else(invalid)?;
    let selected_shape =
        crate::structural_inputs::structural_reference_input::shape(selected, types)
            .ok_or_else(invalid)?;
    let expected_shape = calling_conventions::ValueShape::borrowed_reference(
        selected_shape.byte_size,
        selected_shape.alignment,
    );
    let source_shape =
        crate::structural_inputs::structural_reference_input::shape(resolved.referent, types)
            .ok_or_else(invalid)?;
    if !has_exact_home
        || !access_is_compatible
        || selected != declaration.structural_type
        || destination.shape != expected_shape
        || selected_shape
            .byte_size
            .checked_add(source_byte_offset)
            .is_none_or(|end| end > source_shape.byte_size)
    {
        return Err(invalid());
    }
    Ok(Some(target_operations::TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type: resolved.result.structural_type,
        structural_type: selected,
        shape: destination.shape,
        source_byte_offset,
        fixed_array_length: None,
        element_stride: None,
        indices: Vec::new(),
        source: target_operations::TargetStructuralArgumentSource::StructuralHome {
            psi_operation: resolved.producer,
        },
        destination: destination.clone(),
    }))
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
            // Case payloads hold leaves under field-identity paths as well.
            // Identical segments across cases collapse into the single leaf
            // the authored path names.
            StructuralTypeShape::Sum { cases } => {
                for field in cases.iter().flat_map(|case| &case.fields).rev() {
                    if let StructuralFieldType::Structural(child) = field.field_type {
                        if !contains_reference(types, child) {
                            continue;
                        }
                        if pending.len().checked_add(output.len())? >= maximum_leaves {
                            return None;
                        }
                        let mut child_path = path.clone();
                        child_path.push(StructuralPathSegment::Field(field.identity.clone()));
                        pending.push((child, child_path));
                    }
                }
            }
            StructuralTypeShape::Mixed { fields, cases } => {
                for field in cases
                    .iter()
                    .flat_map(|case| &case.fields)
                    .chain(fields)
                    .rev()
                {
                    if let StructuralFieldType::Structural(child) = field.field_type {
                        if !contains_reference(types, child) {
                            continue;
                        }
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
    output.sort();
    output.dedup();
    Some(output)
}

/// A whole-carrier result admits one borrow permission over a primitive
/// scalar referent; shared and exclusive carriers share that loan shape.
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
            access:
                StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow,
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
    } = &types
        .iter()
        .find(|declaration| declaration.id == structural_type)?
        .shape
    else {
        return None;
    };
    matches!(
        types
            .iter()
            .find(|declaration| declaration.id == *referent)
            .map(|declaration| &declaration.shape),
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
    match &types
        .iter()
        .find(|declaration| declaration.id == leaf)?
        .shape
    {
        StructuralTypeShape::Reference { .. } => plain_record_referent(types, leaf),
        _ => Some(leaf),
    }
}

/// The structural type the field-only path names beneath `current`: a sum or
/// mixed payload keeps a segment legal while every matching case field
/// resolves to one structural type, and a plain leaf field resolves to its
/// canonical standalone declaration.
fn leaf_type(
    types: &[StructuralTypeDeclaration],
    mut current: StructuralTypeId,
    fields: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for segment in fields {
        if let StructuralPathSegment::FixedIndex(index) = segment {
            let declaration = types.iter().find(|declaration| declaration.id == current)?;
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
        let declaration = types.iter().find(|declaration| declaration.id == current)?;
        let matching: Vec<&terminal_psi::StructuralFieldDeclaration> = match &declaration.shape {
            StructuralTypeShape::Record { fields } => fields.iter().collect(),
            StructuralTypeShape::Sum { cases } => {
                cases.iter().flat_map(|case| case.fields.iter()).collect()
            }
            StructuralTypeShape::Mixed { fields, cases } => fields
                .iter()
                .chain(cases.iter().flat_map(|case| case.fields.iter()))
                .collect(),
            _ => return None,
        };
        let mut resolved = None;
        for field in matching {
            if field.identity != *identity || field.relevance.is_erased() {
                continue;
            }
            let child = match &field.field_type {
                StructuralFieldType::Structural(child) => *child,
                leaf => {
                    let shape = leaf.canonical_leaf_shape()?;
                    types
                        .iter()
                        .find(|declaration| declaration.shape == shape)
                        .map(|declaration| declaration.id)?
                }
            };
            match resolved {
                None => resolved = Some(child),
                Some(previous) if previous != child => return None,
                _ => {}
            }
        }
        current = resolved?;
    }
    Some(current)
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

/// Resolve direct storage from its declared contract, rather than requiring
/// that the owner originated in the machine's input telescope. Reference
/// crossings still require the separately reconstructed live loan.
fn direct_storage_type(
    function: &PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
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
        let (_, result) = super::structural_case::source_result(function, source.place).ok()?;
        (
            result.structural_type,
            result.multiplicity,
            StructuralAccess::Owned,
            result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty()
                && result.qualification_establishments.is_empty(),
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
            .entry_claim_declarations
            .iter()
            .any(|claim| claim.input == source.place)
        || function
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == source.place)
    {
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
    if let Some((StructuralPathSegment::Referent, fields)) = source.path.split_last() {
        let carrier = source_contract(function, custody, source.place)?;
        return leaf_referent(types, carrier, fields);
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
            .and_then(|leaf| types.iter().find(|declaration| declaration.id == leaf))
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
    function: &PsiOptimizationFunction,
    custody: &Custody,
    types: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> Result<(Root, Option<(PlaceId, Vec<StructuralPathSegment>)>), LegalizationError> {
    if let Some((StructuralPathSegment::Referent, carrier_path)) = source.path.split_last() {
        let Some(leaf) = custody.leaf(source.place, carrier_path) else {
            // Member storage under a carrier never held as a live reference
            // borrows the carrier's own storage; only a non-reference member
            // may root there — a missing reference carrier is a custody gap.
            let carrier =
                source_contract(function, custody, source.place).ok_or_else(|| invalid())?;
            return match leaf_type(types, carrier, carrier_path).and_then(|leaf| {
                types
                    .iter()
                    .find(|declaration| declaration.id == leaf)
                    .and_then(|declaration| match declaration.shape {
                        StructuralTypeShape::Reference { .. } => None,
                        _ => Some(()),
                    })
            }) {
                Some(()) => Ok((Root::Ingress(source.place, carrier_path.to_vec()), None)),
                None => Err(invalid()),
            };
        };
        if custody.leaves.iter().any(|((carrier, path), child)| {
            child.parent.as_ref() == Some(&leaf.identity)
                && !shared_source_compatible(function, custody, types, carrier, path, source.access)
        }) {
            return Err(invalid());
        }
        Ok((leaf.root.clone(), Some(leaf.identity.clone())))
    } else if source.path.is_empty() {
        if custody.leaves.iter().any(|((carrier, path), reference)| {
            reference.root.place() == source.place
                && !shared_source_compatible(function, custody, types, carrier, path, source.access)
        }) {
            return Err(invalid());
        }
        Ok((Root::Place(source.place), None))
    } else if direct_storage_type(function, types, source).is_some() {
        let root = Root::Ingress(source.place, source.path.clone());
        if custody.leaves.iter().any(|((carrier, path), leaf)| {
            leaf.root.place() == source.place
                && !shared_source_compatible(function, custody, types, carrier, path, source.access)
        }) {
            return Err(invalid());
        }
        Ok((root, None))
    } else {
        Err(invalid())
    }
}

/// Substitute a callee's formal result-source mapping into the caller's actual
/// argument at the same structural-parameter position.
fn call_source(
    callee: &PsiOptimizationFunction,
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
    function: &PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> Option<Root> {
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
        .any(|reference| reference.root.place() == place)
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
        if matches!(argument.path.first(), Some(StructuralPathSegment::Referent))
            && let Some(resolved) = returned_mutable_record_projection(
                custody,
                function,
                types,
                argument.place,
                &argument.path,
            )
        {
            if argument.access == StructuralAccess::Owned {
                return Err(invalid());
            }
            normalized.push((resolved.root, argument.access, true));
            continue;
        }
        if argument_owns_references(function, custody, types, argument) {
            if !argument.path.is_empty() {
                return Err(invalid());
            }
            let contract =
                source_contract(function, custody, argument.place).ok_or_else(|| invalid())?;
            let paths =
                leaf_paths(types, contract, custody.leaves.len()).ok_or_else(|| invalid())?;
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
                let leaf = custody
                    .leaf(argument.place, &path)
                    .ok_or_else(|| invalid())?;
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
                .any(|reference| reference.root.place() == argument.place)
        {
            if !is_reference_projection(function, custody, types, argument) {
                return Err(invalid());
            }
            let (root, _) = normalized_source(function, custody, types, argument)?;
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
                    && roots_overlap(root, other)
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
    let mut roots = BTreeSet::new();
    let mut reference_results = Vec::new();
    for mapping in &callee_result.reference_sources {
        let expected =
            leaf_referent(types, result.structural_type, &mapping.path).ok_or_else(|| invalid())?;
        validate_alternatives(mapping, expected, types)?;
        let mut candidates = Vec::with_capacity(mapping.sources.len());
        for formal in &mapping.sources {
            let source = call_source(callee_function, structural_arguments, formal)
                .ok_or_else(|| invalid())?;
            if reference_source_type(function, custody, types, &source) != Some(expected) {
                return Err(invalid());
            }
            let leaf = if matches!(
                formal_origin(callee_function, types, formal),
                Some(Root::Ingress(place, path))
                    if callee_function
                        .structural_parameters
                        .iter()
                        .find(|parameter| parameter.place == place)
                        .and_then(|parameter| {
                            leaf_type(types, parameter.structural_type, &path)
                        })
                        .and_then(|leaf| {
                            types
                                .iter()
                                .find(|declaration| declaration.id == leaf)
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
                let (root, parent) = normalized_source(function, custody, types, &source)?;
                Leaf {
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
        let leaf = combine_alternatives(candidates)?;
        if roots.iter().any(|root| roots_overlap(root, &leaf.root)) {
            return Err(invalid());
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
        custody.leaves.insert(key, leaf);
    }
    Ok(reference_results)
}

fn validate_alternatives(
    mapping: &terminal_psi::StructuralReferenceResultSource,
    referent: StructuralTypeId,
    types: &[StructuralTypeDeclaration],
) -> Result<(), LegalizationError> {
    let Some(first) = mapping.sources.first() else {
        return Err(invalid());
    };
    if mapping.sources.windows(2).any(|pair| pair[0] >= pair[1])
        || (mapping.sources.len() > 1
            && (!mapping.path.is_empty()
                || !types.iter().any(|declaration| {
                    declaration.id == referent
                        && matches!(declaration.shape, StructuralTypeShape::Record { .. })
                })
                || mapping.sources.iter().any(|formal| {
                    formal.place != first.place
                        || formal.access != StructuralAccess::MutableBorrow
                        || formal.path.is_empty()
                        || formal.path.iter().any(|segment| {
                            !matches!(
                                segment,
                                StructuralPathSegment::Field(_)
                                    | StructuralPathSegment::FixedIndex(_)
                            )
                        })
                })))
    {
        return Err(invalid());
    }
    Ok(())
}

/// Alternatives retain one returned permission, not independently live loans.
fn combine_alternatives(candidates: Vec<Leaf>) -> Result<Leaf, LegalizationError> {
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
        let Root::Ingress(_, path) = candidate.root else {
            return Err(invalid());
        };
        paths.push(path);
    }
    if !paths.is_empty() {
        let Root::Ingress(place, path) = &leaf.root else {
            return Err(invalid());
        };
        paths.push(path.clone());
        paths.sort();
        if paths.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid());
        }
        leaf.root = Root::Alternatives(*place, paths);
    }
    Ok(leaf)
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
    let paths = leaf_paths(types, contract, custody.leaves.len()).ok_or_else(|| invalid())?;
    let mut released = BTreeSet::new();
    for path in paths.iter().rev() {
        let leaf = custody.leaf(source, path).ok_or_else(|| invalid())?;
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
            let referent = referent(types, result.structural_type)
                .or_else(|| plain_record_referent(types, result.structural_type))
                .ok_or_else(|| invalid())?;
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
                || !reference_source_type(function, custody, types, source).is_some_and(|actual| {
                    terminal_semantics::reference_source_compatible(
                        types.iter(),
                        actual,
                        result.structural_type,
                    )
                })
            {
                return Err(invalid());
            }
            let (root, parent) = normalized_source(function, custody, types, source)?;
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
            let declarations = declarations
                .iter()
                .filter(|field| !field.relevance.is_erased());
            if fields.len() != declarations.clone().count() {
                return Err(invalid());
            }
            let mut relocations = Vec::new();
            let mut moved = BTreeSet::new();
            for (field, declaration) in fields.iter().zip(declarations) {
                if field.field != declaration.id {
                    return Err(invalid());
                }
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
                    source_contract(function, custody, argument.place).ok_or_else(|| invalid())?;
                if contract != nested {
                    return Err(invalid());
                }
                let paths =
                    leaf_paths(types, contract, custody.leaves.len()).ok_or_else(|| invalid())?;
                for path in &paths {
                    let leaf = custody
                        .leaf(argument.place, path)
                        .ok_or_else(|| invalid())?;
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
            result: AbstractStructuralCallResult::Structural { result, .. },
            callee,
            structural_arguments,
            ..
        } => {
            let moved = call_arguments(function, types, custody, structural_arguments)?;
            let callee_function = unit
                .functions
                .iter()
                .find(|function| function.machine == *callee)
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
            Root::Alternatives(place, paths) => paths
                .iter()
                .all(|path| declared(&Root::Ingress(*place, path.clone()))),
            root => declared(root),
        };
        if !admitted
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
                let place = match action {
                    terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place) => *place,
                    terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup)
                        if cleanup.requirement_obligations.is_empty()
                            && source_contract(function, custody, cleanup.place)
                                == Some(cleanup.structural_type) =>
                    {
                        // The call's exact callee, borrowed receiver and action
                        // ordinal are independently reconstructed by cleanup_calls.
                        // Its normal completion retires this same owned root and
                        // must release embedded loans under the usual checks.
                        cleanup.place
                    }
                    _ => return Err(invalid()),
                };
                places.push(place);
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
    // Every edge cleanup roster declares the same places; a `&` carrier's
    // own leaf ends once, however many edges carry the discard.
    // Keep action order: a preceding carrier release may make its backing
    // discardable. Deduplicate sibling-edge repeats without sorting away
    // that dependency.
    let mut visited = BTreeSet::new();
    for place in places {
        if !visited.insert(place) {
            continue;
        }
        // A suspended referent root cannot be discarded while its loan lives.
        if is_suspended_root(custody, place) {
            return Err(invalid());
        }
        discard_owned(function, custody, types, place)?;
    }
    Ok(())
}

/// A whole owned edge move re-homes the moved record's loan leaves onto the
/// bound name: the custody key is the current carrier location and follows
/// the value across the edge. Formation identity, result row, root, parent
/// and referent are provenance and stay put.
fn re_home_bound_leaves(
    custody: &mut Custody,
    bindings: &[abstract_operations::AbstractStructuralBinding],
    parameters: &[terminal_psi::StructuralParameterDeclaration],
) {
    for (binding, parameter) in bindings.iter().zip(parameters) {
        if parameter.access != StructuralAccess::Owned
            || binding.argument.access != StructuralAccess::Owned
            || !binding.argument.path.is_empty()
            || binding.argument.place == parameter.place
        {
            continue;
        }
        let moved: Vec<(PlaceId, Vec<StructuralPathSegment>)> = custody
            .leaves
            .keys()
            .filter(|(carrier, _)| *carrier == binding.argument.place)
            .cloned()
            .collect();
        for (carrier, path) in moved {
            if let Some(leaf) = custody.leaves.remove(&(carrier, path.clone())) {
                custody.leaves.insert((parameter.place, path), leaf);
            }
        }
    }
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
                .ok_or_else(|| invalid())?;
            let mut custody = states[dominator].clone().ok_or_else(|| invalid())?;
            // A dominating edge that binds this block's parameters carries
            // each whole owned move's loans onto the bound places. When the
            // dominator reaches this block through an intermediate block —
            // a merge point — no binding happens on its terminator and the
            // entry state stays its exit state unchanged.
            if let Some(edge) = blocks[dominator].nodes.last().and_then(|terminator| {
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
        // Only owned affine carriers may enter with loans; the verified
        // contract rejects every other reference-bearing entry parameter.
        if parameter.access != StructuralAccess::Owned
            || parameter.multiplicity != StructuralMultiplicity::Affine
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
        let Some(declaration) = types.iter().find(|declaration| declaration.id == current) else {
            return false;
        };
        let matching: Vec<&terminal_psi::StructuralFieldDeclaration> = match &declaration.shape {
            StructuralTypeShape::Record { fields } => fields.iter().collect(),
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
        .ok_or_else(|| invalid())?;
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
        .ok_or_else(|| invalid())?;
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
            let referent_shape = super::scalar_shape(value.scalar_type).ok_or_else(|| invalid())?;
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
                .ok_or_else(|| invalid())?;
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
        let reference_type = source_contract(caller, custody, argument.place)
            .and_then(|carrier| leaf_type(types, carrier, carrier_path))
            .ok_or_else(invalid)?;
        if argument.access != StructuralAccess::SharedBorrow
            || !terminal_semantics::reference_source_compatible(
                types.iter(),
                root_type,
                reference_type,
            )
        {
            return Err(invalid());
        }
    }
    Ok(target_operations::TargetStructuralArgument {
        place: *root,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type: root_type,
        structural_type: declaration.structural_type,
        shape: destination.shape,
        source_byte_offset: 0,
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
    let mut roots = BTreeSet::new();
    let mut expected = Vec::new();
    for mapping in &callee_result.reference_sources {
        let expected_referent =
            leaf_referent(types, result_type, &mapping.path).ok_or_else(|| invalid())?;
        validate_alternatives(mapping, expected_referent, types)?;
        let mut candidates = Vec::with_capacity(mapping.sources.len());
        for formal in &mapping.sources {
            let source = call_source(callee, arguments, formal).ok_or_else(invalid)?;
            if reference_source_type(caller, custody, types, &source) != Some(expected_referent) {
                return Err(invalid());
            }
            let (root, parent) = normalized_source(caller, custody, types, &source)?;
            if mapping.sources.len() > 1 && parent.is_some() {
                return Err(invalid());
            }
            candidates.push(root);
        }
        let mut candidates = candidates.into_iter();
        let mut root = candidates.next().ok_or_else(invalid)?;
        let mut paths = Vec::new();
        for candidate in candidates {
            let Root::Ingress(place, path) = candidate else {
                return Err(invalid());
            };
            if place != root.place() {
                return Err(invalid());
            }
            paths.push(path);
        }
        if !paths.is_empty() {
            let Root::Ingress(place, path) = root else {
                return Err(invalid());
            };
            paths.push(path);
            paths.sort();
            if paths.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(invalid());
            }
            root = Root::Alternatives(place, paths);
        }
        if roots.iter().any(|other| roots_overlap(other, &root)) {
            return Err(invalid());
        }
        roots.insert(root.clone());
        expected.push(target_operations::TargetReferenceResult {
            path: mapping.path.clone(),
            root: root.place(),
            root_paths: root.paths(),
        });
    }
    Ok(expected)
}
