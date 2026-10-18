//! Live loan replay over the current ownership frontier.
//!
//! This mirrors the verified Terminal-Psi reference walk: carriers own loan
//! permission, never their referent storage. Origins and immediate parents
//! are reconstructed from operations and checked call interfaces; no
//! producer-supplied lifetime or origin table is authority.

use super::address_joins::borrowed_regions::BorrowedReferenceSources;
use crate::OptimizationUnitValidationError;
use crate::unit_validation::operation_contracts::structural_source_contract;
use crate::unit_validation::references::{
    LiveReference, ReferenceIdentity, ReferenceOrigin, ReferenceOriginIdentity, ReferenceParent,
    access_path_type, access_referent_tail, contains_reference, formal_origin,
    is_reference_projection, leaf_access_within, leaf_paths, leaf_referent, leaf_type,
    placed_reference_source_compatible, reference_access, reference_source_type,
    storage_projection,
};
use abstract_operations::AbstractOperation as O;
use abstract_operations::AbstractStructuralCallResult;
use abstract_operations::call_source;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, MachineId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralAccess, StructuralArgument, StructuralTypeDeclaration};

// Incoming types have no explicit leaf roster to bound reconstruction. Keep
// this private verification-work capacity separate from resource authority;
// local/result rosters remain bounded by their existing captured rows.
const MAX_INCOMING_REFERENCE_LEAVES: usize = 4096;

/// Carrier storage and its pointees are distinct aliasing regions.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CallRegion {
    OrdinaryStorage,
    CarrierStorage,
    Referent,
}

mod access_paths;
pub(super) mod call_effects;
mod windows;
use access_paths::{access_reference_custody, call_dependency_conflict};

fn origin_place(origin: &ReferenceOrigin) -> PlaceId {
    match origin {
        ReferenceOrigin::Invocation { result, .. } => result.place,
        ReferenceOrigin::Primitive(place) => *place,
        ReferenceOrigin::IngressLeaf(identity) => identity.place,
        ReferenceOrigin::Alternatives { place, .. } => *place,
    }
}

/// Interpretation is a coordinate over the same original storage. Carrier
/// identities and immediate parents never acquire these origin-only steps.
fn projected_origin(
    origin: &ReferenceOrigin,
    suffix: &[terminal_psi::StructuralPathSegment],
) -> ReferenceOrigin {
    projected_origin_family(
        origin,
        &suffix
            .iter()
            .cloned()
            .map(terminal_psi::ReferenceOriginProjection::Exact)
            .collect::<Vec<_>>(),
        semantic_vocabulary::ReferenceOriginExtent::Projected,
    )
}

fn projected_origin_family(
    origin: &ReferenceOrigin,
    suffix: &[terminal_psi::ReferenceOriginProjection],
    extent: semantic_vocabulary::ReferenceOriginExtent,
) -> ReferenceOrigin {
    // Projected origins can narrow to a region at a call. Further operations
    // project the actual carrier and its parents, not that public envelope:
    // a returned record field need not be a field of the supplying slice.
    if suffix.is_empty() && extent == semantic_vocabulary::ReferenceOriginExtent::Projected {
        return origin.clone();
    }
    match origin {
        ReferenceOrigin::Invocation {
            result,
            carrier_type,
            path,
            extent: previous,
        } => {
            let mut path = path.clone();
            let extent = if *previous == semantic_vocabulary::ReferenceOriginExtent::Projected {
                path.extend_from_slice(suffix);
                extent
            } else {
                *previous
            };
            ReferenceOrigin::Invocation {
                result: result.clone(),
                carrier_type: *carrier_type,
                path,
                extent,
            }
        }
        ReferenceOrigin::Primitive(place) => {
            ReferenceOrigin::IngressLeaf(ReferenceOriginIdentity {
                place: *place,
                path: suffix.to_vec(),
                extent,
            })
        }
        ReferenceOrigin::IngressLeaf(identity) => {
            let mut identity = identity.clone();
            if identity.extent == semantic_vocabulary::ReferenceOriginExtent::Projected {
                identity.path.extend_from_slice(suffix);
                identity.extent = extent;
            }
            ReferenceOrigin::IngressLeaf(identity)
        }
        ReferenceOrigin::Alternatives { place, origins } => {
            let mut origins = origins.clone();
            for origin in &mut origins {
                if origin.extent == semantic_vocabulary::ReferenceOriginExtent::Projected {
                    origin.path.extend_from_slice(suffix);
                    origin.extent = extent;
                }
            }
            origins.sort();
            origins.dedup();
            ReferenceOrigin::Alternatives {
                place: *place,
                origins,
            }
        }
    }
}

fn origins_overlap(left: &ReferenceOrigin, right: &ReferenceOrigin) -> bool {
    // Separate external invocations do not prove spatial disjointness.
    if matches!(left, ReferenceOrigin::Invocation { .. })
        || matches!(right, ReferenceOrigin::Invocation { .. })
    {
        return true;
    }
    if origin_place(left) != origin_place(right) {
        return false;
    }
    // Distinct runtime selector identities may select the same element. Origin
    // replay retains their coordinates but cannot infer disjointness from them.
    let overlaps_path = |path: &[terminal_psi::ReferenceOriginProjection]| match right {
        ReferenceOrigin::Invocation { .. } | ReferenceOrigin::Primitive(_) => true,
        ReferenceOrigin::IngressLeaf(identity) => {
            terminal_semantics::reference_origin_paths_may_overlap(path, &identity.path)
        }
        ReferenceOrigin::Alternatives { origins, .. } => origins
            .iter()
            .any(|other| terminal_semantics::reference_origin_paths_may_overlap(path, &other.path)),
    };
    match left {
        ReferenceOrigin::Invocation { .. } | ReferenceOrigin::Primitive(_) => true,
        ReferenceOrigin::IngressLeaf(identity) => overlaps_path(&identity.path),
        ReferenceOrigin::Alternatives { origins, .. } => {
            origins.iter().any(|origin| overlaps_path(&origin.path))
        }
    }
}

fn invalid(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: Option<u32>,
    reason: &'static str,
) -> OptimizationUnitValidationError {
    OptimizationUnitValidationError::CurrentReferenceCustodyViolation {
        machine: function.machine,
        block,
        node,
        reason,
    }
}

/// Whether a selected carrier owns this resident; crossing a pointer selects
/// external storage whose descriptors do not move or retire with that carrier.
pub(super) fn owned_resident(
    path: &[terminal_psi::StructuralPathSegment],
    prefix: &[terminal_psi::StructuralPathSegment],
) -> bool {
    path.strip_prefix(prefix).is_some_and(|suffix| {
        !suffix.iter().any(|segment| {
            matches!(
                segment,
                terminal_psi::StructuralPathSegment::Referent
                    | terminal_psi::StructuralPathSegment::RepresentationView { .. }
            )
        })
    })
}

/// A surviving permission prevents moving its actual owned backing, not the
/// pointer carrier whose ingress coordinate names external storage.
pub(super) fn borrows_owned_storage(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    reference: &LiveReference,
    place: PlaceId,
) -> bool {
    let ingress = |identity: ReferenceOriginIdentity| {
        if identity.place != place {
            return false;
        }
        let expected =
            structural_source_contract(function, reference.carrier).and_then(|signature| {
                leaf_referent(types, signature.structural_type, &reference.carrier_path)
            });
        expected
            .and_then(|expected| {
                identity
                    .exact_identity()
                    .and_then(|identity| storage_cursor(function, types, identity, expected))
            })
            // The interpretation has no owned residence of its own, but a
            // live view still prevents moving the original owned backing.
            .is_none_or(|cursor| {
                !cursor
                    .path
                    .contains(&terminal_psi::StructuralPathSegment::Referent)
            })
    };
    match &reference.root {
        ReferenceOrigin::Invocation { .. } => false,
        ReferenceOrigin::Primitive(root) => *root == place,
        ReferenceOrigin::IngressLeaf(identity) => ingress(identity.clone()),
        ReferenceOrigin::Alternatives { origins, .. } => {
            origins.iter().any(|origin| ingress(origin.clone()))
        }
    }
}

/// Convert logical ingress custody to a typed storage cursor without changing
/// its permission identity or inventing any pointee descriptor at entry.
fn storage_cursor(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    mut identity: ReferenceIdentity,
    expected: StructuralTypeId,
) -> Option<ReferenceIdentity> {
    let signature = structural_source_contract(function, identity.place)?;
    let selected = leaf_type(types, signature.structural_type, &identity.path)?;
    if selected != expected {
        let parameter = function.structural_parameter(identity.place)?;
        if parameter.structural_type != signature.structural_type
            || identity.path.iter().any(|segment| {
                !matches!(
                    segment,
                    terminal_psi::StructuralPathSegment::Field(_)
                        | terminal_psi::StructuralPathSegment::Case(_)
                        | terminal_psi::StructuralPathSegment::FixedIndex(_)
                )
            })
            || !matches!(types.get(&selected).map(|declaration| &declaration.shape),
                Some(terminal_psi::StructuralTypeShape::Reference { referent, .. }) if *referent == expected)
        {
            return None;
        }
        identity
            .path
            .push(terminal_psi::StructuralPathSegment::Referent);
    }
    Some(identity)
}

