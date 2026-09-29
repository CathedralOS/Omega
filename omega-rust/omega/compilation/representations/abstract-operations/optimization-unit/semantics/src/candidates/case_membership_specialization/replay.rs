//! Independent case-membership specialization replay mechanics.
//!
//! Validation never trusts the candidate's membership rows: it re-derives the
//! claimed place's proof evidence — an `EstablishCase` producer on an
//! operation-result place at an empty path, the same producer reached
//! through `Field` descents across owned, complete structural children
//! under an `EstablishRecord` or through `Case` then `Field` descents into
//! the payload child an `EstablishCase` stores whole, a chain reached
//! through a uniform block-parameter binding or a case-payload
//! `StructuralLeafCopy` whose canonical projection prefixes the path still
//! to resolve, an `EstablishZeroedStructural` producer
//! whose all-zero storage selects the first declared case at every
//! position of a zero-constructible shape, or a declared closed
//! `Sum`/`Mixed` roster of exactly one case at the position each
//! observation resolves to — replays the admissible observation set for
//! that place, requires the claimed rows to equal the replayed rows
//! exactly, rebuilds the output itself, and reconstructs the exact node
//! custody the folded observations carry.

use crate::candidates::state_specialization::cyclic_machines;
use crate::candidates::structural_bindings::bound_place;
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite, validate_psi_optimization_unit};
use abstract_operations::AbstractOperation as O;
use abstract_operations::StructuralCopyPath;
use optimization_core::OptimizationValidatorIdentity;
use optimization_unit::{
    FoldedCaseMembershipRow, OptimizationFact, OptimizationNode, ProvenanceDisposition,
    ProvenanceRewrite, PsiOptimizationFunction, PsiOptimizationUnit, PsiProvenance,
    PsiRealizationSite, PsiRewriteCandidate, PsiRewritePatch,
    recompute_psi_optimization_unit_identity,
};
use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, MachineId, OperationId, PlaceId, ScalarType,
    StructuralCaseId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    RecordFieldValue, StructuralAccess, StructuralCaseDeclaration, StructuralFieldDeclaration,
    StructuralFieldType, StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeShape,
};

/// The evidence every membership observing `place` resolves against: the
/// place's rostered declaration, its declared structural type (when the place
/// kind carries one), the function every proof replays inside, and the root
/// proof — the establishment-or-roster basis empty-path observations may
/// draw on. `None` when `place` is not rostered in `function`.
struct MembershipEvidence<'a> {
    declaration: &'a StructuralPlaceDeclaration,
    root_type: Option<StructuralTypeId>,
    root_basis: Option<(StructuralCaseId, Option<OperationId>)>,
    function: &'a PsiOptimizationFunction,
}

/// The membership evidence for one rostered place, or `None` when the place
/// does not exist in `function`.
fn membership_evidence<'a>(
    unit: &PsiOptimizationUnit,
    function: &'a PsiOptimizationFunction,
    place: PlaceId,
) -> Option<MembershipEvidence<'a>> {
    let declaration = function
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)?;
    Some(MembershipEvidence {
        declaration,
        root_type: declared_structural_type(function, declaration),
        root_basis: proof_basis(unit, function, declaration),
        function,
    })
}

/// The case `place`'s root is proven to hold, plus its establishment
/// producer when the proof is one `EstablishCase` operation result —
/// whether the place is itself that result or a block parameter every
/// incoming edge binds to it whole.
fn proof_basis(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
) -> Option<(StructuralCaseId, Option<OperationId>)> {
    if let Some((producer, case)) = established_case_at(unit, function, declaration, &[]) {
        return Some((case, Some(producer)));
    }
    sole_case(unit, function, declaration).map(|case| (case, None))
}

/// The establishing operation a place's producer or uniform binding
/// resolves to, when it is one this family's proofs can draw on: `Record`
/// for an `EstablishRecord`, `Variant` for an `EstablishCase`
/// carrying its `result_case`, `Zeroed` for an `EstablishZeroedStructural`
/// whose all-zero storage selects each admissible sum's first declared
/// case, `None` for every other producer or place kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Establishment {
    None,
    Record(OperationId),
    Variant(OperationId, StructuralCaseId),
    Zeroed(OperationId),
}

