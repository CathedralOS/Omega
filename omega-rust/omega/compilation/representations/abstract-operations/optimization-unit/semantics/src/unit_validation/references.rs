//! Reference-carrier custody helpers mirrored from the verified Terminal
//! contract. A carrier owns loan permission, never its referent's storage;
//! every origin and parent is reconstructed from operations and signatures.
use crate::OptimizationUnitValidationError;
use crate::unit_validation::operation_contracts::{constructible, structural_source_contract};
use abstract_operations::AbstractOperation as O;
use abstract_operations::AbstractStructuralCallResult;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};

/// Stable loan identity: the place and path where the carrier was formed.
/// Owned moves relocate the carrier, never this identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ReferenceIdentity {
    pub(crate) place: PlaceId,
    pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReferenceParent {
    Root(ReferenceOrigin),
    Reference(ReferenceIdentity),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ReferenceOrigin {
    Primitive(PlaceId),
    /// An internal owned ingress assumes one existing permission per leaf.
    IngressLeaf(ReferenceIdentity),
    /// One selected referent, conservatively suspending every possible path.
    /// These paths belong to one permission carrier, not separate loans.
    Alternatives {
        place: PlaceId,
        paths: Vec<Vec<terminal_psi::StructuralPathSegment>>,
    },
}

/// One outstanding loan: which carrier leaf holds it, which original referent
/// root it suspends, and which parent it descends from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiveReference {
    pub(crate) identity: ReferenceIdentity,
    pub(crate) carrier: PlaceId,
    pub(crate) carrier_path: Vec<terminal_psi::StructuralPathSegment>,
    pub(crate) root: ReferenceOrigin,
    pub(crate) parent: ReferenceParent,
}

/// Inspect owned containment only. A reference's referent is not its payload.
pub(crate) fn contains_reference(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    root: StructuralTypeId,
) -> bool {
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
                terminal_psi::StructuralFieldType::Structural(child) => Some(child),
                _ => None,
            }));
        };
        match &declaration.shape {
            terminal_psi::StructuralTypeShape::Reference { .. } => return true,
            terminal_psi::StructuralTypeShape::Record { fields } => push_fields(fields),
            terminal_psi::StructuralTypeShape::Sum { cases } => {
                for case in cases {
                    push_fields(&case.fields);
                }
            }
            terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
                push_fields(fields);
                for case in cases {
                    push_fields(&case.fields);
                }
            }
            terminal_psi::StructuralTypeShape::FixedArray { element, .. }
            | terminal_psi::StructuralTypeShape::ElementView { element } => pending.push(*element),
            terminal_psi::StructuralTypeShape::FusedServiceBinding
            | terminal_psi::StructuralTypeShape::PrimitiveScalar(_)
            | terminal_psi::StructuralTypeShape::ByteSequence(_) => {}
        }
    }
    false
}