/// Seed one ingress leaf per reference leaf of each reference-bearing entry
/// parameter. Owned ingress assumes one existing permission per leaf; callers
/// must supply the actual lineage, this is not local backing.
pub(super) fn entry_references(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<Vec<LiveReference>, OptimizationUnitValidationError> {
    let mut live = Vec::new();
    for parameter in &function.structural_parameters {
        if !contains_reference(structural_types, parameter.structural_type) {
            continue;
        }
        let paths = leaf_paths(
            structural_types,
            parameter.structural_type,
            MAX_INCOMING_REFERENCE_LEAVES - live.len(),
        )
        .ok_or_else(|| {
            invalid(
                function,
                function.entry,
                None,
                "incoming reference leaf reconstruction capacity exceeded",
            )
        })?;
        for path in paths {
            let identity = ReferenceIdentity {
                place: parameter.place,
                path: path.clone(),
            };
            let root = ReferenceOrigin::IngressLeaf(identity.clone().into());
            live.push(LiveReference {
                identity,
                carrier: parameter.place,
                carrier_path: path,
                root: root.clone(),
                parent: ReferenceParent::Root(root),
            });
        }
    }
    live.sort_by(|left, right| left.identity.cmp(&right.identity));
    Ok(live)
}

/// One operation's structural-argument slice.
fn operation_structural_arguments(operation: &O) -> &[StructuralArgument] {
    match operation {
        abstract_operations::call_structural_or_boundary_call_pattern!(structural_arguments) => {
            structural_arguments.as_slice()
        }
        // A window repair consumes its value like one owned whole call
        // argument; any reference leaves it carries settle with the store.
        O::StoreStructuralField { value, .. } => std::slice::from_ref(value),
        _ => &[],
    }
}

/// The spelled hole one `MoveStructuralField` opens beneath `root`: the
/// operation's path plus the declared identity of the field it vacates, or
/// the empty path of a root window's whole referent. `None` means the
/// destination does not host a structural field there — the unit's catalog
/// contract rejects that shape separately.
pub(super) fn window_hole(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    place: PlaceId,
    root: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
    required_access: StructuralAccess,
) -> Option<Vec<terminal_psi::StructuralPathSegment>> {
    let (geometry_root, suffix) = if path.contains(&terminal_psi::StructuralPathSegment::Referent) {
        crate::unit_validation::references::record_field_destination(
            function,
            types,
            place,
            path,
            required_access,
        )?
    } else {
        (root, path)
    };
    let projection = terminal_semantics::structural_window_projection(
        types.values().copied(),
        geometry_root,
        suffix,
        field,
    )?;
    let mut hole = path[..path.len() - suffix.len()].to_vec();
    hole.extend(projection.path);
    Some(hole)
}

pub(super) fn check_root_access(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: u32,
    live: &[LiveReference],
    place: PlaceId,
) -> Result<(), OptimizationUnitValidationError> {
    check_region_access(function, block, node, live, place, &[])
}

/// A selected loan suspends every candidate region, not every sibling under
/// their shared root. Whole-root consumers use the same rule with an empty
/// path. Geometry grants no permission through a reference carrier.
pub(super) fn check_region_access(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: u32,
    live: &[LiveReference],
    place: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
) -> Result<(), OptimizationUnitValidationError> {
    check_region_access_with(function, block, node, live, place, path, |_| false)
}

/// Shared observations may coexist with shared origins, never with overlapping
/// carrier storage. Mutations and custody transfers supply no compatibility.
fn check_region_access_with(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: u32,
    live: &[LiveReference],
    place: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    compatible_origin: impl Fn(&LiveReference) -> bool,
) -> Result<(), OptimizationUnitValidationError> {
    let overlaps = |other: &[terminal_psi::StructuralPathSegment]| {
        terminal_semantics::structural_paths_may_overlap(path, other)
    };
    let origin_path = path
        .iter()
        .cloned()
        .map(terminal_psi::ReferenceOriginProjection::Exact)
        .collect::<Vec<_>>();
    let overlaps_origin = |other: &[terminal_psi::ReferenceOriginProjection]| {
        terminal_semantics::reference_origin_paths_may_overlap(&origin_path, other)
    };
    if live.iter().any(|reference| {
        (reference.carrier == place && overlaps(&reference.carrier_path))
            || (origin_place(&reference.root) == place
                && !compatible_origin(reference)
                && match &reference.root {
                    ReferenceOrigin::Invocation { .. } | ReferenceOrigin::Primitive(_) => true,
                    ReferenceOrigin::IngressLeaf(identity) => overlaps_origin(&identity.path),
                    ReferenceOrigin::Alternatives { origins, .. } => {
                        origins.iter().any(|origin| overlaps_origin(&origin.path))
                    }
                })
    }) {
        return Err(invalid(
            function,
            block,
            Some(node),
            "operation accesses suspended referent or treats reference carrier as owned data",
        ));
    }
    Ok(())
}

fn check_record_field_access(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    node: u32,
    live: &[LiveReference],
    place: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
    required_access: StructuralAccess,
) -> Result<(), OptimizationUnitValidationError> {
    if !path.contains(&terminal_psi::StructuralPathSegment::Referent) {
        let region = super::storage_regions::projected(function, types, place, path, field);
        return check_region_access(function, block, node, live, place, region.direct_path());
    }
    access_reference_custody(
        function,
        types,
        block,
        node,
        live,
        &StructuralArgument {
            place,
            path: path.to_vec(),
            access: required_access,
        },
    )?;
    Ok(())
}

/// A readable whole reference observes its referent under its live permission.
/// Ordinary roots instead observe just the resolved storage region. Type alone
/// cannot revive a released carrier or bypass an outstanding child loan.
fn check_observation_access(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    node: u32,
    live: &[LiveReference],
    place: PlaceId,
    region: &[terminal_psi::StructuralPathSegment],
    compatible_origin: impl Fn(&LiveReference) -> bool,
) -> Result<(), OptimizationUnitValidationError> {
    let access = structural_source_contract(function, place)
        .and_then(|contract| types.get(&contract.structural_type))
        .and_then(|declaration| match declaration.shape {
            terminal_psi::StructuralTypeShape::Reference { access, .. } => Some(access),
            _ => None,
        });
    let Some(access) = access else {
        return check_region_access_with(
            function,
            block,
            node,
            live,
            place,
            region,
            compatible_origin,
        );
    };
    let mut carriers = live
        .iter()
        .filter(|reference| reference.carrier == place && reference.carrier_path.is_empty());
    let available = carriers.next().is_some_and(|carrier| {
        !live.iter().any(|child| origin_place(&child.root) == place || matches!(&child.parent, ReferenceParent::Reference(parent) if parent == &carrier.identity))
    }) && carriers.next().is_none();
    if !available || !matches!(access, terminal_psi::reading_borrow_pair_pattern!()) {
        return Err(invalid(
            function,
            block,
            Some(node),
            "observation has no active readable reference carrier",
        ));
    }
    Ok(())
}

/// A `.., Referent` source names a suspended child of the located carrier
/// leaf; an empty path names a referent root directly. An exact static
/// projection of an entry loan retains that complete projected origin.
// A shared cohort freezes mutation until every member ends; each member
// retains its own formation identity and immediate parent.
fn shared_source_compatible(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    reference: &LiveReference,
    access: StructuralAccess,
) -> bool {
    access == StructuralAccess::SharedBorrow
        && structural_source_contract(function, reference.carrier)
            .and_then(|contract| {
                leaf_type(
                    structural_types,
                    contract.structural_type,
                    &reference.carrier_path,
                )
            })
            .and_then(|leaf| reference_access(structural_types, leaf))
            == Some(StructuralAccess::SharedBorrow)
}

/// Capture shared descriptor occurrences without moving their source carriers.
/// The same storage read serves value copies and block-argument snapshots;
/// backing and immediate-parent lineage survive, but each destination has its
/// own descriptor identity. Owned geometry alone grants no copy permission.
pub(super) fn copy_reference_carriers(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    node: u32,
    live: &[LiveReference],
    source: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    selected_type: StructuralTypeId,
    destination: PlaceId,
) -> Result<Vec<LiveReference>, OptimizationUnitValidationError> {
    let refusal = |message| invalid(function, block, Some(node), message);
    if structural_source_contract(function, source)
        .and_then(|contract| leaf_type(types, contract.structural_type, path))
        != Some(selected_type)
        || !access_paths::descriptor_storage_available(
            function,
            types,
            live,
            source,
            path,
            &[],
            StructuralAccess::SharedBorrow,
        )
    {
        return Err(refusal("value copy has no available exact source storage"));
    }
    let paths = leaf_paths(types, selected_type, live.len())
        .ok_or_else(|| refusal("reference copy has no complete source leaf roster"))?;
    let mut copied = Vec::with_capacity(paths.len());
    for relative in &paths {
        if leaf_type(types, selected_type, relative).and_then(|leaf| reference_access(types, leaf))
            != Some(StructuralAccess::SharedBorrow)
        {
            return Err(refusal(
                "reference copy requires available shared descriptor storage",
            ));
        }
        let mut selected = path.to_vec();
        selected.extend_from_slice(relative);
        let mut observation = selected.clone();
        observation.push(terminal_psi::StructuralPathSegment::Referent);
        access_reference_custody(
            function,
            types,
            block,
            node,
            live,
            &StructuralArgument {
                place: source,
                path: observation,
                access: StructuralAccess::SharedBorrow,
            },
        )?;
        let mut matching = live
            .iter()
            .filter(|reference| reference.carrier == source && reference.carrier_path == selected);
        let source_leaf = matching
            .next()
            .ok_or_else(|| refusal("reference copy source leaf is absent"))?;
        if matching.next().is_some() {
            return Err(refusal(
                "reference copy source leaf is ambiguous or suspended",
            ));
        }
        copied.push(LiveReference {
            identity: ReferenceIdentity {
                place: destination,
                path: relative.clone(),
            },
            carrier: destination,
            carrier_path: relative.clone(),
            root: source_leaf.root.clone(),
            parent: source_leaf.parent.clone(),
        });
    }
    if live.iter().any(|reference| {
        reference.carrier == source
            && reference
                .carrier_path
                .strip_prefix(path)
                .is_some_and(|relative| {
                    !relative.contains(&terminal_psi::StructuralPathSegment::Referent)
                        && !paths.iter().any(|expected| expected == relative)
                })
    }) {
        return Err(refusal("reference copy source has an unaccounted leaf"));
    }
    Ok(copied)
}

/// Resolve a borrowed arrival at its binding point before checking the current
/// loan frontier. Every possible backing retains its own custody check.
fn normalized_source(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    borrowed_sources: &BorrowedReferenceSources,
    block: BlockId,
    node: u32,
    live: &[LiveReference],
    source: &StructuralArgument,
) -> Result<(ReferenceOrigin, ReferenceParent), OptimizationUnitValidationError> {
    let Some(origins) = borrowed_sources.get(&source.place) else {
        return normalized_direct_source(function, structural_types, block, node, live, source);
    };
    let invalid = || {
        invalid(
            function,
            block,
            Some(node),
            "borrowed reference source has no exact incoming storage",
        )
    };
    let origins = origins
        .as_ref()
        .filter(|origins| !origins.is_empty())
        .ok_or_else(invalid)?;
    let expected = storage_projection(function, structural_types, source).ok_or_else(invalid)?;
    let mut alternatives = Vec::with_capacity(origins.len());
    for origin in origins {
        let mut resolved = origin.clone();
        resolved.path.extend_from_slice(&source.path);
        resolved.access = source.access;
        // Rebinding may change the carrier's presentation, never a window's
        // original backing or exact interval. Keep every possible origin.
        if !crate::unit_validation::references::reference_source_accessible(
            function,
            structural_types,
            &resolved,
        ) || !storage_projection(function, structural_types, &resolved)
            .is_some_and(|actual| actual.matches(structural_types.values().copied(), expected))
        {
            return Err(invalid());
        }
        alternatives.push(normalized_direct_source(
            function,
            structural_types,
            block,
            node,
            live,
            &resolved,
        )?);
    }
    combine_alternative_origins(function, block, node, alternatives)
}

fn normalized_direct_source(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    node: u32,
    live: &[LiveReference],
    source: &StructuralArgument,
) -> Result<(ReferenceOrigin, ReferenceParent), OptimizationUnitValidationError> {
    if let Some(crossing) = source
        .path
        .iter()
        .rposition(|segment| *segment == terminal_psi::StructuralPathSegment::Referent)
    {
        let carrier_path = &source.path[..crossing];
        let suffix = &source.path[crossing + 1..];
        if let Some(parent) = live.iter().find(|reference| {
            reference.carrier == source.place && reference.carrier_path == carrier_path
        }) {
            if live
                .iter()
                .any(|reference| matches!(&reference.parent, ReferenceParent::Reference(identity) if identity == &parent.identity)
                    && !shared_source_compatible(function, structural_types, reference, source.access))
            {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "reference parent remains suspended by a live child",
                ));
            }
            return Ok((
                projected_origin(&parent.root, suffix),
                ReferenceParent::Reference(parent.identity.clone()),
            ));
        }
        // A `[fields.., Referent]` leaf that is not itself a reference borrows
        // a member's own storage: the loan roots at that member under the
        // source place rather than continuing a live parent loan.
        let leaf = structural_source_contract(function, source.place)
            .and_then(|contract| {
                crate::unit_validation::references::leaf_type(
                    structural_types,
                    contract.structural_type,
                    carrier_path,
                )
            })
            .ok_or_else(|| {
                invalid(
                    function,
                    block,
                    Some(node),
                    "projected reference custody is not yet supported",
                )
            })?;
        if structural_types.get(&leaf).is_some_and(|declaration| {
            matches!(
                declaration.shape,
                terminal_psi::StructuralTypeShape::Reference { .. }
            )
        }) {
            return Err(invalid(
                function,
                block,
                Some(node),
                "reference carrier is no longer live",
            ));
        }
        if !suffix.is_empty() {
            return Err(invalid(
                function,
                block,
                Some(node),
                "reference projection has no live supplying descriptor",
            ));
        }
        let root = ReferenceOrigin::IngressLeaf(
            ReferenceIdentity {
                place: source.place,
                path: carrier_path.to_vec(),
            }
            .into(),
        );
        if live.iter().any(|reference| {
            origins_overlap(&reference.root, &root)
                && !shared_source_compatible(function, structural_types, reference, source.access)
        }) {
            return Err(invalid(
                function,
                block,
                Some(node),
                "projected referent remains suspended by a live reference",
            ));
        }
        Ok((root.clone(), ReferenceParent::Root(root)))
    } else if source.path.is_empty() {
        if live.iter().any(|reference| {
            origin_place(&reference.root) == source.place
                && !shared_source_compatible(function, structural_types, reference, source.access)
        }) {
            return Err(invalid(
                function,
                block,
                Some(node),
                "original referent remains suspended by a live reference",
            ));
        }
        let root = ReferenceOrigin::Primitive(source.place);
        Ok((root.clone(), ReferenceParent::Root(root)))
    } else if storage_projection(function, structural_types, source).is_some() {
        let root = ReferenceOrigin::IngressLeaf(
            ReferenceIdentity {
                place: source.place,
                path: source.path.clone(),
            }
            .into(),
        );
        if live.iter().any(|reference| {
            origins_overlap(&reference.root, &root)
                && !shared_source_compatible(function, structural_types, reference, source.access)
        }) {
            return Err(invalid(
                function,
                block,
                Some(node),
                "projected referent remains suspended by a live reference",
            ));
        }
        Ok((root.clone(), ReferenceParent::Root(root)))
    } else {
        Err(invalid(
            function,
            block,
            Some(node),
            "projected reference custody is not yet supported",
        ))
    }
}

