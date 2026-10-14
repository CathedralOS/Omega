//! Reference descriptor storage through native lowering. Psi has already
//! checked loan permissions, aliasing and disposal order. This stage transports
//! only the metadata needed by native operations and their independent audit:
//! current carrier type, referent type and possible storage origins.
//!
//! Origins describe possible backing; they never select a runtime address.
//! Projection uses the pointer actually stored in the current carrier home.
//! Conditional payload slots join by union without manufacturing a descriptor
//! for an inactive case. Moves, copies and calls follow current storage paths,
//! not reconstructed parent loans or formation histories.
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{
    TargetStructuralArgument, TargetStructuralParameter, TargetUnitOperation, TerminalPsiProvenance,
};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralPathSegment, StructuralTypeShape,
};
use terminal_psi::{StructuralArgument, StructuralOperationResult};

/// Exact incoming addresses for borrowed state parameters. The module query
/// runs once, then each lowering frontier shares its immutable function roster.
pub(in crate::lowering) type BorrowedSources =
    std::sync::Arc<BTreeMap<PlaceId, Option<Vec<StructuralArgument>>>>;

pub(in crate::lowering) fn borrowed_source_catalog(
    functions: &BTreeMap<semantic_vocabulary::MachineId, &AbstractFunction>,
    boundary_machines: &[terminal_psi::BoundaryMachineDeclaration],
    types: &StructuralTypeLookup<'_>,
) -> BTreeMap<semantic_vocabulary::MachineId, BorrowedSources> {
    use terminal_semantics::descriptor_lifetimes::{DescriptorLocation, descriptor_path};
    use terminal_semantics::descriptor_results::DescriptorAnalysis;
    let mut rosters = BTreeMap::new();
    for function in functions.values() {
        let mut sources = BTreeMap::new();
        for parameter in function.block_structural_parameters() {
            if parameter.access != StructuralAccess::Owned
                && !terminal_semantics::descriptor_results::is_shared_descriptor(
                    types.values().copied(),
                    parameter.structural_type,
                )
            {
                sources.insert(parameter.place, None);
            }
        }
        if !sources.is_empty() {
            rosters.insert(function.machine, sources);
        }
    }
    if rosters.is_empty() {
        return BTreeMap::new();
    }
    if let Some(graph) = optimization_unit_semantics::descriptor_results::abstract_catalog_graph(
        functions.values().copied(),
        boundary_machines,
        types.catalog(),
    ) {
        let mut analysis = DescriptorAnalysis::new(types.catalog(), &graph);
        for function in functions.values() {
            let Some(sources) = rosters.get_mut(&function.machine) else {
                continue;
            };
            let Some(current) = terminal_semantics::descriptor_results::descriptor_machine(
                &graph,
                function.machine,
            ) else {
                continue;
            };
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
                                            types.catalog(),
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
        }
    }
    rosters
        .into_iter()
        .map(|(machine, sources)| (machine, std::sync::Arc::new(sources)))
        .collect()
}

/// Metadata for a descriptor at its current carrier location. Permission and
/// disposal identities remain with Psi, not a second native ownership model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReferenceStorage {
    pub(super) root: ReferenceRoot,
    pub(super) carrier_type: StructuralTypeId,
    pub(super) referent: StructuralTypeId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ReferenceRoot {
    /// A mutable borrowed parameter or activation-local primitive place.
    Place(PlaceId),
    /// One leaf of an owned reference-bearing ingress parameter; the referent
    /// storage belongs to the caller, so no local pointer exists for it.
    Ingress(PlaceId, Vec<StructuralPathSegment>),
    /// Possible backing paths for one runtime-selected descriptor.
    Alternatives(Vec<target_operations::TargetReferenceOrigin>),
}

impl ReferenceRoot {
    /// Compose presentation metadata without selecting a runtime address.
    fn projected(&self, suffix: &[StructuralPathSegment]) -> Self {
        self.projected_family(
            &suffix
                .iter()
                .cloned()
                .map(terminal_psi::ReferenceOriginProjection::Exact)
                .collect::<Vec<_>>(),
            semantic_vocabulary::ReferenceOriginExtent::Projected,
        )
    }

    fn projected_family(
        &self,
        suffix: &[terminal_psi::ReferenceOriginProjection],
        extent: semantic_vocabulary::ReferenceOriginExtent,
    ) -> Self {
        if suffix.is_empty() && extent == semantic_vocabulary::ReferenceOriginExtent::Projected {
            return self.clone();
        }
        let mut origins = self.origins();
        for origin in &mut origins {
            // A child's actual descriptor supplies its projection. Its public
            // envelope remains the original input region, whose type may be a
            // collection rather than the returned record being projected.
            if origin.extent == semantic_vocabulary::ReferenceOriginExtent::Projected {
                origin.path.extend_from_slice(suffix);
                origin.extent = extent;
            }
        }
        if origins.len() == 1
            && let Some(path) = origins[0].exact_path()
        {
            Self::Ingress(origins[0].root, path)
        } else {
            origins.sort();
            origins.dedup();
            Self::Alternatives(origins)
        }
    }

