//! Live loan replay over the current ownership frontier.
//!
//! This mirrors the verified Terminal-Psi reference walk: carriers own loan
//! permission, never their referent storage. Origins and immediate parents
//! are reconstructed from operations and checked call interfaces; no
//! producer-supplied lifetime or origin table is authority.

use crate::OptimizationUnitValidationError;
use crate::unit_validation::operation_contracts::structural_source_contract;
use crate::unit_validation::references::{
    LiveReference, ReferenceIdentity, ReferenceOrigin, ReferenceParent, argument_owns_references,
    contains_reference, formal_origin, is_reference_projection, leaf_access_within, leaf_paths,
    leaf_referent, leaf_type, mutable_record_call_argument_type, reference_access,
    reference_source_type,
};
use abstract_operations::AbstractOperation as O;
use abstract_operations::AbstractStructuralCallResult;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, MachineId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralAccess, StructuralArgument, StructuralTypeDeclaration};

// Incoming types have no explicit leaf roster to bound reconstruction. Keep
// this private verification-work capacity separate from resource authority;
// local/result rosters remain bounded by their existing captured rows.
const MAX_INCOMING_REFERENCE_LEAVES: usize = 4096;

#[cfg(test)]
mod alternative_result_tests;
#[cfg(test)]
mod record_store_tests;
#[cfg(test)]
mod region_access_tests;

fn origin_place(origin: &ReferenceOrigin) -> PlaceId {
    match origin {
        ReferenceOrigin::Primitive(place) => *place,
        ReferenceOrigin::IngressLeaf(identity) => identity.place,
        ReferenceOrigin::Alternatives { place, .. } => *place,
    }
}

fn origins_overlap(left: &ReferenceOrigin, right: &ReferenceOrigin) -> bool {
    if origin_place(left) != origin_place(right) {
        return false;
    }
    let overlaps_path = |path: &[terminal_psi::StructuralPathSegment]| match right {
        ReferenceOrigin::Primitive(_) => true,
        ReferenceOrigin::IngressLeaf(identity) => {
            path.starts_with(&identity.path) || identity.path.starts_with(path)
        }
        ReferenceOrigin::Alternatives { paths, .. } => paths
            .iter()
            .any(|other| path.starts_with(other) || other.starts_with(path)),
    };
    match left {
        ReferenceOrigin::Primitive(_) => true,
        ReferenceOrigin::IngressLeaf(identity) => overlaps_path(&identity.path),
        ReferenceOrigin::Alternatives { paths, .. } => paths.iter().any(|path| overlaps_path(path)),
    }
}

fn contains_origin(origins: &ReferenceOrigin, candidate: &ReferenceOrigin) -> bool {
    match (origins, candidate) {
        (
            ReferenceOrigin::Alternatives { place, paths },
            ReferenceOrigin::IngressLeaf(identity),
        ) => *place == identity.place && paths.binary_search(&identity.path).is_ok(),
        _ => origins == candidate,
    }
}