/// End one exact stored carrier after every child loan and enclosing storage
/// loan has ended. The empty path names the root carrier. Referent storage is
/// unaffected, and sibling descriptor identities remain live.
pub(super) fn release(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    node: Option<u32>,
    live: &mut Vec<LiveReference>,
    source: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
) -> Result<(), OptimizationUnitValidationError> {
    if !access_paths::descriptor_storage_available(
        function,
        structural_types,
        live,
        source,
        path,
        &[],
        StructuralAccess::Owned,
    ) {
        return Err(invalid(
            function,
            block,
            node,
            "release carrier storage remains borrowed",
        ));
    }
    let position = live
        .iter()
        .position(|reference| reference.carrier == source && reference.carrier_path == path)
        .ok_or_else(|| {
            invalid(
                function,
                block,
                node,
                "release requires one live reference carrier",
            )
        })?;
    if live
        .iter()
        .any(|reference| matches!(&reference.parent, ReferenceParent::Reference(identity) if identity == &live[position].identity))
    {
        return Err(invalid(
            function,
            block,
            node,
            "a reference cannot end before its child",
        ));
    }
    live.remove(position);
    Ok(())
}

/// Whole-owner cleanup is the empty projection of the same subtree disposal.
pub(super) fn discard_owned(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    live: &mut Vec<LiveReference>,
    source: PlaceId,
) -> Result<(), OptimizationUnitValidationError> {
    discard_subtree(function, structural_types, block, live, source, &[])
}

/// Reconstruct every live descriptor in one exact owned subtree. The complete
/// schedule is checked before removing storage keys, so shared provenance in
/// another carrier is never mistaken for the descriptor being disposed.
pub(super) fn discard_subtree(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    live: &mut Vec<LiveReference>,
    source: PlaceId,
    prefix: &[terminal_psi::StructuralPathSegment],
) -> Result<(), OptimizationUnitValidationError> {
    let Some(signature) = structural_source_contract(function, source) else {
        return Ok(());
    };
    let invalid_discard = || {
        invalid(
            function,
            block,
            None,
            "discard requires an exact live reference subtree",
        )
    };
    let selected = leaf_type(structural_types, signature.structural_type, prefix)
        .ok_or_else(invalid_discard)?;
    let paths = leaf_paths(structural_types, selected, live.len()).ok_or_else(invalid_discard)?;
    let mut released = BTreeSet::new();
    let mut released_paths = BTreeSet::new();
    for path in paths.iter().rev() {
        let mut selected_path = prefix.to_vec();
        selected_path.extend_from_slice(path);
        let reference = live
            .iter()
            .find(|reference| {
                reference.carrier == source && reference.carrier_path == selected_path
            })
            .ok_or_else(invalid_discard)?;
        if live.iter().any(|child| {
            matches!(&child.parent, ReferenceParent::Reference(identity)
                if identity == &reference.identity)
                && !released.contains(&child.identity)
        }) {
            return Err(invalid(
                function,
                block,
                None,
                "a reference cannot end before its child",
            ));
        }
        released.insert(reference.identity.clone());
        released_paths.insert(selected_path);
    }
    if live.iter().any(|reference| {
        reference.carrier == source
            && owned_resident(&reference.carrier_path, prefix)
            && !released_paths.contains(&reference.carrier_path)
    }) {
        return Err(invalid_discard());
    }
    live.retain(|reference| {
        reference.carrier != source || !released_paths.contains(&reference.carrier_path)
    });
    Ok(())
}

