//! Ordered cleanup consumes exact live subjects, independent of root origin or action kind.
use crate::OptimizationUnitValidationError;
use crate::current_ownership::residuals::partial_affine_residuals;
use crate::current_ownership::structural::place_structural_type;
use crate::current_ownership::{CurrentOwnership, parameter_establishment_order};
use abstract_operations::AbstractFunctionResult;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, MachineId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralFieldType, StructuralMultiplicity, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalAffineCleanupAction,
};

pub(super) fn validate_unit_cleanup_actions(
    function: &PsiOptimizationFunction,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    frontier: &CurrentOwnership,
    actions: &[TerminalAffineCleanupAction],
) -> Result<(), OptimizationUnitValidationError> {
    validate_scalar_cleanup_actions(
        function,
        functions,
        structural_types,
        block,
        frontier,
        actions,
    )?;
    Ok(())
}

pub(super) fn validate_scalar_cleanup_actions(
    function: &PsiOptimizationFunction,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    frontier: &CurrentOwnership,
    actions: &[TerminalAffineCleanupAction],
) -> Result<CurrentOwnership, OptimizationUnitValidationError> {
    let mut remaining = frontier.clone();
    forget_drained_partial_roots(function, structural_types, &mut remaining);
    super::continuation::apply_edge_cleanup_actions(
        function,
        functions,
        structural_types,
        block,
        &mut remaining,
        actions,
    )?;
    if !remaining.owned_places.is_empty()
        || !remaining.partial_custody_paths.is_empty()
        || !remaining.claims.is_empty()
    {
        return Err(OptimizationUnitValidationError::CurrentCleanupMismatch {
            machine: function.machine,
            block,
        });
    }
    super::references::validate_normal_return(
        function,
        structural_types,
        block,
        &remaining.live_references,
    )?;
    Ok(remaining)
}

/// Root ordering includes nominal and linear owners. Their eligibility is
/// checked separately; cleanup kind cannot change establishment order.
pub(super) fn cleanup_root_order(
    function: &PsiOptimizationFunction,
    frontier: &CurrentOwnership,
) -> Vec<PlaceId> {
    let mut results = Vec::new();
    let mut locals = Vec::new();
    for place in &function.structural_places {
        if !frontier.owned_places.contains_key(&place.id) {
            continue;
        }
        match place.kind {
            semantic_vocabulary::StructuralPlaceKind::OperationResult { producer, .. } => {
                results.push((producer, place.id))
            }
            semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                ..
            } => locals.push((declaration_ordinal, place.id)),
            _ => {}
        }
    }
    results.sort_by_key(|(producer, _)| std::cmp::Reverse(*producer));
    locals.sort_by_key(|(ordinal, _)| std::cmp::Reverse(*ordinal));
    results
        .into_iter()
        .map(|(_, place)| place)
        .chain(locals.into_iter().map(|(_, place)| place))
        .chain(
            parameter_establishment_order(function)
                .into_iter()
                .rev()
                .filter(|parameter| frontier.owned_places.contains_key(&parameter.place))
                .map(|parameter| parameter.place),
        )
        .collect()
}

struct CleanupSubject {
    path: Vec<StructuralPathSegment>,
    structural_type: StructuralTypeId,
    nominal: bool,
}

/// Expand only containers that own executable child cleanup. Untouched
/// no-code subtrees remain maximal; the bound is the supplied action count.
fn cleanup_subjects(
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    root: StructuralTypeId,
    moved: Option<&BTreeSet<Vec<StructuralPathSegment>>>,
    limit: usize,
) -> Option<Vec<CleanupSubject>> {
    let nominal: BTreeSet<_> = functions
        .values()
        .filter_map(|function| function.nominal_cleanup_subject)
        .collect();
    if let Some(moved) = moved {
        for path in moved {
            for depth in 0..path.len() {
                let ancestor = crate::unit_validation::structural_catalog::resolve_structural_path(
                    types,
                    root,
                    &path[..depth],
                )?;
                if nominal.contains(&ancestor) {
                    return None;
                }
            }
        }
    }
    let roots = match moved {
        Some(moved) => partial_affine_residuals(types, root, moved, limit)?,
        None => vec![(Vec::new(), root)],
    };
    let mut subjects = Vec::new();
    let mut contains = BTreeMap::new();
    for (path, structural_type) in roots {
        append_cleanup_subjects(
            types,
            &nominal,
            &mut contains,
            structural_type,
            path,
            limit,
            &mut subjects,
        )?;
    }
    Some(subjects)
}