/// The establishing producer `declaration`'s operation-result kind names,
/// or `Establishment::None` when the place is not an operation result or
/// its producer is not an `EstablishRecord`/`EstablishCase`/
/// `EstablishZeroedStructural` in `function`.
fn establishment_producer(
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
) -> Establishment {
    let StructuralPlaceKind::OperationResult { producer, .. } = declaration.kind else {
        return Establishment::None;
    };
    for node in function.blocks.iter().flat_map(|block| &block.nodes) {
        match &node.operation {
            O::EstablishRecord {
                psi_operation,
                result,
                ..
            } if *psi_operation == producer && result.place == declaration.id => {
                return Establishment::Record(producer);
            }
            O::EstablishCase {
                psi_operation,
                result,
                result_case,
                ..
            } if *psi_operation == producer && result.place == declaration.id => {
                return Establishment::Variant(producer, *result_case);
            }
            O::EstablishZeroedStructural {
                psi_operation,
                result,
            } if *psi_operation == producer && result.place == declaration.id => {
                return Establishment::Zeroed(producer);
            }
            _ => {}
        }
    }
    Establishment::None
}

/// One unresolved path segment inside `established_case_at`: either a
/// name-spelled segment of the observation's own path or a verifier-handle
/// `CanonicalStructuralPathSegment` a case-payload `StructuralLeafCopy`
/// spliced in from its case-qualified projection. The observation's own
/// path never spells a `Case` step — `StructuralPathSegment` has none — so
/// a case-qualified descent appears only under a copy crossing.
#[derive(Debug, Clone, Copy)]
enum PathStep<'a> {
    Ordinary(&'a StructuralPathSegment),
    Canonical(CanonicalStructuralPathSegment),
}