/// Record construction moves carriers, not referents. Exact static bindings
/// and the owning frontier are checked in this same operation transaction;
/// relocation cannot turn an owned move into a second, child loan.
fn establish_record(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    node: u32,
    result: &terminal_psi::StructuralOperationResult,
    fields: &[terminal_psi::RecordFieldInitializer],
    live: &mut [LiveReference],
) -> Result<(), OptimizationUnitValidationError> {
    let declarations = match structural_types
        .get(&result.structural_type)
        .map(|declaration| &declaration.shape)
    {
        Some(terminal_psi::StructuralTypeShape::Record { fields, .. }) => fields,
        _ => {
            return Err(invalid(
                function,
                block,
                Some(node),
                "record transfer requires a record establishment",
            ));
        }
    };
    let declarations = declarations
        .iter()
        .filter(|field| !field.relevance.is_erased());
    if declarations.clone().count() != fields.len() {
        return Err(invalid(
            function,
            block,
            Some(node),
            "record transfer field roster differs from its type",
        ));
    }
    let operands = declarations.zip(fields).filter_map(|(declaration, field)| {
        let terminal_psi::RecordFieldValue::Structural(argument) = &field.value else {
            return None;
        };
        Some((
            terminal_psi::StructuralPathSegment::Field(declaration.identity.clone()),
            argument,
        ))
    });
    establish_constructor(
        function,
        structural_types,
        block,
        node,
        result,
        operands,
        live,
    )
}

fn establish_constructor<'a>(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    node: u32,
    result: &terminal_psi::StructuralOperationResult,
    operands: impl Iterator<Item = (terminal_psi::StructuralPathSegment, &'a StructuralArgument)>,
    live: &mut [LiveReference],
) -> Result<(), OptimizationUnitValidationError> {
    if live.iter().any(|reference| {
        reference.carrier == result.place || reference.identity.place == result.place
    }) {
        return Err(invalid(
            function,
            block,
            Some(node),
            "record destination still owns live reference custody",
        ));
    }
    let mut relocations = Vec::new();
    let mut moved = BTreeSet::new();
    for (destination, argument) in operands {
        let signature = structural_source_contract(function, argument.place).ok_or_else(|| {
            invalid(
                function,
                block,
                Some(node),
                "record operand has no structural source",
            )
        })?;
        let selected_type = leaf_type(structural_types, signature.structural_type, &argument.path)
            .ok_or_else(|| {
                invalid(
                    function,
                    block,
                    Some(node),
                    "constructor operand has no exact selected type",
                )
            })?;
        let paths = leaf_paths(structural_types, selected_type, live.len()).ok_or_else(|| {
            invalid(
                function,
                block,
                Some(node),
                "constructor operand requires more reference leaves than are live",
            )
        })?;
        for path in paths {
            let mut source_path = argument.path.clone();
            source_path.extend_from_slice(&path);
            let position = live
                .iter()
                .position(|reference| {
                    reference.carrier == argument.place && reference.carrier_path == source_path
                })
                .ok_or_else(|| {
                    invalid(
                        function,
                        block,
                        Some(node),
                        "record operand does not own its reference leaf",
                    )
                })?;
            if !moved.insert(position) {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "record cannot duplicate reference custody",
                ));
            }
            let mut destination = vec![destination.clone()];
            destination.extend(path);
            relocations.push((position, destination));
        }
        if live.iter().enumerate().any(|(position, reference)| {
            reference.carrier == argument.place
                && owned_resident(&reference.carrier_path, &argument.path)
                && !moved.contains(&position)
        }) {
            return Err(invalid(
                function,
                block,
                Some(node),
                "record operand reference roster differs from its type",
            ));
        }
    }
    for (position, path) in relocations {
        live[position].carrier = result.place;
        live[position].carrier_path = path;
    }
    Ok(())
}

/// Substitute a callee's formal result-source mapping into the caller's actual
/// argument at the same structural-parameter position.