/// Resolve a call-only borrow through a returned mutable record carrier. The
/// carrier remains the permission identity; the normalized origin appends the
/// selected child to its one exact static ingress path for alias checks.
fn mutable_record_call_origin(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    node: u32,
    live: &[LiveReference],
    argument: &StructuralArgument,
) -> Result<Option<ReferenceOrigin>, OptimizationUnitValidationError> {
    if mutable_record_call_argument_type(function, structural_types, argument).is_none() {
        return Ok(None);
    }
    let Some((terminal_psi::StructuralPathSegment::Referent, suffix)) = argument.path.split_first()
    else {
        return Ok(None);
    };
    let mut carriers = live.iter().filter(|reference| {
        reference.carrier == argument.place && reference.carrier_path.is_empty()
    });
    let carrier = carriers.next().ok_or_else(|| {
        invalid(
            function,
            block,
            Some(node),
            "returned record call argument has no live reference carrier",
        )
    })?;
    if carriers.next().is_some()
        || live.iter().any(|child| {
            matches!(&child.parent, ReferenceParent::Reference(parent) if parent == &carrier.identity)
        })
    {
        return Err(invalid(
            function,
            block,
            Some(node),
            "returned record call argument has ambiguous or suspended custody",
        ));
    }
    let root_place = match &carrier.root {
        ReferenceOrigin::IngressLeaf(root) => root.place,
        ReferenceOrigin::Alternatives { place, .. } => *place,
        ReferenceOrigin::Primitive(_) => {
            return Err(invalid(
                function,
                block,
                Some(node),
                "returned record call argument has no exact ingress",
            ));
        }
    };
    let Some(parameter) = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == root_place)
    else {
        return Err(invalid(
            function,
            block,
            Some(node),
            "returned record call argument root is not an ingress parameter",
        ));
    };
    let access_is_compatible = parameter.access == StructuralAccess::MutableBorrow
        && matches!(
            argument.access,
            StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        );
    if !access_is_compatible {
        return Err(invalid(
            function,
            block,
            Some(node),
            "returned record call argument widens ingress access",
        ));
    }
    // The selected pointer is exact at runtime, but alias checks retain every
    // complete parent candidate. A synchronous borrow creates no child loan.
    let ReferenceOrigin::IngressLeaf(root) = &carrier.root else {
        return Ok(Some(carrier.root.clone()));
    };
    let mut path = root.path.clone();
    path.extend_from_slice(suffix);
    Ok(Some(ReferenceOrigin::IngressLeaf(ReferenceIdentity {
        place: root.place,
        path,
    })))
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
            let root = ReferenceOrigin::IngressLeaf(identity.clone());
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

/// One operation's structural-argument slice, including the dynamic-dispatch
/// rebound receiver which carries the same loan/custody rules.
fn operation_structural_arguments(operation: &O) -> &[StructuralArgument] {
    match operation {
        // A Unit call that forwards descriptors contributes no arguments
        // here; the scalar-result call contributes them in both forms.
        O::CallStructural {
            dynamic_arguments,
            result: AbstractStructuralCallResult::Unit,
            ..
        } if !dynamic_arguments.is_empty() => &[],
        O::CallStructural {
            structural_arguments,
            ..
        }
        | O::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments.as_slice(),
        // A window repair consumes its value like one owned whole call
        // argument; any reference leaves it carries settle with the store.
        O::StoreStructuralField { value, .. } => std::slice::from_ref(value),
        O::CallDynamic {
            dispatch: abstract_operations::AbstractDynamicDispatch::Rebound(dynamic_dispatch),
            result: abstract_operations::AbstractDynamicCallResult::Scalar(_),
            ..
        } => std::slice::from_ref(&dynamic_dispatch.rebound.source),
        _ => &[],
    }
}