/// The case an establishing producer proves at `path`'s resolved position
/// under `declaration`'s place, witnessed by the `EstablishCase` or
/// `EstablishZeroedStructural` operation that fixes it. `None` when any
/// step is not established: a `Field` segment crosses only into an
/// `EstablishRecord` field stored as an owned, complete structural child
/// whose declared type is exactly the field's declared carrier, a `Case`
/// then `Field` pair descends an `EstablishCase` into the payload child it
/// stores whole for exactly the case it establishes, and a block parameter
/// — which holds no producer of its own — crosses only into the one place
/// every incoming edge binds it to whole when nothing rewrites or mutably
/// re-lends it. A case-payload `StructuralLeafCopy` result crosses into
/// its source with the copy's canonical projection prefixed to the path
/// still to resolve. `FixedIndex`, `RuntimeIndex`, `FixedByteRange`, and
/// `Referent` segments never cross a record or variant establishment: an
/// `EstablishScalarArray` element is not a placed child. A zeroed
/// establishment crosses every `Field`, `Case`, and index segment inside
/// its own storage instead — the verifier admits
/// `EstablishZeroedStructural` only on zero-constructible shapes, so every
/// sum the declared path resolves holds its first declared case.
fn established_case_at(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
    path: &[StructuralPathSegment],
) -> Option<(OperationId, StructuralCaseId)> {
    let mut place = declaration.id;
    let mut producer = establishment_producer(function, declaration);
    let mut declared = declared_structural_type(function, declaration);
    // Owned because a case-payload copy prepends its own canonical
    // projection to the path still to resolve beneath its source.
    let mut segments: Vec<PathStep> = path.iter().map(PathStep::Ordinary).collect();
    let mut visiting = BTreeSet::from([place]);
    loop {
        match (producer, segments.as_slice()) {
            (Establishment::Variant(producer, case), []) => {
                // An `EstablishCase` proves the case of the position
                // its result occupies permanently.
                return Some((producer, case));
            }
            (Establishment::Zeroed(operation), _) => {
                // Zeroed storage selects each admissible sum's first
                // declared case — the tag's all-zero bits name case ordinal
                // zero — at every position inside the place's own storage.
                // The remaining path descends the declared type alone:
                // `Field`, `Case`, and both index segments stay inside the
                // zeroed storage — a `Case` step enters a payload namespace
                // whose fields read the same zeroed bytes — while
                // `Referent` and `FixedByteRange` leave it and never
                // resolve under a zero-admissible shape anyway.
                let mut position = Position::Type(declared?);
                for step in &segments {
                    position = position.descend(unit, *step)?;
                }
                let Position::Type(terminal) = position else {
                    return None;
                };
                return roster_first_case(unit, terminal).map(|case| (operation, case));
            }
            (
                Establishment::Variant(operation, result_case),
                [
                    PathStep::Canonical(CanonicalStructuralPathSegment::Case(observed_case)),
                    PathStep::Canonical(CanonicalStructuralPathSegment::Field(identity)),
                    ..,
                ],
            ) if result_case == *observed_case => {
                // A `Case` then `Field` pair descends into the payload
                // child an `EstablishCase` stores whole — the
                // structural-case establishment. The same custody as a
                // record child applies: the payload carrier must be
                // exactly the child place's declared type, reached as one
                // owned, complete structural argument. A different
                // observed case never crosses: its payload overlays
                // storage this establishment did not write.
                let identity = *identity;
                let Position::Type(next) = Position::Type(declared?)
                    .descend(unit, segments[0])?
                    .descend(unit, segments[1])?
                else {
                    return None;
                };
                let child = case_structural_child(function, operation, place, identity)?;
                let child_declaration = function
                    .structural_places
                    .iter()
                    .find(|candidate| candidate.id == child)?;
                if declared_structural_type(function, child_declaration) != Some(next) {
                    return None;
                }
                if !visiting.insert(child) {
                    return None;
                }
                place = child;
                producer = establishment_producer(function, child_declaration);
                declared = Some(next);
                segments.drain(..2);
            }
            (Establishment::Record(operation), [step, ..]) => {
                // A `Field` segment descends into the nested carrier the
                // record stores whole, whether it spells a field identity —
                // the observation's own path — or a verifier field handle —
                // a spliced copy projection: the declared carrier must be
                // exactly the child place's declared type, and the child
                // must reach this producer's initializer as one owned,
                // complete structural argument. A pathed or borrowed
                // argument does not carry the child's own establishment.
                // Index, referent, and byte-range segments never cross —
                // an `EstablishScalarArray` element is not a placed child.
                let (field, next) = match step {
                    PathStep::Ordinary(StructuralPathSegment::Field(identity)) => {
                        descended_field(unit, declared?, identity)?
                    }
                    PathStep::Canonical(CanonicalStructuralPathSegment::Field(identity)) => {
                        descended_field_id(unit, declared?, *identity)?
                    }
                    _ => return None,
                };
                let child = structural_child(function, operation, place, field)?;
                let child_declaration = function
                    .structural_places
                    .iter()
                    .find(|candidate| candidate.id == child)?;
                if declared_structural_type(function, child_declaration) != Some(next) {
                    return None;
                }
                if !visiting.insert(child) {
                    return None;
                }
                place = child;
                producer = establishment_producer(function, child_declaration);
                declared = Some(next);
                segments.drain(..1);
            }
            (Establishment::None, _) => {
                // A case-payload copy — the place a `Case { payload }` arm
                // binding mints — holds exactly its source's contents at
                // the copied projection: the copy is fresh storage of an
                // `Unrestricted` or owned non-reference leaf, and the
                // source resolves only through establishments and uniform
                // bindings nothing rewrites. The copy's canonical
                // projection therefore prefixes the path still to resolve
                // beneath its source, and the source's own chain decides
                // the proof; a source that establishes a different case
                // declines at the `Case` step. Ordinary copies still
                // decline: their field steps are identity strings rather
                // than the verifier field handles canonical steps carry,
                // so crossing one needs its own identity-to-handle
                // resolution first.
                if let Some((source, copied)) = case_payload_copy(function, place) {
                    let source_declaration = function
                        .structural_places
                        .iter()
                        .find(|candidate| candidate.id == source)?;
                    let source_type = declared_structural_type(function, source_declaration)?;
                    let mut position = Position::Type(source_type);
                    for segment in copied {
                        position = position.descend(unit, PathStep::Canonical(*segment))?;
                    }
                    if !matches!(position, Position::Type(landed) if Some(landed) == declared) {
                        return None;
                    }
                    if !visiting.insert(source) {
                        return None;
                    }
                    place = source;
                    producer = establishment_producer(function, source_declaration);
                    declared = Some(source_type);
                    segments.splice(..0, copied.iter().copied().map(PathStep::Canonical));
                    continue;
                }
                // The position's place is not itself established, but a
                // block parameter arrives carrying exactly the place its
                // uniform binding names — the bound place's own
                // establishment is the parameter's.
                let bound = bound_place(function, place)?;
                let bound_declaration = function
                    .structural_places
                    .iter()
                    .find(|candidate| candidate.id == bound)?;
                let bound_type = declared_structural_type(function, bound_declaration);
                if declared.is_some() && bound_type != declared {
                    return None;
                }
                if !visiting.insert(bound) {
                    return None;
                }
                place = bound;
                producer = establishment_producer(function, bound_declaration);
                declared = bound_type;
            }
            _ => return None,
        }
    }
}

