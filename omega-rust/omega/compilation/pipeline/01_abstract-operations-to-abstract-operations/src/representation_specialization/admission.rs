//! Optimizer module role: admission leaf. Proven-case predicates behind the plan.
//!
//! The plan is built from two predicates only: the place's
//! declaration/type/proof evidence and the per-node admissibility that
//! resolves one membership observation's proven case. The independent
//! validator in `optimization-unit-semantics` re-derives the same predicates
//! from the candidate's rows rather than trusting this enumeration — a
//! matcher cannot attest to itself.

use super::{
    FoldedCaseMembershipRow, NodeLocation, O, OperationId, PlaceId, PsiOptimizationFunction,
    PsiOptimizationUnit, ScalarType, StructuralPlaceKind,
};
use abstract_operations::{AbstractDynamicDispatch, StructuralCopyPath};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, StructuralCaseId, StructuralFieldId, StructuralTypeId,
};
use std::collections::BTreeSet;
use terminal_psi::{
    RecordFieldValue, StructuralAccess, StructuralArgument, StructuralCaseDeclaration,
    StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeShape,
};

/// The evidence every membership observing `place` resolves against: the
/// place's rostered declaration, its declared structural type (when the place
/// kind carries one), the function every proof replays inside, and the root
/// proof — the establishment-or-roster basis empty-path observations may
/// draw on. `None` when `place` is not rostered in `function`.
pub(super) struct MembershipEvidence<'a> {
    pub(super) declaration: &'a StructuralPlaceDeclaration,
    pub(super) root_type: Option<StructuralTypeId>,
    pub(super) root_basis: Option<(StructuralCaseId, Option<OperationId>)>,
    pub(super) function: &'a PsiOptimizationFunction,
}

/// The membership evidence for one rostered place, or `None` when the place
/// does not exist in `function`.
pub(super) fn membership_evidence<'a>(
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