fn contains_nominal_cleanup(
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    nominal: &BTreeSet<StructuralTypeId>,
    cache: &mut BTreeMap<StructuralTypeId, bool>,
    active: &mut BTreeSet<StructuralTypeId>,
    structural_type: StructuralTypeId,
) -> Option<bool> {
    if nominal.contains(&structural_type) {
        return Some(true);
    }
    if let Some(result) = cache.get(&structural_type) {
        return Some(*result);
    }
    if !active.insert(structural_type) {
        return None;
    }
    let mut result = false;
    match &types.get(&structural_type)?.shape {
        StructuralTypeShape::Record { fields } => {
            for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
                if let StructuralFieldType::Structural(child) = field.field_type {
                    result |= contains_nominal_cleanup(types, nominal, cache, active, child)?;
                }
            }
        }
        StructuralTypeShape::FixedArray { element, length } if *length != 0 => {
            result = contains_nominal_cleanup(types, nominal, cache, active, *element)?
        }
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
            let common = match &types.get(&structural_type)?.shape {
                StructuralTypeShape::Mixed { fields, .. } => fields.as_slice(),
                _ => &[],
            };
            for field in common
                .iter()
                .chain(cases.iter().flat_map(|case| &case.fields))
            {
                if !field.relevance.is_erased()
                    && let StructuralFieldType::Structural(child) = field.field_type
                {
                    result |= contains_nominal_cleanup(types, nominal, cache, active, child)?;
                }
            }
        }
        _ => {}
    }
    active.remove(&structural_type);
    cache.insert(structural_type, result);
    Some(result)
}

fn append_cleanup_subjects(
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    nominal: &BTreeSet<StructuralTypeId>,
    cache: &mut BTreeMap<StructuralTypeId, bool>,
    structural_type: StructuralTypeId,
    path: Vec<StructuralPathSegment>,
    limit: usize,
    subjects: &mut Vec<CleanupSubject>,
) -> Option<()> {
    if nominal.contains(&structural_type)
        || !contains_nominal_cleanup(types, nominal, cache, &mut BTreeSet::new(), structural_type)?
    {
        if subjects.len() >= limit {
            return None;
        }
        subjects.push(CleanupSubject {
            path,
            structural_type,
            nominal: nominal.contains(&structural_type),
        });
        return Some(());
    }
    match &types.get(&structural_type)?.shape {
        StructuralTypeShape::Record { fields } => {
            for field in fields
                .iter()
                .rev()
                .filter(|field| !field.relevance.is_erased())
            {
                if let StructuralFieldType::Structural(child) = field.field_type {
                    let mut child_path = path.clone();
                    child_path.push(StructuralPathSegment::Field(field.identity.clone()));
                    append_cleanup_subjects(
                        types, nominal, cache, child, child_path, limit, subjects,
                    )?;
                }
            }
        }
        StructuralTypeShape::FixedArray { element, length } => {
            if u128::from(*length) > limit.checked_sub(subjects.len())? as u128 {
                return None;
            }
            for index in (0..*length).rev() {
                let mut child_path = path.clone();
                child_path.push(StructuralPathSegment::FixedIndex(index));
                append_cleanup_subjects(
                    types, nominal, cache, *element, child_path, limit, subjects,
                )?;
            }
        }
        _ => return None,
    }
    Some(())
}

