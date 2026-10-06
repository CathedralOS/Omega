//! Type-derived ordered disposition of a whole owner or its live complement.
//!
//! Nominal hooks replace the no-code disposition of their exact subtree; they
//! do not retire an enclosing owner or authorize disposal of a moved sibling.

use super::{
    BTreeMap, BTreeSet, BlockId, MachineId, ModuleError, PlaceId, StructuralMultiplicity,
    StructuralOwnershipFrontier, StructuralPathSegment, StructuralTypeId, StructuralTypeShape,
    TerminalAffineCleanupAction, TerminalMachine, TerminalModule, consume_nominal_cleanup,
    partial_affine_residuals, partial_affine_root_type,
};

pub(super) struct CleanupSubject {
    pub path: Vec<StructuralPathSegment>,
    pub structural_type: StructuralTypeId,
    pub nominal: bool,
}

/// Expand only subtrees with executable cleanup. Cleanup-free subtrees remain
/// maximal, so an ordinary array does not acquire one action per element.
pub(super) fn subjects(
    module: &TerminalModule,
    root: StructuralTypeId,
    moved: Option<&BTreeSet<Vec<StructuralPathSegment>>>,
    limit: usize,
) -> Option<Vec<CleanupSubject>> {
    let remaining = if let Some(moved) = moved {
        // A nominal hook is entitled to its complete receiver. An ordinary
        // parent may lose that child whole, but no move may cut inside it.
        for path in moved {
            let mut current = root;
            for segment in path {
                if module
                    .machines
                    .iter()
                    .any(|machine| machine.nominal_cleanup_subject == Some(current))
                {
                    return None;
                }
                current = super::super::resolve_structural_path(
                    module,
                    current,
                    std::slice::from_ref(segment),
                )?;
            }
        }
        partial_affine_residuals(module, root, moved, limit)?
    } else {
        vec![(Vec::new(), root)]
    };
    let mut subjects = Vec::new();
    for (mut path, structural_type) in remaining {
        append_subjects(module, structural_type, &mut path, limit, &mut subjects)?;
    }
    Some(subjects)
}

fn contains_nominal(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    ancestors: &mut BTreeSet<StructuralTypeId>,
) -> Option<bool> {
    if module
        .machines
        .iter()
        .any(|machine| machine.nominal_cleanup_subject == Some(structural_type))
    {
        return Some(true);
    }
    if !ancestors.insert(structural_type) {
        return None;
    }
    let declaration = module
        .structural_types
        .iter()
        .find(|item| item.id == structural_type)?;
    let mut contains = false;
    match &declaration.shape {
        StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. } => {
            for field in fields {
                if let terminal_psi::StructuralFieldType::Structural(child) = field.field_type {
                    contains |= contains_nominal(module, child, ancestors)?;
                }
            }
        }
        StructuralTypeShape::FixedArray { element, length } if *length != 0 => {
            contains = contains_nominal(module, *element, ancestors)?;
        }
        _ => {}
    }
    if let StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } =
        &declaration.shape
    {
        for case in cases {
            for field in &case.fields {
                if let terminal_psi::StructuralFieldType::Structural(child) = field.field_type {
                    contains |= contains_nominal(module, child, ancestors)?;
                }
            }
        }
    }
    ancestors.remove(&structural_type);
    Some(contains)
}

fn append_subjects(
    module: &TerminalModule,
    structural_type: StructuralTypeId,
    path: &mut Vec<StructuralPathSegment>,
    limit: usize,
    subjects: &mut Vec<CleanupSubject>,
) -> Option<()> {
    let nominal = module
        .machines
        .iter()
        .any(|machine| machine.nominal_cleanup_subject == Some(structural_type));
    if nominal || !contains_nominal(module, structural_type, &mut BTreeSet::new())? {
        if subjects.len() >= limit {
            return None;
        }
        subjects.push(CleanupSubject {
            path: path.clone(),
            structural_type,
            nominal,
        });
        return Some(());
    }
    let declaration = module
        .structural_types
        .iter()
        .find(|item| item.id == structural_type)?;
    match &declaration.shape {
        StructuralTypeShape::Record { fields } => {
            for field in fields.iter().rev() {
                if let terminal_psi::StructuralFieldType::Structural(child) = field.field_type {
                    if field.relevance.is_erased() {
                        return None;
                    }
                    path.push(StructuralPathSegment::Field(field.identity.clone()));
                    append_subjects(module, child, path, limit, subjects)?;
                    path.pop();
                }
            }
        }
        StructuralTypeShape::FixedArray { element, length } => {
            // Every live element needs at least one action. Check the bound
            // before enumerating an untrusted literal dimension.
            if usize::try_from(*length).ok()? > limit.checked_sub(subjects.len())? {
                return None;
            }
            for index in (0..*length).rev() {
                path.push(StructuralPathSegment::FixedIndex(index));
                append_subjects(module, *element, path, limit, subjects)?;
                path.pop();
            }
        }
        _ => return None,
    }
    Some(())
}

/// The caller selects owners in reverse establishment order. This operation
/// proves and consumes their exact full partition, independently of action kind.
#[allow(clippy::too_many_arguments)]
pub(super) fn consume_owner(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    block: BlockId,
    frontier: &mut StructuralOwnershipFrontier,
    place: PlaceId,
    actions: &mut std::slice::Iter<'_, TerminalAffineCleanupAction>,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidPartialAffineCleanup {
        machine: machine.id,
        block,
    };
    let root_type =
        super::super::structural::operations::caller_structural_root_type(machine, place)
            .ok_or_else(invalid)?;
    let multiplicity = frontier
        .owned_places
        .get(&place)
        .copied()
        .ok_or_else(invalid)?;
    let moved = frontier.partial_custody_paths.get(&place);
    if frontier.restoration_debt.contains_key(&place)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == place)
        || (moved.is_some() && partial_affine_root_type(machine, place).is_none())
    {
        return Err(invalid());
    }
    let subjects = subjects(module, root_type, moved, actions.len()).ok_or_else(invalid)?;
    if subjects.is_empty() {
        return Err(invalid());
    }
    for subject in subjects {
        let action = actions.next().ok_or_else(invalid)?;
        if subject.nominal {
            let TerminalAffineCleanupAction::InvokeNominal(cleanup) = action else {
                return Err(invalid());
            };
            if cleanup.place != place
                || cleanup.path != subject.path
                || cleanup.structural_type != subject.structural_type
            {
                return Err(invalid());
            }
            consume_nominal_cleanup(module, machine, machines, block, frontier, cleanup)?;
        } else {
            if multiplicity != StructuralMultiplicity::Affine
                || frontier
                    .claims
                    .values()
                    .any(|claim| claim.input == Some(place))
            {
                return Err(invalid());
            }
            if subject.path.is_empty() {
                if action != &TerminalAffineCleanupAction::DiscardRoot(place) {
                    return Err(invalid());
                }
                super::super::references::discard_owned(
                    module,
                    machine,
                    &mut frontier.references,
                    place,
                )?;
            } else {
                let expected = TerminalAffineCleanupAction::DiscardResidual(
                    terminal_psi::StructuralAffineDiscard {
                        place,
                        path: subject.path.clone(),
                        structural_type: subject.structural_type,
                    },
                );
                if action != &expected {
                    return Err(invalid());
                }
                super::super::references::discard_residual(
                    module,
                    machine,
                    &mut frontier.references,
                    place,
                    &subject.path,
                    subject.structural_type,
                )?;
            }
        }
    }
    frontier.partial_custody_paths.remove(&place);
    frontier.owned_places.remove(&place);
    Ok(())
}
