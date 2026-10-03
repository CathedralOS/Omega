//! Independent replay of retained structural-result `Call` reference rosters.
//!
//! A structural-result `Call` carries `reference_results`: the ordered leaf
//! roster declaring which suspended referent root each returned reference
//! carrier names. The producer resolves that roster through live custody —
//! owned arguments move their complete leaf set, `.., Referent` mappings
//! descend through the callee's formal ingress, and surviving leaves keep the
//! root they were formed with. This module recomputes the same roster from the
//! caller's own operation stream alone: entry seeding, carrier moves, record
//! relocations, call results, and edge discards are walked in dominator order,
//! so the retained rows are compared against an independent derivation rather
//! than trusted. Producer admission gates that cannot change an emitted row —
//! suspended-root checks, parent/child exclusivity, referent typing — are
//! source verification's concern, so this replay only tracks which leaf lives
//! at which carrier path and which root it suspends.

use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::{
    AbstractBoundaryResult, AbstractFunction, AbstractOperation, AbstractStructuralCallResult,
};
use semantic_vocabulary::{MachineId, OperationId, PlaceId, StructuralTypeId};
use target_operations::TargetReferenceResult;
use terminal_psi::{
    RecordFieldValue, StructuralAccess, StructuralArgument, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalAffineCleanupAction,
};

/// Which retained row family a source `BoundaryCall` lowered into. Only the
/// installed-provider lane replays the call's custody effects; a settlement
/// establishes only its declared structural result home.
pub(super) enum BoundaryCallRow {
    /// Installed-provider call resolved against this candidate machine.
    Installed(MachineId),
    /// Boundary settlement: a structural result still establishes its home.
    Settlement,
    /// Any other retained row — or none — has no custody effect.
    Other,
}

/// The referent root one live carrier leaf suspends. `Place` names local
/// referent storage (a primitive local or a mutable-borrowed primitive
/// parameter); `Ingress` names an owned reference-bearing entry parameter
/// whose referent storage stays with the caller. The row retains only the
/// root's place, so the replay does the same while keeping the two origins
/// distinct for `formal_origin` routing.
#[derive(Clone)]
enum Root {
    Place(PlaceId),
    Ingress(PlaceId, Vec<StructuralPathSegment>),
    Alternatives(PlaceId, Vec<Vec<StructuralPathSegment>>),
}