    pub(super) fn origins(&self) -> Vec<target_operations::TargetReferenceOrigin> {
        match self {
            Self::Place(root) => vec![target_operations::TargetReferenceOrigin {
                root: *root,
                extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                path: Vec::new(),
            }],
            Self::Ingress(root, path) => vec![target_operations::TargetReferenceOrigin {
                root: *root,
                extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                path: path
                    .iter()
                    .cloned()
                    .map(terminal_psi::ReferenceOriginProjection::Exact)
                    .collect(),
            }],
            Self::Alternatives(origins) => origins.clone(),
        }
    }
}

/// A live reference's payload producer and geometry after dereferencing it.
/// Custody retains every possible origin; physical addressing loads the one
/// address stored in the carrier, never a selected entry from that origin set.
pub(in crate::lowering) struct ReferentProjection {
    pub(in crate::lowering) carrier_type: StructuralTypeId,
    pub(in crate::lowering) referent: StructuralTypeId,
    /// Exact descriptor prefix before the final crossing.
    pub(in crate::lowering) carrier_path: Vec<StructuralPathSegment>,
    /// Relative to the referent, excluding the explicit crossing.
    pub(in crate::lowering) path: Vec<StructuralPathSegment>,
}

pub(super) fn referent_projection(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    carrier: PlaceId,
    path: &[StructuralPathSegment],
    required_access: StructuralAccess,
) -> Option<ReferentProjection> {
    let home = live.structural_homes.get(&carrier)?;
    if home.access() != StructuralAccess::Owned
        || home.multiplicity() != StructuralMultiplicity::Affine
        || !home.qualifications().is_empty()
        || !home.projected_qualifications().is_empty()
        || home.operation_result().is_some_and(|(_, result)| {
            !result.qualification_establishments.is_empty() || !result.claims.is_empty()
        })
    {
        return None;
    }
    let projected = window_projection(function, live, types, carrier, path, required_access)?;
    matches!(
        types.get(&projected.referent)?.shape,
        StructuralTypeShape::Record { .. }
    )
    .then_some(projected)
}

/// Resolve a window through the exact stored descriptor; its enclosing home
/// remains the physical root, including when the referent is an array.
pub(super) fn window_projection(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    carrier: PlaceId,
    path: &[StructuralPathSegment],
    required_access: StructuralAccess,
) -> Option<ReferentProjection> {
    let crossing = path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)?;
    let carrier_path = &path[..crossing];
    let root_type = source_contract(function, live, carrier)?;
    let descriptor = leaf_type(types, root_type, carrier_path)?;
    let StructuralTypeShape::Reference { referent, access } = types.get(&descriptor)?.shape else {
        return None;
    };
    if !required_access.admits_referent(access) {
        return None;
    }
    crate::lowering::structural_layout::reference_referent_projection(
        root_type,
        path,
        required_access,
        types,
    )
    .ok()?;
    let descriptors = window_descriptors(function, types, live, carrier, path).ok()?;
    if descriptors
        .iter()
        .any(|leaf| leaf.carrier_type != descriptor || leaf.referent != referent)
    {
        return None;
    }
    Some(ReferentProjection {
        carrier_type: root_type,
        carrier_path: carrier_path.to_vec(),
        referent,
        path: path[crossing + 1..].to_vec(),
    })
}

/// Borrow through the live carrier's address word, regardless of which
/// operation established or relocated it. Custody proves the leaf is live;
/// the typed prefix selects its descriptor and the suffix selects backing.
/// Never substitute one possible origin for the pointer actually stored.
pub(super) fn reference_call_argument(
    argument: &StructuralArgument,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    destination: &TargetStructuralParameter,
    function: &AbstractFunction,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
) -> Result<TargetStructuralArgument, LoweringError> {
    if argument.access == StructuralAccess::Owned
        || argument.access != declaration.access
        || declaration.multiplicity != StructuralMultiplicity::Unrestricted
        || !declaration.qualifications.is_empty()
        || !declaration.projected_qualifications.is_empty()
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let (root_type, source) = if let Some(home) = live.structural_homes.get(&argument.place) {
        if home.has_claims()
            || home.multiplicity() == StructuralMultiplicity::Linear
            || !home.qualifications().is_empty()
            || !home.projected_qualifications().is_empty()
            || home
                .operation_result()
                .is_some_and(|(_, result)| !result.qualification_establishments.is_empty())
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let source = home.argument_source();
        (home.structural_type(), source)
    } else {
        let parameter = prepared
            .parameter(argument.place)
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        if parameter.access == StructuralAccess::WriteOnlyBorrow {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        (
            parameter.structural_type,
            parameter.placement.clone().into(),
        )
    };
    let (selected, shape, offset, runs) =
        crate::lowering::structural_layout::reference_referent_projection(
            root_type,
            &argument.path,
            argument.access,
            types,
        )?;
    let indices = super::scalar_sources::runtime_indices(
        runs,
        function,
        &super::scalar_sources::ScalarSources::from(live),
    )?;
    if selected != declaration.structural_type
        || crate::lowering::structural_layout::internal_parameter_shape(
            selected,
            shape,
            declaration.access,
            types,
        ) != destination.shape
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    Ok(TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type: root_type,
        structural_type: selected,
        shape: destination.shape,
        source_byte_offset: offset,
        fixed_array_length: None,
        element_stride: None,
        indices,
        source,
        destination: destination.placement.clone(),
    })
}