/// Exact declaration-order reference leaves of the supported owned shape.
/// `maximum_leaves` bounds reconstruction exactly like the verified rule.
pub(crate) fn leaf_paths(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    root: StructuralTypeId,
    maximum_leaves: usize,
) -> Option<Vec<Vec<terminal_psi::StructuralPathSegment>>> {
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
            terminal_psi::StructuralTypeShape::Reference { .. } => {
                if output.len() == maximum_leaves {
                    return None;
                }
                output.push(path);
            }
            terminal_psi::StructuralTypeShape::Record { fields } => {
                for field in fields.iter().rev() {
                    if let terminal_psi::StructuralFieldType::Structural(child) = field.field_type {
                        if !contains_reference(types, child) {
                            continue;
                        }
                        // Every pending owned subtree owes at least one leaf.
                        // Bound the worklist before a wide DAG can amplify it.
                        if pending.len().checked_add(output.len())? >= maximum_leaves {
                            return None;
                        }
                        let mut child_path = path.clone();
                        child_path.push(terminal_psi::StructuralPathSegment::Field(
                            field.identity.clone(),
                        ));
                        pending.push((child, child_path));
                    }
                }
            }
            // Case payloads hold leaves under field-identity paths as well.
            // Identical segments across cases collapse into the single leaf
            // the authored path names.
            terminal_psi::StructuralTypeShape::Sum { cases } => {
                for field in cases.iter().flat_map(|case| &case.fields).rev() {
                    if let terminal_psi::StructuralFieldType::Structural(child) = field.field_type {
                        if !contains_reference(types, child) {
                            continue;
                        }
                        if pending.len().checked_add(output.len())? >= maximum_leaves {
                            return None;
                        }
                        let mut child_path = path.clone();
                        child_path.push(terminal_psi::StructuralPathSegment::Field(
                            field.identity.clone(),
                        ));
                        pending.push((child, child_path));
                    }
                }
            }
            terminal_psi::StructuralTypeShape::Mixed { fields, cases } => {
                for field in cases
                    .iter()
                    .flat_map(|case| &case.fields)
                    .chain(fields)
                    .rev()
                {
                    if let terminal_psi::StructuralFieldType::Structural(child) = field.field_type {
                        if !contains_reference(types, child) {
                            continue;
                        }
                        if pending.len().checked_add(output.len())? >= maximum_leaves {
                            return None;
                        }
                        let mut child_path = path.clone();
                        child_path.push(terminal_psi::StructuralPathSegment::Field(
                            field.identity.clone(),
                        ));
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
/// scalar or a plain owned record referent, never ownership of that storage.
/// Shared, mutable and write-only carriers share the same loan structure; the
/// access stored on the carrier, not the referent identity, separates them.
pub(crate) fn referent(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    let declaration = types.get(&structural_type)?;
    match declaration.shape {
        terminal_psi::StructuralTypeShape::Reference {
            referent,
            access:
                terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow,
        } if matches!(
            types.get(&referent).map(|declaration| &declaration.shape),
            Some(terminal_psi::StructuralTypeShape::PrimitiveScalar(_))
        ) || (access_is_mutable_record(types, structural_type, referent)) =>
        {
            Some(referent)
        }
        _ => None,
    }
}

fn access_is_mutable_record(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    carrier: StructuralTypeId,
    referent: StructuralTypeId,
) -> bool {
    reference_access(types, carrier) == Some(terminal_psi::StructuralAccess::MutableBorrow)
        && matches!(
            types.get(&referent).map(|row| &row.shape),
            Some(terminal_psi::StructuralTypeShape::Record { .. })
        )
        && plain_referent_storage(types, referent)
}

/// The Terminal plain-owned referent rule, independently reconstructed from
/// declarations. Byte predicates constrain owned bytes; views and references
/// instead carry separate permissions and cannot be absorbed into this rule.
fn plain_referent_storage(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    root: StructuralTypeId,
) -> bool {
    use terminal_psi::{ByteSequenceCarrier, StructuralFieldType, StructuralTypeShape};
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
        let Some(declaration) = types.get(&current) else {
            return false;
        };
        pending.push((current, true));
        let fields: Vec<_> = match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. }) => {
                continue;
            }
            StructuralTypeShape::Record { fields } => fields.iter().collect(),
            StructuralTypeShape::Sum { cases } => {
                cases.iter().flat_map(|case| &case.fields).collect()
            }
            StructuralTypeShape::Mixed { fields, cases } => fields
                .iter()
                .chain(cases.iter().flat_map(|case| &case.fields))
                .collect(),
            StructuralTypeShape::FixedArray { element, .. } => {
                pending.push((*element, false));
                continue;
            }
            _ => return false,
        };
        for field in fields {
            if field.relevance.is_erased() {
                return false;
            }
            match &field.field_type {
                StructuralFieldType::Scalar(_)
                | StructuralFieldType::Atomic(_)
                | StructuralFieldType::BoundedInteger(_)
                | StructuralFieldType::IeeeFloat(_) => {}
                StructuralFieldType::Structural(child) => pending.push((*child, false)),
                StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. })
                | StructuralFieldType::PredicateByteSequence {
                    carrier: ByteSequenceCarrier::BoundedOwned { .. },
                    ..
                } => {}
                _ => return false,
            }
        }
    }
    true
}