impl Root {
    fn place(&self) -> PlaceId {
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

/// The custody record the replay retains for one live leaf: the referent root
/// it suspends and the structural contract its carrier place reports while
/// holding the leaf. Formation identity and parentage decide admission only —
/// never which row is emitted — so they are not retained.
#[derive(Clone)]
struct Leaf {
    root: Root,
    structural_type: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
}

/// Live custody at one point in the operation stream: which leaf each carrier
/// location currently holds, and which contract each operation-established
/// home still reports. Block parameters and caller parameters keep their
/// declared contracts, so they are not stored here.
#[derive(Clone, Default)]
struct Custody {
    leaves: BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
    homes: BTreeMap<PlaceId, (StructuralTypeId, StructuralMultiplicity)>,
}

fn lookup(
    declarations: &[StructuralTypeDeclaration],
    identity: StructuralTypeId,
) -> Option<&StructuralTypeDeclaration> {
    declarations
        .iter()
        .find(|declaration| declaration.id == identity)
}

/// A record member that is realizable storage without a catalog contract of
/// its own: a fixed array whose elements descend to record or scalar leaves.
/// Its custody lives on the member's home row, exactly like a scalar array.
fn aggregate_member_storage(
    declarations: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
) -> bool {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        match lookup(declarations, current).map(|declaration| &declaration.shape) {
            Some(StructuralTypeShape::FixedArray { element, .. }) => pending.push(*element),
            Some(StructuralTypeShape::Record { .. })
            | Some(StructuralTypeShape::PrimitiveScalar(_)) => {}
            _ => return false,
        }
    }
    true
}

/// Owned containment only: a reference's referent is not its payload.
fn contains_reference(declarations: &[StructuralTypeDeclaration], root: StructuralTypeId) -> bool {
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        let Some(declaration) = lookup(declarations, current) else {
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

/// Exact declaration-order reference leaves of the supported owned shape,
/// bounded exactly as the producer bounds them.
fn leaf_paths(
    declarations: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
    maximum_leaves: usize,
) -> Option<Vec<Vec<StructuralPathSegment>>> {
    let mut output = Vec::new();
    let mut pending = vec![(root, Vec::new())];
    while let Some((current, path)) = pending.pop() {
        if !contains_reference(declarations, current) {
            continue;
        }
        let Some(declaration) = lookup(declarations, current) else {
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
                        if !contains_reference(declarations, child) {
                            continue;
                        }
                        // Every pending owned subtree owes at least one leaf.
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

/// A whole-carrier leaf admits one borrow permission over a primitive scalar
/// referent; shared and exclusive carriers share that loan shape.
fn referent(
    declarations: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    let declaration = lookup(declarations, structural_type)?;
    match declaration.shape {
        StructuralTypeShape::Reference {
            referent,
            access:
                StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow,
        } if lookup(declarations, referent).is_some_and(|declaration| {
            matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
        }) =>
        {
            Some(referent)
        }
        _ => None,
    }
}

/// The aggregate bare-reference carrier native lowering realizes: a shared,
/// mutable or write-only borrow of a plain record. Its referent is ordinary
/// storage rather than a nested loan roster; reference-bearing record
/// referents remain outside this ABI.
fn plain_record_referent(
    declarations: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    let StructuralTypeShape::Reference {
        referent,
        access:
            StructuralAccess::SharedBorrow
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow,
    } = &lookup(declarations, structural_type)?.shape
    else {
        return None;
    };
    matches!(
        lookup(declarations, *referent).map(|declaration| &declaration.shape),
        Some(StructuralTypeShape::Record { .. })
    )
    .then_some(*referent)
    .filter(|referent| !contains_reference(declarations, *referent))
}

fn bare_referent(
    declarations: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    referent(declarations, structural_type)
        .or_else(|| plain_record_referent(declarations, structural_type))
}

/// The structural type a field-only path names beneath `root`: record and
/// mixed fields descend structurally while a plain leaf field resolves to
/// its canonical standalone declaration.
fn leaf_type(
    declarations: &[StructuralTypeDeclaration],
    mut current: StructuralTypeId,
    fields: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for segment in fields {
        if let StructuralPathSegment::FixedIndex(index) = segment {
            let StructuralTypeShape::FixedArray { element, length } =
                &lookup(declarations, current)?.shape
            else {
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
        let fields = match &lookup(declarations, current)?.shape {
            StructuralTypeShape::Record { fields } => fields,
            StructuralTypeShape::Mixed { fields, .. } => fields,
            _ => return None,
        };
        let field = fields.iter().find(|field| &field.identity == identity)?;
        if field.relevance.is_erased() {
            return None;
        }
        current = match &field.field_type {
            StructuralFieldType::Structural(child) => *child,
            leaf => {
                let shape = leaf.canonical_leaf_shape()?;
                declarations
                    .iter()
                    .find(|declaration| declaration.shape == shape)
                    .map(|declaration| declaration.id)?
            }
        };
    }
    Some(current)
}

/// Every owned-carrier ingress parameter is a constructible record: scalar
/// leaves and bare primitive-reference leaves compose the complete type.
fn constructible_record(
    declarations: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
) -> bool {
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
        }) = lookup(declarations, current)
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
                | StructuralFieldType::Atomic(_)
                | StructuralFieldType::IeeeFloat(_)
                | StructuralFieldType::RefinedScalar(_)
                | StructuralFieldType::BoundedInteger(_) => {}
                StructuralFieldType::Structural(child)
                    if referent(declarations, child).is_some() => {}
                StructuralFieldType::Structural(child) => pending.push((child, false)),
                _ => return false,
            }
        }
    }
    true
}

/// The structural contract a place currently reports: signature parameter,
/// block-entry parameter, operation-established home, or live bare carrier.
fn source_contract(
    function: &AbstractFunction,
    custody: &Custody,
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
    if let Some(contract) = custody.homes.get(&place) {
        return Some(*contract);
    }
    custody
        .leaves
        .get(&(place, Vec::new()))
        .map(|leaf| (leaf.structural_type, leaf.multiplicity))
}

/// A result's formal origin must belong to the callee's input telescope.
/// Unlike an actual call operand, it cannot name callee-local storage.
fn formal_projection_type(
    function: &AbstractFunction,
    declarations: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> Option<StructuralTypeId> {
    if source.path.is_empty()
        || source
            .path
            .iter()
            .any(|segment| matches!(segment, StructuralPathSegment::Referent))
    {
        return None;
    }
    let parameter = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source.place)?;
    let access_is_compatible = match source.access {
        StructuralAccess::SharedBorrow => matches!(
            parameter.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ),
        StructuralAccess::MutableBorrow => parameter.access == StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow => matches!(
            parameter.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        ),
        StructuralAccess::Owned => false,
    };
    if !access_is_compatible
        || parameter.multiplicity == StructuralMultiplicity::Linear
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || function
            .entry_claims
            .iter()
            .any(|claim| claim.input == source.place)
    {
        return None;
    }
    leaf_type(declarations, parameter.structural_type, &source.path)
}

/// A `.., Referent` source names the suspended root of the located carrier
/// leaf; an empty path names a primitive referent root place directly. A
/// static field/fixed-index path retains the exact borrowed ingress address.
fn normalized_source(
    function: &AbstractFunction,
    custody: &Custody,
    declarations: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> Option<Root> {
    if let Some((StructuralPathSegment::Referent, carrier_path)) = source.path.split_last() {
        if let Some(leaf) = custody.leaves.get(&(source.place, carrier_path.to_vec())) {
            Some(leaf.root.clone())
        } else {
            // Member storage under a carrier never held as a live reference:
            // the loan roots at the carrier's own ingress storage, provided
            // the named member is not itself a reference.
            let (carrier, _) = source_contract(function, custody, source.place)?;
            leaf_type(declarations, carrier, carrier_path).and_then(|leaf| {
                match &lookup(declarations, leaf)?.shape {
                    StructuralTypeShape::Reference { .. } => None,
                    _ => Some(Root::Ingress(source.place, carrier_path.to_vec())),
                }
            })
        }
    } else {
        // Reconstruct the actual storage coordinate from the currently live
        // contract. A projected local and a projected parameter denote the
        // same custody operation; neither is a callee-local formal origin.
        let (contract, _) = source_contract(function, custody, source.place)?;
        leaf_type(declarations, contract, &source.path)?;
        Some(if source.path.is_empty() {
            Root::Place(source.place)
        } else {
            Root::Ingress(source.place, source.path.clone())
        })
    }
}

/// Substitute a callee's formal result-source mapping into the caller's actual
/// argument at the same structural-parameter position.
fn call_source(
    callee: &AbstractFunction,
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
    function: &AbstractFunction,
    declarations: &[StructuralTypeDeclaration],
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
    } else if let Some((StructuralPathSegment::Referent, carrier_path)) = source.path.split_last()
        && matches!(
            parameter.access,
            StructuralAccess::Owned | StructuralAccess::SharedBorrow
        )
    {
        Some(Root::Ingress(source.place, carrier_path.to_vec()))
    } else if formal_projection_type(function, declarations, source).is_some() {
        Some(Root::Ingress(source.place, source.path.clone()))
    } else {
        None
    }
}

/// Seed one ingress leaf per reference leaf of each reference-bearing entry
/// parameter. A non-admissible carrier — borrowed, qualified, claimed, or not
/// constructible — has no honest lowering, so the whole replay fails rather
/// than seeding a partial roster.
fn entry(
    function: &AbstractFunction,
    declarations: &[StructuralTypeDeclaration],
) -> Option<Custody> {
    let mut custody = Custody::default();
    for parameter in &function.structural_parameters {
        if !contains_reference(declarations, parameter.structural_type) {
            continue;
        }
        if parameter.access != StructuralAccess::Owned
            || parameter.multiplicity != StructuralMultiplicity::Affine
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || !constructible_record(declarations, parameter.structural_type)
            || function
                .entry_claims
                .iter()
                .any(|claim| claim.input == parameter.place)
        {
            return None;
        }
        let paths = leaf_paths(
            declarations,
            parameter.structural_type,
            4096usize.saturating_sub(custody.leaves.len()),
        )?;
        for path in paths {
            custody.leaves.insert(
                (parameter.place, path.clone()),
                Leaf {
                    root: Root::Ingress(parameter.place, path),
                    structural_type: parameter.structural_type,
                    multiplicity: parameter.multiplicity,
                },
            );
        }
    }
    Some(custody)
}

/// The owned-carrier leaves a call's arguments move, keyed by their current
/// carrier path. Borrowed and non-carrier arguments change no custody here;
/// the producer's remaining argument checks are admission, not emission.
fn argument_moves(
    function: &AbstractFunction,
    declarations: &[StructuralTypeDeclaration],
    custody: &Custody,
    arguments: &[StructuralArgument],
) -> Option<BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>> {
    let mut moved = BTreeMap::new();
    for argument in arguments {
        if argument.access != StructuralAccess::Owned {
            continue;
        }
        let Some((contract, _)) = source_contract(function, custody, argument.place) else {
            continue;
        };
        if !contains_reference(declarations, contract) {
            continue;
        }
        if !argument.path.is_empty() {
            return None;
        }
        let paths = leaf_paths(declarations, contract, custody.leaves.len())?;
        if paths.len()
            != custody
                .leaves
                .keys()
                .filter(|(carrier, _)| *carrier == argument.place)
                .count()
        {
            return None;
        }
        for path in paths {
            let leaf = custody.leaves.get(&(argument.place, path.clone()))?.clone();
            if moved.insert((argument.place, path), leaf).is_some() {
                return None;
            }
        }
    }
    Some(moved)
}

/// Commit the staged argument moves: moved carrier leaves transfer to the
/// callee and leave the caller's live map.
fn commit_argument_moves(
    custody: &mut Custody,
    moved: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
) {
    for key in moved.keys() {
        custody.leaves.remove(key);
    }
}

/// The declared result leaf roster a structural call establishes. Each
/// `reference_sources` mapping resolves through caller custody — a moved
/// ingress leaf for an owned formal, a live leaf's suspended root otherwise —
/// and lands at the declared result path. Returns the emitted rows and the
/// leaves the result carrier establishes.
fn call_result_leaves(
    callee: &AbstractFunction,
    function: &AbstractFunction,
    declarations: &[StructuralTypeDeclaration],
    result: &StructuralOperationResult,
    structural_arguments: &[StructuralArgument],
    custody: &Custody,
    moved: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
) -> Option<(
    Vec<TargetReferenceResult>,
    Vec<((PlaceId, Vec<StructuralPathSegment>), Leaf)>,
)> {
    if !contains_reference(declarations, result.structural_type) {
        return Some((Vec::new(), Vec::new()));
    }
    if matches!(
        lookup(declarations, result.structural_type).map(|declaration| &declaration.shape),
        Some(StructuralTypeShape::Reference { .. })
    ) && bare_referent(declarations, result.structural_type).is_none()
    {
        return None;
    }
    // The result place is fresh on an admissible plan: no live carrier leaf
    // may already name it.
    if custody
        .leaves
        .keys()
        .any(|(carrier, _)| *carrier == result.place)
    {
        return None;
    }
    let callee_result = callee.result.structural()?;
    let mut roster = Vec::with_capacity(callee_result.reference_sources.len());
    let mut established = Vec::with_capacity(callee_result.reference_sources.len());
    for mapping in &callee_result.reference_sources {
        if mapping.sources.is_empty() || mapping.sources.windows(2).any(|pair| pair[0] >= pair[1]) {
            return None;
        }
        if mapping.sources.len() > 1 {
            let referent = bare_referent(declarations, result.structural_type)?;
            if !mapping.path.is_empty()
                || !matches!(
                    lookup(declarations, referent).map(|declaration| &declaration.shape),
                    Some(StructuralTypeShape::Record { .. })
                )
                || mapping.sources.iter().any(|formal| {
                    formal.place != mapping.sources[0].place
                        || formal.access != StructuralAccess::MutableBorrow
                        || formal.path.is_empty()
                        || formal.path.iter().any(|segment| {
                            !matches!(
                                segment,
                                StructuralPathSegment::Field(_)
                                    | StructuralPathSegment::FixedIndex(_)
                            )
                        })
                })
            {
                return None;
            }
        }
        let mut candidates = Vec::with_capacity(mapping.sources.len());
        for formal in &mapping.sources {
            let source = call_source(callee, structural_arguments, formal)?;
            let leaf = if matches!(
                formal_origin(callee, declarations, formal),
                Some(Root::Ingress(..))
                    if formal
                        .path
                        .split_last()
                        .and_then(|(tail, carrier_path)| {
                            if !matches!(tail, StructuralPathSegment::Referent) {
                                return None;
                            }
                            let parameter = callee
                                .structural_parameters
                                .iter()
                                .find(|parameter| parameter.place == formal.place)?;
                            leaf_type(declarations, parameter.structural_type, carrier_path)
                        })
                        .and_then(|leaf| lookup(declarations, leaf))
                        .is_some_and(|declaration| {
                            matches!(
                                declaration.shape,
                                StructuralTypeShape::Reference { .. }
                            )
                        })
            ) {
                // An owned-carrier ingress mapping relocates one of the leaves the
                // moved arguments just transferred; it keeps its root.
                let Some((StructuralPathSegment::Referent, carrier_path)) =
                    source.path.split_last()
                else {
                    return None;
                };
                moved.get(&(source.place, carrier_path.to_vec()))?.clone()
            } else {
                let root = normalized_source(function, custody, declarations, &source)?;
                Leaf {
                    root,
                    structural_type: result.structural_type,
                    multiplicity: result.multiplicity,
                }
            };
            candidates.push(leaf);
        }
        let leaf = combine_alternatives(candidates)?;
        roster.push(TargetReferenceResult {
            path: mapping.path.clone(),
            root: leaf.root.place(),
            root_paths: leaf.root.paths(),
        });
        established.push(((result.place, mapping.path.clone()), leaf));
    }
    Some((roster, established))
}

/// Reconstruct one carrier's possible roots without consulting the target roster.
fn combine_alternatives(candidates: Vec<Leaf>) -> Option<Leaf> {
    let mut candidates = candidates.into_iter();
    let mut leaf = candidates.next()?;
    let mut paths = Vec::new();
    for candidate in candidates {
        if candidate.structural_type != leaf.structural_type
            || candidate.multiplicity != leaf.multiplicity
            || candidate.root.place() != leaf.root.place()
        {
            return None;
        }
        let Root::Ingress(_, path) = candidate.root else {
            return None;
        };
        paths.push(path);
    }
    if !paths.is_empty() {
        let Root::Ingress(place, path) = &leaf.root else {
            return None;
        };
        paths.push(path.clone());
        paths.sort();
        if paths.windows(2).any(|pair| pair[0] == pair[1]) {
            return None;
        }
        leaf.root = Root::Alternatives(*place, paths);
    }
    Some(leaf)
}

/// The custody effect of one authored or resolved provider call: owned
/// carriers move, the declared result leaf roster lands as live custody, and
/// a non-reference-only result establishes its durable home. A Unit result
/// moves arguments only.
#[allow(clippy::too_many_arguments)]
fn apply_call(
    custody: &mut Custody,
    expected: &mut BTreeMap<OperationId, Vec<TargetReferenceResult>>,
    function: &AbstractFunction,
    source_functions: &[AbstractFunction],
    declarations: &[StructuralTypeDeclaration],
    psi_operation: OperationId,
    callee: MachineId,
    result: Option<&StructuralOperationResult>,
    structural_arguments: &[StructuralArgument],
) -> Option<()> {
    let moved = argument_moves(function, declarations, custody, structural_arguments)?;
    commit_argument_moves(custody, &moved);
    let Some(result) = result else {
        return Some(());
    };
    let callee_function = source_functions
        .iter()
        .find(|function| function.machine == callee)?;
    let (roster, established) = call_result_leaves(
        callee_function,
        function,
        declarations,
        result,
        structural_arguments,
        custody,
        &moved,
    )?;
    for (key, leaf) in established {
        custody.leaves.insert(key, leaf);
    }
    expected.insert(psi_operation, roster);
    // A bare reference result is custody only: no physical result home is
    // established for a `Reference`-shaped carrier.
    if !matches!(
        lookup(declarations, result.structural_type).map(|declaration| &declaration.shape),
        Some(StructuralTypeShape::Reference { .. })
    ) && custody
        .homes
        .insert(result.place, (result.structural_type, result.multiplicity))
        .is_some()
    {
        return None;
    }
    Some(())
}

/// Record construction moves carriers, not referents: every live leaf of a
/// reference-bearing structural field argument relocates under the result's
/// field path, and a consumed affine home stops reporting a contract.
fn establish_fields(
    function: &AbstractFunction,
    declarations: &[StructuralTypeDeclaration],
    custody: &mut Custody,
    result: &StructuralOperationResult,
    fields: &[terminal_psi::RecordFieldInitializer],
    result_case: Option<semantic_vocabulary::StructuralCaseId>,
) -> Option<()> {
    let shape = &lookup(declarations, result.structural_type)?.shape;
    let declared = match (shape, result_case) {
        (StructuralTypeShape::Record { fields }, None) => fields,
        (StructuralTypeShape::Sum { cases }, Some(result_case)) => {
            &cases.iter().find(|case| case.id == result_case)?.fields
        }
        _ => return None,
    };
    let declared = declared.iter().filter(|field| !field.relevance.is_erased());
    if declared.clone().count() != fields.len() {
        return None;
    }
    let mut consumed = BTreeSet::new();
    let mut relocations = Vec::new();
    for (field, declaration) in fields.iter().zip(declared) {
        if field.field != declaration.id || declaration.relevance.is_erased() {
            return None;
        }
        let RecordFieldValue::Structural(argument) = &field.value else {
            continue;
        };
        let StructuralFieldType::Structural(nested) = declaration.field_type else {
            return None;
        };
        let reference_bearing = contains_reference(declarations, nested);
        if result_case.is_some() && reference_bearing {
            return None;
        }
        if !matches!(
            lookup(declarations, nested).map(|declaration| &declaration.shape),
            Some(
                StructuralTypeShape::Record { .. }
                    | StructuralTypeShape::Sum { .. }
                    | StructuralTypeShape::Mixed { .. }
            )
        ) && !reference_bearing
            && terminal_semantics::scalar_array_leaf_shape(declarations.iter(), nested).is_none()
            && !aggregate_member_storage(declarations, nested)
        {
            return None;
        }
        // A bare carrier field contributes custody, not storage: its operand
        // contract comes from the live leaf map, not a structural home.
        let bare_carrier = reference_bearing
            && matches!(
                lookup(declarations, nested).map(|declaration| &declaration.shape),
                Some(StructuralTypeShape::Reference { .. })
            );
        let multiplicity = if bare_carrier {
            custody
                .leaves
                .get(&(argument.place, Vec::new()))
                .filter(|leaf| leaf.structural_type == nested)?
                .multiplicity
        } else if let Some(&(structural_type, multiplicity)) = custody.homes.get(&argument.place) {
            if structural_type != nested {
                return None;
            }
            multiplicity
        } else {
            let mut parameters = function
                .structural_parameters
                .iter()
                .filter(|parameter| parameter.place == argument.place);
            let parameter = parameters.next()?;
            if parameters.next().is_some()
                || parameter.structural_type != nested
                || parameter.access != StructuralAccess::Owned
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return None;
            }
            parameter.multiplicity
        };
        if argument.access != StructuralAccess::Owned
            || !argument.path.is_empty()
            || multiplicity == StructuralMultiplicity::Linear
            || (multiplicity == StructuralMultiplicity::Affine && !consumed.insert(argument.place))
        {
            return None;
        }
        if !reference_bearing {
            continue;
        }
        let (contract, _) = source_contract(function, custody, argument.place)?;
        if contract != nested {
            return None;
        }
        let paths = leaf_paths(declarations, contract, custody.leaves.len())?;
        for path in &paths {
            if !custody.leaves.contains_key(&(argument.place, path.clone())) {
                return None;
            }
            let mut destination = vec![StructuralPathSegment::Field(declaration.identity.clone())];
            destination.extend(path.iter().cloned());
            relocations.push(((argument.place, path.clone()), destination));
        }
        // Every leaf the argument carrier owns must relocate; a differing
        // roster cannot describe the operand's declared type.
        if custody
            .leaves
            .keys()
            .any(|(carrier, path)| *carrier == argument.place && !paths.contains(path))
        {
            return None;
        }
    }
    // The result place is fresh on an admissible plan: no live carrier leaf or
    // existing home may already name it.
    if custody.homes.contains_key(&result.place)
        || custody
            .leaves
            .keys()
            .any(|(carrier, _)| *carrier == result.place)
    {
        return None;
    }
    for place in consumed {
        custody.homes.remove(&place);
    }
    for (from, to) in relocations {
        if let Some(leaf) = custody.leaves.remove(&from) {
            custody.leaves.insert((result.place, to), leaf);
        }
    }
    custody
        .homes
        .insert(result.place, (result.structural_type, result.multiplicity));
    Some(())
}

/// Whole-owner disposal ends every reference leaf the carrier still owns. A
/// carrier whose live roster diverges from its declared leaves has no honest
/// lowering, so the replay fails rather than releasing partially.
fn discard_owned(
    function: &AbstractFunction,
    declarations: &[StructuralTypeDeclaration],
    custody: &mut Custody,
    source: PlaceId,
) -> Option<()> {
    let Some((contract, _)) = source_contract(function, custody, source) else {
        return Some(());
    };
    if !contains_reference(declarations, contract) {
        return Some(());
    }
    let paths = leaf_paths(declarations, contract, custody.leaves.len())?;
    for path in paths.iter().rev() {
        custody.leaves.remove(&(source, path.clone()))?;
    }
    if custody.leaves.keys().any(|(carrier, _)| *carrier == source) {
        return None;
    }
    Some(())
}

/// Apply one body operation's custody effect. A terminator inside a block body
/// — or any operation the producer cannot lower — has no honest pair, so the
/// replay fails rather than guessing an effect.
fn apply(
    custody: &mut Custody,
    expected: &mut BTreeMap<OperationId, Vec<TargetReferenceResult>>,
    operation: &AbstractOperation,
    function: &AbstractFunction,
    source_functions: &[AbstractFunction],
    declarations: &[StructuralTypeDeclaration],
    boundary_calls: &BTreeMap<OperationId, BoundaryCallRow>,
) -> Option<()> {
    match operation {
        AbstractOperation::EstablishReference { result, source, .. } => {
            // A result place already carrying leaves — or already established
            // as a home — cannot be re-established on an admissible plan.
            if custody.homes.contains_key(&result.place)
                || bare_referent(declarations, result.structural_type).is_none()
                || custody
                    .leaves
                    .keys()
                    .any(|(carrier, _)| *carrier == result.place)
            {
                return None;
            }
            let root = normalized_source(function, custody, declarations, source)?;
            custody.leaves.insert(
                (result.place, Vec::new()),
                Leaf {
                    root,
                    structural_type: result.structural_type,
                    multiplicity: result.multiplicity,
                },
            );
        }
        AbstractOperation::ReleaseReference { source, .. } => {
            custody.leaves.remove(&(*source, Vec::new()))?;
        }
        AbstractOperation::EstablishRecord { result, fields, .. } => {
            establish_fields(function, declarations, custody, result, fields, None)?;
        }
        AbstractOperation::EstablishCase {
            result,
            result_case,
            fields,
            ..
        } => {
            establish_fields(
                function,
                declarations,
                custody,
                result,
                fields,
                Some(*result_case),
            )?;
        }
        AbstractOperation::EstablishPrimitiveLocal { result, .. }
        | AbstractOperation::EstablishZeroedStructural { result, .. }
        | AbstractOperation::EstablishScalarArray { result, .. } => {
            if custody
                .homes
                .insert(result.place, (result.structural_type, result.multiplicity))
                .is_some()
            {
                return None;
            }
        }
        // A call that forwards descriptors moves no argument custody here.
        AbstractOperation::CallStructural {
            dynamic_arguments,
            result: AbstractStructuralCallResult::Unit | AbstractStructuralCallResult::Scalar(_),
            ..
        } if !dynamic_arguments.is_empty() => {}
        AbstractOperation::CallStructural {
            structural_arguments,
            result: AbstractStructuralCallResult::Unit | AbstractStructuralCallResult::Scalar(_),
            ..
        } => {
            let moved = argument_moves(function, declarations, custody, structural_arguments)?;
            commit_argument_moves(custody, &moved);
        }
        AbstractOperation::CallStructural {
            psi_operation,
            result: AbstractStructuralCallResult::Structural { result, .. },
            callee,
            structural_arguments,
            ..
        } => {
            apply_call(
                custody,
                expected,
                function,
                source_functions,
                declarations,
                *psi_operation,
                *callee,
                Some(result),
                structural_arguments,
            )?;
        }
        AbstractOperation::BoundaryCall {
            psi_operation,
            result,
            structural_arguments,
            ..
        } => match boundary_calls.get(psi_operation) {
            Some(BoundaryCallRow::Installed(callee)) => {
                let result = match result {
                    AbstractBoundaryResult::Structural(result) => Some(result),
                    _ => None,
                };
                apply_call(
                    custody,
                    expected,
                    function,
                    source_functions,
                    declarations,
                    *psi_operation,
                    *callee,
                    result,
                    structural_arguments,
                )?;
            }
            // A settlement carries no reference custody rows; a structural
            // result still establishes its durable home.
            Some(BoundaryCallRow::Settlement) => {
                if let AbstractBoundaryResult::Structural(result) = result
                    && custody
                        .homes
                        .insert(result.place, (result.structural_type, result.multiplicity))
                        .is_some()
                {
                    return None;
                }
            }
            _ => {}
        },
        // Terminators never appear inside a block body on an admissible plan.
        AbstractOperation::Jump { .. }
        | AbstractOperation::Conditional { .. }
        | AbstractOperation::StructuralCase { .. }
        | AbstractOperation::Return { .. }
        | AbstractOperation::ReturnUnit { .. }
        | AbstractOperation::ReturnStructural { .. }
        | AbstractOperation::Crash { .. } => return None,
        _ => {}
    }
    Some(())
}

/// Apply one terminator's custody effect: edge discards end the leaves of the
/// owners they name. A structural case discards home storage only — it never
/// releases a reference leaf — and non-admissible cleanup shapes have no
/// honest lowering.
fn apply_terminator(
    custody: &mut Custody,
    operation: &AbstractOperation,
    function: &AbstractFunction,
    declarations: &[StructuralTypeDeclaration],
) -> Option<()> {
    let mut discards = Vec::new();
    match operation {
        AbstractOperation::Return {
            cleanup_actions, ..
        }
        | AbstractOperation::ReturnUnit {
            cleanup_actions, ..
        } => {
            for action in cleanup_actions {
                let TerminalAffineCleanupAction::DiscardRoot(place) = action else {
                    return None;
                };
                discards.push(*place);
            }
        }
        AbstractOperation::Jump {
            trivial_affine_discards,
            residual_affine_discards,
            ..
        } => {
            if !residual_affine_discards.is_empty() {
                return None;
            }
            discards.extend(trivial_affine_discards.iter().copied());
        }
        AbstractOperation::Conditional {
            when_true,
            when_false,
            ..
        } => {
            discards.extend(when_true.trivial_affine_discards.iter().copied());
            discards.extend(when_false.trivial_affine_discards.iter().copied());
        }
        AbstractOperation::ReturnStructural {
            trivial_affine_discards,
            ..
        } => {
            discards.extend(trivial_affine_discards.iter().copied());
        }
        _ => {}
    }
    for place in discards {
        discard_owned(function, declarations, custody, place)?;
    }
    Some(())
}

/// Dominator-order block schedule: each block inherits its immediate
/// dominator's exit custody. Unreachable or predecessor-free non-entry blocks
/// have no honest lowering.
fn schedule(
    incoming: &[Vec<usize>],
    outgoing: &[Vec<usize>],
    entry: usize,
) -> Option<Vec<(usize, Option<usize>)>> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![entry];
    while let Some(position) = pending.pop() {
        if reachable.insert(position) {
            pending.extend(outgoing.get(position)?.iter().copied());
        }
    }
    if reachable.len() != incoming.len() {
        return None;
    }
    let mut dominators = vec![reachable; incoming.len()];
    dominators[entry] = BTreeSet::from([entry]);
    loop {
        let mut changed = false;
        for (position, predecessors) in incoming.iter().enumerate() {
            if position == entry {
                continue;
            }
            let (first, others) = predecessors.split_first()?;
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
    // Strict dominators form a chain; the deepest strict dominator is the
    // immediate dominator whose exit custody every arrival inherits.
    let mut order = (0..incoming.len()).collect::<Vec<_>>();
    order.sort_by_key(|position| dominators[*position].len());
    order
        .into_iter()
        .map(|position| {
            if position == entry {
                Some((position, None))
            } else {
                dominators[position]
                    .iter()
                    .copied()
                    .filter(|dominator| *dominator != position)
                    .max_by_key(|dominator| dominators[*dominator].len())
                    .map(|dominator| (position, Some(dominator)))
            }
        })
        .collect()
}

/// Replay the caller's custody in dominator order and collect the reference
/// leaf roster each structural-result `CallStructural` — authored or
/// installed-provider — establishes. A source whose custody stream cannot be
/// replayed yields no expected rows, and a retained structural-result `Call`
/// row without an expected entry rejects.
pub(super) fn expected(
    function: &AbstractFunction,
    source_functions: &[AbstractFunction],
    declarations: &[StructuralTypeDeclaration],
    boundary_calls: &BTreeMap<OperationId, BoundaryCallRow>,
) -> Option<BTreeMap<OperationId, Vec<TargetReferenceResult>>> {
    let entries = &function.block_entries;
    if entries.is_empty() || entries[0].operation_offset != 0 {
        return None;
    }
    let mut ranges = Vec::with_capacity(entries.len());
    let mut incoming = vec![Vec::new(); entries.len()];
    let mut outgoing = vec![Vec::new(); entries.len()];
    let mut entry_position = None;
    for (position, entry) in entries.iter().enumerate() {
        if entries[..position]
            .iter()
            .any(|earlier| earlier.block == entry.block)
        {
            return None;
        }
        if entry.block == function.entry {
            entry_position = Some(position);
        }
        let end = entries
            .get(position + 1)
            .map_or(function.operations.len(), |next| next.operation_offset);
        if entry.operation_offset >= end || end > function.operations.len() {
            return None;
        }
        ranges.push(entry.operation_offset..end);
        let targets = match &function.operations[end - 1] {
            AbstractOperation::ReturnStructural { .. } => Vec::new(),
            AbstractOperation::Return {
                cleanup_actions, ..
            }
            | AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } if cleanup_actions
                .iter()
                .all(|action| matches!(action, TerminalAffineCleanupAction::DiscardRoot(_))) =>
            {
                Vec::new()
            }
            AbstractOperation::Jump {
                target,
                residual_affine_discards,
                ..
            } if residual_affine_discards.is_empty() => vec![*target],
            AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.target, when_false.target],
            AbstractOperation::StructuralCase { cases, .. } => {
                cases.iter().map(|case| case.target).collect()
            }
            _ => return None,
        };
        for target in targets {
            let target_position = entries
                .iter()
                .position(|candidate| candidate.block == target)?;
            if !outgoing[position].contains(&target_position) {
                outgoing[position].push(target_position);
                incoming[target_position].push(position);
            }
        }
    }
    let entry_position = entry_position?;
    if !incoming[entry_position].is_empty() {
        return None;
    }
    let mut exits: Vec<Option<Custody>> = vec![None; entries.len()];
    let mut expected = BTreeMap::new();
    for (position, dominator) in schedule(&incoming, &outgoing, entry_position)? {
        let mut custody = match dominator {
            None => entry(function, declarations)?,
            Some(dominator) => exits[dominator].clone()?,
        };
        let range = &ranges[position];
        for operation in &function.operations[range.start..range.end - 1] {
            apply(
                &mut custody,
                &mut expected,
                operation,
                function,
                source_functions,
                declarations,
                boundary_calls,
            )?;
        }
        apply_terminator(
            &mut custody,
            &function.operations[range.end - 1],
            function,
            declarations,
        )?;
        exits[position] = Some(custody);
    }
    Some(expected)
}
