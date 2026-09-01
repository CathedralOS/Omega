//! Producer-local data, declared-barrier, and slice-overlap edges.

use crate::{
    AbstractSpillAccessConstraintError, AbstractSpillAccessDependency,
    AbstractSpillAccessDependencyReason, AbstractSpillAccessKind, AbstractSpillAccessPlacement,
    AbstractSpillMemoryEffect, FunctionAbstractSpillMemoryEffects,
};

pub(super) fn derive(
    function: usize,
    source: &FunctionAbstractSpillMemoryEffects,
    placements: &[AbstractSpillAccessPlacement],
) -> Result<Vec<AbstractSpillAccessDependency>, AbstractSpillAccessConstraintError> {
    let mut dependencies = Vec::new();
    for placement in placements {
        if placement.kind != AbstractSpillAccessKind::Read {
            continue;
        }
        let writes = placements
            .iter()
            .filter(|candidate| {
                candidate.kind == AbstractSpillAccessKind::Write
                    && candidate.storage == placement.storage
            })
            .collect::<Vec<_>>();
        let write = match writes.as_slice() {
            [] => {
                return Err(AbstractSpillAccessConstraintError::MissingWrite {
                    function,
                    storage: placement.storage,
                });
            }
            [write] => *write,
            _ => {
                return Err(AbstractSpillAccessConstraintError::DuplicateWrite {
                    function,
                    storage: placement.storage,
                });
            }
        };
        if !ordered_before(write, placement) {
            return Err(AbstractSpillAccessConstraintError::InvalidAccessOrder { function });
        }
        dependencies.push(AbstractSpillAccessDependency {
            before: write.pseudo,
            after: placement.pseudo,
            reason: AbstractSpillAccessDependencyReason::StoredValue {
                storage: placement.storage,
            },
        });
    }
    for effect in &source.effects {
        if let AbstractSpillMemoryEffect::Write {
            pseudo,
            before_reload: Some(after),
            ..
        } = effect
        {
            let before = find(function, placements, *pseudo)?;
            let after = find(function, placements, *after)?;
            if before.kind != AbstractSpillAccessKind::Write
                || after.kind != AbstractSpillAccessKind::Read
                || !ordered_before(before, after)
            {
                return Err(AbstractSpillAccessConstraintError::InvalidBeforeReload {
                    function,
                    pseudo: *pseudo,
                });
            }
            dependencies.push(AbstractSpillAccessDependency {
                before: *pseudo,
                after: after.pseudo,
                reason: AbstractSpillAccessDependencyReason::DeclaredBeforeReload,
            });
        }
    }
    for (index, before) in placements.iter().enumerate() {
        for after in &placements[index + 1..] {
            if before.block == after.block {
                if let Some((spill_area_offset, size_bytes)) = overlap(before, after)? {
                    dependencies.push(AbstractSpillAccessDependency {
                        before: before.pseudo,
                        after: after.pseudo,
                        reason: AbstractSpillAccessDependencyReason::OverlappingAbstractSlice {
                            spill_area_offset,
                            size_bytes,
                        },
                    });
                }
            }
        }
    }
    dependencies.sort();
    Ok(dependencies)
}

fn find(
    function: usize,
    placements: &[AbstractSpillAccessPlacement],
    pseudo: crate::SpillPseudoInstructionId,
) -> Result<&AbstractSpillAccessPlacement, AbstractSpillAccessConstraintError> {
    placements
        .iter()
        .find(|row| row.pseudo == pseudo)
        .ok_or(AbstractSpillAccessConstraintError::InvalidBeforeReload { function, pseudo })
}

fn ordered_before(
    before: &AbstractSpillAccessPlacement,
    after: &AbstractSpillAccessPlacement,
) -> bool {
    before.block == after.block && before.block_ordinal < after.block_ordinal
}

fn overlap(
    left: &AbstractSpillAccessPlacement,
    right: &AbstractSpillAccessPlacement,
) -> Result<Option<(u64, u64)>, AbstractSpillAccessConstraintError> {
    let start = left.spill_area_offset.max(right.spill_area_offset);
    let left_end = left
        .spill_area_offset
        .checked_add(left.size_bytes)
        .ok_or(AbstractSpillAccessConstraintError::WorkOverflow)?;
    let right_end = right
        .spill_area_offset
        .checked_add(right.size_bytes)
        .ok_or(AbstractSpillAccessConstraintError::WorkOverflow)?;
    let end = left_end.min(right_end);
    Ok((start < end).then_some((start, end - start)))
}
