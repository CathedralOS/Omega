//! Replay-local indexed dependency reconstruction.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    AbstractSpillAccessConstraintError, AbstractSpillAccessDependency,
    AbstractSpillAccessDependencyReason, AbstractSpillAccessKind, AbstractSpillAccessPlacement,
    AbstractSpillMemoryEffect, FunctionAbstractSpillMemoryEffects,
};

pub(super) fn reconstruct(
    function: usize,
    source: &FunctionAbstractSpillMemoryEffects,
    placements: &[AbstractSpillAccessPlacement],
) -> Result<Vec<AbstractSpillAccessDependency>, AbstractSpillAccessConstraintError> {
    let by_pseudo = placements
        .iter()
        .map(|row| (row.pseudo, row))
        .collect::<BTreeMap<_, _>>();
    let mut write_by_storage = BTreeMap::new();
    for placement in placements {
        if placement.kind == AbstractSpillAccessKind::Write
            && write_by_storage
                .insert(placement.storage, placement)
                .is_some()
        {
            return Err(AbstractSpillAccessConstraintError::DuplicateWrite {
                function,
                storage: placement.storage,
            });
        }
    }
    let mut dependencies = BTreeSet::new();
    for placement in placements {
        if placement.kind == AbstractSpillAccessKind::Read {
            let before = write_by_storage.get(&placement.storage).copied().ok_or(
                AbstractSpillAccessConstraintError::MissingWrite {
                    function,
                    storage: placement.storage,
                },
            )?;
            require_order(function, before, placement)?;
            dependencies.insert(AbstractSpillAccessDependency {
                before: before.pseudo,
                after: placement.pseudo,
                reason: AbstractSpillAccessDependencyReason::StoredValue {
                    storage: placement.storage,
                },
            });
        }
    }
    for effect in &source.effects {
        if let AbstractSpillMemoryEffect::Write {
            pseudo,
            before_reload: Some(after),
            ..
        } = effect
        {
            let before = by_pseudo.get(pseudo).copied().ok_or(
                AbstractSpillAccessConstraintError::InvalidBeforeReload {
                    function,
                    pseudo: *pseudo,
                },
            )?;
            let after = by_pseudo.get(after).copied().ok_or(
                AbstractSpillAccessConstraintError::InvalidBeforeReload {
                    function,
                    pseudo: *pseudo,
                },
            )?;
            if before.kind != AbstractSpillAccessKind::Write
                || after.kind != AbstractSpillAccessKind::Read
            {
                return Err(AbstractSpillAccessConstraintError::InvalidBeforeReload {
                    function,
                    pseudo: *pseudo,
                });
            }
            require_order(function, before, after)?;
            dependencies.insert(AbstractSpillAccessDependency {
                before: *pseudo,
                after: after.pseudo,
                reason: AbstractSpillAccessDependencyReason::DeclaredBeforeReload,
            });
        }
    }
    for (index, before) in placements.iter().enumerate() {
        for after in &placements[index + 1..] {
            if before.block == after.block {
                if let Some((spill_area_offset, size_bytes)) = intersect(before, after)? {
                    dependencies.insert(AbstractSpillAccessDependency {
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
    Ok(dependencies.into_iter().collect())
}

fn require_order(
    function: usize,
    before: &AbstractSpillAccessPlacement,
    after: &AbstractSpillAccessPlacement,
) -> Result<(), AbstractSpillAccessConstraintError> {
    if before.block != after.block || before.block_ordinal >= after.block_ordinal {
        Err(AbstractSpillAccessConstraintError::InvalidAccessOrder { function })
    } else {
        Ok(())
    }
}

fn intersect(
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
    Ok((start < end).then(|| (start, end - start)))
}