/// Replay one operation against the live loan set. This runs inside the same
/// operation transaction as the owning frontier: reads and writes cannot touch
/// a suspended referent, owned record construction relocates leaves, calls
/// transfer or borrow exact leaves, and establishment/release mutate custody.
#[allow(clippy::too_many_lines)]
pub(super) fn apply_operation(
    function: &PsiOptimizationFunction,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    borrowed_sources: &BorrowedReferenceSources,
    call_effects: &call_effects::ReferenceCallEffects,
    block: BlockId,
    node: u32,
    operation: &O,
    live: &mut Vec<LiveReference>,
) -> Result<(), OptimizationUnitValidationError> {
    // Every publication rebinds its result storage, regardless of producer
    // family or disposal multiplicity. The validated current place catalog
    // identifies the producer; a surviving loan pins that previous iteration.
    if let Some(operation) = operation.psi_operation()
        && live.iter().any(|reference| {
            [origin_place(&reference.root), reference.carrier, reference.identity.place]
                .into_iter()
                .any(|place| function.structural_places.iter().any(|declaration| {
                    declaration.id == place && matches!(declaration.kind,
                        semantic_vocabulary::StructuralPlaceKind::OperationResult { producer, .. } if producer == operation)
                }))
        })
    {
        return Err(invalid(function, block, Some(node), "operation result remains pinned by live reference custody"));
    }
    match operation {
        O::AtomicEvent { event, .. } => {
            if let Some(location) = event.location() {
                // Atomic identity grants shared access to the cell; every
                // stored pointer still owes its current readable loan.
                if location
                    .path
                    .contains(&terminal_psi::StructuralPathSegment::Referent)
                {
                    let path = match location.path.split_last() {
                        Some((terminal_psi::StructuralPathSegment::Case(_), prefix)) => prefix,
                        _ => &location.path,
                    };
                    access_reference_custody(
                        function,
                        structural_types,
                        block,
                        node,
                        live,
                        &StructuralArgument {
                            place: location.root,
                            path: path.to_vec(),
                            access: StructuralAccess::SharedBorrow,
                        },
                    )?;
                } else {
                    let region = super::storage_regions::projected(
                        function,
                        structural_types,
                        location.root,
                        &location.path,
                        Some(location.field),
                    );
                    check_observation_access(
                        function,
                        structural_types,
                        block,
                        node,
                        live,
                        location.root,
                        region.direct_path(),
                        |reference| {
                            shared_source_compatible(
                                function,
                                structural_types,
                                reference,
                                StructuralAccess::SharedBorrow,
                            )
                        },
                    )?;
                }
            }
        }
        O::ReleaseReference { source, path, .. } => {
            return release(
                function,
                structural_types,
                block,
                Some(node),
                live,
                *source,
                path,
            );
        }
        O::EstablishCase { fields, .. } => {
            // Plain case payloads carry owned data, not relocated references.
            // Static shape checks reject reference leaves; replay still owes
            // the live-loan check for every consumed child and fresh result.
            for field in fields {
                if let terminal_psi::RecordFieldValue::Structural(argument) = &field.value {
                    check_root_access(function, block, node, live, argument.place)?;
                }
            }
        }
        O::EstablishStructuralArray {
            result, elements, ..
        } => {
            return establish_constructor(
                function,
                structural_types,
                block,
                node,
                result,
                elements.iter().enumerate().map(|(index, argument)| {
                    (
                        terminal_psi::StructuralPathSegment::FixedIndex(index as u64),
                        argument,
                    )
                }),
                live,
            );
        }
        O::EstablishRecord { result, fields, .. } => {
            return establish_record(
                function,
                structural_types,
                block,
                node,
                result,
                fields,
                live,
            );
        }
        O::PrimitiveScalarRead { source, path, .. }
            if path.contains(&terminal_psi::StructuralPathSegment::Referent) =>
        {
            access_reference_custody(
                function,
                structural_types,
                block,
                node,
                live,
                &StructuralArgument {
                    place: *source,
                    path: path.clone(),
                    access: StructuralAccess::SharedBorrow,
                },
            )?;
        }
        abstract_operations::scalar_read_or_membership_pattern!(source, path, ..) => {
            let region =
                super::storage_regions::projected(function, structural_types, *source, path, None);
            check_observation_access(
                function,
                structural_types,
                block,
                node,
                live,
                *source,
                region.direct_path(),
                |reference| {
                    shared_source_compatible(
                        function,
                        structural_types,
                        reference,
                        StructuralAccess::SharedBorrow,
                    )
                },
            )?;
        }
        O::StructuralScalarFieldRead {
            source,
            path,
            field,
            ..
        } => {
            let region = super::storage_regions::canonical_field(
                function,
                structural_types,
                *source,
                path,
                *field,
            );
            check_observation_access(
                function,
                structural_types,
                block,
                node,
                live,
                *source,
                &region,
                |reference| {
                    shared_source_compatible(
                        function,
                        structural_types,
                        reference,
                        StructuralAccess::SharedBorrow,
                    )
                },
            )?;
        }
        O::StructuralCase { source, path, .. } => {
            check_observation_access(
                function,
                structural_types,
                block,
                node,
                live,
                *source,
                path,
                |_| false,
            )?;
        }
        O::PrimitiveLocalStore { destination, .. } => {
            check_root_access(function, block, node, live, *destination)?
        }
        abstract_operations::byte_len_or_field_read_pattern!(source, path, field, ..) => {
            let region = super::storage_regions::projected(
                function,
                structural_types,
                *source,
                path,
                Some(*field),
            );
            check_observation_access(
                function,
                structural_types,
                block,
                node,
                live,
                *source,
                region.direct_path(),
                |reference| {
                    shared_source_compatible(
                        function,
                        structural_types,
                        reference,
                        StructuralAccess::SharedBorrow,
                    )
                },
            )?;
        }
        O::EstablishByteSequenceConcatenation { .. }
        | O::ByteSequenceEqual { .. }
        | O::ByteSequencePredicate { .. } => {
            let operands = match operation {
                O::EstablishByteSequenceConcatenation { left, right, .. }
                | O::ByteSequenceEqual { left, right, .. } => [Some(left), Some(right)],
                O::ByteSequencePredicate { operand, .. } => [Some(operand), None],
                _ => unreachable!(),
            };
            for operand in operands.into_iter().flatten() {
                match operand {
                    terminal_psi::control_flow::ByteSequenceOperand::Whole { source, .. } => {
                        check_record_field_access(
                            function,
                            structural_types,
                            block,
                            node,
                            live,
                            source.place,
                            &source.path,
                            None,
                            StructuralAccess::SharedBorrow,
                        )?;
                    }
                    terminal_psi::control_flow::ByteSequenceOperand::Field {
                        source,
                        field,
                        ..
                    } => check_record_field_access(
                        function,
                        structural_types,
                        block,
                        node,
                        live,
                        source.place,
                        &source.path,
                        Some(*field),
                        StructuralAccess::MutableBorrow,
                    )?,
                }
            }
        }
        O::WriteOnlyPrimitiveStore {
            destination, path, ..
        } => {
            if path.contains(&terminal_psi::StructuralPathSegment::Referent) {
                access_reference_custody(
                    function,
                    structural_types,
                    block,
                    node,
                    live,
                    &StructuralArgument {
                        place: destination.place(),
                        path: path.clone(),
                        access: StructuralAccess::WriteOnlyBorrow,
                    },
                )?;
                return Ok(());
            }
            let region = super::storage_regions::projected(
                function,
                structural_types,
                destination.place(),
                path,
                None,
            );
            check_region_access(
                function,
                block,
                node,
                live,
                destination.place(),
                region.direct_path(),
            )?;
        }
        O::StructuralScalarFieldStore {
            destination,
            path,
            field,
            ..
        } => {
            check_record_field_access(
                function,
                structural_types,
                block,
                node,
                live,
                destination.place(),
                path,
                Some(*field),
                StructuralAccess::WriteOnlyBorrow,
            )?;
        }
        O::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            source,
            ..
        } => {
            check_record_field_access(
                function,
                structural_types,
                block,
                node,
                live,
                *destination,
                path,
                Some(*field),
                StructuralAccess::MutableBorrow,
            )?;
            check_root_access(function, block, node, live, *source)?;
        }
        O::StructuralByteSequenceFieldByteStore {
            destination,
            path,
            field,
            ..
        } => {
            check_record_field_access(
                function,
                structural_types,
                block,
                node,
                live,
                *destination,
                path,
                Some(*field),
                StructuralAccess::MutableBorrow,
            )?;
        }
        O::MoveStructuralField {
            source,
            path,
            field,
            result,
            ..
        } => {
            return windows::transfer(
                function,
                structural_types,
                block,
                node,
                live,
                source,
                path,
                *field,
                StructuralAccess::MutableBorrow,
                result.structural_type,
                result.place,
                true,
                false,
            );
        }
        O::StoreStructuralField {
            disposition,
            destination,
            path,
            field,
            value,
            ..
        } => {
            let value_type = structural_source_contract(function, value.place)
                .ok_or_else(|| {
                    invalid(
                        function,
                        block,
                        Some(node),
                        "window store value has no declaration",
                    )
                })?
                .structural_type;
            return windows::transfer(
                function,
                structural_types,
                block,
                node,
                live,
                destination,
                path,
                *field,
                disposition.required_access(),
                value_type,
                value.place,
                false,
                *disposition == terminal_psi::StructuralStoreDisposition::ReplaceDiscardable,
            );
        }
        O::StructuralLeafCopy {
            result,
            source,
            path,
            ..
        } if contains_reference(structural_types, result.structural_type) => {
            // Copy carrier storage, not its referent or a new reborrow. Each
            // copied permission joins the source's exact backing and immediate
            // parent; distinct identities keep both shared cohort members live
            // until their independent releases.
            let abstract_operations::StructuralCopyPath::Ordinary(path) = path else {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "reference copy needs an exact ordinary carrier path",
                ));
            };
            let shared_descriptor = terminal_semantics::is_shared_reference_carrier(
                structural_types.values().copied(),
                result.structural_type,
            );
            let source_type = structural_source_contract(function, *source)
                .and_then(|signature| leaf_type(structural_types, signature.structural_type, path));
            if source_type != Some(result.structural_type)
                || (result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                    && !(result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                        && shared_descriptor))
                || !result.claims.is_empty()
                || !result.qualification_establishments.is_empty()
                || live.iter().any(|reference| {
                    reference.carrier == result.place || reference.identity.place == result.place
                })
            {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "reference copy has no exact shared source or fresh destination",
                ));
            }
            let copied = copy_reference_carriers(
                function,
                structural_types,
                block,
                node,
                live,
                *source,
                path,
                result.structural_type,
                result.place,
            )?;
            live.extend(copied);
        }
        _ => {}
    }
    let mut normalized_arguments = Vec::new();
    let mut moved = BTreeSet::new();
    let arguments = operation_structural_arguments(operation);
    for (argument_position, argument) in arguments.iter().enumerate() {
        if argument
            .path
            .contains(&terminal_psi::StructuralPathSegment::Referent)
        {
            let custody =
                access_reference_custody(function, structural_types, block, node, live, argument)?;
            if arguments.iter().enumerate().any(|(position, other)| {
                position != argument_position
                    && call_dependency_conflict(&custody.dependencies, other)
            }) {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "call consumes or mutates an intermediate reference carrier",
                ));
            }
            normalized_arguments.push((
                custody.origins,
                argument.access,
                CallRegion::Referent,
                custody.dependencies,
                argument_position,
            ));
            continue;
        }
        if structural_source_contract(function, argument.place)
            .is_some_and(|source| contains_reference(structural_types, source.structural_type))
        {
            if matches!(operation, O::BoundaryCall { .. }) {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "reference-bearing owned calls require a whole internal argument",
                ));
            }
            let signature =
                structural_source_contract(function, argument.place).ok_or_else(|| {
                    invalid(
                        function,
                        block,
                        Some(node),
                        "owned reference argument has no source",
                    )
                })?;
            if !crate::unit_validation::references::access_covers(signature.access, argument.access)
            {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "carrier borrow exceeds source authority",
                ));
            }
            // Ownership moves the selected subtree's entire leaf roster, not
            // the containing root. Resolve the declared path, then rebase each
            // leaf onto its actual carrier; untouched siblings stay live.
            let projected_type = argument
                .path
                .iter()
                .all(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Field(_)))
                .then(|| {
                    terminal_semantics::runtime_structural_path_tip(
                        structural_types.values().copied(),
                        signature.structural_type,
                        &argument.path,
                    )
                })
                .flatten()
                .ok_or_else(|| {
                    invalid(
                        function,
                        block,
                        Some(node),
                        "projected reference argument path is not a declared record edge",
                    )
                })?;
            let paths =
                leaf_paths(structural_types, projected_type.id, live.len()).ok_or_else(|| {
                    invalid(
                        function,
                        block,
                        Some(node),
                        "owned argument reference roster exceeds live custody",
                    )
                })?;
            if paths.len()
                != live
                    .iter()
                    .filter(|reference| {
                        reference.carrier == argument.place
                            && owned_resident(&reference.carrier_path, &argument.path)
                    })
                    .count()
            {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "owned argument reference roster differs from its type",
                ));
            }
            let storage = ReferenceOrigin::IngressLeaf(
                ReferenceIdentity {
                    place: argument.place,
                    path: argument.path.clone(),
                }
                .into(),
            );
            if live.iter().any(|reference| {
                !(reference.carrier == argument.place
                    && owned_resident(&reference.carrier_path, &argument.path))
                    && origins_overlap(&storage, &reference.root)
            }) {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "carrier storage has an outstanding loan",
                ));
            }
            normalized_arguments.push((
                vec![storage],
                argument.access,
                CallRegion::CarrierStorage,
                Vec::new(),
                argument_position,
            ));
            // Write-only access lends these carrier bytes, without lending the
            // permissions stored inside them. Their children keep the backing
            // they captured even when the callee replaces the descriptors.
            if argument.access == StructuralAccess::WriteOnlyBorrow {
                continue;
            }
            for path in paths {
                let permission = leaf_type(structural_types, projected_type.id, &path)
                    .and_then(|leaf| reference_access(structural_types, leaf))
                    .ok_or_else(|| {
                        invalid(
                            function,
                            block,
                            Some(node),
                            "carrier leaf lacks declared permission",
                        )
                    })?;
                let mut carrier_path = argument.path.clone();
                carrier_path.extend(path);
                let reference = live
                    .iter()
                    .find(|reference| {
                        reference.carrier == argument.place
                            && reference.carrier_path == carrier_path
                    })
                    .ok_or_else(|| {
                        invalid(
                            function,
                            block,
                            Some(node),
                            "owned argument reference leaf is not live",
                        )
                    })?;
                // Abstract callee ingress promises independently available
                // leaves. A type cannot describe an unknown suspended parent
                // or a parent/child pair packed into the same incoming record.
                if live.iter().any(|child| matches!(&child.parent, ReferenceParent::Reference(identity) if identity == &reference.identity))
                    || (argument.access == StructuralAccess::Owned && !moved.insert(reference.identity.clone()))
                {
                    return Err(invalid(
                        function,
                        block,
                        Some(node),
                        "owned reference argument duplicates or suspends a leaf",
                    ));
                }
                normalized_arguments.push((
                    vec![reference.root.clone()],
                    if crate::unit_validation::references::access_covers(
                        argument.access,
                        permission,
                    ) {
                        permission
                    } else {
                        argument.access
                    },
                    CallRegion::Referent,
                    Vec::new(),
                    argument_position,
                ));
            }
            continue;
        }
        let carrier_or_suspended = structural_source_contract(function, argument.place)
            .is_some_and(|source| contains_reference(structural_types, source.structural_type))
            || live
                .iter()
                .any(|reference| origin_place(&reference.root) == argument.place);
        if carrier_or_suspended || borrowed_sources.contains_key(&argument.place) {
            if carrier_or_suspended
                && !is_reference_projection(function, structural_types, argument)
            {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "call cannot access a suspended root or move its referent",
                ));
            }
            let (root, _) = normalized_source(
                function,
                structural_types,
                borrowed_sources,
                block,
                node,
                live,
                argument,
            )?;
            normalized_arguments.push((
                vec![root],
                argument.access,
                CallRegion::Referent,
                Vec::new(),
                argument_position,
            ));
        } else {
            normalized_arguments.push((
                vec![ReferenceOrigin::Primitive(argument.place)],
                argument.access,
                CallRegion::OrdinaryStorage,
                Vec::new(),
                argument_position,
            ));
        }
    }
    for (position, (roots, access, reference_access, dependencies, argument_position)) in
        normalized_arguments.iter().enumerate()
    {
        if normalized_arguments[position + 1..].iter().any(
            |(others, other_access, other_reference_access, other_dependencies, other_position)| {
                ((*reference_access != CallRegion::OrdinaryStorage
                    || *other_reference_access != CallRegion::OrdinaryStorage)
                    && (argument_position != other_position
                        || (*reference_access == CallRegion::Referent
                            && *other_reference_access == CallRegion::Referent))
                    && (*access != StructuralAccess::SharedBorrow
                        || *other_access != StructuralAccess::SharedBorrow)
                    && roots
                        .iter()
                        .any(|root| others.iter().any(|other| origins_overlap(root, other))))
                    || (*other_access != StructuralAccess::SharedBorrow
                        && dependencies
                            .iter()
                            .any(|root| others.iter().any(|other| origins_overlap(root, other))))
                    || (*access != StructuralAccess::SharedBorrow
                        && roots.iter().any(|root| {
                            other_dependencies
                                .iter()
                                .any(|other| origins_overlap(root, other))
                        }))
            },
        ) {
            return Err(invalid(
                function,
                block,
                Some(node),
                "call arguments overlap through reference origins or descriptor dependencies",
            ));
        }
    }
    let stored = call_effects.stage(
        function,
        functions,
        structural_types,
        borrowed_sources,
        block,
        node,
        operation,
        live,
        &moved,
    )?;
    let sources = match operation {
        O::EstablishReference { source, .. } | O::EstablishPlacedReference { source, .. } => {
            let transferred = transferred_ingress_leaf(function, structural_types, live, source);
            if let Some(identity) = &transferred {
                moved.insert(identity.clone());
            }
            vec![terminal_semantics::ReferenceCallResult {
                path: Vec::new(),
                sources: vec![terminal_semantics::ReferenceCallSource {
                    origin: terminal_semantics::ReferenceCallOrigin::Parameter {
                        source: source.clone(),
                        transfers_existing: transferred.is_some(),
                    },
                    suffix: Vec::new(),
                    extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                    endpoint: reference_source_type(function, structural_types, source)
                        .ok_or_else(|| {
                            invalid(
                                function,
                                block,
                                Some(node),
                                "reference formation has no source type",
                            )
                        })?,
                }],
            }]
        }
        O::CallStructural {
            callee,
            structural_arguments,
            result: AbstractStructuralCallResult::Structural { result, .. },
            ..
        } if contains_reference(structural_types, result.structural_type) => {
            let fail = |reason| invalid(function, block, Some(node), reason);
            let contract = match callee {
                abstract_operations::AbstractCallTarget::Machine(machine) => {
                    let callee = functions
                        .get(machine)
                        .ok_or_else(|| fail("reference call target is absent"))?;
                    let declaration = callee
                        .result
                        .structural()
                        .ok_or_else(|| fail("reference call target has no structural result"))?;
                    terminal_semantics::ReferenceCallContract::Machine {
                        parameters: &callee.structural_parameters,
                        sources: &declaration.reference_sources,
                    }
                }
                abstract_operations::AbstractCallTarget::Dynamic {
                    interface, slot, ..
                } => {
                    let requirement = dynamic_dispatch
                        .interface(*interface)
                        .and_then(|interface| interface.slots.get(*slot as usize))
                        .ok_or_else(|| fail("reference call requirement is absent"))?;
                    terminal_semantics::ReferenceCallContract::Requirement(requirement)
                }
            };
            terminal_semantics::bind_reference_call_results(
                structural_types.values().copied(),
                contract,
                structural_arguments,
            )
            .map_err(fail)?
        }
        _ => Vec::new(),
    };
    if !sources.is_empty() {
        let result = match operation {
            O::EstablishReference { result, .. }
            | O::EstablishPlacedReference { result, .. }
            | O::CallStructural {
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            } => result,
            _ => {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "reference establishment result is absent",
                ));
            }
        };
        if live.iter().any(|reference| {
            reference.carrier == result.place || reference.identity.place == result.place
        }) {
            return Err(invalid(
                function,
                block,
                Some(node),
                "reference carrier is already live",
            ));
        }
        // Stage the complete result before publication. Independent leaves
        // cannot duplicate an exclusive origin, even through distinct actual
        // argument spellings. Borrowing a result does not move its parent loan.
        let mut established = Vec::with_capacity(sources.len());
        let mut roots = Vec::new();
        let mut identities = BTreeSet::new();
        for terminal_semantics::ReferenceCallResult {
            path,
            sources: candidates,
        } in sources
        {
            let expected = leaf_referent(structural_types, result.structural_type, &path)
                .ok_or_else(|| {
                    invalid(
                        function,
                        block,
                        Some(node),
                        "reference call result leaf is unsupported",
                    )
                })?;
            let access = leaf_type(structural_types, result.structural_type, &path)
                .and_then(|leaf| reference_access(structural_types, leaf))
                .ok_or_else(|| {
                    invalid(
                        function,
                        block,
                        Some(node),
                        "reference call result access is absent",
                    )
                })?;
            let mut alternatives = Vec::with_capacity(candidates.len());
            for terminal_semantics::ReferenceCallSource {
                origin,
                suffix,
                extent,
                endpoint,
            } in candidates
            {
                let (source, transfers_existing) = match origin {
                    terminal_semantics::ReferenceCallOrigin::Parameter {
                        source,
                        transfers_existing,
                    } => (source, transfers_existing),
                    terminal_semantics::ReferenceCallOrigin::Invocation {
                        carrier_type,
                        path: origin_path,
                        access: supplied,
                    } => {
                        if endpoint != expected
                            || extent != semantic_vocabulary::ReferenceOriginExtent::Projected
                            || !leaf_access_within(
                                structural_types,
                                result.structural_type,
                                &path,
                                supplied,
                            )
                        {
                            return Err(invalid(
                                function,
                                block,
                                Some(node),
                                "invocation result changes admitted reference geometry",
                            ));
                        }
                        let identity = ReferenceIdentity {
                            place: result.place,
                            path: path.clone(),
                        };
                        let root = ReferenceOrigin::Invocation {
                            result: identity.clone(),
                            carrier_type,
                            path: origin_path,
                            extent,
                        };
                        alternatives.push(LiveReference {
                            identity,
                            carrier: result.place,
                            carrier_path: path.clone(),
                            parent: ReferenceParent::Root(root.clone()),
                            root,
                        });
                        continue;
                    }
                };
                let compatible = reference_source_type(function, structural_types, &source)
                    .and_then(|root| {
                        if suffix.is_empty() {
                            Some(root)
                        } else {
                            terminal_semantics::reference_origin_type(
                                structural_types.values().copied(),
                                root,
                                &suffix,
                            )
                        }
                    })
                    .is_some_and(|actual| match operation {
                        O::EstablishPlacedReference { correspondence, .. } => {
                            placed_reference_source_compatible(
                                structural_types,
                                actual,
                                result.structural_type,
                                correspondence,
                            )
                        }
                        O::EstablishReference {
                            storage: Some(storage),
                            ..
                        } => terminal_semantics::reference_storage_source_compatible(
                            structural_types.values().copied(),
                            actual,
                            result.structural_type,
                            storage.stored_scalar,
                        ),
                        O::EstablishReference { storage: None, .. } => {
                            terminal_semantics::reference_source_compatible(
                                structural_types.values().copied(),
                                actual,
                                result.structural_type,
                            )
                        }
                        _ => {
                            extent == semantic_vocabulary::ReferenceOriginExtent::WithinRegion
                                || actual == expected
                        }
                    });
                if !compatible
                    || !leaf_access_within(
                        structural_types,
                        result.structural_type,
                        &path,
                        source.access,
                    )
                {
                    return Err(invalid(
                        function,
                        block,
                        Some(node),
                        "reference result changes its mapped source type",
                    ));
                }
                let reference = if transfers_existing {
                    let Some((terminal_psi::StructuralPathSegment::Referent, carrier_path)) =
                        source.path.split_last()
                    else {
                        return Err(invalid(
                            function,
                            block,
                            Some(node),
                            "owned result mapping has no incoming leaf",
                        ));
                    };
                    let mut reference = live
                        .iter()
                        .find(|reference| {
                            reference.carrier == source.place
                                && reference.carrier_path == carrier_path
                        })
                        .filter(|reference| moved.contains(&reference.identity))
                        .cloned()
                        .ok_or_else(|| {
                            invalid(
                                function,
                                block,
                                Some(node),
                                "returned reference is not among the transferred leaves",
                            )
                        })?;
                    reference.carrier = result.place;
                    reference.carrier_path = path.clone();
                    reference
                } else {
                    let (mut root, mut parent) = normalized_source(
                        function,
                        structural_types,
                        borrowed_sources,
                        block,
                        node,
                        live,
                        &source,
                    )?;
                    if !suffix.is_empty()
                        || extent == semantic_vocabulary::ReferenceOriginExtent::WithinRegion
                    {
                        let projected = projected_origin_family(&root, &suffix, extent);
                        if parent == ReferenceParent::Root(root.clone()) {
                            parent = ReferenceParent::Root(projected.clone());
                        }
                        root = projected;
                    }
                    if let O::EstablishReference {
                        storage: Some(storage),
                        ..
                    } = operation
                    {
                        let mut offsets =
                            function
                                .block_nodes()
                                .filter_map(|node| match node.operation {
                                    O::IntegerConstant { result, value, .. }
                                        if result == storage.byte_offset =>
                                    {
                                        Some(value)
                                    }
                                    _ => None,
                                });
                        if offsets.next() != Some(semantic_vocabulary::IntegerValue::Unsigned(0))
                            || offsets.next().is_some()
                            || terminal_semantics::representation_view_declaration(
                                structural_types.values().copied(),
                                expected,
                                storage.stored_scalar,
                            )
                            .is_none()
                        {
                            return Err(invalid(
                                function,
                                block,
                                Some(node),
                                "returned interpretation differs from actual view formation",
                            ));
                        }
                        let interpreted = projected_origin(
                            &root,
                            &[terminal_psi::StructuralPathSegment::RepresentationView {
                                structural_type: expected,
                                stored_scalar: storage.stored_scalar,
                            }],
                        );
                        if parent == ReferenceParent::Root(root.clone()) {
                            parent = ReferenceParent::Root(interpreted.clone());
                        }
                        root = interpreted;
                    }
                    LiveReference {
                        identity: ReferenceIdentity {
                            place: result.place,
                            path: path.clone(),
                        },
                        carrier: result.place,
                        carrier_path: path.clone(),
                        root,
                        parent,
                    }
                };
                alternatives.push(reference);
            }
            let reference = combine_alternative_references(function, block, node, alternatives)?;
            // Distinct shared loans may observe the same backing. A moved
            // carrier identity still cannot be returned into two result slots.
            if !identities.insert(reference.identity.clone())
                || roots.iter().any(|(root, prior_access)| {
                    origins_overlap(root, &reference.root)
                        && (access != StructuralAccess::SharedBorrow
                            || *prior_access != StructuralAccess::SharedBorrow)
                })
            {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "reference result aliases its mapped sources",
                ));
            }
            roots.push((reference.root.clone(), access));
            established.push(reference);
        }
        // Normal completion consumes the incoming owners. Returned carriers
        // preserve their identities and external parents; all other incoming
        // leaves were independently disposed by the callee's checked exits.
        live.retain(|reference| !moved.contains(&reference.identity));
        live.extend(established);
        live.sort_by(|left, right| left.identity.cmp(&right.identity));
    } else {
        live.retain(|reference| !moved.contains(&reference.identity));
    }
    stored.commit(function, structural_types, block, node, live)?;
    Ok(())
}

