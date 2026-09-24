//! Reference-carrier custody helpers mirrored from the verified Terminal
//! contract. A carrier owns loan permission, never its referent's storage;
//! every origin and parent is reconstructed from operations and signatures.
use crate::OptimizationUnitValidationError;
use crate::unit_validation::operation_contracts::{
    constructible_record, structural_source_contract,
};
use abstract_operations::AbstractOperation as O;
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
            terminal_psi::StructuralTypeShape::PrimitiveScalar(_)
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
            _ => {}
        }
    }
    Some(output)
}

/// A whole-carrier result admits an exclusive permission over a primitive
/// scalar referent; that is the complete established shape this layer replays.
/// Mutable and write-only carriers share the same loan structure; the access
/// stored on the carrier, not the referent identity, separates them.
pub(crate) fn referent(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    let declaration = types.get(&structural_type)?;
    match declaration.shape {
        terminal_psi::StructuralTypeShape::Reference {
            referent,
            access:
                terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow,
        } if matches!(
            types.get(&referent).map(|declaration| &declaration.shape),
            Some(terminal_psi::StructuralTypeShape::PrimitiveScalar(_))
        ) =>
        {
            Some(referent)
        }
        _ => None,
    }
}

/// A field-only carrier path ending at a reference leaf, then its referent.
pub(crate) fn leaf_referent(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    mut current: StructuralTypeId,
    fields: &[terminal_psi::StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for segment in fields {
        let terminal_psi::StructuralPathSegment::Field(identity) = segment else {
            return None;
        };
        let declaration = types.get(&current)?;
        let terminal_psi::StructuralTypeShape::Record { fields } = &declaration.shape else {
            return None;
        };
        let field = fields.iter().find(|field| &field.identity == identity)?;
        if field.relevance.is_erased() {
            return None;
        }
        let terminal_psi::StructuralFieldType::Structural(child) = field.field_type else {
            return None;
        };
        current = child;
    }
    referent(types, current)
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
    let current = structural_source_contract(function, source.place, false)?.structural_type;
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

/// The primitive referent type an establishment or projection may name. Empty
/// paths name whole primitive places; nonempty paths must end in `Referent`.
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
    if !source.path.is_empty() {
        return projected_carrier_type(function, types, source);
    }
    if let Some(parameter) = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source.place)
    {
        if parameter.access != terminal_psi::StructuralAccess::MutableBorrow
            || parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
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
        return primitive_scalar_type(types, parameter.structural_type);
    }
    let result = primitive_local_result(function, source.place)?;
    primitive_scalar_type(types, result.structural_type)
}

fn primitive_scalar_type(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    matches!(
        types
            .get(&structural_type)
            .map(|declaration| &declaration.shape),
        Some(terminal_psi::StructuralTypeShape::PrimitiveScalar(_))
    )
    .then_some(structural_type)
}

fn primitive_local_result(
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<&terminal_psi::StructuralOperationResult> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            O::EstablishPrimitiveLocal { result, .. } if result.place == place => Some(result),
            _ => None,
        })
}

/// An owned argument that carries at least one reference leaf moves the loans
/// with it; the callers' ingress replay tracks each leaf exactly.
pub(crate) fn argument_owns_references(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    argument: &terminal_psi::StructuralArgument,
) -> bool {
    argument.access == terminal_psi::StructuralAccess::Owned
        && structural_source_contract(function, argument.place, false)
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
    if source.path.is_empty() && parameter.access == terminal_psi::StructuralAccess::MutableBorrow {
        Some(ReferenceOrigin::Primitive(source.place))
    } else if let Some((terminal_psi::StructuralPathSegment::Referent, path)) =
        source.path.split_last()
        && parameter.access == terminal_psi::StructuralAccess::Owned
    {
        Some(ReferenceOrigin::IngressLeaf(ReferenceIdentity {
            place: source.place,
            path: path.to_vec(),
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
        | O::EstablishScalarArray { result, .. }
        | O::EstablishScalarCase { result, .. }
        | O::EstablishRecord { result, .. }
        | O::EstablishReference { result, .. }
        // A window extraction publishes a structural result too; its type
        // joins the reference-bearing result contract like every other
        // producer's.
        | O::MoveStructuralField { result, .. }
        | O::CallStructural { result, .. } => Some(result),
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
                O::EstablishRecord { .. } | O::CallStructural { .. }
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
                O::EstablishReference { .. } | O::CallStructural { .. }
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
            || !constructible_record(types, parameter.structural_type)
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
    if function
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters)
        .any(|parameter| contains_reference(types, parameter.structural_type))
    {
        return Err(invalid());
    }
    // Binding any subtree of a reference-bearing root would need partial-move
    // custody, even when the selected target parameter itself has no references.
    if function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .flat_map(|node| &node.successors)
        .flat_map(|edge| &edge.structural_bindings)
        .any(|binding| {
            structural_source_contract(function, binding.argument.place, false)
                .is_some_and(|source| contains_reference(types, source.structural_type))
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
    if referent(types, result.structural_type).is_none()
        && !constructible_record(types, result.structural_type)
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
    let mut sources = BTreeSet::new();
    for (path, mapping) in paths.iter().zip(&result.reference_sources) {
        let expected_referent =
            leaf_referent(types, result.structural_type, path).ok_or_else(invalid)?;
        if mapping.path != *path
            || mapping.source.access != terminal_psi::StructuralAccess::MutableBorrow
            || !sources.insert((mapping.source.place, mapping.source.path.clone()))
            || !function
                .structural_parameters
                .iter()
                .any(|parameter| parameter.place == mapping.source.place)
            || reference_source_type(function, types, &mapping.source) != Some(expected_referent)
            || formal_origin(function, &mapping.source).is_none()
        {
            return Err(invalid());
        }
    }
    Ok(())
}
