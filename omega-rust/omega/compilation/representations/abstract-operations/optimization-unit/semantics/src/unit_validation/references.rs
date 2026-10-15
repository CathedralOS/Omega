//! Reference-carrier custody helpers mirrored from the verified Terminal
//! contract. A carrier owns loan permission, never its referent's storage;
//! every origin and parent is reconstructed from operations and signatures.
use crate::OptimizationUnitValidationError;
use crate::unit_validation::catalog_mismatch;
use crate::unit_validation::operation_contracts::{constructible, structural_source_contract};
use abstract_operations::AbstractOperation as O;
use abstract_operations::AbstractStructuralCallResult;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::StructuralAccess;

mod access_paths;
pub(crate) use access_paths::{
    access_path_type, access_referent_tail, call_source_type, project_referent_tail,
};

/// Stable loan identity: the place and path where the carrier was formed.
/// Owned moves relocate the carrier, never this identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ReferenceIdentity {
    pub(crate) place: PlaceId,
    pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
}

/// Provenance can name an element family at a call boundary. Carrier identity
/// and parent resources remain exact executable paths; they never use this
/// coordinate vocabulary to manufacture a descriptor or address.
/// Region extent belongs to each alternative: a join cannot turn an unknown
/// subregion into an exact descriptor resource just because its path is empty.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ReferenceOriginIdentity {
    pub(crate) place: PlaceId,
    pub(crate) path: Vec<terminal_psi::ReferenceOriginProjection>,
    pub(crate) extent: semantic_vocabulary::ReferenceOriginExtent,
}

impl From<ReferenceIdentity> for ReferenceOriginIdentity {
    fn from(identity: ReferenceIdentity) -> Self {
        Self {
            place: identity.place,
            extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
            path: identity
                .path
                .into_iter()
                .map(terminal_psi::ReferenceOriginProjection::Exact)
                .collect(),
        }
    }
}