/// A sole leaf in an owned ingress record can move into a returned carrier.
/// This is the same permission, not a child loan that would escape its parent.
/// Local carriers and reference-typed roots still reborrow; additional leaves
/// would require independent residual custody and cannot be silently retired.
pub(super) fn transferred_ingress_leaf(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    live: &[LiveReference],
    source: &StructuralArgument,
) -> Option<ReferenceIdentity> {
    let parameter = function
        .structural_parameter(source.place)
        .filter(|parameter| parameter.access == StructuralAccess::Owned)?;
    if !matches!(
        formal_origin(types, function, source),
        Some(ReferenceOrigin::IngressLeaf(_))
    ) || matches!(
        types.get(&parameter.structural_type)?.shape,
        terminal_psi::StructuralTypeShape::Reference { .. }
    ) {
        return None;
    }
    let (terminal_psi::StructuralPathSegment::Referent, path) = source.path.split_last()? else {
        return None;
    };
    let paths = leaf_paths(types, parameter.structural_type, live.len())?;
    if paths.as_slice() != [path.to_vec()] {
        return None;
    }
    let mut leaves = live.iter().filter(|reference| {
        reference.carrier == source.place && owned_resident(&reference.carrier_path, &[])
    });
    let leaf = leaves.next()?;
    (leaves.next().is_none()
        && leaf.carrier_path == path
        && leaf.parent == ReferenceParent::Root(leaf.root.clone())
        && !live.iter().any(|child| {
            matches!(&child.parent,
            ReferenceParent::Reference(identity) if identity == &leaf.identity)
        }))
    .then(|| leaf.identity.clone())
}