/// One structural position an unresolved path reaches inside
/// `established_case_at`: either a declared structural type or one case's
/// payload-field roster. The roster position exists only between a `Case`
/// step and the payload `Field` that leaves it; a path cannot end there
/// because a payload namespace is no type — nothing proves a case of it.
#[derive(Debug, Clone, Copy)]
enum Position<'a> {
    Type(StructuralTypeId),
    CasePayload(&'a StructuralCaseDeclaration),
}

impl<'a> Position<'a> {
    /// The position one unresolved step descends to, or `None` when the
    /// step does not apply to this position's shape. Ordinary `Referent`
    /// and `FixedByteRange` steps decline: both leave the place's own
    /// storage, which no establishment chain here crosses. An ordinary
    /// `Field` on a case-payload roster declines for the opposite reason:
    /// a validated copy projection always leaves the namespace with its
    /// canonical `Field`, so the mixture never spells from source.
    fn descend(self, unit: &'a PsiOptimizationUnit, step: PathStep<'_>) -> Option<Position<'a>> {
        match (self, step) {
            (Self::Type(current), PathStep::Ordinary(segment)) => match segment {
                StructuralPathSegment::Field(identity) => {
                    descended_field(unit, current, identity).map(|(_, next)| Self::Type(next))
                }
                StructuralPathSegment::FixedIndex(_)
                | StructuralPathSegment::RuntimeIndex { .. } => match shape(unit, current)? {
                    StructuralTypeShape::FixedArray { element, .. } => Some(Self::Type(*element)),
                    _ => None,
                },
                StructuralPathSegment::Referent | StructuralPathSegment::FixedByteRange { .. } => {
                    None
                }
            },
            (
                Self::Type(current),
                PathStep::Canonical(CanonicalStructuralPathSegment::Field(identity)),
            ) => descended_field_id(unit, current, identity).map(|(_, next)| Self::Type(next)),
            (
                Self::Type(current),
                PathStep::Canonical(CanonicalStructuralPathSegment::FixedIndex(_)),
            ) => match shape(unit, current)? {
                StructuralTypeShape::FixedArray { element, .. } => Some(Self::Type(*element)),
                _ => None,
            },
            (
                Self::Type(current),
                PathStep::Canonical(CanonicalStructuralPathSegment::Case(identity)),
            ) => {
                let cases = match shape(unit, current)? {
                    StructuralTypeShape::Sum { cases }
                    | StructuralTypeShape::Mixed { cases, .. } => cases,
                    _ => return None,
                };
                cases
                    .iter()
                    .find(|case| case.id == identity)
                    .map(Self::CasePayload)
            }
            (
                Self::CasePayload(case),
                PathStep::Canonical(CanonicalStructuralPathSegment::Field(identity)),
            ) => match case
                .fields
                .iter()
                .find(|field| field.id == identity)?
                .field_type
            {
                StructuralFieldType::Structural(next) => Some(Self::Type(next)),
                _ => None,
            },
            (Self::CasePayload(_), _) => None,
        }
    }
}

/// The declared structural shape of `type_id` in `unit`.
fn shape(unit: &PsiOptimizationUnit, type_id: StructuralTypeId) -> Option<&StructuralTypeShape> {
    unit.structural_types
        .as_slice()
        .iter()
        .find(|entry| entry.id == type_id)
        .map(|entry| &entry.shape)
}

/// The field `identity` names inside `current`'s `Record`/`Mixed`
/// common-field roster, with the structural type the field declares.
/// `None` when the shape has no common-field roster, the name is absent, or
/// the field's declared type is not structural.
fn descended_field(
    unit: &PsiOptimizationUnit,
    current: StructuralTypeId,
    identity: &str,
) -> Option<(StructuralFieldId, StructuralTypeId)> {
    descended_field_matching(unit, current, |field| field.identity == identity)
}

/// The same common-field descent by verifier field handle, for the
/// canonical steps a spliced copy projection carries.
fn descended_field_id(
    unit: &PsiOptimizationUnit,
    current: StructuralTypeId,
    identity: StructuralFieldId,
) -> Option<(StructuralFieldId, StructuralTypeId)> {
    descended_field_matching(unit, current, |field| field.id == identity)
}

