//! Resolve the live supplying allocation before selecting a storage view's
//! geometry. A projected alias cannot stand in for its enclosing owner.

use crate::{
    errors::{TerminalInterpretError, malformed},
    execution::TerminalExecution,
    values::TerminalStructuralValue,
};
use semantic_vocabulary::{MachineId, PlaceId};
use terminal_psi::StructuralAccess;

pub(crate) struct OriginalAllocation {
    pub(crate) depth: usize,
    pub(crate) machine: MachineId,
    pub(crate) place: PlaceId,
    pub(crate) value: TerminalStructuralValue,
}

impl TerminalExecution<'_> {
    /// Select the shortest enclosing live owned occurrence, independent of
    /// physical representation or the requested view. Equal-depth competing
    /// owners reject; a later geometry check cannot choose a convenient alias.
    pub(crate) fn original_owned_allocation(
        &self,
        source: &TerminalStructuralValue,
    ) -> Result<OriginalAllocation, TerminalInterpretError> {
        let homes = self
            .call_stack
            .iter()
            .enumerate()
            .map(|(depth, frame)| (depth, frame.current_machine, &frame.structural_values))
            .chain(std::iter::once((
                self.call_stack.len(),
                self.current_machine,
                &self.structural_values,
            )));
        let mut selected: Option<OriginalAllocation> = None;
        let mut competing = false;
        for (depth, machine_id, values) in homes {
            let machine = self
                .program
                .machines()
                .get(&machine_id)
                .ok_or_else(malformed)?;
            for (place, allocation) in values {
                if allocation.opaque_identity != source.opaque_identity
                    || !source.path.starts_with(&allocation.path)
                {
                    continue;
                }
                let owned = machine
                    .structural_parameter(*place)
                    .is_some_and(|parameter| {
                        parameter.access == StructuralAccess::Owned
                            && parameter.structural_type == allocation.structural_type
                    })
                    || machine
                        .producing_operation(*place)
                        .is_some_and(|(operation, ty)| {
                            operation.result.structural().is_some_and(|result| {
                                result.structural_type == ty && ty == allocation.structural_type
                            })
                        });
                if !owned {
                    continue;
                }
                if let Some(previous) = &selected {
                    match allocation.path.len().cmp(&previous.value.path.len()) {
                        std::cmp::Ordering::Greater => continue,
                        std::cmp::Ordering::Equal => {
                            competing = true;
                            continue;
                        }
                        std::cmp::Ordering::Less => {}
                    }
                }
                selected = Some(OriginalAllocation {
                    depth,
                    machine: machine_id,
                    place: *place,
                    value: allocation.clone(),
                });
                competing = false;
            }
        }
        if competing {
            return Err(malformed());
        }
        selected.ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
            "storage source has no live supplying allocation",
        ))
    }
}