/// Publish one carrier only after every possible origin has independently
/// rejoined the same leaf identity and direct ingress custody.
fn combine_alternative_references(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: u32,
    alternatives: Vec<LiveReference>,
) -> Result<LiveReference, OptimizationUnitValidationError> {
    let invalid = || {
        invalid(
            function,
            block,
            Some(node),
            "reference result alternatives change carrier or ingress custody",
        )
    };
    let mut alternatives = alternatives.into_iter();
    let mut reference = alternatives.next().ok_or_else(invalid)?;
    let mut origins = vec![(reference.root.clone(), reference.parent.clone())];
    for alternative in alternatives {
        if reference.identity != alternative.identity
            || reference.carrier != alternative.carrier
            || reference.carrier_path != alternative.carrier_path
        {
            return Err(invalid());
        }
        origins.push((alternative.root, alternative.parent));
    }
    (reference.root, reference.parent) =
        combine_alternative_origins(function, block, node, origins)?;
    Ok(reference)
}

/// Multiple storage alternatives preserve direct custody under one ingress.
/// This joins origins without manufacturing a carrier or a parent resource.
fn combine_alternative_origins(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: u32,
    alternatives: Vec<(ReferenceOrigin, ReferenceParent)>,
) -> Result<(ReferenceOrigin, ReferenceParent), OptimizationUnitValidationError> {
    let invalid = || {
        invalid(
            function,
            block,
            Some(node),
            "reference result alternatives change ingress custody",
        )
    };
    let mut alternatives = alternatives.into_iter();
    let (root, parent) = alternatives.next().ok_or_else(invalid)?;
    if alternatives.len() == 0 {
        return Ok((root, parent));
    }
    let place = origin_place(&root);
    let mut origins = Vec::new();
    for (origin, origin_parent) in std::iter::once((root, parent)).chain(alternatives) {
        if origin_parent != ReferenceParent::Root(origin.clone()) || origin_place(&origin) != place
        {
            return Err(invalid());
        }
        match origin {
            ReferenceOrigin::IngressLeaf(identity) => origins.push(identity),
            ReferenceOrigin::Alternatives {
                origins: alternatives,
                ..
            } => origins.extend(alternatives),
            ReferenceOrigin::Invocation { .. } | ReferenceOrigin::Primitive(_) => {
                return Err(invalid());
            }
        }
    }
    origins.sort();
    if origins.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid());
    }
    let root = ReferenceOrigin::Alternatives { place, origins };
    Ok((root.clone(), ReferenceParent::Root(root)))
}

