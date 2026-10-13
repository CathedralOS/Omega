//! A normal result transfers its actual loans before frame-local disposal.
//! The interface may authenticate origins, but cannot erase missing ancestry
//! or a surviving shared-cohort member. Consumers reconstruct occurrences
//! independently and supply only the selected exit's actual dispositions.

use semantic_vocabulary::PlaceId;
use terminal_psi::{StructuralPathSegment, TerminalAffineCleanupAction};

/// Invocation-local custody graph, not another serialized loan certificate.
pub struct ReferenceCompletionOccurrence<'a, Identity> {
    pub identity: &'a Identity,
    pub parents: Vec<&'a Identity>,
    pub returned: bool,
    pub disposed: bool,
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
/// Every intermediate parent must have a real disposal in this same exit;
/// neither home retirement nor another edge's cleanup satisfies that debt.
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
        .filter(|(_, occurrence)| occurrence.returned)
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
                if !occurrences[parent_position].disposed || occurrences[parent_position].returned {
                    return Err(ReferenceCompletionError::UndisposedParent);
                }
                closing[parent_position] = true;
                pending.push((parent_position, false));
            }
        }
    }
    for (position, occurrence) in occurrences.iter().enumerate() {
        if !closing[position] {
            continue;
        }
        if occurrences.iter().any(|child| {
            child.parents.contains(&occurrence.identity) && !child.returned && !child.disposed
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
