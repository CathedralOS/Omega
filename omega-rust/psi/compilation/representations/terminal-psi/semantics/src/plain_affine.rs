//! Plain affine records retain one owned value until it moves or is discarded.
//! Reconstruct that custody from actual operations and control-flow edges. The
//! source producer materializes the required normal cleanup; checking derives
//! the same obligations independently of the supplied cleanup lists.

mod context;
mod effects;
mod flow;

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{BlockId, EdgeId, OperationId, PlaceId};
use terminal_psi::{TerminalAffineCleanupAction as Action, TerminalMachine};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlainAffineSite {
    Block(BlockId),
    Operation(OperationId),
    Edge(EdgeId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlainAffineUse {
    pub site: PlainAffineSite,
    pub place: PlaceId,
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlainAffineFailure {
    Unsupported(&'static str),
    Malformed(&'static str),
    InvalidUse(PlainAffineUse),
    InvalidType(semantic_vocabulary::StructuralTypeId, &'static str),
}

/// The remaining custody on every normal edge, in reverse establishment order.
/// A no-normal call has no cleanup edge: the caller's remaining storage stays
/// suspended until the called computation ends its domain or diverges.
pub struct PlainAffinePlan {
    cleanup: BTreeMap<EdgeId, Vec<Action>>,
    owned: BTreeSet<PlaceId>,
    invalid: Option<PlainAffineUse>,
}

impl PlainAffinePlan {
    /// Emit implicit source cleanup after reconstructing the complete flow.
    /// Invalid source is left intact for the checking owner to reject with its
    /// actual use site. Existing actions must be part of the derived schedule.
    pub fn materialize(&self, machine: &mut TerminalMachine) -> Result<(), PlainAffineFailure> {
        if self.invalid.is_some() || self.owned.is_empty() {
            return Ok(());
        }
        for block in &mut machine.blocks {
            effects::cleanup_mut(&mut block.terminator, |edge, actions| {
                let Some(required) = self.cleanup.get(&edge) else {
                    return Ok(());
                };
                for action in actions
                    .iter()
                    .filter(|action| self.owned.contains(&action.place()))
                {
                    if !required.contains(action) {
                        return Err(PlainAffineFailure::Malformed(
                            "existing cleanup does not dispose a remaining owned value",
                        ));
                    }
                }
                for action in required {
                    if !actions.contains(action) {
                        actions.push(action.clone());
                    }
                }
                // The existing source producer emits only whole discards and
                // home retirements for these plain records. Preserve that one
                // ordered schedule; nominal/partial disposal has another rule.
                if actions
                    .iter()
                    .any(|action| !matches!(action, Action::DiscardRoot(_) | Action::RetireHome(_)))
                {
                    return Err(PlainAffineFailure::Unsupported(
                        "mixed nominal or partial affine cleanup",
                    ));
                }
                // Required actions already carry execution order. Interleave
                // unrelated, unrestricted retirements without changing their
                // relative order or any owned action's relative order.
                let owned = actions
                    .iter()
                    .filter(|action| self.owned.contains(&action.place()))
                    .count();
                if owned != required.len() {
                    return Err(PlainAffineFailure::Malformed(
                        "owned cleanup repeats an action",
                    ));
                }
                let mut next = required.iter();
                for action in actions
                    .iter_mut()
                    .filter(|action| self.owned.contains(&action.place()))
                {
                    *action = next.next().expect("the action counts agree").clone();
                }
                Ok(())
            })?;
        }
        Ok(())
    }

    /// Check the authored/lowered schedule without repairing it. Invalid moves
    /// are source errors; missing or foreign actions are producer defects.
    pub fn verify(&self, machine: &TerminalMachine) -> Result<(), PlainAffineFailure> {
        if machine
            .blocks
            .iter()
            .flat_map(|block| block.terminator.cleanup_rosters())
            .any(|(_, actions)| {
                actions
                    .iter()
                    .any(|action| !matches!(action, Action::DiscardRoot(_) | Action::RetireHome(_)))
            })
        {
            return Err(PlainAffineFailure::Unsupported(
                "nominal or partial affine cleanup",
            ));
        }
        if let Some(invalid) = &self.invalid {
            return Err(PlainAffineFailure::InvalidUse(invalid.clone()));
        }
        if self.owned.is_empty() {
            return Ok(());
        }
        for block in &machine.blocks {
            for (edge, actions) in block.terminator.cleanup_rosters() {
                let actual = actions
                    .iter()
                    .filter(|action| self.owned.contains(&action.place()))
                    .cloned()
                    .collect::<Vec<_>>();
                let Some(expected) = self.cleanup.get(&edge) else {
                    continue;
                };
                if actual.as_slice() != expected.as_slice() {
                    return Err(PlainAffineFailure::Malformed(
                        "normal cleanup differs from the remaining owned frontier",
                    ));
                }
            }
        }
        Ok(())
    }
}

pub fn reconstruct_plain_affine_custody(
    types: &[terminal_psi::StructuralTypeDeclaration],
    machine: &TerminalMachine,
) -> Result<PlainAffinePlan, PlainAffineFailure> {
    flow::reconstruct(types, machine)
}

/// An explicit copy request cannot erase a contained affine or linear value.
pub fn verify_plain_record_copyability(
    types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<(), PlainAffineFailure> {
    for ty in types.iter().filter(|ty| ty.copyable) {
        if ty.owns_linear_claim {
            return Err(PlainAffineFailure::InvalidType(
                ty.id,
                "copy data owns a linear claim",
            ));
        }
        let fields = match &ty.shape {
            terminal_psi::StructuralTypeShape::Record { fields, .. } => {
                fields.iter().collect::<Vec<_>>()
            }
            terminal_psi::StructuralTypeShape::Sum { cases } => {
                cases.iter().flat_map(|case| &case.fields).collect()
            }
            _ => continue,
        };
        for field in fields {
            if let terminal_psi::StructuralFieldType::Structural(child) = &field.field_type {
                if !crate::is_copyable_carrier(types.iter(), *child) {
                    return Err(PlainAffineFailure::InvalidType(
                        ty.id,
                        "copy data contains non-copyable storage",
                    ));
                }
            }
        }
    }
    Ok(())
}