/// Inspect owned containment only. A reference's referent is not its payload.
pub(super) fn contains_reference(types: &StructuralTypeLookup<'_>, root: StructuralTypeId) -> bool {
    terminal_psi::structural_type_contains_reference_by(|id| types.get(&id).copied(), root)
}

/// Possible descriptor slots in owned storage, stopping at reference edges.
/// Case segments guard payload namespaces: these are not simultaneously live
/// permissions, nor do different case IDs imply disjoint physical storage.
pub(super) fn leaf_paths(
    types: &StructuralTypeLookup<'_>,
    root: StructuralTypeId,
    maximum_leaves: usize,
) -> Option<Vec<Vec<StructuralPathSegment>>> {
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
            StructuralTypeShape::Reference { .. } => {
                if output.len() == maximum_leaves {
                    return None;
                }
                output.push(path);
            }
            StructuralTypeShape::Record { .. }
            | StructuralTypeShape::Sum { .. }
            | StructuralTypeShape::Mixed { .. } => {
                let fields = declaration
                    .shape
                    .direct_fields()
                    .into_iter()
                    .flatten()
                    .map(|field| (None, field))
                    .chain(
                        declaration
                            .shape
                            .sum_cases()
                            .into_iter()
                            .flatten()
                            .flat_map(|case| {
                                case.fields.iter().map(move |field| (Some(case.id), field))
                            }),
                    );
                for (case, field) in fields.rev() {
                    if field.relevance.is_erased() {
                        continue;
                    }
                    if let Some(child) = field.field_type.as_structural() {
                        if !contains_reference(types, child) {
                            continue;
                        }
                        // Every pending owned subtree owes at least one leaf.
                        // Bound the worklist before a wide DAG can amplify it.
                        if pending.len().checked_add(output.len())? >= maximum_leaves {
                            return None;
                        }
                        let mut child_path = path.clone();
                        if let Some(case) = case {
                            child_path.push(StructuralPathSegment::Case(case));
                        }
                        child_path.push(StructuralPathSegment::Field(field.identity.clone()));
                        pending.push((child, child_path));
                    }
                }
            }
            StructuralTypeShape::FixedArray { element, length } => {
                if !contains_reference(types, *element) {
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

/// A reference owns one address descriptor, independent of its referent's
/// shape or contained loans. Looking up its declared referent grants neither
/// payload observation nor ownership of nested descriptors. Establishment and
/// each use separately check source compatibility, access and exact paths;
/// writes retain their ordinary validity and displaced-custody obligations.
pub(super) fn referent(
    types: &StructuralTypeLookup<'_>,
    structural_type: StructuralTypeId,
) -> Option<StructuralTypeId> {
    match types.get(&structural_type)?.shape {
        StructuralTypeShape::Reference {
            referent,
            access:
                StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow,
        } => Some(referent),
        _ => None,
    }
}

/// A field-only carrier path ending at a reference leaf, then its referent;
/// a non-reference leaf names its own storage type.
pub(super) fn leaf_referent(
    types: &StructuralTypeLookup<'_>,
    current: StructuralTypeId,
    fields: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    let leaf = leaf_type(types, current, fields)?;
    match &types.get(&leaf)?.shape {
        StructuralTypeShape::Reference { .. } => referent(types, leaf),
        _ => Some(leaf),
    }
}

/// The structural type the path names beneath `current`: each `Field`
/// segment descends through record or mixed fields, with a plain leaf field
/// resolving to its canonical standalone declaration, and a `FixedIndex`
/// segment descends into a fixed array's element within its declared extent.
pub(super) fn leaf_type(
    types: &StructuralTypeLookup<'_>,
    current: StructuralTypeId,
    fields: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    // Native custody uses the same declaration-relative geometry as Terminal.
    // A re-view changes observation, never a carrier home or emitted pointer.
    terminal_semantics::runtime_structural_path_tip(types.values().copied(), current, fields)
        .map(|declaration| declaration.id)
        .or_else(|| {
            terminal_semantics::primitive_projection_declaration(
                types.values().copied(),
                current,
                fields,
            )
        })
}

/// The structural contract a place currently carries: a signature parameter, a
/// block-entry parameter, an ordinary home, a live established view, or a live
/// bare carrier result. A shared view has no home of its own; its descriptor
/// still names the element or byte geometry a loan through it selects.
fn source_contract(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    place: PlaceId,
) -> Option<StructuralTypeId> {
    if let Some(parameter) = function.arrival_parameter(place) {
        return Some(parameter.structural_type);
    }
    if let Some(home) = live.structural_homes.get(&place) {
        return Some(home.structural_type());
    }
    if let Some((_, view)) = live.views.get(&place) {
        return Some(*view);
    }
    live.references
        .get(&(place, Vec::new()))
        .map(|leaf| leaf.carrier_type)
}

/// Borrow storage through its published contract, regardless of whether it
/// arrived as a parameter or was constructed in this activation. Lending
/// storage does not move it or discharge the owner's claims: multiplicity
/// constrains disposal, not this type lookup. Qualifications retain semantic
/// obligations but do not change storage geometry or disqualify its borrow.
/// Access still limits the projection. A reference crossing resolves the
/// actual carrier type instead of treating its pointee as owned storage.
fn direct_storage_type(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> Option<StructuralTypeId> {
    if !source.path.iter().all(|segment| {
        matches!(
            segment,
            StructuralPathSegment::Field(_)
                | StructuralPathSegment::FixedIndex(_)
                | StructuralPathSegment::RuntimeIndex { .. }
                | StructuralPathSegment::RepresentationView { .. }
        )
    }) {
        return None;
    }
    let (structural_type, access) =
        if let Some(parameter) = function.arrival_parameter(source.place) {
            (parameter.structural_type, parameter.access)
        } else if let Some(home) = live.structural_homes.get(&source.place) {
            (home.structural_type(), home.access())
        } else {
            // A shared view keeps no home, but its producer published the same
            // operation-result contract a home retains, with the same authority.
            let result = live.view_result(function, source.place)?;
            (result.structural_type, StructuralAccess::Owned)
        };
    if !source.access.admits(access) {
        return None;
    }
    if source.access == StructuralAccess::SharedBorrow
        && let Some(observed) = terminal_semantics::borrowed_primitive_projection_declaration(
            types.values().copied(),
            structural_type,
            &source.path,
            None,
        )
    {
        return Some(observed);
    }
    leaf_type(types, structural_type, &source.path)
}

/// The referent type an establishment or projection may name. An empty path
/// names a whole ingress place, a trailing `Referent` dereferences a live
/// carrier, and a field/fixed-index path names exact borrowed ingress storage.
pub(super) fn reference_source_type(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
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
    if source.path.contains(&StructuralPathSegment::Referent) {
        let mut current = source_contract(function, live, source.place)?;
        let mut start = 0;
        for (position, segment) in source.path.iter().enumerate() {
            if *segment != StructuralPathSegment::Referent {
                continue;
            }
            current = leaf_type(types, current, &source.path[start..position])?;
            let StructuralTypeShape::Reference { referent, access } = &types.get(&current)?.shape
            else {
                // Preserve the existing terminal spelling for a direct leaf.
                return (position + 1 == source.path.len()).then_some(current);
            };
            let compatible = source.access.admits_referent(*access);
            if !compatible {
                return None;
            }
            current = *referent;
            start = position + 1;
        }
        return leaf_type(types, current, &source.path[start..]);
    }
    direct_storage_type(function, live, types, source)
}

/// Rebind ordinary borrowed storage without granting a new reference. Its
/// operation owns bounds, case and access checks; formation and result mapping
/// separately retain reference_source_type's stricter eligibility rules.
fn storage_projection(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> Option<optimization_unit_semantics::StructuralStorageProjection> {
    let carrier = source_contract(function, live, source.place)?;
    optimization_unit_semantics::structural_borrowed_storage_projection(
        types.values().copied(),
        carrier,
        &source.path,
        source.access,
    )
}

/// A `.., Referent` source follows the located carrier's possible backing;
/// an empty path names the whole ingress referent directly. Every
/// selected payload and runtime index retains its exact storage coordinate.
fn normalized_source(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> Result<ReferenceRoot, LoweringError> {
    let Some(origins) = live.borrowed_reference_sources.get(&source.place) else {
        return normalized_direct_source(function, live, types, source);
    };
    let origins = origins
        .as_ref()
        .filter(|origins| !origins.is_empty())
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let expected = storage_projection(function, live, types, source)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let mut candidates = Vec::with_capacity(origins.len());
    for origin in origins {
        let mut resolved = origin.clone();
        resolved.path.extend_from_slice(&source.path);
        resolved.access = source.access;
        // Rebinding may change the carrier's presentation, never a window's
        // original backing or exact interval. Keep every possible origin.
        if !storage_projection(function, live, types, &resolved)
            .is_some_and(|actual| actual.matches(types.values().copied(), expected))
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        candidates.push(normalized_direct_source(function, live, types, &resolved)?);
    }
    combine_candidate_origins(function, candidates)
}

fn normalized_direct_source(
    function: &AbstractFunction,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
    source: &StructuralArgument,
) -> Result<ReferenceRoot, LoweringError> {
    if let Some(crossing) = source
        .path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)
    {
        let carrier_path = &source.path[..crossing];
        let suffix = &source.path[crossing + 1..];
        let Some(leaf) = live.references.get(&(source.place, carrier_path.to_vec())) else {
            // The member leaf of a carrier never held as a live reference
            // borrows that carrier's own storage; only a non-reference member
            // may root there — a missing reference carrier is a custody gap.
            let carrier = source_contract(function, live, source.place)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            return match leaf_type(types, carrier, carrier_path).and_then(|leaf| {
                types
                    .get(&leaf)
                    .and_then(|declaration| match declaration.shape {
                        StructuralTypeShape::Reference { .. } => None,
                        _ if suffix.is_empty() => Some(()),
                        _ => None,
                    })
            }) {
                Some(()) => Ok(ReferenceRoot::Ingress(source.place, carrier_path.to_vec())),
                None => Err(LoweringError::unsupported_control_flow(function.machine)),
            };
        };
        // The runtime source stays the retained carrier. Custody is inherited
        // from that carrier, while typed descendants select its live backing;
        // an initializer or a guessed origin never supplies the address.
        if leaf_type(types, leaf.referent, suffix).is_none() {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        Ok(leaf.root.projected(suffix))
    } else if source.path.is_empty() {
        Ok(ReferenceRoot::Place(source.place))
    } else if storage_projection(function, live, types, source).is_some() {
        Ok(ReferenceRoot::Ingress(source.place, source.path.clone()))
    } else {
        Err(LoweringError::unsupported_control_flow(function.machine))
    }
}

fn access_covers(authority: StructuralAccess, requested: StructuralAccess) -> bool {
    authority == requested
        || authority == StructuralAccess::Owned
        || authority == StructuralAccess::MutableBorrow
            && matches!(
                requested,
                StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
            )
}

mod copies;
pub(super) use copies::copied_leaves;
mod arrivals;
pub(super) use arrivals::merge_arrivals;
mod stored_calls;
pub(in crate::lowering) use stored_calls::{StoredEffectCatalog, catalog as stored_effect_catalog};
pub(super) use stored_calls::{commit as commit_stored_effects, stage as stage_stored_effects};

/// Transport existing descriptor metadata for owned call operands. Psi owns
/// whether these operands may move or alias; native lowering needs only the
/// leaves whose carrier storage actually moves. Borrowing a carrier or crossing
/// its pointer does not consume that carrier's descriptor storage.
///
/// Select the live metadata by its exact prefix instead of reconstructing a
/// type-wide roster. Case-qualified paths stay conditional; a call cannot
/// manufacture an occurrence for an inactive payload.
pub(super) fn moved_argument_leaves(
    live: &LiveDefinitions,
    arguments: &[StructuralArgument],
) -> BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceStorage> {
    let mut moved = BTreeMap::new();
    for argument in arguments {
        if argument.access != StructuralAccess::Owned
            || argument.path.contains(&StructuralPathSegment::Referent)
        {
            continue;
        }
        for ((carrier, path), leaf) in &live.references {
            if *carrier == argument.place
                && path
                    .strip_prefix(argument.path.as_slice())
                    .is_some_and(|relative| !relative.contains(&StructuralPathSegment::Referent))
            {
                moved.insert((*carrier, path.clone()), leaf.clone());
            }
        }
    }
    moved
}

/// Apply the staged argument moves once every other call check succeeded.
/// Moves are keyed by current carrier path, so result leaves re-established
/// under a moved identity survive the commit.
pub(super) fn commit_argument_moves(
    live: &mut LiveDefinitions,
    moved: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceStorage>,
) {
    for key in moved.keys() {
        live.references.remove(key);
    }
}

/// Establish descriptor storage; the backing root remains metadata only.
pub(super) fn establish(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::EstablishReference {
        psi_operation,
        result,
        source,
        storage,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let referent = referent(types, result.structural_type)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    if !result.is_affine_plain()
        || live.structural_homes.contains_key(&result.place)
        || live
            .references
            .keys()
            .any(|(carrier, _)| *carrier == result.place)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let actual = reference_source_type(function, live, types, source);
    if !actual.is_some_and(|actual| match storage {
        Some(storage) => terminal_semantics::reference_storage_source_compatible(
            types.values().copied(),
            actual,
            result.structural_type,
            storage.stored_scalar,
        ),
        None => terminal_semantics::reference_source_compatible(
            types.values().copied(),
            actual,
            result.structural_type,
        ),
    }) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    // EstablishReference always borrows, including a projection through an
    // owned record. Moving a stored descriptor uses the checked owned-value
    // projection/transfer operation, not an ingress-shape reinterpretation.
    let mut root = normalized_source(function, live, types, source)?;
    if let Some(storage) = storage {
        root = root.projected(&[StructuralPathSegment::RepresentationView {
            structural_type: referent,
            stored_scalar: storage.stored_scalar,
        }]);
    }
    live.references.insert(
        (result.place, Vec::new()),
        ReferenceStorage {
            root,
            carrier_type: result.structural_type,
            referent,
        },
    );
    // A reference owns descriptor storage just like every other structural
    // result. The metadata follows its descriptor, not referent ownership.
    let result_home = super::aggregate_results::home(*psi_operation, result, types)?;
    live.structural_homes
        .insert(result.place, result_home.clone());
    operations.push(TargetUnitOperation::EstablishReference {
        psi_operation: *psi_operation,
        result_home,
        source: source.clone(),
        storage: *storage,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

/// Retire metadata for the exact stored descriptor, never its referent storage.
pub(super) fn release(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::ReleaseReference {
        psi_operation,
        source,
        path,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    live.references
        .remove(&(*source, path.clone()))
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    operations.push(TargetUnitOperation::ReleaseReference {
        psi_operation: *psi_operation,
        source: *source,
        path: path.clone(),
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

/// Whole cleanup is the empty projection of exact subtree disposal.
pub(super) fn discard_owned(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    source: PlaceId,
) -> Result<bool, LoweringError> {
    if live.retired_reference_roots.contains(&source) {
        return Ok(true);
    }
    let discarded = discard_subtree(function, types, live, source, &[])?;
    if discarded {
        live.retired_reference_roots.insert(source);
    }
    Ok(discarded)
}

/// Dispose only the live descriptor slots selected by an authenticated cleanup
/// subtree. Removal follows current storage, not a formation identity.
pub(super) fn discard_subtree(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    source: PlaceId,
    prefix: &[StructuralPathSegment],
) -> Result<bool, LoweringError> {
    let Some(contract) = source_contract(function, live, source) else {
        return Ok(false);
    };
    let selected = leaf_type(types, contract, prefix)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    if !contains_reference(types, selected) {
        return Ok(false);
    }
    // Cleanup disposes the metadata actually resident in this subtree. A
    // constructor need not supply descriptors for its inactive alternatives.
    live.references.retain(|(carrier, path), _| {
        *carrier != source
            || !path
                .strip_prefix(prefix)
                .is_some_and(|relative| !relative.contains(&StructuralPathSegment::Referent))
    });
    Ok(true)
}

/// Authenticate the descriptor before the final reference crossing. Runtime
/// selector prefixes enumerate their declared candidates, each of which must
/// retain its live permission. The final backing may use an interpreted byte
/// presentation: authority does not require it to have the referent's type.
pub(super) fn window_descriptors<'live>(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &'live LiveDefinitions,
    place: PlaceId,
    path: &[StructuralPathSegment],
) -> Result<Vec<&'live ReferenceStorage>, LoweringError> {
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let crossing = path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)
        .ok_or_else(invalid)?;
    let regions = copies::storage_regions(function, types, live, place, &path[..crossing])?;
    let budget = live
        .references
        .len()
        .max(1)
        .checked_mul(path.len().max(1))
        .ok_or_else(invalid)?;
    let mut descriptors = Vec::new();
    for (carrier, prefix) in regions {
        let root = source_contract(function, live, carrier).ok_or_else(invalid)?;
        let candidates = terminal_semantics::stored_descriptor_paths(
            types.values().copied(),
            root,
            &prefix,
            budget,
        )
        .ok_or_else(invalid)?;
        for candidate in candidates {
            let leaf = live
                .references
                .get(&(carrier, candidate.clone()))
                .ok_or_else(invalid)?;
            if leaf_type(types, root, &candidate) != Some(leaf.carrier_type) {
                return Err(invalid());
            }
            descriptors.push(leaf);
        }
    }
    if descriptors.is_empty() {
        return Err(invalid());
    }
    Ok(descriptors)
}

/// Mutation follows one actual backing region. Possible origins cannot be
/// erased together merely because one runtime descriptor selects among them.
pub(super) fn window_storage(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &LiveDefinitions,
    place: PlaceId,
    path: &[StructuralPathSegment],
) -> Result<(PlaceId, Vec<StructuralPathSegment>), LoweringError> {
    let mut regions = copies::storage_regions(function, types, live, place, path)?;
    if regions.len() != 1 {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    regions
        .pop()
        .ok_or(LoweringError::unsupported_control_flow(function.machine))
}

/// Structural storage transfer moves carriers, not referents. Stage the exact
/// selected roster under its destination prefix before changing either home.
/// Construction and window replacement preserve possible backing origins.
/// Capture descriptor rows before retiring sources: one unrestricted operand
/// may supply several destinations without losing its own readable storage.
pub(super) fn subtree_leaves(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &LiveDefinitions,
    destination: &[StructuralPathSegment],
    nested: StructuralTypeId,
    argument: &StructuralArgument,
    transfers_storage: bool,
    relocations: &mut Vec<(ReferenceStorage, Vec<StructuralPathSegment>)>,
    moved: &mut BTreeSet<(PlaceId, Vec<StructuralPathSegment>)>,
) -> Result<(), LoweringError> {
    if !contains_reference(types, nested) {
        return Ok(());
    }
    let contract = source_contract(function, live, argument.place)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    if leaf_type(types, contract, &argument.path) != Some(nested) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    for ((carrier, source_path), descriptor) in &live.references {
        if *carrier != argument.place {
            continue;
        }
        let Some(path) = source_path.strip_prefix(argument.path.as_slice()) else {
            continue;
        };
        if path.contains(&StructuralPathSegment::Referent) {
            continue;
        }
        if transfers_storage {
            moved.insert((*carrier, source_path.clone()));
        }
        let mut destination = destination.to_vec();
        destination.extend(path.iter().cloned());
        relocations.push((descriptor.clone(), destination));
    }
    Ok(())
}

/// Apply staged relocations only after the complete operation checks out.
pub(super) fn relocate_leaves(
    live: &mut LiveDefinitions,
    result_place: PlaceId,
    relocations: Vec<(ReferenceStorage, Vec<StructuralPathSegment>)>,
    moved: BTreeSet<(PlaceId, Vec<StructuralPathSegment>)>,
) {
    for source in moved {
        live.references.remove(&source);
    }
    for (descriptor, destination) in relocations {
        live.references
            .insert((result_place, destination), descriptor);
    }
}

/// Establish the declared result reference roster of a structural-result
/// `CallStructural` whose result carries descriptor storage, resolved against
/// the already-committed `moved` argument leaves. Returns the ordered resolved
/// leaf roster retained on the target row.
pub(super) fn call_results(
    function: &AbstractFunction,
    declaration: abstract_operations::AbstractCallSignature<'_>,
    types: &StructuralTypeLookup<'_>,
    result: &StructuralOperationResult,
    structural_arguments: &[StructuralArgument],
    live: &mut LiveDefinitions,
    moved: &BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceStorage>,
) -> Result<Vec<target_operations::TargetReferenceResult>, LoweringError> {
    if !contains_reference(types, result.structural_type) {
        return Ok(Vec::new());
    }
    let contract = match declaration {
        abstract_operations::AbstractCallSignature::Machine(callee) => {
            let result = callee
                .result
                .structural()
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            terminal_semantics::ReferenceCallContract::Machine {
                parameters: &callee.structural_parameters,
                sources: &result.reference_sources,
            }
        }
        abstract_operations::AbstractCallSignature::Requirement(slot) => {
            terminal_semantics::ReferenceCallContract::Requirement(slot)
        }
    };
    let mappings = terminal_semantics::bind_reference_call_results(
        types.values().copied(),
        contract,
        structural_arguments,
    )
    .map_err(|reason| LoweringError::InvalidReferenceResult {
        machine: function.machine,
        reason,
    })?;
    if live
        .references
        .keys()
        .any(|(carrier, _)| *carrier == result.place)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    // Stage the complete declared result before publication. Psi has joined
    // the result's claim and descriptor custody, including conditional case
    // paths. Distinct result paths may describe mutually exclusive residences
    // of the same incoming leaf; native metadata must retain that relationship
    // rather than treating every signature slot as simultaneously resident.
    let mut established = Vec::new();
    let mut reference_results = Vec::new();
    for mapping in mappings {
        if mapping.sources.is_empty() {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let expected = leaf_referent(types, result.structural_type, &mapping.path)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        let result_access = leaf_type(types, result.structural_type, &mapping.path)
            .and_then(|leaf| types.get(&leaf))
            .and_then(|declaration| match declaration.shape {
                StructuralTypeShape::Reference { access, .. } => Some(access),
                _ => None,
            })
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        let mut candidates = Vec::with_capacity(mapping.sources.len());
        for binding in &mapping.sources {
            let source = &binding.source;
            if !access_covers(source.access, result_access) {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            if reference_source_type(function, live, types, source)
                .and_then(|root| {
                    terminal_semantics::reference_origin_type(
                        types.values().copied(),
                        root,
                        &binding.suffix,
                    )
                })
                .is_none_or(|endpoint| {
                    binding.extent == semantic_vocabulary::ReferenceOriginExtent::Projected
                        && endpoint != expected
                })
            {
                return Err(LoweringError::InvalidReferenceResult {
                    machine: function.machine,
                    reason: "returned reference origin does not resolve to its declared referent storage type",
                });
            }
            let leaf = if binding.transfers_existing {
                // An owned-carrier ingress mapping relocates one of the leaves the
                // moved arguments just transferred; possible backing follows
                // the descriptor bytes to the declared result path.
                let Some((StructuralPathSegment::Referent, carrier_path)) =
                    source.path.split_last()
                else {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                };
                let Some(leaf) = moved.get(&(source.place, carrier_path.to_vec())) else {
                    // The declared slot may be guarded by an inactive input
                    // case. Compose resident storage with declared origins;
                    // type geometry alone cannot supply its descriptor.
                    if !carrier_path
                        .iter()
                        .any(|segment| matches!(segment, StructuralPathSegment::Case(_)))
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                    continue;
                };
                leaf.clone()
            } else {
                ReferenceStorage {
                    root: normalized_source(function, live, types, source)?
                        .projected_family(&binding.suffix, binding.extent),
                    carrier_type: leaf_type(types, result.structural_type, &mapping.path)
                        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?,
                    referent: expected,
                }
            };
            candidates.push(leaf);
        }
        if candidates.is_empty() {
            continue;
        }
        let mut leaf = combine_alternatives(function, candidates)?;
        leaf.carrier_type = leaf_type(types, result.structural_type, &mapping.path)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        reference_results.push(target_operations::TargetReferenceResult {
            path: mapping.path.clone(),
            origins: leaf.root.origins(),
        });
        established.push(((result.place, mapping.path.clone()), leaf));
    }
    for (key, leaf) in established {
        live.references.insert(key, leaf);
    }
    Ok(reference_results)
}

/// A result descriptor retains every possible backing origin. This is an
/// address provenance join, not a new loan or a permission union.
fn combine_alternatives(
    function: &AbstractFunction,
    candidates: Vec<ReferenceStorage>,
) -> Result<ReferenceStorage, LoweringError> {
    let mut candidates = candidates.into_iter();
    let mut leaf = candidates
        .next()
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let mut origins = vec![leaf.root.clone()];
    for candidate in candidates {
        if leaf.referent != candidate.referent {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        origins.push(candidate.root);
    }
    leaf.root = combine_candidate_origins(function, origins)?;
    Ok(leaf)
}

fn combine_candidate_origins(
    function: &AbstractFunction,
    candidates: Vec<ReferenceRoot>,
) -> Result<ReferenceRoot, LoweringError> {
    let mut candidates = candidates.into_iter();
    let root = candidates
        .next()
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    if candidates.len() == 0 {
        return Ok(root);
    }
    let mut origins = root.origins();
    for candidate in candidates {
        origins.extend(candidate.origins());
    }
    origins.sort();
    origins.dedup();
    Ok(ReferenceRoot::Alternatives(origins))
}

/// Retain possible incoming descriptor storage, not a type-wide permission
/// grant. Case guards stay attached across branches and their joins; only a
/// checked case-qualified operation can select the corresponding descriptor.
pub(super) fn entry(
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
) -> Result<BTreeMap<(PlaceId, Vec<StructuralPathSegment>), ReferenceStorage>, LoweringError> {
    let mut references = BTreeMap::new();
    for parameter in &function.structural_parameters {
        if !contains_reference(types, parameter.structural_type) {
            continue;
        }
        // Seed possible descriptor slots from the signature. Case guards stay
        // on their paths; this metadata grants no inactive-payload permission.
        let paths = leaf_paths(
            types,
            parameter.structural_type,
            4096usize.saturating_sub(references.len()),
        )
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        for path in paths {
            references.insert(
                (parameter.place, path.clone()),
                ReferenceStorage {
                    root: ReferenceRoot::Ingress(parameter.place, path.clone()),
                    carrier_type: leaf_type(types, parameter.structural_type, &path)
                        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?,
                    referent: leaf_referent(types, parameter.structural_type, &path)
                        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?,
                },
            );
        }
    }
    Ok(references)
}
