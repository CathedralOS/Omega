//! Live loan replay over the current ownership frontier.
//!
//! This mirrors the verified Terminal-Psi reference walk: carriers own loan
//! permission, never their primitive referent. Origins and immediate parents
//! are reconstructed from operations and checked call interfaces; no
//! producer-supplied lifetime or origin table is authority.

use crate::O;
use crate::OptimizationUnitValidationError;
use crate::unit_validation::operation_contracts::structural_source_contract;
use crate::unit_validation::references::{
    LiveReference, ReferenceIdentity, ReferenceOrigin, ReferenceParent, argument_owns_references,
    contains_reference, formal_origin, is_reference_projection, leaf_paths, leaf_referent,
    reference_source_type,
};
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::BlockId;
use semantic_vocabulary::MachineId;
use semantic_vocabulary::PlaceId;
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use terminal_psi::StructuralAccess;
use terminal_psi::StructuralArgument;
use terminal_psi::StructuralTypeDeclaration;

// Incoming types have no explicit leaf roster to bound reconstruction. Keep
// this private verification-work capacity separate from resource authority;
// local/result rosters remain bounded by their existing captured rows.
const MAX_INCOMING_REFERENCE_LEAVES: usize = 4096;

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
        O::CallUnit {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalarWithDynamicArguments {
            structural_arguments,
            ..
        }
        | O::CallStructural {
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
        O::CallDynamicScalar {
            dynamic_dispatch, ..
        } => std::slice::from_ref(&dynamic_dispatch.rebound.source),
        _ => &[],
    }
}