/// Check one root's complete live partition before retiring that root. Hook
/// and no-code subjects share the same exact path/type order.
pub(super) fn consume_owner_cleanup(
    function: &PsiOptimizationFunction,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    frontier: &mut CurrentOwnership,
    place: PlaceId,
    actions: &[TerminalAffineCleanupAction],
) -> Result<(), OptimizationUnitValidationError> {
    let mismatch = || OptimizationUnitValidationError::CurrentCleanupMismatch {
        machine: function.machine,
        block,
    };
    let multiplicity = *frontier.owned_places.get(&place).ok_or_else(mismatch)?;
    let root = place_structural_type(function, place).ok_or_else(mismatch)?;
    if function
        .content_entry_claims
        .iter()
        .any(|claim| claim.input.root == place)
    {
        return Err(mismatch());
    }
    let subjects = cleanup_subjects(
        functions,
        types,
        root,
        frontier.partial_custody_paths.get(&place),
        actions.len(),
    )
    .ok_or_else(mismatch)?;
    if subjects.len() != actions.len() {
        return Err(mismatch());
    }
    for (subject, action) in subjects.iter().zip(actions) {
        if action.place() != place {
            return Err(mismatch());
        }
        match action {
            TerminalAffineCleanupAction::InvokeNominal(cleanup)
                if subject.nominal
                    && cleanup.path == subject.path
                    && cleanup.structural_type == subject.structural_type =>
            {
                consume_nominal_cleanup(function, functions, types, block, frontier, cleanup)?;
            }
            TerminalAffineCleanupAction::DiscardRoot(_)
                if !subject.nominal
                    && subject.path.is_empty()
                    && multiplicity == StructuralMultiplicity::Affine
                    && !frontier
                        .claims
                        .values()
                        .any(|claim| claim.input == Some(place)) =>
            {
                super::references::discard_owned(
                    function,
                    types,
                    block,
                    &mut frontier.live_references,
                    place,
                )?;
            }
            TerminalAffineCleanupAction::DiscardResidual(discard)
                if !subject.nominal
                    && !subject.path.is_empty()
                    && discard.path == subject.path
                    && discard.structural_type == subject.structural_type
                    && multiplicity == StructuralMultiplicity::Affine
                    && !frontier
                        .claims
                        .values()
                        .any(|claim| claim.input == Some(place)) =>
            {
                super::references::discard_subtree(
                    function,
                    types,
                    block,
                    &mut frontier.live_references,
                    place,
                    &subject.path,
                )?;
            }
            _ => return Err(mismatch()),
        }
    }
    if frontier
        .claims
        .values()
        .any(|claim| claim.input == Some(place))
    {
        return Err(mismatch());
    }
    frontier.owned_places.remove(&place);
    frontier.partial_custody_paths.remove(&place);
    Ok(())
}

/// Authenticate the exact nominal subject and its claim roster. The enclosing
/// ordered partition retires the root only after all sibling subjects finish.
pub(super) fn consume_nominal_cleanup(
    function: &PsiOptimizationFunction,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    frontier: &mut CurrentOwnership,
    cleanup: &terminal_psi::NominalAffineCleanup,
) -> Result<(), OptimizationUnitValidationError> {
    let mismatch = || OptimizationUnitValidationError::CurrentCleanupMismatch {
        machine: function.machine,
        block,
    };
    let multiplicity = *frontier
        .owned_places
        .get(&cleanup.place)
        .ok_or_else(mismatch)?;
    let root = place_structural_type(function, cleanup.place).ok_or_else(mismatch)?;
    let claims = frontier
        .claims
        .iter()
        .filter(|(_, claim)| {
            claim.input == Some(cleanup.place)
                && terminal_semantics::structural_paths_may_overlap(&claim.path, &cleanup.path)
        })
        .collect::<Vec<_>>();
    let selected = crate::unit_validation::structural_catalog::resolve_structural_path(
        structural_types,
        root,
        &cleanup.path,
    );
    if !cleanup.requirement_obligations.is_empty()
        || selected != Some(cleanup.structural_type)
        || !valid_nominal_cleanup(function, functions, structural_types, cleanup)
        || frontier
            .partial_custody_paths
            .get(&cleanup.place)
            .is_some_and(|paths| {
                paths.iter().any(|path| {
                    terminal_semantics::structural_paths_may_overlap(path, &cleanup.path)
                })
            })
        || claims.iter().any(|(_, claim)| {
            claim.path != cleanup.path || claim.multiplicity != Some(StructuralMultiplicity::Linear)
        })
        || (cleanup.path.is_empty()
            && match multiplicity {
                StructuralMultiplicity::Affine => !claims.is_empty(),
                StructuralMultiplicity::Linear => claims.len() != 1,
                StructuralMultiplicity::Unrestricted => true,
            })
        || (!cleanup.path.is_empty() && claims.len() > 1)
        || cleanup.consumed_claims
            != claims
                .iter()
                .map(|(identity, _)| **identity)
                .collect::<Vec<_>>()
    {
        return Err(mismatch());
    }
    let ordinal = function
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)
        .and_then(|block| u32::try_from(block.nodes.len().saturating_sub(1)).ok())
        .ok_or_else(mismatch)?;
    super::references::check_region_access(
        function,
        block,
        ordinal,
        &frontier.live_references,
        cleanup.place,
        &cleanup.path,
    )?;
    for claim in &cleanup.consumed_claims {
        frontier.claims.remove(claim);
    }
    Ok(())
}

