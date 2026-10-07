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
use abstract_operations::StructuralCopyPath;
use optimization_unit::{
    Establishment, canonicalize_ordinary_path, declared_structural_type, descended_field,
    descended_field_child, descended_field_id, establishment_producer, leaf_copy_source,
    resolved_sole_case, roster_first_case, shape, sole_case,
};
use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralCaseId, StructuralTypeId};
use std::collections::BTreeSet;
use terminal_psi::{
    RecordFieldValue, StructuralAccess, StructuralArgument, StructuralCaseDeclaration,
    StructuralFieldType, StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeShape,
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
    // uniform block-parameter binding or a `StructuralLeafCopy` whose
    // projection the walk prefixes. Failing an
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

/// One unresolved path segment inside `established_case_at`: either a
/// name-spelled segment of the observation's own path or a verifier-handle
/// `CanonicalStructuralPathSegment` a `StructuralLeafCopy` spliced in from
/// its projection — a case-qualified path directly, an ordinary path after
/// its field identities resolve to roster handles. The observation's own
/// path never spells a `Case` step, so a case-qualified descent appears
/// only under a copy crossing.
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
/// re-lends it. A `StructuralLeafCopy` result crosses into its source with
/// the copy's projection — canonical steps directly, ordinary field
/// identities resolved to their roster handles — prefixed to the path
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
    // Owned because a leaf copy prepends its own canonical projection —
    // case-qualified steps directly, ordinary field identities resolved to
    // roster handles — to the path still to resolve beneath its source.
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
                let (child, child_producer) = descended_field_child(
                    function,
                    operation,
                    place,
                    identity,
                    next,
                    &mut visiting,
                )?;
                place = child;
                producer = child_producer;
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
                let (child, child_producer) =
                    descended_field_child(function, operation, place, field, next, &mut visiting)?;
                place = child;
                producer = child_producer;
                declared = Some(next);
                segments.drain(..1);
            }
            (Establishment::None, _) => {
                // A leaf copy — the case-payload place a `Case { payload }`
                // arm binding mints, or an ordinary projection's fresh leaf
                // — holds exactly its source's contents at the copied
                // projection: the copy is fresh storage of an
                // `Unrestricted` or owned non-reference leaf, and the
                // source resolves only through establishments and uniform
                // bindings nothing rewrites. The copy's canonical
                // projection therefore prefixes the path still to resolve
                // beneath its source, and the source's own chain decides
                // the proof; a source that establishes a different case
                // declines at the `Case` step. Ordinary projections spell
                // field identities, so the crossing first resolves each
                // name to its roster's verifier handle; a name absent from
                // the reached roster, a non-structural field carrier, and
                // every segment with no canonical step leave the copy
                // uncrossed.
                if let Some((source, spelled)) = leaf_copy_source(function, place) {
                    let source_declaration = function
                        .structural_places
                        .iter()
                        .find(|candidate| candidate.id == source)?;
                    let source_type = declared_structural_type(function, source_declaration)?;
                    let copied = match spelled {
                        StructuralCopyPath::Case(path) => path.clone(),
                        StructuralCopyPath::Ordinary(path) => {
                            canonicalize_ordinary_path(unit, source_type, path)?
                        }
                    };
                    let mut position = Position::Type(source_type);
                    for segment in &copied {
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
/// delivered value, and call argument rows.
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