/// The `Record`/`Mixed` common-field entry `matching` selects inside
/// `current`, with the structural type the field declares. `None` when the
/// shape has no common-field roster, no field matches, or the field's
/// declared type is not structural.
fn descended_field_matching(
    unit: &PsiOptimizationUnit,
    current: StructuralTypeId,
    matching: impl Fn(&StructuralFieldDeclaration) -> bool,
) -> Option<(StructuralFieldId, StructuralTypeId)> {
    let fields = match shape(unit, current)? {
        StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. } => {
            fields
        }
        _ => return None,
    };
    let field = fields.iter().find(|field| matching(field))?;
    match field.field_type {
        StructuralFieldType::Structural(next) => Some((field.id, next)),
        _ => None,
    }
}

/// The child place the `EstablishRecord` producer stores whole into
/// `field`, or `None` when the producer node is absent, is not an
/// `EstablishRecord` on this place, or `field`'s initializer is not an
/// owned, complete structural argument — a pathed argument or a borrow does
/// not carry the child's own establishment.
fn structural_child(
    function: &PsiOptimizationFunction,
    producer: OperationId,
    place: PlaceId,
    field: StructuralFieldId,
) -> Option<PlaceId> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            O::EstablishRecord {
                psi_operation,
                result,
                fields,
            } if *psi_operation == producer && result.place == place => Some(fields),
            _ => None,
        })?
        .iter()
        .find(|initializer| initializer.field == field)
        .and_then(|initializer| match &initializer.value {
            RecordFieldValue::Structural(argument)
                if argument.path.is_empty() && argument.access == StructuralAccess::Owned =>
            {
                Some(argument.place)
            }
            _ => None,
        })
}

/// The child place the `EstablishCase` producer stores whole into payload
/// `field`, or `None` when the producer node is absent, is not an
/// `EstablishCase` on this place, or `field`'s initializer is not an owned,
/// complete structural argument — a pathed or borrowed argument does not
/// carry the child's own establishment.
fn case_structural_child(
    function: &PsiOptimizationFunction,
    producer: OperationId,
    place: PlaceId,
    field: StructuralFieldId,
) -> Option<PlaceId> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            O::EstablishCase {
                psi_operation,
                result,
                fields,
                ..
            } if *psi_operation == producer && result.place == place => Some(fields),
            _ => None,
        })?
        .iter()
        .find(|initializer| initializer.field == field)
        .and_then(|initializer| match &initializer.value {
            RecordFieldValue::Structural(argument)
                if argument.path.is_empty() && argument.access == StructuralAccess::Owned =>
            {
                Some(argument.place)
            }
            _ => None,
        })
}

/// The source place and case-qualified projection of the
/// `StructuralLeafCopy` that produces `place`, or `None` when `place` is
/// not that copy's operation result or the copy's projection is ordinary.
/// Ordinary copies decline: their field steps name identities rather than
/// the verifier field handles canonical steps carry, so crossing one needs
/// its own identity-to-handle resolution first.
fn case_payload_copy(
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Option<(PlaceId, &[CanonicalStructuralPathSegment])> {
    let StructuralPlaceKind::OperationResult { producer, .. } = function
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)?
        .kind
    else {
        return None;
    };
    function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            O::StructuralLeafCopy {
                psi_operation,
                result,
                source,
                path: StructuralCopyPath::Case(path),
            } if *psi_operation == producer && result.place == place => {
                Some((*source, path.as_slice()))
            }
            _ => None,
        })
}

/// The declared structural type of one rostered place, or `None` when the
/// place kind carries no resolvable type in this function.
fn declared_structural_type(
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
) -> Option<StructuralTypeId> {
    match declaration.kind {
        StructuralPlaceKind::OperationResult {
            structural_type, ..
        }
        | StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        }
        | StructuralPlaceKind::TrivialAffineLocal {
            structural_type, ..
        } => Some(structural_type),
        StructuralPlaceKind::ProviderAttachment { attachment, .. } => Some(attachment),
        StructuralPlaceKind::Parameter { .. } => function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == declaration.id)
            .map(|parameter| parameter.structural_type),
        StructuralPlaceKind::BlockParameter { .. } => function
            .blocks
            .iter()
            .flat_map(|block| &block.structural_parameters)
            .find(|parameter| parameter.place == declaration.id)
            .map(|parameter| parameter.structural_type),
        StructuralPlaceKind::Result => function
            .result
            .structural()
            .and_then(|result| (result.place == declaration.id).then_some(result.structural_type)),
    }
}