/// A structural return whose source carries reference leaves must match the
/// declared result roster exactly: every returned leaf still sits at its
/// declared carrier path and descends directly from a checked formal origin.
pub(super) fn transfer_return(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    source: PlaceId,
    live: &mut Vec<LiveReference>,
    observed: &mut Vec<terminal_psi::StructuralReferenceResultSource>,
    cleanup_actions: &[terminal_psi::TerminalAffineCleanupAction],
) -> Result<bool, OptimizationUnitValidationError> {
    let Some(signature) = structural_source_contract(function, source) else {
        return Ok(false);
    };
    if !contains_reference(structural_types, signature.structural_type) {
        return Ok(false);
    }
    let result = function
        .result
        .structural()
        .ok_or_else(|| invalid(function, block, None, "reference return has no declaration"))?;
    if signature.structural_type != result.structural_type
        || live
            .iter()
            .filter(|reference| {
                reference.carrier == source && owned_resident(&reference.carrier_path, &[])
            })
            .count()
            != result.reference_sources.len()
    {
        return Err(invalid(
            function,
            block,
            None,
            "returned reference leaf roster differs from its declaration",
        ));
    }
    // Interfaces cannot supply origins: compare every actual returned leaf.
    // Only direct ingress custody may escape; moving the carrier into a record
    // must not hide a local root or skip an intermediate live parent.
    for mapping in &result.reference_sources {
        let reference = live
            .iter()
            .find(|reference| reference.carrier == source && reference.carrier_path == mapping.path)
            .ok_or_else(|| {
                invalid(
                    function,
                    block,
                    None,
                    "returned reference carrier is not live",
                )
            })?;
        // Reconstruct the summary from the actual live loan. The declaration
        // is compared with that observation, never copied into the evidence.
        // Runtime selectors remain exact until this interface boundary.
        let access = leaf_type(
            structural_types,
            signature.structural_type,
            &reference.carrier_path,
        )
        .and_then(|leaf| reference_access(structural_types, leaf))
        .ok_or_else(|| {
            invalid(
                function,
                block,
                None,
                "returned carrier has no reference access",
            )
        })?;
        let referent = leaf_referent(
            structural_types,
            signature.structural_type,
            &reference.carrier_path,
        )
        .ok_or_else(|| {
            invalid(
                function,
                block,
                None,
                "returned carrier has no referent type",
            )
        })?;
        if let ReferenceOrigin::Invocation {
            carrier_type,
            path: origin_path,
            extent,
            ..
        } = &reference.root
        {
            let source = terminal_psi::StructuralReferenceOrigin::Invocation {
                carrier_type: *carrier_type,
                path: origin_path.clone(),
                extent: *extent,
                access,
            };
            if !mapping.sources.contains(&source)
                || !matches!(&reference.parent, ReferenceParent::Root(root) if root == &reference.root)
                || live.iter().any(|child| matches!(&child.parent, ReferenceParent::Reference(identity) if identity == &reference.identity)) {
                return Err(invalid(function, block, None, "invocation reference return differs from completed call custody"));
            }
            observed.push(terminal_psi::StructuralReferenceResultSource {
                path: mapping.path.clone(),
                sources: vec![source],
            });
            continue;
        }
        let sources = return_origin_family(
            function,
            structural_types,
            &reference.root,
            referent,
            access,
        )
        .ok_or_else(|| {
            invalid(
                function,
                block,
                None,
                "reference return origin family is unavailable",
            )
        })?;
        if sources.is_empty() || sources.iter().any(|source| {
            let Some(parameter) = function.structural_parameter(source.place) else {
                return true;
            };
            // Compare admitted supplying permissions at the returned view's
            // access while preserving the whole declared origin geometry.
            let supplying = mapping.sources.iter()
                .filter_map(terminal_psi::StructuralReferenceOrigin::parameter)
                .filter(|allowed| crate::unit_validation::references::access_covers(
                    allowed.access, source.access,
                ))
                .map(|allowed| terminal_psi::ReferenceOriginSource {
                    access: source.access,
                    ..allowed.clone()
                })
                .collect::<Vec<_>>();
            !terminal_semantics::reference_origin_is_covered(
                structural_types.values().copied(), parameter.structural_type, source, &supplying,
            )
        })
            || matches!(&reference.parent, ReferenceParent::Root(root) if *root != reference.root)
            || live.iter().any(|child| matches!(&child.parent, ReferenceParent::Reference(identity) if identity == &reference.identity))
        {
            return Err(invalid(
                function,
                block,
                None,
                "reference return escapes local custody or differs from its formal source",
            ));
        }
        observed.push(terminal_psi::StructuralReferenceResultSource {
            path: mapping.path.clone(),
            sources: sources
                .into_iter()
                .map(terminal_psi::StructuralReferenceOrigin::Parameter)
                .collect(),
        });
    }
    // A result copy keeps its immediate parents until this boundary. Verify
    // their complete selected disposal cohort before transferring the result;
    // this is not permission to retire a parent at an ordinary statement.
    let occurrences = live
        .iter()
        .map(
            |reference| terminal_semantics::ReferenceCompletionOccurrence {
                identity: &reference.identity,
                parents: match &reference.parent {
                    ReferenceParent::Root(_) => Vec::new(),
                    ReferenceParent::Reference(parent) => vec![parent],
                },
                returned: reference.carrier == source
                    && owned_resident(&reference.carrier_path, &[]),
                disposed: function
                    .structural_parameter(reference.carrier)
                    .is_none_or(|parameter| parameter.access == StructuralAccess::Owned)
                    && terminal_semantics::cleanup_disposes_reference(
                        cleanup_actions,
                        reference.carrier,
                        &reference.carrier_path,
                    ),
            },
        )
        .collect::<Vec<_>>();
    terminal_semantics::validate_reference_result_completion(&occurrences)
        .map_err(|reason| invalid(function, block, None, reason.description()))?;
    // This removes frame-local ownership rows, not the referents or the loans.
    // The caller independently instantiates the checked result source roster.
    live.retain(|reference| {
        reference.carrier != source || !owned_resident(&reference.carrier_path, &[])
    });
    Ok(true)
}

/// Summarize only independently reconstructed live origins. There is no array
/// expansion: a runtime selection contributes one typed element relation.
/// Stored ingress descriptors recover their final crossing by referent type,
/// while the loan's formation identity and parent lineage stay unchanged.
fn return_origin_family(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    origin: &ReferenceOrigin,
    expected: StructuralTypeId,
    access: StructuralAccess,
) -> Option<Vec<terminal_psi::ReferenceOriginSource>> {
    use terminal_psi::ReferenceOriginProjection::{Element, Exact};
    use terminal_psi::StructuralPathSegment::{Referent, RuntimeIndex};
    let primitive = ReferenceOriginIdentity {
        place: origin_place(origin),
        path: Vec::new(),
        extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
    };
    let origins = match origin {
        ReferenceOrigin::Invocation { .. } => return None,
        ReferenceOrigin::IngressLeaf(identity) => std::slice::from_ref(identity),
        ReferenceOrigin::Alternatives { origins, .. } => origins.as_slice(),
        ReferenceOrigin::Primitive(_) => std::slice::from_ref(&primitive),
    };
    let mut sources = Vec::with_capacity(origins.len());
    for origin in origins {
        let parameter = function.structural_parameter(origin.place)?;
        let mut path = origin
            .path
            .iter()
            .map(|segment| match segment {
                Exact(RuntimeIndex { .. }) => Element,
                _ => segment.clone(),
            })
            .collect::<Vec<_>>();
        let endpoint = terminal_semantics::reference_origin_type(
            types.values().copied(),
            parameter.structural_type,
            &path,
        )?;
        if origin.extent == semantic_vocabulary::ReferenceOriginExtent::Projected
            && endpoint != expected
        {
            if !matches!(types.get(&endpoint).map(|declaration| &declaration.shape),
                Some(terminal_psi::StructuralTypeShape::Reference { referent, access: supplied })
                    if *referent == expected && crate::unit_validation::references::access_covers(*supplied, access))
            {
                return None;
            }
            path.push(Exact(Referent));
        }
        sources.push(terminal_psi::ReferenceOriginSource {
            place: origin.place,
            path,
            extent: origin.extent,
            access,
        });
    }
    sources.sort();
    sources.dedup();
    Some(sources)
}

/// Individual exits prove membership. Their union accounts for precise private
/// alternatives; public region allowances need not all execute. The CFG owner
/// supplies only reachable returns.
pub(super) fn validate_return_coverage(
    function: &PsiOptimizationFunction,
    observed: &[terminal_psi::StructuralReferenceResultSource],
) -> Result<(), OptimizationUnitValidationError> {
    let Some(result) = function.result.structural() else {
        return Ok(());
    };
    for mapping in &result.reference_sources {
        let sources = observed
            .iter()
            .filter(|returned| returned.path == mapping.path)
            .flat_map(|returned| returned.sources.iter().cloned())
            .collect::<BTreeSet<_>>();
        // Precise private summaries still account for every reachable origin.
        // A region is a public allowance, not a claim that every possibility
        // executes (an implementation may return only the empty case).
        if mapping.sources.iter().any(|source| {
            source.extent() == semantic_vocabulary::ReferenceOriginExtent::Projected
                && !sources.contains(source)
        }) {
            return Err(invalid(
                function,
                function.entry,
                None,
                "reference result alternatives differ from reachable return origins",
            ));
        }
    }
    Ok(())
}

/// Normal completion accounts for independently reconstructed stored effects,
/// restores untouched borrowed ingress, and closes every local permission.
pub(super) fn validate_normal_return(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    effects: &call_effects::ReferenceCallEffects,
    block: BlockId,
    live: &[LiveReference],
) -> Result<(), OptimizationUnitValidationError> {
    let mut expected = entry_references(function, types)?;
    expected.retain(|reference| {
        function.structural_parameters.iter().any(|parameter| {
            parameter.place == reference.carrier && parameter.access != StructuralAccess::Owned
        })
    });
    let mut ordered = effects.remaining_at_return(function, types, block, &mut expected, live)?;
    expected.sort_by(|left, right| left.identity.cmp(&right.identity));
    ordered.sort_by(|left, right| left.identity.cmp(&right.identity));
    if expected.len() == ordered.len()
        && expected
            .iter()
            .zip(ordered)
            .all(|(expected, actual)| expected == actual)
    {
        Ok(())
    } else {
        Err(invalid(
            function,
            block,
            None,
            "normal completion does not restore borrowed reference custody",
        ))
    }
}