/// The admissibility of one node under `evidence`: a `StructuralCaseMembership`
/// observing the evidence's place into a Boolean result, whose verdict the
/// unit proves — the establishment chain under the place at the observed
/// path, or the sole-case roster at the resolved position. `None` for every
/// other node.
pub(super) fn admit_membership_node(
    unit: &PsiOptimizationUnit,
    evidence: &MembershipEvidence<'_>,
    machine: semantic_vocabulary::MachineId,
    block: semantic_vocabulary::BlockId,
    node_index: usize,
    node: &optimization_unit::OptimizationNode,
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
        site: NodeLocation {
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
                StructuralPathSegment::Referent
                | StructuralPathSegment::Case(_)
                | StructuralPathSegment::FixedByteRange { .. } => None,
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

/// The one place `place` is bound to by every incoming edge when `place` is
/// a structural block parameter, or `None` when it is not a block
/// parameter, when any incoming edge fails to bind it or binds a different
/// place, when any bound argument is pathed or carries write authority, or
/// when the parameter's contents may still be rewritten or mutably re-lent
/// inside `function`. A uniform whole `Owned`/`SharedBorrow` argument means
/// the parameter's contents are exactly the bound place's for the block's
/// lifetime — an owned binding transfers the storage itself, and a shared
/// loan freezes the bound place's contents for the borrow — so the bound
/// place's establishment is the parameter's own.
pub(crate) fn bound_place(function: &PsiOptimizationFunction, place: PlaceId) -> Option<PlaceId> {
    let StructuralPlaceKind::BlockParameter { block, .. } = function
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)?
        .kind
    else {
        return None;
    };
    if place_is_rewritten(function, place) {
        return None;
    }
    let mut bound = None;
    for edge in function
        .blocks
        .iter()
        .flat_map(|owner| owner.nodes.iter())
        .flat_map(|node| node.successors.iter())
        .filter(|edge| edge.target == block)
    {
        let binding = edge
            .structural_bindings
            .iter()
            .find(|binding| binding.parameter == place)?;
        let argument = &binding.argument;
        if !argument.path.is_empty()
            || !matches!(
                argument.access,
                StructuralAccess::Owned | StructuralAccess::SharedBorrow
            )
        {
            return None;
        }
        match bound {
            None => bound = Some(argument.place),
            Some(seen) if seen != argument.place => return None,
            _ => {}
        }
    }
    bound
}

/// Whether `place`'s contents can change inside `function` after its
/// binding: a store or vacating move naming it, a mutable or pathed
/// structural argument re-lending it, or an atomic event joining its
/// residence each break the uniform-binding proof.
fn place_is_rewritten(function: &PsiOptimizationFunction, place: PlaceId) -> bool {
    function.blocks.iter().any(|block| {
        block.nodes.iter().any(|node| {
            operation_rewrites_place(&node.operation, place)
                || node.successors.iter().any(|edge| {
                    edge.structural_bindings
                        .iter()
                        .any(|binding| argument_rewrites_place(&binding.argument, place))
                })
        })
    })
}

/// Whether `operation` writes or vacates `place`, or re-lends it with
/// authority another party can write through.
fn operation_rewrites_place(operation: &O, place: PlaceId) -> bool {
    if match operation {
        // Bare-place destinations that replace or re-establish contents.
        O::PrimitiveLocalStore { destination, .. }
        | O::ByteSequenceWrite { destination, .. }
        | O::StructuralByteSequenceFieldByteStore { destination, .. }
        | O::ByteSequenceStore { destination, .. }
        | O::StructuralByteSequenceFieldStore { destination, .. }
        | O::EstablishElementView { destination, .. } => *destination == place,
        // Parameter-row destinations and vacating move sources.
        O::WriteOnlyPrimitiveStore { destination, .. } => destination.place() == place,
        O::StoreStructuralField { destination, .. } => destination.place() == place,
        O::StructuralScalarFieldStore { destination, .. } => destination.place() == place,
        O::MoveStructuralField { source, .. } => source.place() == place,
        // The stored descriptor's selection source is the written aggregate:
        // whatever the argument's access, a field beneath it is established.
        O::StoreDynamicDescriptor { stored, .. } => stored.selection.source.place == place,
        // An atomic resident's contents may change under another event.
        O::AtomicEvent { event, .. } => event.place() == Some(place),
        _ => false,
    } {
        return true;
    }
    structural_argument_operands(operation).any(|argument| argument_rewrites_place(argument, place))
}

/// Every structural-argument operand `operation` carries: record children
/// stored whole, element-view and reference sources, a reseating store's
/// delivered value, call argument rows, and the conformance-selection
/// sources dynamic dispatches and stored descriptors retain.
fn structural_argument_operands(operation: &O) -> impl Iterator<Item = &StructuralArgument> {
    let mut operands: Vec<&StructuralArgument> = Vec::new();
    match operation {
        O::EstablishCase { fields, .. } | O::EstablishRecord { fields, .. } => {
            for initializer in fields {
                if let RecordFieldValue::Structural(argument) = &initializer.value {
                    operands.push(argument);
                }
            }
        }
        O::EstablishElementView { source, .. } | O::EstablishReference { source, .. } => {
            operands.push(source);
        }
        O::StoreStructuralField { value, .. } => operands.push(value),
        O::CallStructural {
            structural_arguments,
            ..
        }
        | O::BoundaryCall {
            structural_arguments,
            ..
        } => operands.extend(structural_arguments.iter()),
        O::CallDynamic { dispatch, .. } => match dispatch {
            AbstractDynamicDispatch::Rebound(dispatch) => {
                operands.push(&dispatch.initial.source);
                operands.push(&dispatch.rebound.source);
            }
            AbstractDynamicDispatch::Stored(dispatch) => {
                operands.push(&dispatch.stored.selection.source);
            }
            AbstractDynamicDispatch::Parameter(_) => {}
        },
        _ => {}
    }
    operands.into_iter()
}

/// Whether `argument` gives `place`'s contents away: a pathed argument
/// projects or vacates only a subtree, while a whole `MutableBorrow` or
/// `WriteOnlyBorrow` argument lends write authority over the whole place.
/// Whole `Owned`/`SharedBorrow` arguments preserve the observed contents —
/// a move transfers the storage itself and a shared loan freezes it.
fn argument_rewrites_place(argument: &StructuralArgument, place: PlaceId) -> bool {
    argument.place == place
        && (!argument.path.is_empty()
            || matches!(
                argument.access,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            ))
}

/// The declared structural type of one rostered place, or `None` when the
/// place kind carries no resolvable type in this function.
pub(crate) fn declared_structural_type(
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
        StructuralPathSegment::Case(_) | StructuralPathSegment::FixedByteRange { .. } => None,
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