/// The spelled hole one `MoveStructuralField` opens beneath `root`: the
/// operation's path plus the declared identity of the field it vacates, or
/// the empty path of a root window's whole referent. `None` means the
/// destination does not host a structural field there — the unit's catalog
/// contract rejects that shape separately.
fn window_hole(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    place: PlaceId,
    root: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
) -> Option<Vec<terminal_psi::StructuralPathSegment>> {
    let (geometry_root, suffix) = if matches!(
        path.first(),
        Some(terminal_psi::StructuralPathSegment::Referent)
    ) {
        crate::unit_validation::references::mutable_record_field_destination(
            function, types, place, path,
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

/// A restoration pair does not move its parent permission carrier. It borrows
/// the referent through the same active carrier as a field store; the exact
/// carrier-relative hole remains on the operation. Terminal verification owns
/// restoration-debt replay; this ledger independently preserves loan authority,
/// parent suspension and the prohibition on displacing contained permissions.
#[allow(clippy::too_many_arguments)]
fn check_window_access(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    node: u32,
    live: &[LiveReference],
    root: &abstract_operations::StructuralWindowRoot,
    path: &[terminal_psi::StructuralPathSegment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
) -> Result<(), OptimizationUnitValidationError> {
    let crosses_reference = matches!(
        path.first(),
        Some(terminal_psi::StructuralPathSegment::Referent)
    );
    if crosses_reference {
        check_record_field_access(
            function,
            types,
            block,
            node,
            live,
            root.place(),
            path,
            field,
        )?;
    }
    let hole = window_hole(
        function,
        types,
        root.place(),
        root.structural_type(),
        path,
        field,
    )
    .ok_or_else(|| {
        invalid(
            function,
            block,
            Some(node),
            "restoration window does not resolve to a declared field",
        )
    })?;
    let accessed = ReferenceOrigin::IngressLeaf(ReferenceIdentity {
        place: root.place(),
        path: hole.clone(),
    });
    if live.iter().any(|reference| {
        // Exclude exactly the already-checked parent carrier, not every
        // permission beneath this root or any original ingress candidate.
        let parent_carrier = crosses_reference
            && reference.carrier == root.place()
            && reference.carrier_path.is_empty();
        (!parent_carrier
            && reference.carrier == root.place()
            && (reference.carrier_path.starts_with(&hole)
                || hole.starts_with(&reference.carrier_path)))
            || (!crosses_reference && origins_overlap(&reference.root, &accessed))
    }) {
        return Err(invalid(
            function,
            block,
            Some(node),
            "restoration window accesses suspended or reference-bearing storage",
        ));
    }
    Ok(())
}

fn check_root_access(
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
fn check_region_access(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: u32,
    live: &[LiveReference],
    place: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
) -> Result<(), OptimizationUnitValidationError> {
    let overlaps = |other: &[terminal_psi::StructuralPathSegment]| {
        terminal_semantics::structural_paths_may_overlap(path, other)
    };
    if live.iter().any(|reference| {
        (reference.carrier == place && overlaps(&reference.carrier_path))
            || (origin_place(&reference.root) == place
                && match &reference.root {
                    ReferenceOrigin::Primitive(_) => true,
                    ReferenceOrigin::IngressLeaf(identity) => overlaps(&identity.path),
                    ReferenceOrigin::Alternatives { paths, .. } => {
                        paths.iter().any(|path| overlaps(path))
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
) -> Result<(), OptimizationUnitValidationError> {
    if !matches!(
        path.first(),
        Some(terminal_psi::StructuralPathSegment::Referent)
    ) {
        let region = super::storage_regions::projected(function, types, place, path, field);
        return check_region_access(function, block, node, live, place, region.direct_path());
    }
    let valid_shape = crate::unit_validation::references::mutable_record_field_destination(
        function, types, place, path,
    )
    .is_some();
    let mut carriers = live
        .iter()
        .filter(|reference| reference.carrier == place && reference.carrier_path.is_empty());
    let available = carriers.next().is_some_and(|carrier| {
        !live.iter().any(|child| matches!(&child.parent, ReferenceParent::Reference(parent) if parent == &carrier.identity))
    }) && carriers.next().is_none();
    if !valid_shape || !available {
        return Err(invalid(
            function,
            block,
            Some(node),
            "record field access has no active mutable reference carrier",
        ));
    }
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
) -> Result<(), OptimizationUnitValidationError> {
    let access = structural_source_contract(function, place)
        .and_then(|contract| types.get(&contract.structural_type))
        .and_then(|declaration| match declaration.shape {
            terminal_psi::StructuralTypeShape::Reference { access, .. } => Some(access),
            _ => None,
        });
    let Some(access) = access else {
        return check_region_access(function, block, node, live, place, region);
    };
    let mut carriers = live
        .iter()
        .filter(|reference| reference.carrier == place && reference.carrier_path.is_empty());
    let available = carriers.next().is_some_and(|carrier| {
        !live.iter().any(|child| origin_place(&child.root) == place || matches!(&child.parent, ReferenceParent::Reference(parent) if parent == &carrier.identity))
    }) && carriers.next().is_none();
    if !available
        || !matches!(
            access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        )
    {
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

fn normalized_source(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    node: u32,
    live: &[LiveReference],
    source: &StructuralArgument,
) -> Result<(ReferenceOrigin, ReferenceParent), OptimizationUnitValidationError> {
    if let Some((terminal_psi::StructuralPathSegment::Referent, carrier_path)) =
        source.path.split_last()
    {
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
                parent.root.clone(),
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
        let root = ReferenceOrigin::IngressLeaf(ReferenceIdentity {
            place: source.place,
            path: carrier_path.to_vec(),
        });
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
    } else if reference_source_type(function, structural_types, source).is_some()
        && source.path.iter().all(|segment| {
            matches!(
                segment,
                terminal_psi::StructuralPathSegment::Field(_)
                    | terminal_psi::StructuralPathSegment::FixedIndex(_)
            )
        })
    {
        let root = ReferenceOrigin::IngressLeaf(ReferenceIdentity {
            place: source.place,
            path: source.path.clone(),
        });
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

/// End the whole carrier at `source` after every child loan has ended. This
/// removes the carrier row; referent storage is unaffected.
pub(super) fn release(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: Option<u32>,
    live: &mut Vec<LiveReference>,
    source: PlaceId,
) -> Result<(), OptimizationUnitValidationError> {
    let position = live
        .iter()
        .position(|reference| reference.carrier == source && reference.carrier_path.is_empty())
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

/// Whole-owner disposal visits reference leaves in reverse declaration order.
/// Validate the entire schedule before ending any loan; a live external child
/// (or an incorrectly ordered contained child) keeps its parent suspended.
pub(super) fn discard_owned(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    live: &mut Vec<LiveReference>,
    source: PlaceId,
) -> Result<(), OptimizationUnitValidationError> {
    let Some(signature) = structural_source_contract(function, source) else {
        return Ok(());
    };
    let paths =
        leaf_paths(structural_types, signature.structural_type, live.len()).ok_or_else(|| {
            invalid(
                function,
                block,
                None,
                "discard type requires more reference leaves than are live",
            )
        })?;
    let mut released = BTreeSet::new();
    for path in paths.iter().rev() {
        let reference = live
            .iter()
            .find(|reference| reference.carrier == source && reference.carrier_path == *path)
            .ok_or_else(|| {
                invalid(
                    function,
                    block,
                    None,
                    "discard requires every owned reference leaf to be live",
                )
            })?;
        if live
            .iter()
            .any(|child| matches!(&child.parent, ReferenceParent::Reference(identity) if identity == &reference.identity) && !released.contains(&child.identity))
        {
            return Err(invalid(
                function,
                block,
                None,
                "a reference cannot end before its child",
            ));
        }
        released.insert(reference.identity.clone());
    }
    if live
        .iter()
        .any(|reference| reference.carrier == source && !released.contains(&reference.identity))
    {
        return Err(invalid(
            function,
            block,
            None,
            "discard reference roster differs from its owned type",
        ));
    }
    live.retain(|reference| !released.contains(&reference.identity));
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
        Some(terminal_psi::StructuralTypeShape::Record { fields }) => fields,
        _ => {
            return Err(invalid(
                function,
                block,
                Some(node),
                "record transfer requires a record establishment",
            ));
        }
    };
    if declarations.len() != fields.len() {
        return Err(invalid(
            function,
            block,
            Some(node),
            "record transfer field roster differs from its type",
        ));
    }
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
    for (declaration, field) in declarations.iter().zip(fields) {
        let terminal_psi::RecordFieldValue::Structural(argument) = &field.value else {
            continue;
        };
        let signature = structural_source_contract(function, argument.place).ok_or_else(|| {
            invalid(
                function,
                block,
                Some(node),
                "record operand has no structural source",
            )
        })?;
        let paths = leaf_paths(structural_types, signature.structural_type, live.len())
            .ok_or_else(|| {
                invalid(
                    function,
                    block,
                    Some(node),
                    "record operand requires more reference leaves than are live",
                )
            })?;
        for path in paths {
            let position = live
                .iter()
                .position(|reference| {
                    reference.carrier == argument.place && reference.carrier_path == path
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
            let mut destination = vec![terminal_psi::StructuralPathSegment::Field(
                declaration.identity.clone(),
            )];
            destination.extend(path);
            relocations.push((position, destination));
        }
        if live.iter().enumerate().any(|(position, reference)| {
            reference.carrier == argument.place && !moved.contains(&position)
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
fn call_source(
    callee: &PsiOptimizationFunction,
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

/// Replay one operation against the live loan set. This runs inside the same
/// operation transaction as the owning frontier: reads and writes cannot touch
/// a suspended referent, owned record construction relocates leaves, calls
/// transfer or borrow exact leaves, and establishment/release mutate custody.
#[allow(clippy::too_many_lines)]
pub(super) fn apply_operation(
    function: &PsiOptimizationFunction,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
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
                check_record_field_access(
                    function,
                    structural_types,
                    block,
                    node,
                    live,
                    location.root,
                    &location.path,
                    Some(location.field),
                )?;
            }
        }
        O::ReleaseReference { source, .. } => {
            return release(function, block, Some(node), live, *source);
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
        | O::StructuralCaseMembership { source, path, .. } => {
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
            )?;
        }
        O::StructuralCase { source, .. } => {
            check_observation_access(function, structural_types, block, node, live, *source, &[])?;
        }
        O::PrimitiveLocalStore { destination, .. } => {
            check_root_access(function, block, node, live, *destination)?
        }
        O::StructuralByteSequenceFieldLength {
            source,
            path,
            field,
            ..
        }
        | O::StructuralByteSequenceFieldRead {
            source,
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
                *source,
                path,
                Some(*field),
            )?;
        }
        O::EstablishByteSequenceConcatenation { left, right, .. } => {
            // These operands name byte storage, not reference carriers whose
            // own loan could authorize an observation through a referent.
            for operand in [left, right] {
                match operand {
                    terminal_psi::control_flow::ByteSequenceOperand::Whole { source, .. } => {
                        check_root_access(function, block, node, live, *source)?
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
                    )?,
                }
            }
        }
        O::WriteOnlyPrimitiveStore {
            destination, path, ..
        } => {
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
            )?;
        }
        O::MoveStructuralField {
            source,
            path,
            field,
            ..
        } => {
            check_window_access(
                function,
                structural_types,
                block,
                node,
                live,
                source,
                path,
                *field,
            )?;
        }
        O::StoreStructuralField {
            destination,
            path,
            field,
            ..
        } => {
            check_window_access(
                function,
                structural_types,
                block,
                node,
                live,
                destination,
                path,
                *field,
            )?;
        }
        O::StructuralLeafCopy { result, source, .. } => {
            // A `&`-leaf copy relocates the source's existing loan
            // descriptor: the result is a fresh live carrier rooted at that
            // same primitive loan — no new edge, no consumption of `source`.
            // The case-qualified variant relocates the same loan out of a
            // proven-active case payload.
            if structural_types
                .get(&result.structural_type)
                .is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::Reference {
                            access: terminal_psi::StructuralAccess::SharedBorrow,
                            ..
                        }
                    )
                })
            {
                let root = ReferenceOrigin::Primitive(*source);
                live.push(LiveReference {
                    identity: ReferenceIdentity {
                        place: result.place,
                        path: Vec::new(),
                    },
                    carrier: result.place,
                    carrier_path: Vec::new(),
                    root: root.clone(),
                    parent: ReferenceParent::Root(root),
                });
            }
        }
        _ => {}
    }
    let mut normalized_arguments = Vec::new();
    let mut moved = BTreeSet::new();
    for argument in operation_structural_arguments(operation) {
        if let Some(root) =
            mutable_record_call_origin(function, structural_types, block, node, live, argument)?
        {
            normalized_arguments.push((root, argument.access, true));
            continue;
        }
        if argument_owns_references(function, structural_types, argument) {
            if !argument.path.is_empty() || matches!(operation, O::BoundaryCall { .. }) {
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
            let paths = leaf_paths(structural_types, signature.structural_type, live.len())
                .ok_or_else(|| {
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
                    .filter(|reference| reference.carrier == argument.place)
                    .count()
            {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "owned argument reference roster differs from its type",
                ));
            }
            for path in paths {
                let reference = live
                    .iter()
                    .find(|reference| {
                        reference.carrier == argument.place && reference.carrier_path == path
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
                    || !moved.insert(reference.identity.clone())
                {
                    return Err(invalid(
                        function,
                        block,
                        Some(node),
                        "owned reference argument duplicates or suspends a leaf",
                    ));
                }
                normalized_arguments.push((reference.root.clone(), argument.access, true));
            }
            continue;
        }
        if structural_source_contract(function, argument.place)
            .is_some_and(|source| contains_reference(structural_types, source.structural_type))
            || live
                .iter()
                .any(|reference| origin_place(&reference.root) == argument.place)
        {
            if !is_reference_projection(function, structural_types, argument) {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "call cannot access a suspended root or move its referent",
                ));
            }
            let (root, _) =
                normalized_source(function, structural_types, block, node, live, argument)?;
            normalized_arguments.push((root, argument.access, true));
        } else {
            normalized_arguments.push((
                ReferenceOrigin::Primitive(argument.place),
                argument.access,
                false,
            ));
        }
    }
    for (position, (root, access, reference_access)) in normalized_arguments.iter().enumerate() {
        if normalized_arguments[position + 1..].iter().any(
            |(other, other_access, other_reference_access)| {
                (*reference_access || *other_reference_access)
                    && origins_overlap(root, other)
                    && (*access != StructuralAccess::SharedBorrow
                        || *other_access != StructuralAccess::SharedBorrow)
            },
        ) {
            return Err(invalid(
                function,
                block,
                Some(node),
                "call arguments overlap through reference origins",
            ));
        }
    }
    let sources = match operation {
        O::EstablishReference { source, .. } => vec![(Vec::new(), vec![(source.clone(), false)])],
        O::CallStructural {
            callee,
            structural_arguments,
            result: AbstractStructuralCallResult::Structural { result, .. },
            ..
        } if contains_reference(structural_types, result.structural_type) => {
            let callee = functions.get(callee).ok_or_else(|| {
                invalid(
                    function,
                    block,
                    Some(node),
                    "reference call target is absent",
                )
            })?;
            let declaration = callee.result.structural().ok_or_else(|| {
                invalid(
                    function,
                    block,
                    Some(node),
                    "reference call target has no structural result",
                )
            })?;
            declaration
                .reference_sources
                .iter()
                .map(|mapping| {
                    let candidates = mapping
                        .sources
                        .iter()
                        .map(|formal| {
                            call_source(callee, structural_arguments, formal)
                        .map(|source| {
                            (
                                source,
                                matches!(
                                    formal_origin(callee, formal),
                                    Some(ReferenceOrigin::IngressLeaf(identity))
                                        if structural_source_contract(
                                            callee,
                                            identity.place,
)
                                        .and_then(|signature| {
                                            leaf_type(
                                                structural_types,
                                                signature.structural_type,
                                                &identity.path,
                                            )
                                        })
                                        .is_some_and(|leaf| {
                                            reference_access(structural_types, leaf).is_some()
                                        })
                                ),
                            )
                        })
                        .ok_or_else(|| {
                            invalid(
                                function,
                                block,
                                Some(node),
                                "reference call source mapping is absent",
                            )
                        })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok((mapping.path.clone(), candidates))
                })
                .collect::<Result<Vec<_>, _>>()?
        }
        _ => Vec::new(),
    };
    if !sources.is_empty() {
        let result = match operation {
            O::EstablishReference { result, .. }
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
        let mut roots = BTreeSet::new();
        for (path, candidates) in sources {
            let expected = leaf_referent(structural_types, result.structural_type, &path)
                .ok_or_else(|| {
                    invalid(
                        function,
                        block,
                        Some(node),
                        "reference call result leaf is unsupported",
                    )
                })?;
            let mut alternatives = Vec::with_capacity(candidates.len());
            for (source, transfers_existing) in candidates {
                let compatible = reference_source_type(function, structural_types, &source)
                    .is_some_and(|actual| {
                        if matches!(operation, O::EstablishReference { .. }) {
                            terminal_semantics::reference_source_compatible(
                                structural_types.values().copied(),
                                actual,
                                result.structural_type,
                            )
                        } else {
                            actual == expected
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
                    let (root, parent) =
                        normalized_source(function, structural_types, block, node, live, &source)?;
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
            if roots
                .iter()
                .any(|root| origins_overlap(root, &reference.root))
            {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "reference result aliases its mapped sources",
                ));
            }
            roots.insert(reference.root.clone());
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
    Ok(())
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
    let mut paths = Vec::new();
    for alternative in alternatives {
        if reference.identity != alternative.identity
            || reference.carrier != alternative.carrier
            || reference.carrier_path != alternative.carrier_path
            || reference.parent != ReferenceParent::Root(reference.root.clone())
            || alternative.parent != ReferenceParent::Root(alternative.root.clone())
            || origin_place(&reference.root) != origin_place(&alternative.root)
        {
            return Err(invalid());
        }
        let ReferenceOrigin::IngressLeaf(identity) = alternative.root else {
            return Err(invalid());
        };
        paths.push(identity.path);
    }
    if !paths.is_empty() {
        let ReferenceOrigin::IngressLeaf(identity) = &reference.root else {
            return Err(invalid());
        };
        paths.push(identity.path.clone());
        paths.sort();
        if paths.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid());
        }
        reference.root = ReferenceOrigin::Alternatives {
            place: identity.place,
            paths,
        };
        reference.parent = ReferenceParent::Root(reference.root.clone());
    }
    Ok(reference)
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
            .filter(|reference| reference.carrier == source)
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
        let sources = mapping
            .sources
            .iter()
            .filter(|source| {
                formal_origin(function, source)
                    .is_some_and(|origin| contains_origin(&reference.root, &origin))
            })
            .cloned()
            .collect::<Vec<_>>();
        let origin_count = match &reference.root {
            ReferenceOrigin::Alternatives { paths, .. } => paths.len(),
            _ => 1,
        };
        if sources.len() != origin_count
            || reference.parent != ReferenceParent::Root(reference.root.clone())
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
            sources,
        });
    }
    // This removes frame-local ownership rows, not the referents or the loans.
    // The caller independently instantiates the checked result source roster.
    live.retain(|reference| reference.carrier != source);
    Ok(true)
}

/// Individual exits prove membership; their union proves that no declared
/// alternative was invented. The CFG owner supplies only reachable returns.
pub(super) fn validate_return_coverage(
    function: &PsiOptimizationFunction,
    observed: &[terminal_psi::StructuralReferenceResultSource],
) -> Result<(), OptimizationUnitValidationError> {
    let Some(result) = function.result.structural() else {
        return Ok(());
    };
    for mapping in &result.reference_sources {
        if mapping.sources.len() <= 1 {
            continue;
        }
        let sources = observed
            .iter()
            .filter(|returned| returned.path == mapping.path)
            .flat_map(|returned| returned.sources.iter().cloned())
            .collect::<BTreeSet<_>>();
        if sources.into_iter().collect::<Vec<_>>() != mapping.sources {
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

/// Normal completion must leave no outstanding loans: every carrier was either
/// released, consumed by an owner discard, or transferred through a checked
/// result roster.
pub(super) fn require_no_references(
    function: &PsiOptimizationFunction,
    block: BlockId,
    live: &[LiveReference],
) -> Result<(), OptimizationUnitValidationError> {
    if live.is_empty() {
        Ok(())
    } else {
        Err(invalid(
            function,
            block,
            None,
            "normal completion leaves reference custody unaccounted for",
        ))
    }
}
