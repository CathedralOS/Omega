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

use crate::lowering::dominator_schedule;
use abstract_operations::call_source;
use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::{
    AbstractBoundaryResult, AbstractFunction, AbstractOperation, AbstractStructuralCallResult,
};
use semantic_vocabulary::{BlockId, MachineId, OperationId, PlaceId, StructuralTypeId};
use target_operations::TargetReferenceResult;
use terminal_psi::{
    RecordFieldValue, StructuralAccess, StructuralArgument, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalAffineCleanupAction,
    find_structural_case, find_structural_type, structural_children,
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
    Alternatives(Vec<target_operations::TargetReferenceOrigin>),
}

impl Root {
    pub(super) fn origins(&self) -> Vec<target_operations::TargetReferenceOrigin> {
        match self {
            Self::Place(root) => vec![target_operations::TargetReferenceOrigin {
                root: *root,
                path: Vec::new(),
            }],
            Self::Ingress(root, path) => vec![target_operations::TargetReferenceOrigin {
                root: *root,
                path: path.clone(),
            }],
            Self::Alternatives(origins) => origins.clone(),
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
    borrowed_sources: std::sync::Arc<BTreeMap<PlaceId, Option<Vec<StructuralArgument>>>>,
    leaves: BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
    edge_exits: Vec<(
        BlockId,
        BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
    )>,
    homes: BTreeMap<PlaceId, (StructuralTypeId, StructuralMultiplicity)>,
}

impl Custody {
    /// Cleanup only removes leaves. Each outgoing edge retains an independent
    /// snapshot; descendants without a direct edge inherit the common survivors.
    fn finish_edges(
        &mut self,
        exits: Vec<(
            BlockId,
            BTreeMap<(PlaceId, Vec<StructuralPathSegment>), Leaf>,
        )>,
    ) {
        self.leaves
            .retain(|key, _| exits.iter().all(|(_, leaves)| leaves.contains_key(key)));
        self.edge_exits = exits;
    }

    fn enter_edge(&mut self, target: BlockId) {
        let mut matching = self.edge_exits.iter().filter(|(block, _)| *block == target);
        if let Some((_, leaves)) = matching.next() {
            self.leaves = leaves.clone();
            for (_, leaves) in matching {
                self.leaves.retain(|key, _| leaves.contains_key(key));
            }
        }
        self.edge_exits.clear();
    }
}

/// Reconstruct immutable borrowed arrivals from current source operations.
/// The emitted target roster and the producer's cache are never consulted.
fn borrowed_sources(
    function: &AbstractFunction,
    functions: &[AbstractFunction],
    boundary_machines: &[terminal_psi::BoundaryMachineDeclaration],
    declarations: &[StructuralTypeDeclaration],
) -> BTreeMap<PlaceId, Option<Vec<StructuralArgument>>> {
    use terminal_semantics::descriptor_lifetimes::{DescriptorLocation, descriptor_path};
    use terminal_semantics::descriptor_results::DescriptorAnalysis;
    let mut sources = BTreeMap::new();
    for parameter in function.block_structural_parameters() {
        if parameter.access != StructuralAccess::Owned
            && !terminal_semantics::descriptor_results::is_shared_descriptor(
                declarations.iter(),
                parameter.structural_type,
            )
        {
            sources.insert(parameter.place, None);
        }
    }
    if sources.is_empty() {
        return sources;
    }
    let Some(graph) = optimization_unit_semantics::descriptor_results::abstract_catalog_graph(
        functions.iter(),
        boundary_machines,
        declarations,
    ) else {
        return sources;
    };
    let Some(current) =
        terminal_semantics::descriptor_results::descriptor_machine(&graph, function.machine)
    else {
        return sources;
    };
    let mut analysis = DescriptorAnalysis::new(declarations, &graph);
    for entry in &function.block_entries {
        for parameter in &entry.structural_parameters {
            let Some(slot) = sources.get_mut(&parameter.place) else {
                continue;
            };
            *slot = analysis
                .storage_origins(
                    function.machine,
                    (entry.block, 0),
                    &DescriptorLocation::whole(parameter.place),
                )
                .and_then(|origins| {
                    origins
                        .into_iter()
                        .map(|origin| {
                            Some(StructuralArgument {
                                place: origin.root,
                                path: descriptor_path(
                                    declarations,
                                    *current.place_types.get(&origin.root)?,
                                    &origin.path,
                                )?,
                                access: parameter.access,
                            })
                        })
                        .collect()
                });
        }
    }
    sources
}

fn lookup(
    declarations: &[StructuralTypeDeclaration],
    identity: StructuralTypeId,
) -> Option<&StructuralTypeDeclaration> {
    find_structural_type(declarations, identity)
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
            pending.extend(structural_children(fields));
        };
        match &declaration.shape {
            StructuralTypeShape::Reference { .. } => return true,
            StructuralTypeShape::Record { fields, .. } => push_fields(fields),
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
            StructuralTypeShape::Record { fields, .. } => {
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
            StructuralTypeShape::FixedArray { element, length } => {
                if !contains_reference(declarations, *element) {
                    continue;
                }
                let count = usize::try_from(*length).ok()?;
                if pending
                    .len()
                    .checked_add(output.len())?
                    .checked_add(count)?
                    > maximum_leaves
                {
                    return None;
                }
                for index in (0..*length).rev() {
                    let mut child_path = path.clone();
                    child_path.push(StructuralPathSegment::FixedIndex(index));
                    pending.push((*element, child_path));
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
        if matches!(
            segment,
            StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. }
        ) && matches!(
            lookup(declarations, current)?.shape,
            StructuralTypeShape::ByteSequence(_)
        ) {
            current = terminal_semantics::primitive_projection_declaration(
                declarations.iter(),
                current,
                std::slice::from_ref(segment),
            )?;
            continue;
        }
        if matches!(segment, StructuralPathSegment::RuntimeIndex { .. }) {
            current = match &lookup(declarations, current)?.shape {
                StructuralTypeShape::FixedArray { element, .. }
                | StructuralTypeShape::ElementView { element } => *element,
                _ => return None,
            };
            continue;
        }
        if *segment == StructuralPathSegment::Referent {
            let StructuralTypeShape::Reference { referent, .. } =
                &lookup(declarations, current)?.shape
            else {
                return None;
            };
            current = *referent;
            continue;
        }
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
            StructuralTypeShape::Record { fields, .. } => fields,
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
        if referent(declarations, current).is_some()
            || plain_record_referent(declarations, current).is_some()
        {
            active.remove(&current);
            complete.insert(current);
            continue;
        }
        match lookup(declarations, current).map(|declaration| &declaration.shape) {
            Some(StructuralTypeShape::FixedArray { element, .. }) => {
                pending.push((current, true));
                pending.push((*element, false));
                continue;
            }
            Some(StructuralTypeShape::PrimitiveScalar(_)) => {
                active.remove(&current);
                complete.insert(current);
                continue;
            }
            _ => {}
        }
        let Some(StructuralTypeDeclaration {
            shape: StructuralTypeShape::Record { fields, .. },
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
    if let Some(parameter) = function.structural_parameter(place) {
        return Some((parameter.structural_type, parameter.multiplicity));
    }
    if let Some(parameter) = function.block_structural_parameter(place) {
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
    let parameter = function.structural_parameter(source.place)?;
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
/// complete projection retains selected payload and runtime-index coordinates.
/// Endpoint typing is independent of descriptor-slot identity and formation.
fn normalized_source(
    function: &AbstractFunction,
    custody: &Custody,
    declarations: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> Option<Root> {
    let Some(origins) = custody.borrowed_sources.get(&source.place) else {
        return normalized_direct_source(function, custody, declarations, source);
    };
    let origins = origins.as_ref().filter(|origins| !origins.is_empty())?;
    let projection = |argument: &StructuralArgument| {
        let (contract, _) = source_contract(function, custody, argument.place)?;
        if argument.path.last() == Some(&StructuralPathSegment::Referent) {
            let location = terminal_semantics::descriptor_lifetimes::descriptor_location(
                declarations,
                argument.place,
                contract,
                &argument.path,
                None,
            )?;
            return terminal_semantics::descriptor_lifetimes::descriptor_location_type(
                declarations,
                contract,
                &location.path,
            )
            .map(optimization_unit_semantics::StructuralStorageProjection::Endpoint);
        }
        optimization_unit_semantics::structural_storage_projection(
            declarations.iter(),
            contract,
            &argument.path,
        )
    };
    let expected = projection(source)?;
    let mut roots = Vec::with_capacity(origins.len());
    for origin in origins {
        let mut resolved = origin.clone();
        resolved.path.extend_from_slice(&source.path);
        resolved.access = source.access;
        // Rebinding may change the carrier's presentation, never a window's
        // original backing or exact interval. Keep every possible origin.
        if !projection(&resolved)?.matches(declarations.iter(), expected) {
            return None;
        }
        roots.push(normalized_direct_source(
            function,
            custody,
            declarations,
            &resolved,
        )?);
    }
    combine_roots(roots)
}

fn normalized_direct_source(
    function: &AbstractFunction,
    custody: &Custody,
    declarations: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> Option<Root> {
    if let Some(crossing) = source
        .path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)
    {
        let carrier_path = &source.path[..crossing];
        let suffix = &source.path[crossing + 1..];
        if let Some(leaf) = custody.leaves.get(&(source.place, carrier_path.to_vec())) {
            let StructuralTypeShape::Reference { referent, .. } =
                &lookup(declarations, leaf.structural_type)?.shape
            else {
                return None;
            };
            leaf_type(declarations, *referent, suffix)?;
            Some(leaf.root.clone())
        } else {
            // Member storage under a carrier never held as a live reference:
            // the loan roots at the carrier's own ingress storage, provided
            // the named member is not itself a reference.
            let (carrier, _) = source_contract(function, custody, source.place)?;
            leaf_type(declarations, carrier, carrier_path).and_then(|leaf| {
                match &lookup(declarations, leaf)?.shape {
                    StructuralTypeShape::Reference { .. } => None,
                    _ if suffix.is_empty() => {
                        Some(Root::Ingress(source.place, carrier_path.to_vec()))
                    }
                    _ => None,
                }
            })
        }
    } else {
        // Reconstruct the actual storage coordinate from the currently live
        // contract. A projected local and a projected parameter denote the
        // same custody operation; neither is a callee-local formal origin.
        let (contract, _) = source_contract(function, custody, source.place)?;
        optimization_unit_semantics::structural_storage_projection(
            declarations.iter(),
            contract,
            &source.path,
        )?;
        Some(if source.path.is_empty() {
            Root::Place(source.place)
        } else {
            Root::Ingress(source.place, source.path.clone())
        })
    }
}

/// Substitute a callee's formal result-source mapping into the caller's actual
/// argument at the same structural-parameter position.

/// The formal origin a callee result mapping names: a borrowed primitive
/// parameter or an ingress leaf path under an owned or shared carrier.
fn formal_origin(
    function: &AbstractFunction,
    declarations: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
) -> Option<Root> {
    let parameter = function.structural_parameter(source.place)?;
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
        if !argument
            .path
            .iter()
            .all(|segment| matches!(segment, StructuralPathSegment::Field(_)))
        {
            return None;
        }
        // Replay the selected subtree, including the whole-root empty path.
        // Leaves outside this projection stay in the caller's carrier.
        let selected = leaf_type(declarations, contract, &argument.path)?;
        let paths = leaf_paths(declarations, selected, custody.leaves.len())?;
        if paths.len()
            != custody
                .leaves
                .keys()
                .filter(|(carrier, path)| {
                    *carrier == argument.place && path.starts_with(&argument.path)
                })
                .count()
        {
            return None;
        }
        for path in paths {
            let mut carrier_path = argument.path.clone();
            carrier_path.extend(path);
            let leaf = custody
                .leaves
                .get(&(argument.place, carrier_path.clone()))?
                .clone();
            if moved.insert((argument.place, carrier_path), leaf).is_some() {
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
        let result_access = match lookup(
            declarations,
            leaf_type(declarations, result.structural_type, &mapping.path)?,
        )?
        .shape
        {
            StructuralTypeShape::Reference { access, .. } => access,
            _ => return None,
        };
        let mut candidates = Vec::with_capacity(mapping.sources.len());
        for formal in &mapping.sources {
            if formal.access != result_access
                && !(formal.access == StructuralAccess::MutableBorrow
                    && matches!(
                        result_access,
                        StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
                    ))
            {
                return None;
            }
            let source = call_source(&callee.structural_parameters, structural_arguments, formal)?;
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
                            let parameter = callee.structural_parameter(formal.place)?;
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
            origins: leaf.root.origins(),
        });
        established.push(((result.place, mapping.path.clone()), leaf));
    }
    Some((roster, established))
}

/// Reconstruct one carrier's possible roots without consulting the target roster.
fn combine_alternatives(candidates: Vec<Leaf>) -> Option<Leaf> {
    let mut candidates = candidates.into_iter();
    let mut leaf = candidates.next()?;
    let mut roots = vec![leaf.root.clone()];
    for candidate in candidates {
        if candidate.structural_type != leaf.structural_type
            || candidate.multiplicity != leaf.multiplicity
        {
            return None;
        }
        roots.push(candidate.root);
    }
    leaf.root = combine_roots(roots)?;
    Some(leaf)
}

fn combine_roots(roots: Vec<Root>) -> Option<Root> {
    let mut roots = roots.into_iter();
    let root = roots.next()?;
    if roots.len() == 0 {
        return Some(root);
    }
    let mut origins = root.origins();
    for root in roots {
        origins.extend(root.origins());
    }
    origins.sort();
    origins.dedup();
    Some(Root::Alternatives(origins))
}

/// The custody effect of one authored or resolved provider call: owned
/// carriers move, the declared result leaf roster lands as live custody, and
/// a materialized result establishes its durable home. A Unit result
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
    if custody
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
    establish_constructor(
        function,
        declarations,
        custody,
        result,
        fields,
        result_case,
        None,
    )
}

fn establish_constructor(
    function: &AbstractFunction,
    declarations: &[StructuralTypeDeclaration],
    custody: &mut Custody,
    result: &StructuralOperationResult,
    fields: &[terminal_psi::RecordFieldInitializer],
    result_case: Option<semantic_vocabulary::StructuralCaseId>,
    elements: Option<&[terminal_psi::StructuralArgument]>,
) -> Option<()> {
    let shape = &lookup(declarations, result.structural_type)?.shape;
    // Construction's field roster is common storage followed by the selected
    // payload. A record has only the former and a plain sum only the latter;
    // neither arrangement changes the custody operation on those fields.
    let (common, payload): (
        &[terminal_psi::StructuralFieldDeclaration],
        &[terminal_psi::StructuralFieldDeclaration],
    ) = match (shape, result_case) {
        (StructuralTypeShape::Record { fields, .. }, None) => (fields, &[]),
        (StructuralTypeShape::Sum { cases }, Some(result_case)) => {
            (&[], &find_structural_case(cases, result_case)?.fields)
        }
        (StructuralTypeShape::Mixed { fields, cases }, Some(result_case)) => {
            (fields, &find_structural_case(cases, result_case)?.fields)
        }
        (StructuralTypeShape::FixedArray { length, .. }, None)
            if elements
                .is_some_and(|elements| u64::try_from(elements.len()).ok() == Some(*length)) =>
        {
            (&[], &[])
        }
        _ => return None,
    };
    let declared = common
        .iter()
        .chain(payload)
        .filter(|field| !field.relevance.is_erased());
    if declared.clone().count() != fields.len() {
        return None;
    }
    let mut consumed = BTreeSet::new();
    let mut relocations = Vec::new();
    let element_type = match shape {
        StructuralTypeShape::FixedArray { element, .. } => {
            Some(StructuralFieldType::Structural(*element))
        }
        _ => None,
    };
    let mut operands = Vec::new();
    for (field, declaration) in fields.iter().zip(declared) {
        if field.field != declaration.id || declaration.relevance.is_erased() {
            return None;
        }
        if let RecordFieldValue::Structural(argument) = &field.value {
            operands.push((
                &declaration.field_type,
                StructuralPathSegment::Field(declaration.identity.clone()),
                argument,
            ));
        }
    }
    for (index, argument) in elements.into_iter().flatten().enumerate() {
        operands.push((
            element_type.as_ref()?,
            StructuralPathSegment::FixedIndex(index as u64),
            argument,
        ));
    }
    for (field_type, destination, argument) in operands {
        let (root_type, multiplicity) = source_contract(function, custody, argument.place)?;
        let nested = leaf_type(declarations, root_type, &argument.path)?;
        let source_shape = &lookup(declarations, nested)?.shape;
        let byte_carrier = field_type.byte_sequence_carrier();
        let descriptor_member = source_shape.is_shared_readable_byte_view()
            || matches!(source_shape, StructuralTypeShape::ElementView { .. });
        let declared_matches = match field_type {
            StructuralFieldType::Structural(expected) => nested == *expected,
            _ => match (byte_carrier, source_shape) {
                (
                    Some(expected @ terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }),
                    StructuralTypeShape::ByteSequence(actual),
                ) => {
                    expected == *actual
                        || (expected.is_shared_readable_view()
                            && source_shape.is_shared_readable_byte_view())
                }
                _ => false,
            },
        };
        if !declared_matches {
            return None;
        }
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
            && byte_carrier.is_none()
            && !descriptor_member
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
        if argument.access != StructuralAccess::Owned
            || multiplicity == StructuralMultiplicity::Linear
            || (!argument.path.is_empty() && multiplicity != StructuralMultiplicity::Affine)
            || function
                .arrival_parameter(argument.place)
                .is_some_and(|parameter| {
                    (!descriptor_member && parameter.access != StructuralAccess::Owned)
                        || (byte_carrier.is_none() && !parameter.qualifications.is_empty())
                        || !parameter.projected_qualifications.is_empty()
                })
        {
            return None;
        }
        if bare_carrier
            && custody
                .leaves
                .get(&(argument.place, Vec::new()))?
                .structural_type
                != nested
        {
            return None;
        }
        if multiplicity == StructuralMultiplicity::Affine {
            if consumed
                .iter()
                .any(|(place, path): &(PlaceId, Vec<StructuralPathSegment>)| {
                    *place == argument.place
                        && (path.starts_with(&argument.path) || argument.path.starts_with(path))
                })
            {
                return None;
            }
            consumed.insert((argument.place, argument.path.clone()));
        }
        // Projected non-reference storage does not move an unrelated sibling's
        // loan. Reference-bearing constructors keep the producer's existing
        // complete-carrier relocation contract below.
        if !reference_bearing
            && custody.leaves.keys().any(|(carrier, path)| {
                *carrier == argument.place
                    && (path.starts_with(&argument.path) || argument.path.starts_with(path))
            })
        {
            return None;
        }
        if !reference_bearing {
            continue;
        }
        stage_subtree_leaves(
            function,
            declarations,
            custody,
            argument,
            nested,
            std::slice::from_ref(&destination),
            &mut relocations,
        )?;
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
    for (place, path) in consumed {
        if path.is_empty() {
            custody.homes.remove(&place);
        }
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

/// Reconstruct the declared selected roster once for construction and window
/// transport. Paths select current carrier storage, not the leaf's referent;
/// its backing root moves unchanged, and unrelated siblings stay in place.
fn stage_subtree_leaves(
    function: &AbstractFunction,
    declarations: &[StructuralTypeDeclaration],
    custody: &Custody,
    argument: &terminal_psi::StructuralArgument,
    selected: StructuralTypeId,
    destination: &[StructuralPathSegment],
    relocations: &mut Vec<(
        (PlaceId, Vec<StructuralPathSegment>),
        Vec<StructuralPathSegment>,
    )>,
) -> Option<()> {
    if !contains_reference(declarations, selected) {
        return Some(());
    }
    let (contract, _) = source_contract(function, custody, argument.place)?;
    if argument.access != StructuralAccess::Owned
        || leaf_type(declarations, contract, &argument.path) != Some(selected)
    {
        return None;
    }
    let paths = leaf_paths(declarations, selected, custody.leaves.len())?;
    for path in &paths {
        let mut source = argument.path.clone();
        source.extend_from_slice(path);
        custody.leaves.get(&(argument.place, source.clone()))?;
        let mut target = destination.to_vec();
        target.extend_from_slice(path);
        relocations.push(((argument.place, source), target));
    }
    (!custody.leaves.keys().any(|(carrier, path)| {
        *carrier == argument.place
            && path
                .strip_prefix(argument.path.as_slice())
                .is_some_and(|relative| {
                    !paths.iter().any(|selected| selected.as_slice() == relative)
                })
    }))
    .then_some(())
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
    discard_subtree(declarations, custody, source, &[], contract)
}

/// A residual retires the declaration-derived descriptor roster below its
/// selected storage subtree. Siblings retain their current custody keys;
/// whole ownership conservation is replayed by the optimization receiver.
fn discard_residual(
    function: &AbstractFunction,
    declarations: &[StructuralTypeDeclaration],
    custody: &mut Custody,
    discard: &terminal_psi::StructuralAffineDiscard,
) -> Option<()> {
    let (root, multiplicity) = source_contract(function, custody, discard.place)?;
    if discard.path.is_empty()
        || multiplicity != StructuralMultiplicity::Affine
        || leaf_type(declarations, root, &discard.path) != Some(discard.structural_type)
    {
        return None;
    }
    discard_subtree(
        declarations,
        custody,
        discard.place,
        &discard.path,
        discard.structural_type,
    )
}

/// Descriptor retirement is the same exact subtree operation for no-code and
/// executable cleanup. The action's owner checks remain with its caller.
fn discard_subtree(
    declarations: &[StructuralTypeDeclaration],
    custody: &mut Custody,
    place: PlaceId,
    prefix: &[StructuralPathSegment],
    structural_type: StructuralTypeId,
) -> Option<()> {
    let paths = leaf_paths(declarations, structural_type, custody.leaves.len())?;
    let mut selected = BTreeSet::new();
    for path in paths {
        let mut path_at_root = prefix.to_vec();
        path_at_root.extend(path);
        custody.leaves.get(&(place, path_at_root.clone()))?;
        selected.insert(path_at_root);
    }
    if custody.leaves.keys().any(|(carrier, path)| {
        *carrier == place && path.starts_with(prefix) && !selected.contains(path)
    }) {
        return None;
    }
    custody
        .leaves
        .retain(|(carrier, path), _| *carrier != place || !selected.contains(path));
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
            custody
                .homes
                .insert(result.place, (result.structural_type, result.multiplicity));
        }
        AbstractOperation::ReleaseReference { source, path, .. } => {
            custody.leaves.remove(&(*source, path.clone()))?;
        }
        AbstractOperation::EstablishStructuralArray {
            result, elements, ..
        } => {
            establish_constructor(
                function,
                declarations,
                custody,
                result,
                &[],
                None,
                Some(elements),
            )?;
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
        AbstractOperation::MoveStructuralField {
            result,
            source,
            path,
            field,
            ..
        } => {
            if custody.homes.contains_key(&result.place)
                || custody
                    .leaves
                    .keys()
                    .any(|(carrier, _)| *carrier == result.place)
            {
                return None;
            }
            let (contract, _) = source_contract(function, custody, source.place())?;
            let projection = terminal_semantics::structural_window_projection(
                declarations.iter(),
                contract,
                path,
                *field,
            )?;
            if projection.structural_type != result.structural_type {
                return None;
            }
            let mut relocations = Vec::new();
            stage_subtree_leaves(
                function,
                declarations,
                custody,
                &terminal_psi::StructuralArgument {
                    place: source.place(),
                    path: projection.path,
                    access: StructuralAccess::Owned,
                },
                result.structural_type,
                &[],
                &mut relocations,
            )?;
            for (from, to) in relocations {
                let leaf = custody.leaves.remove(&from)?;
                custody.leaves.insert((result.place, to), leaf);
            }
            custody
                .homes
                .insert(result.place, (result.structural_type, result.multiplicity));
        }
        AbstractOperation::StoreStructuralField {
            destination,
            path,
            field,
            value,
            ..
        } => {
            let (contract, _) = source_contract(function, custody, destination.place())?;
            let projection = terminal_semantics::structural_window_projection(
                declarations.iter(),
                contract,
                path,
                *field,
            )?;
            if custody.leaves.keys().any(|(carrier, path)| {
                *carrier == destination.place() && path.starts_with(&projection.path)
            }) {
                return None;
            }
            let mut relocations = Vec::new();
            stage_subtree_leaves(
                function,
                declarations,
                custody,
                value,
                projection.structural_type,
                &projection.path,
                &mut relocations,
            )?;
            for (from, to) in relocations {
                let leaf = custody.leaves.remove(&from)?;
                custody.leaves.insert((destination.place(), to), leaf);
            }
            let (_, multiplicity) = source_contract(function, custody, value.place)?;
            if multiplicity != StructuralMultiplicity::Unrestricted {
                custody.homes.remove(&value.place);
            }
        }
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
                callee.machine()?,
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
        _ => {
            if let Some((_, result)) = operation.structural_result() {
                // Calls, constructors and reference formations replay their
                // own custody above. Other completed values contribute an
                // exact typed home, never a fabricated ordinary-reference
                // leaf roster. Byte descriptor provenance has its own owner.
                if contains_reference(declarations, result.structural_type)
                    || custody
                        .leaves
                        .keys()
                        .any(|(carrier, _)| *carrier == result.place)
                    || custody
                        .homes
                        .insert(result.place, (result.structural_type, result.multiplicity))
                        .is_some()
                {
                    return None;
                }
            }
        }
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
    let mut exits = Vec::new();
    for (edge, actions) in operation.cleanup_rosters() {
        let owned_transfer = match operation {
            AbstractOperation::Jump {
                target,
                structural_bindings,
                ..
            } => Some((*target, structural_bindings.as_slice())),
            AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => [when_true, when_false]
                .into_iter()
                .find(|successor| successor.psi_edge == edge)
                .map(|successor| (successor.target, successor.structural_bindings.as_slice())),
            _ => None,
        };
        let target = match operation {
            AbstractOperation::Jump { target, .. } => Some(*target),
            AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => [when_true, when_false]
                .into_iter()
                .find(|successor| successor.psi_edge == edge)
                .map(|successor| successor.target),
            AbstractOperation::StructuralCase { cases, .. } => cases
                .iter()
                .find(|successor| successor.psi_edge == edge)
                .map(|successor| successor.target),
            _ => None,
        };
        if let Some(target) = target {
            let mut exit = custody.clone();
            if let Some((target, bindings)) = owned_transfer {
                let destination = function.block_entry(target)?;
                let mut moved = Vec::new();
                for (binding, parameter) in bindings.iter().zip(&destination.structural_parameters)
                {
                    if binding.argument.access != StructuralAccess::Owned
                        || parameter.access != StructuralAccess::Owned
                    {
                        continue;
                    }
                    for ((carrier, path), leaf) in &exit.leaves {
                        if *carrier == binding.argument.place
                            && let Some(relative) =
                                path.strip_prefix(binding.argument.path.as_slice())
                        {
                            moved.push((
                                (*carrier, path.clone()),
                                (parameter.place, relative.to_vec()),
                                leaf.clone(),
                            ));
                        }
                    }
                }
                for (source, _, _) in &moved {
                    exit.leaves.remove(source);
                }
                for (_, destination, leaf) in moved {
                    exit.leaves.insert(destination, leaf);
                }
            }
            apply_cleanup_actions(&mut exit, actions.iter().cloned(), function, declarations)?;
            exits.push((target, exit.leaves));
        } else {
            apply_cleanup_actions(custody, actions.iter().cloned(), function, declarations)?;
        }
    }
    if !exits.is_empty() {
        custody.finish_edges(exits);
    }
    Some(())
}

fn apply_cleanup_actions(
    custody: &mut Custody,
    actions: impl IntoIterator<Item = TerminalAffineCleanupAction>,
    function: &AbstractFunction,
    declarations: &[StructuralTypeDeclaration],
) -> Option<()> {
    for action in actions {
        match action {
            TerminalAffineCleanupAction::RetireHome(place) => {
                source_contract(function, custody, place)?;
                if custody.leaves.keys().any(|(carrier, _)| *carrier == place) {
                    return None;
                }
            }
            TerminalAffineCleanupAction::DiscardRoot(place) => {
                discard_owned(function, declarations, custody, place)?
            }
            TerminalAffineCleanupAction::DiscardResidual(discard) => {
                discard_residual(function, declarations, custody, &discard)?
            }
            TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                let (root, _) = source_contract(function, custody, cleanup.place)?;
                if leaf_type(declarations, root, &cleanup.path) != Some(cleanup.structural_type) {
                    return None;
                }
                // Completing the edge-owned call retires its selected subject,
                // not the surrounding storage or a transferred sibling.
                discard_subtree(
                    declarations,
                    custody,
                    cleanup.place,
                    &cleanup.path,
                    cleanup.structural_type,
                )?;
            }
        }
    }
    Some(())
}

/// Dominator-order block schedule: each block inherits its immediate
/// dominator's exit custody. Unreachable or predecessor-free non-entry blocks
/// have no honest lowering.

/// Replay the caller's custody in dominator order and collect the reference
/// leaf roster each structural-result `CallStructural` — authored or
/// installed-provider — establishes. A source whose custody stream cannot be
/// replayed yields no expected rows, and a retained structural-result `Call`
/// row without an expected entry rejects.
pub(super) fn expected(
    function: &AbstractFunction,
    source_functions: &[AbstractFunction],
    declarations: &[StructuralTypeDeclaration],
    boundary_machines: &[terminal_psi::BoundaryMachineDeclaration],
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
            AbstractOperation::Return { .. }
            | AbstractOperation::ReturnUnit { .. }
            | AbstractOperation::Crash { .. } => Vec::new(),
            AbstractOperation::Jump { target, .. } => vec![*target],
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
    let mut initial = entry(function, declarations)?;
    initial.borrowed_sources = std::sync::Arc::new(borrowed_sources(
        function,
        source_functions,
        boundary_machines,
        declarations,
    ));
    let mut exits: Vec<Option<Custody>> = vec![None; entries.len()];
    let mut expected = BTreeMap::new();
    for (position, dominator) in dominator_schedule(&incoming, &outgoing, entry_position)? {
        let mut custody = match dominator {
            None => initial.clone(),
            Some(dominator) => exits[dominator].clone()?,
        };
        custody.enter_edge(entries[position].block);
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