/// The one case of the closed roster the `path` resolves to under the
/// place's declared structural type, or `None` when the path fails to
/// descend — a `Field` name absent from a `Record`/`Mixed` common-field
/// roster, a `FixedIndex` on a non-array, a `Referent` crossing on a
/// non-reference — or when the resolved end type is not a `Sum`/`Mixed`
/// roster of exactly one case.
fn sole_case_at_path(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
    path: &[StructuralPathSegment],
) -> Option<StructuralCaseId> {
    resolved_sole_case(unit, declared_structural_type(function, declaration)?, path)
}

/// The one case of a declared closed roster, or `None` when the place's
/// declared type is not a sum shape or names more than one case.
fn sole_case(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
) -> Option<StructuralCaseId> {
    sole_case_at_path(unit, function, declaration, &[])
}

/// The sole case of the closed roster at `path`'s end under `root`, or
/// `None` when the path fails to descend or the end type is not a sole-case
/// `Sum`/`Mixed` roster.
fn resolved_sole_case(
    unit: &PsiOptimizationUnit,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralCaseId> {
    let mut current = root;
    for segment in path {
        current = descended_type(unit, current, segment)?;
    }
    roster_sole_case(unit, current)
}

/// The structural type one path segment descends to under `current`, or
/// `None` when the segment does not apply to `current`'s shape or names a
/// field whose declared type is not structural.
fn descended_type(
    unit: &PsiOptimizationUnit,
    current: StructuralTypeId,
    segment: &StructuralPathSegment,
) -> Option<StructuralTypeId> {
    let shape = &unit
        .structural_types
        .as_slice()
        .iter()
        .find(|entry| entry.id == current)?
        .shape;
    match segment {
        StructuralPathSegment::FixedByteRange { .. } => None,
        StructuralPathSegment::Field(identity) => {
            let fields = match shape {
                StructuralTypeShape::Record { fields }
                | StructuralTypeShape::Mixed { fields, .. } => fields,
                _ => return None,
            };
            match fields
                .iter()
                .find(|field| field.identity == *identity)?
                .field_type
            {
                StructuralFieldType::Structural(next) => Some(next),
                _ => None,
            }
        }
        StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. } => {
            match shape {
                StructuralTypeShape::FixedArray { element, .. } => Some(*element),
                _ => None,
            }
        }
        StructuralPathSegment::Referent => match shape {
            StructuralTypeShape::Reference { referent, .. } => Some(*referent),
            _ => None,
        },
    }
}

/// The first declared case of `type_id`'s roster — the case a zeroed tag's
/// ordinal-zero bits select — or `None` when the shape is not a
/// `Sum`/`Mixed` or declares no case at all.
fn roster_first_case(
    unit: &PsiOptimizationUnit,
    type_id: StructuralTypeId,
) -> Option<StructuralCaseId> {
    let cases = match &unit
        .structural_types
        .as_slice()
        .iter()
        .find(|entry| entry.id == type_id)?
        .shape
    {
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => cases,
        _ => return None,
    };
    cases.first().map(|case| case.id)
}

/// The one case of `type_id`'s closed roster, or `None` when its shape is
/// not a `Sum`/`Mixed` or names more than one case.
fn roster_sole_case(
    unit: &PsiOptimizationUnit,
    type_id: StructuralTypeId,
) -> Option<StructuralCaseId> {
    let cases = match &unit
        .structural_types
        .as_slice()
        .iter()
        .find(|entry| entry.id == type_id)?
        .shape
    {
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => cases,
        _ => return None,
    };
    let [sole] = cases.as_slice() else {
        return None;
    };
    Some(sole.id)
}