/// Recover geometry only. The live-reference ledger independently validates
/// the carrier's active loan and rejects suspended or released permissions.
pub(crate) fn mutable_record_field_destination<'path>(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    destination: PlaceId,
    path: &'path [terminal_psi::StructuralPathSegment],
) -> Option<(
    StructuralTypeId,
    &'path [terminal_psi::StructuralPathSegment],
)> {
    let (terminal_psi::StructuralPathSegment::Referent, suffix) = path.split_first()? else {
        return None;
    };
    let source = structural_source_contract(function, destination)?;
    if source.access != terminal_psi::StructuralAccess::Owned
        || source.multiplicity != terminal_psi::StructuralMultiplicity::Affine
        || !source.is_unqualified()
        || source.has_result_evidence()
        || function
            .entry_claim_declarations
            .iter()
            .any(|claim| claim.input == destination)
        || function
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == destination)
    {
        return None;
    }
    let referent = referent(types, source.structural_type)?;
    access_is_mutable_record(types, source.structural_type, referent).then_some((referent, suffix))
}

/// The selected child a call may borrow through one returned mutable record
/// carrier. This is deliberately narrower than `reference_source_type`: it is
/// call-only geometry and must not make the same spelling an escaping
/// `EstablishReference` origin.
pub(crate) fn mutable_record_call_argument_type(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    argument: &terminal_psi::StructuralArgument,
) -> Option<StructuralTypeId> {
    if !matches!(
        argument.access,
        terminal_psi::StructuralAccess::SharedBorrow
            | terminal_psi::StructuralAccess::MutableBorrow
            | terminal_psi::StructuralAccess::WriteOnlyBorrow
    ) {
        return None;
    }
    let (referent, suffix) =
        mutable_record_field_destination(function, types, argument.place, &argument.path)?;
    (!suffix.is_empty()).then_some(())?;
    leaf_type(types, referent, suffix)
}

/// A field-only carrier path ending at a reference leaf, then its referent.
pub(crate) fn leaf_referent(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    current: StructuralTypeId,
    fields: &[terminal_psi::StructuralPathSegment],
) -> Option<StructuralTypeId> {
    let current = leaf_type(types, current, fields)?;
    // A referent projection over a leaf that is not itself a reference names
    // that leaf's own storage, mirroring the codec's structural-path arm. A
    // reference leaf over a record names its whole referent the same way: the
    // mapped source's type is the referent declaration, primitive or not.
    referent(types, current).or_else(|| {
        types
            .get(&current)
            .and_then(|declaration| match declaration.shape {
                terminal_psi::StructuralTypeShape::Reference { referent, .. } => Some(referent),
                _ => Some(current),
            })
    })
}

