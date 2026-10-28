//! Native geometry for alternative task states and parked call arguments.
use super::{identity_text, normalized_id, plan_diagnostic};
use crate::*;
use diagnostics::Diagnostic;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::*;

pub(super) struct TaskStorage {
    shapes: BTreeMap<semantic_vocabulary::StructuralTypeId, calling_conventions::ValueShape>,
    pointer: (u64, u64),
}
impl TaskStorage {
    pub(super) fn new(
        demands: &TerminalTaskActivationDemands,
        profile: target::TargetProfile,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut declarations = Cow::Borrowed(demands.structural_types.as_slice());
        if !demands.target_layout_applications.is_empty() {
            for application in &demands.target_layout_applications {
                let rows = application
                    .realizations
                    .iter()
                    .filter(|row| row.target == profile.target_name())
                    .collect::<Vec<_>>();
                let [row] = rows.as_slice() else {
                    return Err(vec![Diagnostic::error(
                        "task layout requires exactly one selected target realization",
                    )]);
                };
                let declaration = declarations
                    .to_mut()
                    .iter_mut()
                    .find(|declaration| declaration.id == application.record)
                    .ok_or_else(|| vec![Diagnostic::error("task target layout has no record")])?;
                let StructuralTypeShape::Record { storage, .. } = &mut declaration.shape else {
                    return Err(vec![Diagnostic::error(
                        "task target layout does not select a record",
                    )]);
                };
                if *storage != StructuralRecordStorage::TargetLayout(application.id) {
                    return Err(vec![Diagnostic::error(
                        "task layout application does not match its record",
                    )]);
                }
                *storage = StructuralRecordStorage::Authored(row.storage);
            }
        }
        let catalog = declarations.iter().map(|row| (row.id, row)).collect();
        let mut shapes = BTreeMap::new();
        let mut active = BTreeSet::new();
        for demand in &demands.demands {
            let signature = demand
                .signature_arguments
                .iter()
                .chain([&demand.signature_outcome])
                .map(|value| value.value_type);
            let frames = demand
                .frames
                .iter()
                .flat_map(|frame| &frame.states)
                .flat_map(|state| {
                    state
                        .resident
                        .iter()
                        .chain(state.crossing_arguments.iter().flatten())
                })
                .map(|value| value.value_type);
            for value in signature.chain(frames) {
                if let TerminalTaskValueType::Structural(identity) = value {
                    calling_conventions::terminal_layout::structural_shape(
                        identity,
                        &catalog,
                        &mut shapes,
                        &mut active,
                    )
                    .map_err(|error| {
                        vec![Diagnostic::error(format!(
                            "task storage geometry: {error:?}"
                        ))]
                    })?;
                }
            }
        }
        let target = profile.native_target();
        Ok(Self {
            shapes,
            pointer: (target.pointer_size as u64, target.pointer_alignment as u64),
        })
    }

    pub(super) fn value(
        &self,
        value: TerminalTaskValueType,
    ) -> Result<(u64, u64), Vec<Diagnostic>> {
        let shape = match value {
            TerminalTaskValueType::Unit => return Ok((0, 1)),
            TerminalTaskValueType::Scalar(scalar) => {
                calling_conventions::terminal_layout::scalar_shape(scalar)
            }
            TerminalTaskValueType::Structural(identity) => {
                *self.shapes.get(&identity).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "task value is absent from its realized storage catalog",
                    )]
                })?
            }
        };
        Ok((u64::from(shape.byte_size), u64::from(shape.alignment)))
    }

    pub(super) fn frame(
        &self,
        frame: &TerminalTaskFrame,
        identities: &TerminalIdentityTable,
    ) -> Result<ValidatedTaskStackFrameSummary, Vec<Diagnostic>> {
        let identity = normalized_id(frame.identity, TaskStackFrameId::from_normalized_identity)?;
        let mut local_bytes = align(self.pointer.0, self.pointer.1)?;
        let mut local_alignment = self.pointer.1;
        for state in &frame.states {
            let mut size = align(self.pointer.0, self.pointer.1)?;
            let mut alignment = self.pointer.1;
            for value in &state.resident {
                append(&mut size, &mut alignment, self.value(value.value_type)?)?;
            }
            let mut state_peak = align(size, alignment)?;
            let mut state_alignment = alignment;
            for arguments in &state.crossing_arguments {
                let mut crossing_size = size;
                let mut crossing_alignment = alignment;
                for argument in arguments {
                    append(
                        &mut crossing_size,
                        &mut crossing_alignment,
                        self.value(argument.value_type)?,
                    )?;
                }
                state_peak = state_peak.max(align(crossing_size, crossing_alignment)?);
                state_alignment = state_alignment.max(crossing_alignment);
            }
            local_bytes = local_bytes.max(state_peak);
            local_alignment = local_alignment.max(state_alignment);
        }
        // This is occupied extent, not an array stride: the largest extent
        // need not be a multiple of the strongest alternative's base alignment.
        // stack_composition::compose_frame pads when placing a live child.
        let calls = frame
            .calls
            .iter()
            .map(|call| {
                normalized_id(call.callee, TaskStackFrameId::from_normalized_identity)
                    .map(|callee| StackCallContribution::Checked { callee })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let unresolved_calls = frame
            .unresolved_calls
            .iter()
            .map(|site| {
                Ok(UnresolvedCallSite {
                    frame: identity,
                    state: identity_text(identities, site.state)?.to_owned(),
                    statement_index: site.statement_index as usize,
                    call_ordinal: site.call_ordinal as usize,
                    kind: match site.kind {
                        TerminalTaskUnresolvedCallKind::UnresolvedTarget => {
                            UnresolvedCallKind::UnresolvedTarget
                        }
                        TerminalTaskUnresolvedCallKind::NonCheckedSupply => {
                            UnresolvedCallKind::NonCheckedSupply
                        }
                    },
                })
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
        let validation = task_stack_frame_validation_identity(
            identity,
            local_bytes,
            local_alignment,
            &calls,
            &unresolved_calls,
        );
        validate_task_stack_frame_summary(TaskStackFrameSummary {
            frame: identity,
            local_bytes,
            alignment: local_alignment,
            validation,
            calls,
            unresolved_calls,
        })
        .map_err(plan_diagnostic)
    }
}
fn append(size: &mut u64, alignment: &mut u64, value: (u64, u64)) -> Result<(), Vec<Diagnostic>> {
    *size = align(*size, value.1)?
        .checked_add(value.0)
        .ok_or_else(|| vec![Diagnostic::error("task WCSU frame layout size overflow")])?;
    *alignment = (*alignment).max(value.1);
    Ok(())
}
fn align(size: u64, alignment: u64) -> Result<u64, Vec<Diagnostic>> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(vec![Diagnostic::error(
            "task WCSU frame has invalid alignment",
        )]);
    }
    size.checked_add(alignment - 1)
        .map(|value| value / alignment * alignment)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "task WCSU frame layout alignment overflow",
            )]
        })
}