pub(super) fn valid_nominal_cleanup(
    caller: &PsiOptimizationFunction,
    functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    cleanup: &terminal_psi::NominalAffineCleanup,
) -> bool {
    let Some(source) = structural_types.get(&cleanup.structural_type) else {
        return false;
    };
    let Some(target) = functions.get(&cleanup.cleanup_machine).copied() else {
        return false;
    };
    cleanup.cleanup_machine != caller.machine
        && bounded_nominal_cleanup_receiver_shape(&source.shape)
        && target.attachment == Some(cleanup.structural_type)
        && target.nominal_cleanup_subject == Some(cleanup.structural_type)
        && (cleanup.consumed_claims.is_empty() || target.published_service_ceiling.is_empty())
        && functions
            .values()
            .filter(|candidate| candidate.nominal_cleanup_subject == Some(cleanup.structural_type))
            .count()
            == 1
        && cleanup
            .consumed_claims
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        && matches!(target.result, AbstractFunctionResult::Unit)
        && target.parameters.is_empty()
        && match target.structural_parameters.as_slice() {
            [] => cleanup.cleanup_receiver.is_none() && target.structural_places.is_empty(),
            [parameter] => {
                parameter.position == 0
                    && parameter.structural_type == cleanup.structural_type
                    && cleanup.cleanup_receiver == Some(parameter.place)
                    && matches!(
                        parameter.access,
                        terminal_psi::StructuralAccess::MutableBorrow
                            | terminal_psi::StructuralAccess::WriteOnlyBorrow
                    )
                    && target.structural_places.iter().any(|place| {
                        place.id == parameter.place
                            && place.kind
                                == semantic_vocabulary::StructuralPlaceKind::Parameter {
                                    position: parameter.position,
                                }
                    })
            }
            _ => false,
        }
        && target.entry_claim_declarations.is_empty()
        && target.content_entry_claims.is_empty()
        && target
            .published_service_ceiling
            .iter()
            .all(|service| caller.published_service_ceiling.contains(service))
        // A hook's postconditions describe its dying receiver's final state;
        // no continuation consumes them, so they carry no custody obligation.
        // A crash route would add an abnormal exit the cleanup edge cannot
        // represent. Requirement sites were erased at the Terminal firewall.
        && target
            .verified_contract
            .as_ref()
            .is_none_or(|contract| contract.crash_routes.is_empty())
}

pub(super) fn bounded_nominal_cleanup_receiver_shape(shape: &StructuralTypeShape) -> bool {
    let StructuralTypeShape::Record { fields } = shape else {
        return false;
    };
    fields.iter().all(|field| {
        !field.relevance.is_erased()
            && match field.field_type {
                StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Boolean) => true,
                StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Integer(integer)) => {
                    matches!(integer.bits(), 8 | 16 | 32 | 64)
                        && (!integer.is_address() || integer.bits() == 64)
                }
                StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::IeeeFloat(_))
                | StructuralFieldType::Atomic(_)
                | StructuralFieldType::IeeeFloat(_)
                | StructuralFieldType::ByteSequence(_)
                | StructuralFieldType::PredicateByteSequence { .. }
                | StructuralFieldType::Structural(_)
                | StructuralFieldType::Erased { .. }
                | StructuralFieldType::RefinedScalar(_)
                | StructuralFieldType::BoundedInteger(_) => false,
            }
    })
}

/// Scalar storage may outlive its last structural disposal obligation. Only
/// the declaration-derived empty complement permits dropping that path history
/// at frame exit; an unaccounted live child or partial debt still rejects.
fn forget_drained_partial_roots(
    function: &PsiOptimizationFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    frontier: &mut CurrentOwnership,
) {
    let drained = frontier
        .partial_custody_paths
        .keys()
        .copied()
        .filter(|place| {
            !frontier.owned_places.contains_key(place)
                && !frontier
                    .live_references
                    .iter()
                    .any(|reference| reference.carrier == *place)
                && super::structural::projected_root_is_fully_consumed(
                    function,
                    structural_types,
                    frontier,
                    *place,
                )
        })
        .collect::<Vec<_>>();
    for place in drained {
        frontier.partial_custody_paths.remove(&place);
    }
}