/// The leaf type declared at `fields` below `current`: the carrier a
/// `[fields.., Referent]` projection dereferences. Scalar members name the
/// catalog's matching primitive storage declaration.
pub(crate) fn leaf_type(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    mut current: StructuralTypeId,
    fields: &[terminal_psi::StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for segment in fields {
        if let terminal_psi::StructuralPathSegment::FixedIndex(index) = segment {
            let declaration = types.get(&current)?;
            let terminal_psi::StructuralTypeShape::FixedArray { element, length } =
                &declaration.shape
            else {
                return None;
            };
            if index >= length {
                return None;
            }
            current = *element;
            continue;
        }
        let terminal_psi::StructuralPathSegment::Field(identity) = segment else {
            return None;
        };
        let declaration = types.get(&current)?;
        // Field segments name fields by identity; a sum or mixed payload keeps
        // a segment legal while every matching case field resolves to one
        // structural type.
        let matching: Vec<&terminal_psi::StructuralFieldDeclaration> = match &declaration.shape {
            terminal_psi::StructuralTypeShape::Record { fields } => fields.iter().collect(),
            terminal_psi::StructuralTypeShape::Sum { cases } => {
                cases.iter().flat_map(|case| case.fields.iter()).collect()
            }
            terminal_psi::StructuralTypeShape::Mixed { fields, cases } => fields
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
                terminal_psi::StructuralFieldType::Structural(child) => *child,
                // A scalar member names the catalog's matching primitive
                // storage declaration; borrowing it carries that leaf type,
                // not a reference formed at the member.
                terminal_psi::StructuralFieldType::Scalar(scalar) => types
                    .iter()
                    .find(|(_, declaration)| {
                        matches!(
                            declaration.shape,
                            terminal_psi::StructuralTypeShape::PrimitiveScalar(shape)
                                if shape == *scalar
                        )
                    })
                    .map(|(id, _)| *id)?,
                _ => return None,
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

/// The access a reference type grants over its referent.
pub(crate) fn reference_access(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
) -> Option<terminal_psi::StructuralAccess> {
    types
        .get(&structural_type)
        .and_then(|declaration| match declaration.shape {
            terminal_psi::StructuralTypeShape::Reference { access, .. } => Some(access),
            _ => None,
        })
}

/// Loan strength: whether a place held with `authority` may lend `requested`.
/// Ownership lends any loan, a mutable loan may be reborrowed shared or
/// write-only, and a shared loan lends only a shared one.
fn access_covers(
    authority: terminal_psi::StructuralAccess,
    requested: terminal_psi::StructuralAccess,
) -> bool {
    authority == requested
        || matches!(
            (authority, requested),
            (terminal_psi::StructuralAccess::Owned, _)
                | (
                    terminal_psi::StructuralAccess::MutableBorrow,
                    terminal_psi::StructuralAccess::SharedBorrow
                        | terminal_psi::StructuralAccess::WriteOnlyBorrow
                )
        )
}

/// Whether a mapped source's access stays within what the result leaf grants:
/// the leaf at `path` is a reference and the source's access sits at or below
/// its declared loan.
pub(crate) fn leaf_access_within(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    source: terminal_psi::StructuralAccess,
) -> bool {
    leaf_type(types, root, path)
        .and_then(|leaf| reference_access(types, leaf))
        .is_some_and(|granted| access_covers(source, granted))
}

/// The referent type behind a `.., Referent` projection, resolved through the
/// carrier root's signature and a field-only prefix.
pub(crate) fn projected_carrier_type(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    source: &terminal_psi::StructuralArgument,
) -> Option<StructuralTypeId> {
    let (terminal_psi::StructuralPathSegment::Referent, fields) = source.path.split_last()? else {
        return None;
    };
    let current = structural_source_contract(function, source.place)?.structural_type;
    leaf_referent(types, current, fields)
}

/// Whether this argument projects through a reference carrier it borrows.
pub(crate) fn is_reference_projection(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    source: &terminal_psi::StructuralArgument,
) -> bool {
    projected_carrier_type(function, types, source).is_some()
        && source.access != terminal_psi::StructuralAccess::Owned
}

/// Direct storage loans use the same contract for arrival parameters and local
/// operation results. Their field/index paths never dereference a permission
/// carrier; those projections end in `Referent` and retain their own checks.
pub(crate) fn reference_source_type(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    source: &terminal_psi::StructuralArgument,
) -> Option<StructuralTypeId> {
    if !matches!(
        source.access,
        terminal_psi::StructuralAccess::SharedBorrow
            | terminal_psi::StructuralAccess::MutableBorrow
            | terminal_psi::StructuralAccess::WriteOnlyBorrow
    ) {
        return None;
    }
    if source.path.iter().all(|segment| {
        matches!(
            segment,
            terminal_psi::StructuralPathSegment::Field(_)
                | terminal_psi::StructuralPathSegment::FixedIndex(_)
        )
    }) {
        let contract = structural_source_contract(function, source.place)?;
        if !access_covers(contract.access, source.access)
            || contract.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            || !contract.is_unqualified()
            || contract.has_result_evidence()
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
        return leaf_type(types, contract.structural_type, &source.path);
    }
    projected_carrier_type(function, types, source)
}

fn direct_reference_parameter<'a>(
    function: &'a PsiOptimizationFunction,
    source: &terminal_psi::StructuralArgument,
) -> Option<&'a terminal_psi::StructuralParameterDeclaration> {
    let parameter = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source.place)?;
    (matches!(
        parameter.access,
        terminal_psi::StructuralAccess::MutableBorrow
            | terminal_psi::StructuralAccess::SharedBorrow
    ) && access_covers(parameter.access, source.access)
        && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && function
            .entry_claim_declarations
            .iter()
            .all(|claim| claim.input != source.place)
        && function
            .content_entry_claims
            .iter()
            .all(|claim| claim.input.root != source.place))
    .then_some(parameter)
}

/// An owned argument that carries at least one reference leaf moves the loans
/// with it; the callers' ingress replay tracks each leaf exactly.
pub(crate) fn argument_owns_references(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    argument: &terminal_psi::StructuralArgument,
) -> bool {
    argument.access == terminal_psi::StructuralAccess::Owned
        && structural_source_contract(function, argument.place)
            .is_some_and(|source| contains_reference(types, source.structural_type))
}

/// The formal origin a callee result mapping names: a mutable primitive
/// parameter or an owned-carrier ingress leaf path.
pub(crate) fn formal_origin(
    function: &PsiOptimizationFunction,
    source: &terminal_psi::StructuralArgument,
) -> Option<ReferenceOrigin> {
    let parameter = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source.place)?;
    if source.path.is_empty()
        && matches!(
            parameter.access,
            terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::SharedBorrow
        )
    {
        Some(ReferenceOrigin::Primitive(source.place))
    } else if let Some((terminal_psi::StructuralPathSegment::Referent, path)) =
        source.path.split_last()
        && matches!(
            parameter.access,
            terminal_psi::StructuralAccess::Owned | terminal_psi::StructuralAccess::SharedBorrow
        )
    {
        Some(ReferenceOrigin::IngressLeaf(ReferenceIdentity {
            place: source.place,
            path: path.to_vec(),
        }))
    } else if !source.path.is_empty()
        && direct_reference_parameter(function, source).is_some()
        && source.path.iter().all(|segment| {
            matches!(
                segment,
                terminal_psi::StructuralPathSegment::Field(_)
                    | terminal_psi::StructuralPathSegment::FixedIndex(_)
            )
        })
    {
        Some(ReferenceOrigin::IngressLeaf(ReferenceIdentity {
            place: source.place,
            path: source.path.clone(),
        }))
    } else {
        None
    }
}

