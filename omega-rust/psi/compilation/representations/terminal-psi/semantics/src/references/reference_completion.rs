//! A normal result transfers its actual loans before frame-local disposal.
//! The interface may authenticate origins, but cannot erase missing ancestry
//! or a surviving shared-cohort member. Consumers reconstruct occurrences
//! independently and supply actual dispositions on the selected execution path.

use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalAffineCleanupAction,
};

/// Invocation-local custody graph, not another serialized loan certificate.
pub struct ReferenceCompletionOccurrence<'a, Identity> {
    pub identity: &'a Identity,
    pub parents: Vec<&'a Identity>,
    pub returned: bool,
    /// Current payload residence, authenticated independently of its formation.
    /// Caller storage survives this frame; its local ancestors still owe disposal.
    pub in_caller_storage: bool,
    pub disposed: bool,
}

/// Locate a reference payload in an actual borrowed function-entry declaration.
/// This identifies storage scope, not permission or the reference's backing.
/// Following a pointer or view needs its own storage observation and is not
/// inline residence in this entry. Consumers supply the actual declaration and
/// retain all formation, access and lifetime checks independently.
pub fn borrowed_entry_reference_referent<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    parameter: &StructuralParameterDeclaration,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    if parameter.access == StructuralAccess::Owned
        || !crate::projection_stays_in_inline_storage(
            declarations.clone(),
            parameter.structural_type,
            path,
        )
    {
        return None;
    }
    match crate::runtime_structural_path_tip(declarations, parameter.structural_type, path)?.shape {
        StructuralTypeShape::Reference { referent, .. } => Some(referent),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceCompletionError {
    DuplicateOccurrence,
    MissingParent,
    CyclicAncestry,
    UndisposedParent,
    SurvivingChild,
}

impl ReferenceCompletionError {
    pub const fn description(self) -> &'static str {
        match self {
            Self::DuplicateOccurrence => "reference completion has duplicate occurrence identities",
            Self::MissingParent => "reference completion lost an immediate parent",
            Self::CyclicAncestry => "reference completion has cyclic ancestry",
            Self::UndisposedParent => {
                "reference completion does not dispose an intermediate parent"
            }
            Self::SurvivingChild => "reference completion leaves a child of a closing parent live",
        }
    }
}

/// Check complete local ancestry and cohorts before committing result transfer.
/// Every local intermediate parent needs real disposal on the selected path.
/// References written into caller storage also leave this frame, and their
/// complete ancestry is checked even when no explicit result depends on them.
/// Neither home retirement nor another edge's cleanup satisfies a local debt.
pub fn validate_reference_result_completion<Identity: PartialEq>(
    occurrences: &[ReferenceCompletionOccurrence<'_, Identity>],
) -> Result<(), ReferenceCompletionError> {
    for (position, occurrence) in occurrences.iter().enumerate() {
        if occurrences[..position]
            .iter()
            .any(|previous| previous.identity == occurrence.identity)
        {
            return Err(ReferenceCompletionError::DuplicateOccurrence);
        }
    }
    let mut visited = vec![0u8; occurrences.len()];
    let mut closing = vec![false; occurrences.len()];
    for (position, occurrence) in occurrences
        .iter()
        .enumerate()
        .filter(|(_, occurrence)| occurrence.returned || occurrence.in_caller_storage)
    {
        if occurrence.disposed {
            return Err(ReferenceCompletionError::UndisposedParent);
        }
        let mut pending = vec![(position, false)];
        while let Some((position, completed)) = pending.pop() {
            if completed {
                visited[position] = 2;
                continue;
            }
            match visited[position] {
                1 => return Err(ReferenceCompletionError::CyclicAncestry),
                2 => continue,
                _ => {}
            }
            visited[position] = 1;
            pending.push((position, true));
            for parent in occurrences[position].parents.iter().rev() {
                let parent_position = occurrences
                    .iter()
                    .position(|candidate| candidate.identity == *parent)
                    .ok_or(ReferenceCompletionError::MissingParent)?;
                let parent = &occurrences[parent_position];
                if !parent.in_caller_storage && (!parent.disposed || parent.returned) {
                    return Err(ReferenceCompletionError::UndisposedParent);
                }
                closing[parent_position] = !parent.in_caller_storage;
                pending.push((parent_position, false));
            }
        }
    }
    for (position, occurrence) in occurrences.iter().enumerate() {
        if !closing[position] {
            continue;
        }
        if occurrences.iter().any(|child| {
            child.parents.contains(&occurrence.identity)
                && !child.returned
                && !child.in_caller_storage
                && !child.disposed
        }) {
            return Err(ReferenceCompletionError::SurvivingChild);
        }
    }
    Ok(())
}

/// Match descriptor payload disposal, never backing disposal or home retirement.
pub fn cleanup_disposes_reference(
    actions: &[TerminalAffineCleanupAction],
    carrier: PlaceId,
    path: &[StructuralPathSegment],
) -> bool {
    if path.contains(&StructuralPathSegment::Referent) {
        return false;
    }
    actions.iter().any(|action| match action {
        TerminalAffineCleanupAction::DiscardRoot(place) => *place == carrier,
        TerminalAffineCleanupAction::DiscardResidual(discard) => {
            discard.place == carrier
                && path.starts_with(&discard.path)
                && !discard.path.contains(&StructuralPathSegment::Referent)
        }
        TerminalAffineCleanupAction::InvokeNominal(_)
        | TerminalAffineCleanupAction::RetireHome(_) => false,
    })
}