/// The admissibility of one node under `evidence`: a `StructuralCaseMembership`
/// observing the evidence's place into a Boolean result, whose verdict the
/// unit proves — the place's root basis at an empty path, the establishment
/// a non-empty path resolves to through stored-whole children, `EstablishCase`
/// payload children, uniform block-parameter bindings, and case-payload
/// copies, or the sole-case roster at the resolved nested position. `None`
/// for every other node.
fn admit_membership_node(
    unit: &PsiOptimizationUnit,
    evidence: &MembershipEvidence<'_>,
    machine: MachineId,
    block: BlockId,
    node_index: usize,
    node: &OptimizationNode,
) -> Option<FoldedCaseMembershipRow> {
    let O::StructuralCaseMembership {
        psi_operation,
        result,
        source,
        path,
        case,
    } = &node.operation
    else {
        return None;
    };
    if *source != evidence.declaration.id || result.scalar_type != ScalarType::Boolean {
        return None;
    }
    // Every path asks the establishment chain first — at an empty path the
    // place's own producer, and at any depth a `Field` descent into a
    // stored-whole child or a `Case` then `Field` descent into an
    // `EstablishCase` payload, each possibly reached through the place's
    // uniform block-parameter binding or a case-payload copy. Failing an
    // establishment, the sole-case roster at the resolved end type proves
    // the verdict without custody evidence.
    let (proven_case, producer) =
        established_case_at(unit, evidence.function, evidence.declaration, path)
            .map(|(operation, case)| (case, Some(operation)))
            .or_else(|| {
                resolved_sole_case(unit, evidence.root_type?, path).map(|case| (case, None))
            })?;
    Some(FoldedCaseMembershipRow {
        site: optimization_unit::NodeLocation {
            machine,
            block,
            node: u32::try_from(node_index).ok()?,
        },
        psi_operation: *psi_operation,
        result: result.value,
        source: evidence.declaration.id,
        producer,
        observed_case: *case,
        proven_case,
        outcome: proven_case == *case,
    })
}

/// The independently derived fold roster for `place`: every
/// `StructuralCaseMembership` observing it whose verdict the per-site basis
/// proves, sorted by site.
fn plan_memberships(
    unit: &PsiOptimizationUnit,
    evidence: &MembershipEvidence<'_>,
    function: &PsiOptimizationFunction,
) -> Vec<FoldedCaseMembershipRow> {
    let mut memberships = Vec::new();
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let Some(row) =
                admit_membership_node(unit, evidence, function.machine, block.id, node_index, node)
            else {
                continue;
            };
            memberships.push(row);
        }
    }
    memberships.sort_by_key(|row| row.site);
    memberships
}

/// The folded node a row admits at its site: a `BooleanConstant` with the
/// membership's own custody identity and result value, checked against the
/// claimed basis — an establishment producer must be the `EstablishCase`
/// the row's path resolves to through stored-whole children, `EstablishCase`
/// payload children, uniform block-parameter bindings, and case-payload
/// copies, while a roster row must resolve to exactly the claimed case.
fn folded_node(
    row: &FoldedCaseMembershipRow,
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
) -> Result<OptimizationNode, OptimizationUnitValidationError> {
    let index = usize::try_from(row.site.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = function
        .blocks
        .iter()
        .find(|block| block.id == row.site.block)
        .and_then(|block| block.nodes.get(index))
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let O::StructuralCaseMembership {
        psi_operation,
        result,
        source,
        path,
        case,
    } = &node.operation
    else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if row.site.machine != function.machine
        || *psi_operation != row.psi_operation
        || result.value != row.result
        || result.scalar_type != ScalarType::Boolean
        || *source != row.source
        || *case != row.observed_case
        || row.outcome != (row.proven_case == row.observed_case)
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    match row.producer {
        // The claimed producer must be the `EstablishCase` the
        // establishment chain under `row.source` resolves `path` to — the
        // observed place's own producer at an empty path, the operation a
        // uniform block-parameter binding or case-payload copy forwards
        // to, a stored-whole record child's producer at a nested `Field`
        // position, or an `EstablishCase` payload child's producer at a
        // `Case` then `Field` position.
        Some(producer) => {
            let resolved = function
                .structural_places
                .iter()
                .find(|declaration| declaration.id == row.source)
                .and_then(|declaration| established_case_at(unit, function, declaration, path));
            if resolved != Some((producer, row.proven_case)) {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
        }
        // With no producer claim, the membership's path position must itself
        // resolve to a closed roster of exactly the claimed case: the place's
        // root type at an empty path, or the nested type its path descends to.
        None => {
            let declaration = function
                .structural_places
                .iter()
                .find(|declaration| declaration.id == row.source);
            if declaration
                .and_then(|declaration| sole_case_at_path(unit, function, declaration, path))
                != Some(row.proven_case)
            {
                return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
            }
        }
    }
    let operation = O::BooleanConstant {
        psi_operation: *psi_operation,
        result: result.value,
        value: row.outcome,
    };
    Ok(OptimizationNode {
        operation,
        provenance: node.provenance.clone(),
        fuel: node.fuel.clone(),
        effect: node.effect,
        definitions: node.definitions.clone(),
        uses: node.uses.clone(),
        successors: node.successors.clone(),
        ownership: node.ownership.clone(),
    })
}

/// Rebuilds a function's retained optimization-fact index after folding.
/// Every fact row carries its emitting node's `support` custody, so the walk
/// consumes each untouched node's retained rows in order and pushes one
/// `BooleanConstant` fact at each folded site.
fn refresh_facts(
    function: &mut PsiOptimizationFunction,
    rows: &[FoldedCaseMembershipRow],
) -> Result<(), OptimizationUnitValidationError> {
    let folded = rows
        .iter()
        .map(|row| ((row.site.block, row.site.node), row))
        .collect::<BTreeMap<_, _>>();
    let mut retained = std::mem::take(&mut function.facts).into_iter().peekable();
    let mut next_facts = Vec::new();
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            if let Some(PsiProvenance::Operation(operation)) = node.provenance.first() {
                while let Some(fact) = retained.peek() {
                    let support = match fact {
                        OptimizationFact::OperationObligationReference { support, .. }
                        | OptimizationFact::BooleanConstant { support, .. }
                        | OptimizationFact::IntegerConstant { support, .. } => *support,
                    };
                    if support != *operation {
                        break;
                    }
                    next_facts.push(retained.next().expect("peeked fact exists"));
                }
            }
            if let Some(row) = folded.get(&(
                block.id,
                u32::try_from(node_index)
                    .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?,
            )) {
                next_facts.push(OptimizationFact::BooleanConstant {
                    value: row.result,
                    constant: row.outcome,
                    support: row.psi_operation,
                });
            }
        }
    }
    next_facts.extend(retained);
    function.facts = next_facts;
    Ok(())
}