/// The structural result row an operation publishes, when it publishes one.
/// Reference custody cares only about the structural contract; scalar and
/// Unit results carry no loans.
fn operation_structural_result(operation: &O) -> Option<&terminal_psi::StructuralOperationResult> {
    match operation {
        O::EstablishPrimitiveLocal { result, .. }
        | O::ByteSequenceSubslice { result, .. }
        | O::EstablishByteSequenceConcatenation { result, .. }
                | O::EstablishScalarArray { result, .. }
        | O::EstablishCase { result, .. }
        | O::EstablishRecord { result, .. }
        | O::EstablishZeroedStructural { result, .. }
        | O::EstablishReference { result, .. }
        // A window extraction publishes a structural result too; its type
        // joins the reference-bearing result contract like every other
        // producer's.
        | O::MoveStructuralField { result, .. }
        | O::CallStructural {
            result: AbstractStructuralCallResult::Structural { result, .. },
            ..
        } => Some(result),
        O::BoundaryCall {
            result: abstract_operations::AbstractBoundaryResult::Structural(result),
            ..
        } => Some(result),
        _ => None,
    }
}

/// Whole-function reference contract, mirrored from the verified machine
/// checks: which producers may publish reference-bearing results, what an
/// entry parameter carrying carriers must look like, and how a reference
/// result's declared source roster maps to formal ingress leaves.
pub(crate) fn validate_function_references(
    function: &PsiOptimizationFunction,
    functions: &BTreeMap<semantic_vocabulary::MachineId, &PsiOptimizationFunction>,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let invalid = || OptimizationUnitValidationError::StructuralCatalogMismatch {
        machine: Some(function.machine),
    };
    let is_reference = |structural_type| {
        types.get(&structural_type).is_some_and(|declaration| {
            matches!(
                declaration.shape,
                terminal_psi::StructuralTypeShape::Reference { .. }
            )
        })
    };
    for node in function.blocks.iter().flat_map(|block| &block.nodes) {
        let Some(result) = operation_structural_result(&node.operation) else {
            continue;
        };
        if contains_reference(types, result.structural_type)
            && !is_reference(result.structural_type)
            && (!matches!(
                node.operation,
                O::EstablishRecord { .. }
                    | O::CallStructural {
                        result: AbstractStructuralCallResult::Structural { .. },
                        ..
                    }
            ) || result.multiplicity != terminal_psi::StructuralMultiplicity::Affine
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty())
        {
            return Err(invalid());
        }
        if is_reference(result.structural_type)
            && !matches!(
                node.operation,
                O::EstablishReference { .. }
                    | O::CallStructural {
                        result: AbstractStructuralCallResult::Structural { .. },
                        ..
                    }
            )
        {
            return Err(invalid());
        }
    }
    for parameter in function
        .structural_parameters
        .iter()
        .filter(|parameter| contains_reference(types, parameter.structural_type))
    {
        if parameter.access != terminal_psi::StructuralAccess::Owned
            || parameter.multiplicity != terminal_psi::StructuralMultiplicity::Affine
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || !constructible(types, parameter.structural_type)
            || function
                .entry_claim_declarations
                .iter()
                .any(|claim| claim.input == parameter.place)
            || function
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == parameter.place)
        {
            return Err(invalid());
        }
    }
    // A block parameter may hold a whole owned reference record: the same
    // eligibility as an incoming parameter. The replay's live-reference rows
    // re-home onto the bound place at each edge, so edge custody, not the
    // parameter shape, carries the loan's exact evidence.
    for parameter in function
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters)
        .filter(|parameter| contains_reference(types, parameter.structural_type))
    {
        if parameter.access != terminal_psi::StructuralAccess::Owned
            || parameter.multiplicity != terminal_psi::StructuralMultiplicity::Affine
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || !constructible(types, parameter.structural_type)
        {
            return Err(invalid());
        }
    }
    // Binding any subtree of a reference-bearing root would need partial-move
    // custody, even when the selected target parameter itself has no references.
    // A whole owned argument moves the entire record — and every stored loan
    // with it — across the edge; a borrow lane or a projected path cannot.
    if function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .flat_map(|edge| &edge.structural_bindings)
        .any(|binding| {
            structural_source_contract(function, binding.argument.place)
                .is_some_and(|source| contains_reference(types, source.structural_type))
                && (binding.argument.access != terminal_psi::StructuralAccess::Owned
                    || !binding.argument.path.is_empty())
        })
    {
        return Err(invalid());
    }
    let Some(result) = function.result.structural() else {
        return Ok(());
    };
    if !contains_reference(types, result.structural_type) {
        return if result.reference_sources.is_empty() {
            Ok(())
        } else {
            Err(invalid())
        };
    }
    let constructible_result = match types
        .get(&result.structural_type)
        .map(|declaration| &declaration.shape)
    {
        Some(terminal_psi::StructuralTypeShape::Reference { referent, .. }) => {
            constructible(types, *referent)
        }
        _ => false,
    };
    if referent(types, result.structural_type).is_none()
        && !constructible(types, result.structural_type)
        && !constructible_result
    {
        return Err(invalid());
    }
    if result.multiplicity != terminal_psi::StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
    {
        return Err(invalid());
    }
    let mut paths = leaf_paths(
        types,
        result.structural_type,
        result.reference_sources.len(),
    )
    .ok_or_else(invalid)?;
    // Result sources are a canonical map, not a lifecycle schedule. Constructors
    // and cleanup keep declaration order; only this interface comparison sorts.
    paths.sort();
    if paths.len() != result.reference_sources.len() {
        return Err(invalid());
    }
    if result
        .reference_sources
        .iter()
        .any(|mapping| mapping.sources.len() > 1)
    {
        validate_alternative_result_dependencies(function, functions, types)?;
    }
    let mut sources = BTreeSet::new();
    for (path, mapping) in paths.iter().zip(&result.reference_sources) {
        let expected_referent =
            leaf_referent(types, result.structural_type, path).ok_or_else(invalid)?;
        if mapping.path != *path
            || mapping.sources.is_empty()
            || mapping.sources.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid());
        }
        if mapping.sources.len() > 1 {
            let first = &mapping.sources[0];
            if !path.is_empty()
                || !types.get(&expected_referent).is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::Record { .. }
                    )
                })
                || reference_access(types, result.structural_type)
                    != Some(terminal_psi::StructuralAccess::MutableBorrow)
                || mapping.sources.iter().any(|source| {
                    source.place != first.place
                        || source.access != terminal_psi::StructuralAccess::MutableBorrow
                        || source.path.is_empty()
                        || source.path.iter().any(|segment| {
                            !matches!(
                                segment,
                                terminal_psi::StructuralPathSegment::Field(_)
                                    | terminal_psi::StructuralPathSegment::FixedIndex(_)
                            )
                        })
                })
            {
                return Err(invalid());
            }
        }
        for formal in &mapping.sources {
            if !matches!(
                formal.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::SharedBorrow
            ) || !sources.insert((formal.place, formal.path.clone()))
                || !function
                    .structural_parameters
                    .iter()
                    .any(|parameter| parameter.place == formal.place)
                || reference_source_type(function, types, formal) != Some(expected_referent)
                || !leaf_access_within(types, result.structural_type, path, formal.access)
                || formal_origin(function, formal).is_none()
            {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

/// Candidate coverage must ultimately come from bodies, not circular result
/// declarations. Reject every reachable reference-result cycle, including a
/// singleton descendant's recursion; shared acyclic dependencies remain valid.
fn validate_alternative_result_dependencies(
    function: &PsiOptimizationFunction,
    functions: &BTreeMap<semantic_vocabulary::MachineId, &PsiOptimizationFunction>,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let invalid = || OptimizationUnitValidationError::StructuralCatalogMismatch {
        machine: Some(function.machine),
    };
    let mut pending = vec![(function.machine, false)];
    let mut active = BTreeSet::new();
    let mut completed = BTreeSet::new();
    while let Some((current, exiting)) = pending.pop() {
        if exiting {
            active.remove(&current);
            completed.insert(current);
            continue;
        }
        if completed.contains(&current) {
            continue;
        }
        if !active.insert(current) {
            return Err(invalid());
        }
        pending.push((current, true));
        let current = functions.get(&current).ok_or_else(invalid)?;
        for node in current.blocks.iter().flat_map(|block| &block.nodes) {
            if let O::CallStructural {
                callee,
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            } = &node.operation
                && contains_reference(types, result.structural_type)
            {
                pending.push((*callee, false));
            }
        }
    }
    Ok(())
}