/// The spelled hole one `MoveStructuralField` opens beneath `root`: the
/// operation's path plus the declared identity of the field it vacates.
/// `None` means the destination does not host a structural field there —
/// the unit's catalog contract rejects that shape separately.
fn window_hole(
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    root: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Option<Vec<terminal_psi::StructuralPathSegment>> {
    let parent =
        crate::unit_validation::structural_catalog::resolve_structural_path(types, root, path)?;
    let declaration = types.get(&parent)?;
    let fields = match &declaration.shape {
        terminal_psi::StructuralTypeShape::Record { fields }
        | terminal_psi::StructuralTypeShape::Mixed { fields, .. } => fields,
        _ => return None,
    };
    let identity = fields
        .iter()
        .find(|candidate| candidate.id == field && !candidate.relevance.is_erased())?
        .identity
        .clone();
    let mut hole = path.to_vec();
    hole.push(terminal_psi::StructuralPathSegment::Field(identity));
    Some(hole)
}

fn check_root_access(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: u32,
    live: &[LiveReference],
    place: PlaceId,
) -> Result<(), OptimizationUnitValidationError> {
    if live.iter().any(|reference| {
        reference.root == ReferenceOrigin::Primitive(place) || reference.carrier == place
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

/// A `.., Referent` source names a suspended child of the located carrier
/// leaf; an empty path names a primitive referent root directly. Any other
/// projection does not describe reference custody this layer admits.
fn normalized_source(
    function: &PsiOptimizationFunction,
    block: BlockId,
    node: u32,
    live: &[LiveReference],
    source: &StructuralArgument,
) -> Result<(ReferenceOrigin, ReferenceParent), OptimizationUnitValidationError> {
    if let Some((terminal_psi::StructuralPathSegment::Referent, carrier_path)) =
        source.path.split_last()
    {
        let parent = live
            .iter()
            .find(|reference| {
                reference.carrier == source.place && reference.carrier_path == carrier_path
            })
            .ok_or_else(|| {
                invalid(
                    function,
                    block,
                    Some(node),
                    "reference carrier is no longer live",
                )
            })?;
        if live
            .iter()
            .any(|reference| matches!(&reference.parent, ReferenceParent::Reference(identity) if identity == &parent.identity))
        {
            return Err(invalid(
                function,
                block,
                Some(node),
                "reference parent remains suspended by a live child",
            ));
        }
        Ok((
            parent.root.clone(),
            ReferenceParent::Reference(parent.identity.clone()),
        ))
    } else if source.path.is_empty() {
        if live
            .iter()
            .any(|reference| reference.root == ReferenceOrigin::Primitive(source.place))
        {
            return Err(invalid(
                function,
                block,
                Some(node),
                "original referent remains suspended by a live reference",
            ));
        }
        let root = ReferenceOrigin::Primitive(source.place);
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
    let Some(signature) = structural_source_contract(function, source, false) else {
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
        let signature =
            structural_source_contract(function, argument.place, false).ok_or_else(|| {
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
    mapping: &terminal_psi::StructuralReferenceResultSource,
) -> Option<StructuralArgument> {
    let position = callee
        .structural_parameters
        .iter()
        .position(|parameter| parameter.place == mapping.source.place)?;
    let mut source = arguments.get(position)?.clone();
    source.path.extend(mapping.source.path.iter().cloned());
    source.access = mapping.source.access;
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
    match operation {
        O::ReleaseReference { source, .. } => {
            return release(function, block, Some(node), live, *source);
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
        O::PrimitiveScalarRead { source, .. }
        | O::StructuralCaseMembership { source, .. }
        | O::StructuralCase { source, .. }
        | O::IntegerStructuralField { source, .. }
        | O::BooleanStructuralField { source, .. } => {
            check_root_access(function, block, node, live, *source)?
        }
        O::PrimitiveLocalStore { destination, .. } => {
            check_root_access(function, block, node, live, *destination)?
        }
        O::WriteOnlyPrimitiveStore { destination, .. }
        | O::StructuralScalarFieldStore { destination, .. } => {
            check_root_access(function, block, node, live, destination.place)?
        }
        O::EstablishPrimitiveLocal { result, .. } => {
            check_root_access(function, block, node, live, result.place)?
        }
        O::MoveStructuralField {
            source,
            path,
            field,
            ..
        } => {
            // The Terminal verifier proved the moving subtree carries no live
            // loan: replay the carrier half by overlapping every live leaf
            // path beneath this root with the spelled hole. Sibling loans
            // stay legal — only a carrier inside (or carrying) the vacancy
            // would point at relocated storage.
            let hole = window_hole(structural_types, source.structural_type, path, *field)
                .ok_or_else(|| {
                    invalid(
                        function,
                        block,
                        Some(node),
                        "restoration window does not resolve to a declared field",
                    )
                })?;
            if live.iter().any(|reference| {
                reference.carrier == source.place
                    && (reference.carrier_path.starts_with(hole.as_slice())
                        || hole.starts_with(&reference.carrier_path))
            }) {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "window extraction displaces a live reference leaf",
                ));
            }
        }
        _ => {}
    }
    let mut normalized_arguments = Vec::new();
    let mut moved = BTreeSet::new();
    for argument in operation_structural_arguments(operation) {
        if argument_owns_references(function, structural_types, argument) {
            if !argument.path.is_empty() || matches!(operation, O::BoundaryCall { .. }) {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "reference-bearing owned calls require a whole internal argument",
                ));
            }
            let signature = structural_source_contract(function, argument.place, false)
                .ok_or_else(|| {
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
        if structural_source_contract(function, argument.place, false)
            .is_some_and(|source| contains_reference(structural_types, source.structural_type))
            || live
                .iter()
                .any(|reference| reference.root == ReferenceOrigin::Primitive(argument.place))
        {
            if !is_reference_projection(function, structural_types, argument) {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "call cannot access a suspended root or move its referent",
                ));
            }
            let (root, _) = normalized_source(function, block, node, live, argument)?;
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
                    && root == other
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
        O::EstablishReference { source, .. } => vec![(Vec::new(), source.clone(), false)],
        O::CallStructural {
            callee,
            structural_arguments,
            result,
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
                    call_source(callee, structural_arguments, mapping)
                        .map(|source| {
                            (
                                mapping.path.clone(),
                                source,
                                matches!(
                                    formal_origin(callee, &mapping.source),
                                    Some(ReferenceOrigin::IngressLeaf(_))
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
                .collect::<Result<Vec<_>, _>>()?
        }
        _ => Vec::new(),
    };
    if !sources.is_empty() {
        let result = match operation {
            O::EstablishReference { result, .. } | O::CallStructural { result, .. } => result,
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
        for (path, source, transfers_existing) in sources {
            let expected = leaf_referent(structural_types, result.structural_type, &path)
                .ok_or_else(|| {
                    invalid(
                        function,
                        block,
                        Some(node),
                        "reference call result leaf is unsupported",
                    )
                })?;
            if reference_source_type(function, structural_types, &source) != Some(expected) {
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
                        reference.carrier == source.place && reference.carrier_path == carrier_path
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
                reference.carrier_path = path;
                reference
            } else {
                let (root, parent) = normalized_source(function, block, node, live, &source)?;
                LiveReference {
                    identity: ReferenceIdentity {
                        place: result.place,
                        path: path.clone(),
                    },
                    carrier: result.place,
                    carrier_path: path,
                    root,
                    parent,
                }
            };
            if !roots.insert(reference.root.clone()) {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "reference result aliases its mapped sources",
                ));
            }
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

/// A structural return whose source carries reference leaves must match the
/// declared result roster exactly: every returned leaf still sits at its
/// declared carrier path and descends directly from a checked formal origin.
pub(super) fn transfer_return(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    source: PlaceId,
    live: &mut Vec<LiveReference>,
) -> Result<bool, OptimizationUnitValidationError> {
    let Some(signature) = structural_source_contract(function, source, false) else {
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
        if Some(reference.root.clone()) != formal_origin(function, &mapping.source)
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
    }
    // This removes frame-local ownership rows, not the referents or the loans.
    // The caller independently instantiates the checked result source roster.
    live.retain(|reference| reference.carrier != source);
    Ok(true)
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