impl ReferenceOriginIdentity {
    pub(crate) fn exact_identity(&self) -> Option<ReferenceIdentity> {
        if self.extent != semantic_vocabulary::ReferenceOriginExtent::Projected {
            return None;
        }
        Some(ReferenceIdentity {
            place: self.place,
            path: self
                .path
                .iter()
                .map(|projection| match projection {
                    terminal_psi::ReferenceOriginProjection::Exact(segment) => {
                        Some(segment.clone())
                    }
                    terminal_psi::ReferenceOriginProjection::Element => None,
                })
                .collect::<Option<Vec<_>>>()?,
        })
    }
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
    IngressLeaf(ReferenceOriginIdentity),
    /// One selected referent, conservatively suspending every possible path.
    /// These origins belong to one permission carrier, not separate loans.
    Alternatives {
        place: PlaceId,
        origins: Vec<ReferenceOriginIdentity>,
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
    terminal_psi::structural_type_contains_reference_by(|id| types.get(&id).copied(), root)
}

/// Exact owned reference leaves, shared with requirement-region validation.
pub(crate) fn leaf_paths(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    root: StructuralTypeId,
    maximum_leaves: usize,
) -> Option<Vec<Vec<terminal_psi::StructuralPathSegment>>> {
    Some(
        terminal_semantics::reference_leaves(types.values().copied(), root, maximum_leaves)?
            .into_iter()
            .map(|leaf| leaf.path)
            .collect(),
    )
}

/// A carrier identifies its declared referent, never ownership of that storage.
/// Every borrowed access preserves the full referent type; operations separately
/// check their required access, live backing and displaced-content restrictions.
pub(crate) fn referent(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    match types.get(&structural_type)?.shape {
        terminal_psi::StructuralTypeShape::Reference {
            referent,
            access:
                terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow,
        } => types.contains_key(&referent).then_some(referent),
        _ => None,
    }
}

/// Resolve the declared access of a carrier through its stored
/// pointers. Owning the carrier is not required to write its referent. This
/// declaration walk supplies no availability or live-loan evidence. Memberships
/// do not select geometry; each write separately preserves the original root
/// roster at its complete typed path, including reference crossings.
pub fn reference_projection(
    function: &PsiOptimizationFunction,
    types: &[terminal_psi::StructuralTypeDeclaration],
    destination: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    required_access: terminal_psi::StructuralAccess,
) -> Option<StructuralTypeId> {
    let source = structural_source_contract(function, destination)?;
    (source.multiplicity != terminal_psi::StructuralMultiplicity::Linear
        && !source.has_result_evidence()
        && terminal_semantics::reference_source_projection_accessible(
            types.iter(),
            source.structural_type,
            source.multiplicity,
            source.access,
            path,
            required_access,
        )
        && function
            .entry_claim_declarations
            .iter()
            .all(|claim| claim.input != destination)
        && function
            .content_entry_claims
            .iter()
            .all(|claim| claim.input.root != destination))
    .then_some(source.structural_type)
}

/// Declared geometry and attenuated access through stored reference paths.
/// This declaration supplies no live loan or authority over displaced contents;
/// ordered replay authenticates each descriptor and transfers its leaf roster.
pub(crate) fn record_field_destination<'path>(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    destination: PlaceId,
    path: &'path [terminal_psi::StructuralPathSegment],
    required_access: terminal_psi::StructuralAccess,
) -> Option<(
    StructuralTypeId,
    &'path [terminal_psi::StructuralPathSegment],
)> {
    if !path.contains(&terminal_psi::StructuralPathSegment::Referent) {
        return None;
    }
    let (referent, prefix) = access_referent_tail(
        function,
        types,
        &terminal_psi::StructuralArgument {
            place: destination,
            path: path.to_vec(),
            access: required_access,
        },
    )?;
    Some((referent, &path[prefix..]))
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
    let mut payload_fields = None;
    for segment in fields {
        if let terminal_psi::StructuralPathSegment::Case(case) = segment {
            if payload_fields.is_some() {
                return None;
            }
            let cases = types.get(&current)?.shape.sum_cases()?;
            let mut matching = cases.iter().filter(|candidate| candidate.id == *case);
            payload_fields = Some(matching.next()?.fields.as_slice());
            if matching.next().is_some() {
                return None;
            }
            continue;
        }
        if payload_fields.is_some()
            && !matches!(segment, terminal_psi::StructuralPathSegment::Field(_))
        {
            return None;
        }
        if let terminal_psi::StructuralPathSegment::RepresentationView {
            structural_type,
            stored_scalar,
        } = segment
        {
            if matches!(
                types.get(&current)?.shape,
                terminal_psi::StructuralTypeShape::Reference { .. }
            ) {
                return None;
            }
            current = terminal_semantics::representation_view_declaration(
                types.values().copied(),
                *structural_type,
                *stored_scalar,
            )?
            .id;
            continue;
        }
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
        // Case selection establishes the payload namespace. An unguarded
        // field belongs only to the record's unconditional fields; scanning
        // all cases would merge distinct leaves that reuse a field identity.
        let selected_fields = payload_fields
            .take()
            .or_else(|| declaration.shape.direct_fields())?;
        let field = terminal_psi::live_field(selected_fields, *identity)?;
        current = match &field.field_type {
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
    }
    payload_fields.is_none().then_some(current)
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
pub(crate) fn access_covers(
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

/// Resolve an admitted storage region without granting reference formation.
/// Ordinary calls retain their own access, case and index obligations; result
/// formation separately applies `reference_source_type`. Rebinding a borrowed
/// name must preserve its complete path, including selected payloads and indices.
pub(crate) fn storage_projection(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    source: &terminal_psi::StructuralArgument,
) -> Option<super::structural_catalog::StructuralStorageProjection> {
    if source.path.last() == Some(&terminal_psi::StructuralPathSegment::Referent) {
        return projected_carrier_type(function, types, source)
            .map(super::structural_catalog::StructuralStorageProjection::Endpoint);
    }
    let contract = structural_source_contract(function, source.place)?;
    super::structural_catalog::structural_storage_projection(
        types.values().copied(),
        contract.structural_type,
        &source.path,
    )
}

/// Authenticate the supplying authority independently of its endpoint type.
/// Inline storage has no nominal leaf; that must neither prevent a lawful
/// loan nor bypass root custody and each stored reference's access ceiling.
pub(crate) fn reference_source_accessible(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    source: &terminal_psi::StructuralArgument,
) -> bool {
    if source.access == terminal_psi::StructuralAccess::Owned {
        return false;
    }
    let Some(contract) = structural_source_contract(function, source.place) else {
        return false;
    };
    if contract.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || contract.has_result_evidence()
    {
        return false;
    }
    let path = terminal_semantics::reference_source_storage_path(
        types.values().copied(),
        contract.structural_type,
        &source.path,
    );
    if !terminal_semantics::reference_source_projection_accessible(
        types.values().copied(),
        contract.structural_type,
        contract.multiplicity,
        contract.access,
        path,
        source.access,
    ) {
        return false;
    }
    path.contains(&terminal_psi::StructuralPathSegment::Referent)
        || (function
            .entry_claim_declarations
            .iter()
            .all(|claim| claim.input != source.place)
            && function
                .content_entry_claims
                .iter()
                .all(|claim| claim.input.root != source.place))
}

/// Direct storage loans use the same contract for arrival parameters and local
/// operation results. Their field/index paths never dereference a permission
/// carrier; those projections end in `Referent` and retain their own checks.
pub(crate) fn reference_source_type(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    source: &terminal_psi::StructuralArgument,
) -> Option<StructuralTypeId> {
    if !reference_source_accessible(function, types, source) {
        return None;
    }
    let contract = structural_source_contract(function, source.place)?;
    let path = terminal_semantics::reference_source_storage_path(
        types.values().copied(),
        contract.structural_type,
        &source.path,
    );
    if !source
        .path
        .contains(&terminal_psi::StructuralPathSegment::Referent)
    {
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
        return super::structural_catalog::structural_borrowed_storage_projection(
            types.values().copied(),
            contract.structural_type,
            &source.path,
            source.access,
        )
        .and_then(|projection| match projection {
            super::structural_catalog::StructuralStorageProjection::Endpoint(endpoint) => {
                Some(endpoint)
            }
            _ => None,
        });
    }
    let source = terminal_psi::StructuralArgument {
        place: source.place,
        access: source.access,
        path: path.to_vec(),
    };
    projected_carrier_type(function, types, &source)
}

fn direct_reference_parameter<'a>(
    function: &'a PsiOptimizationFunction,
    source: &terminal_psi::StructuralArgument,
) -> Option<&'a terminal_psi::StructuralParameterDeclaration> {
    let parameter = function.structural_parameter(source.place)?;
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

/// The formal origin a callee result mapping names: a mutable primitive
/// parameter or an owned-carrier ingress leaf path.
pub(crate) fn formal_origin(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    function: &PsiOptimizationFunction,
    source: &terminal_psi::StructuralArgument,
) -> Option<ReferenceOrigin> {
    let parameter = function.structural_parameter(source.place)?;
    let path = terminal_semantics::reference_source_storage_path(
        types.values().copied(),
        parameter.structural_type,
        &source.path,
    );
    if path.is_empty()
        && matches!(
            parameter.access,
            terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::SharedBorrow
        )
    {
        Some(ReferenceOrigin::Primitive(source.place))
    } else if let Some((terminal_psi::StructuralPathSegment::Referent, path)) = path.split_last()
        && matches!(
            parameter.access,
            terminal_psi::StructuralAccess::Owned | terminal_psi::StructuralAccess::SharedBorrow
        )
    {
        Some(ReferenceOrigin::IngressLeaf(
            ReferenceIdentity {
                place: source.place,
                path: path.to_vec(),
            }
            .into(),
        ))
    } else if !path.is_empty()
        && direct_reference_parameter(function, source).is_some()
        && path.iter().all(|segment| {
            matches!(
                segment,
                terminal_psi::StructuralPathSegment::Field(_)
                    | terminal_psi::StructuralPathSegment::FixedIndex(_)
                    | terminal_psi::StructuralPathSegment::RepresentationView { .. }
            )
        })
    {
        Some(ReferenceOrigin::IngressLeaf(
            ReferenceIdentity {
                place: source.place,
                path: path.to_vec(),
            }
            .into(),
        ))
    } else {
        None
    }
}

/// Split a formal summary at the supplying exact permission. The suffix is a
/// typed provenance relation over that permission, not an argument projection.
/// Selecting another stored descriptor requires its exact resource identity;
/// an element family cannot invent that identity.
pub(crate) struct FormalReferenceSource {
    pub(crate) endpoint: StructuralTypeId,
    pub(crate) extent: semantic_vocabulary::ReferenceOriginExtent,
    pub(crate) transfers: bool,
}

pub(crate) fn formal_reference_source(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    function: &PsiOptimizationFunction,
    origin: &terminal_psi::ReferenceOriginSource,
) -> Option<FormalReferenceSource> {
    let parameter = function.structural_parameter(origin.place)?;
    let binding = terminal_semantics::bind_reference_origin(
        types.values().copied(),
        parameter.structural_type,
        origin,
    )
    .ok()?;
    let transfers = binding.transfers_existing(
        types.values().copied(),
        parameter.structural_type,
        parameter.access,
    );
    let source = binding.source;
    let endpoint = binding.endpoint;
    reference_source_type(function, types, &source)?;
    formal_origin(types, function, &source)?;
    Some(FormalReferenceSource {
        endpoint,
        extent: binding.extent,
        transfers,
    })
}

/// The structural result row an operation publishes, when it publishes one.
/// Reference custody cares only about the structural contract; scalar and
/// Unit results carry no loans.
fn operation_structural_result(operation: &O) -> Option<&terminal_psi::StructuralOperationResult> {
    match operation {
        abstract_operations::establish_set_quint_pattern!(result, ..)
        | abstract_operations::establish_case_or_record_result_pattern!(result)
        | O::EstablishZeroedStructural { result, .. }
        | O::EstablishReference { result, .. }
        | O::EstablishPlacedReference { result, .. }
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
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
) -> Result<(), OptimizationUnitValidationError> {
    let invalid = || catalog_mismatch(function);
    let is_reference = |structural_type| {
        types.get(&structural_type).is_some_and(|declaration| {
            matches!(
                declaration.shape,
                terminal_psi::StructuralTypeShape::Reference { .. }
            )
        })
    };
    for node in function.block_nodes() {
        let Some(result) = operation_structural_result(&node.operation) else {
            continue;
        };
        if contains_reference(types, result.structural_type)
            && !is_reference(result.structural_type)
            && (!matches!(
                node.operation,
                O::EstablishRecord { .. }
                    | O::EstablishStructuralArray { .. }
                    | O::CallStructural {
                        result: AbstractStructuralCallResult::Structural { .. },
                        ..
                    }
            ) || result.multiplicity != terminal_psi::StructuralMultiplicity::Affine
                || !result.qualifications.is_empty()
                || !result.claims.is_empty())
        {
            return Err(invalid());
        }
        if is_reference(result.structural_type)
            && !matches!(
                node.operation,
                O::EstablishReference { .. }
                    | O::EstablishPlacedReference { .. }
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
        if parameter.multiplicity
            != if parameter.access == terminal_psi::StructuralAccess::Owned {
                terminal_psi::StructuralMultiplicity::Affine
            } else {
                terminal_psi::StructuralMultiplicity::Unrestricted
            }
            || !parameter.qualifications.is_empty()
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
    // A block parameter holds a moved carrier or an independent shared copy.
    // Current replay checks the actual source's multiplicity and complete
    // shared-leaf roster for every unrestricted copy. The receiving annotation
    // cannot authorize copying an exclusive descriptor. Affine arrivals re-home
    // existing live-reference rows instead of minting permission.
    for parameter in function
        .block_structural_parameters()
        .filter(|parameter| contains_reference(types, parameter.structural_type))
    {
        if parameter.access != terminal_psi::StructuralAccess::Owned
            || !matches!(
                parameter.multiplicity,
                terminal_psi::StructuralMultiplicity::Affine
                    | terminal_psi::StructuralMultiplicity::Unrestricted
            )
            || !parameter.qualifications.is_empty()
            || !constructible(types, parameter.structural_type)
        {
            return Err(invalid());
        }
    }
    // A reference-bearing root crosses an edge only by owned transfer. A whole
    // argument moves the entire record — and every stored loan with it; a
    // projected argument moves one subtree's loans under the root's partial
    // custody, which the ownership replay relocates onto the target parameter
    // while the root keeps its siblings' loans. A borrow lane would alias the
    // stored loans instead of moving them.
    if function
        .block_edges()
        .flat_map(|edge| &edge.structural_bindings)
        .any(|binding| {
            structural_source_contract(function, binding.argument.place)
                .is_some_and(|source| contains_reference(types, source.structural_type))
                && binding.argument.access != terminal_psi::StructuralAccess::Owned
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
    if result.reference_sources.iter().any(|mapping| {
        mapping.sources.len() > 1
            || mapping.sources.iter().any(|source| {
                source.extent == semantic_vocabulary::ReferenceOriginExtent::WithinRegion
                    || source
                        .path
                        .contains(&terminal_psi::ReferenceOriginProjection::Element)
            })
    }) {
        validate_alternative_result_dependencies(function, functions, types, dynamic_dispatch)?;
    }
    let mut sources = BTreeMap::new();
    for (path, mapping) in paths.iter().zip(&result.reference_sources) {
        let expected_referent =
            leaf_referent(types, result.structural_type, path).ok_or_else(invalid)?;
        if mapping.path != *path
            || mapping.sources.is_empty()
            || mapping.sources.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(invalid());
        }
        for formal in &mapping.sources {
            let projected = formal_reference_source(types, function, formal).ok_or_else(invalid)?;
            // Repeated observations of borrowed ingress create independent
            // shared descendants. An owned ingress instead transfers existing
            // carrier identities, which cannot fill multiple result slots.
            let shared_descendant = leaf_type(types, result.structural_type, path)
                .and_then(|leaf| reference_access(types, leaf))
                == Some(terminal_psi::StructuralAccess::SharedBorrow)
                && !projected.transfers
                && function.structural_parameters.iter().any(|parameter| {
                    parameter.place == formal.place
                        && parameter.access != terminal_psi::StructuralAccess::Owned
                });
            if !matches!(
                formal.access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::SharedBorrow
            ) || sources
                .insert((formal.place, formal.path.clone()), shared_descendant)
                .is_some_and(|previous_shared| !previous_shared || !shared_descendant)
                || !function.structural_parameter(formal.place).is_some()
                || (projected.extent == semantic_vocabulary::ReferenceOriginExtent::Projected
                    && projected.endpoint != expected_referent)
                || !leaf_access_within(types, result.structural_type, path, formal.access)
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
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
) -> Result<(), OptimizationUnitValidationError> {
    let invalid = || catalog_mismatch(function);
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
        for node in current.block_nodes() {
            if let O::CallStructural {
                callee,
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            } = &node.operation
                && contains_reference(types, result.structural_type)
            {
                match callee {
                    abstract_operations::AbstractCallTarget::Machine(machine) => {
                        pending.push((*machine, false));
                    }
                    abstract_operations::AbstractCallTarget::Dynamic {
                        interface, slot, ..
                    } => {
                        // Every possible executable realization participates in
                        // the dependency graph. No row supplies the public
                        // contract or stands in for the others.
                        for table in &dynamic_dispatch.tables {
                            if table.interface == *interface {
                                let row = table.rows.get(*slot as usize).ok_or_else(invalid)?;
                                if let Some(machine) = row {
                                    pending.push((*machine, false));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// A placed descriptor retains a distinct interpretation of its exact source.
/// Complete correspondence is rejoined to the retained catalog at the unit
/// boundary; loan replay consumes the source/result relationship independently.
pub(crate) fn placed_reference_source_compatible(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    source: StructuralTypeId,
    result: StructuralTypeId,
    correspondence: &terminal_psi::PlacedRamCorrespondence,
) -> bool {
    source == correspondence.schema
        && matches!(types.get(&result).map(|row| &row.shape),
            Some(terminal_psi::StructuralTypeShape::Reference { referent, access })
                if *referent == correspondence.carrier
                    && matches!(access, StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow))
}