/// The exact node custody the folded observations carry: each folded site
/// retains the membership's own provenance and fuel settlement, realized at
/// the same node.
fn accepted_provenance(
    function: &PsiOptimizationFunction,
    rows: &[FoldedCaseMembershipRow],
) -> Result<Vec<ProvenanceRewrite>, OptimizationUnitValidationError> {
    let mut provenance = Vec::new();
    for row in rows {
        let index = usize::try_from(row.site.node)
            .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
        let node = function
            .blocks
            .iter()
            .find(|block| block.id == row.site.block)
            .and_then(|block| block.nodes.get(index))
            .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
        let site = PsiRealizationSite::Node(row.site);
        provenance.push(ProvenanceRewrite {
            input: site,
            disposition: ProvenanceDisposition::RealizedAt(site),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Ok(provenance)
}

pub(super) fn validate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    let PsiRewritePatch::SpecializeCaseMembership(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != patch.memberships.first().map(|row| row.site) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    if patch.memberships.is_empty()
        || candidate.predicted_cost_delta()
            != -i64::try_from(patch.memberships.len())
                .map_err(|_| OptimizationUnitValidationError::CandidatePatchMismatch)?
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    // A machine holding a cyclic component is frozen byte-exact: none of its
    // nodes may fold, so any candidate naming it is rejected before replay.
    if cyclic_machines(input).contains(&patch.machine) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let Some(evidence) = membership_evidence(input, function, patch.place) else {
        return Err(OptimizationUnitValidationError::CandidateLocationMissing);
    };
    if patch.producer != evidence.root_basis.and_then(|(_, producer)| producer) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let replayed = plan_memberships(input, &evidence, function);
    if replayed.is_empty() || patch.memberships != replayed {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let mut expected_blocks = patch
        .memberships
        .iter()
        .map(|row| row.site.block)
        .collect::<Vec<_>>();
    expected_blocks.sort_unstable();
    expected_blocks.dedup();
    let expected_provenance = accepted_provenance(function, &patch.memberships)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != expected_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let input_function = function;
    let folded = patch
        .memberships
        .iter()
        .map(|row| folded_node(row, input, input_function).map(|node| (row.site, node)))
        .collect::<Result<Vec<_>, _>>()?;
    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    for (location, node) in folded {
        let index = usize::try_from(location.node)
            .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
        let Some(slot) = output_function
            .blocks
            .iter_mut()
            .find(|block| block.id == location.block)
            .and_then(|block| block.nodes.get_mut(index))
        else {
            return Err(OptimizationUnitValidationError::CandidateLocationMissing);
        };
        *slot = node;
    }
    refresh_facts(output_function, &patch.memberships)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == patch.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    for input_block in &input_function.blocks {
        if !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|block| block.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    for input_function_other in &input.functions {
        if input_function_other.machine != patch.machine
            && !output.functions.contains(input_function_other)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.case-membership-specialization.v1",
        ),
        provenance: expected_provenance,
    })
}
