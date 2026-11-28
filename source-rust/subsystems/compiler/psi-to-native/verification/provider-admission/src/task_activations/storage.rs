use super::stack::{
    TaskStackFrameSummary, UnresolvedCallSite, ValidatedTaskStackFrameSummary,
    task_stack_frame_validation_identity, validate_task_stack_frame_summary,
};
use super::{identity_text, nonzero, plan_diagnostic};
use calling_conventions::terminal_layout::StructuralLayoutResolver;
use diagnostics::Diagnostic;
use terminal_psi::*;

pub(super) struct TaskStorage<'a> {
    layouts: StructuralLayoutResolver<'a>,
    pointer: (u64, u64),
}

impl<'a> TaskStorage<'a> {
    pub(super) fn new(
        declarations: impl Iterator<Item = &'a StructuralTypeDeclaration>,
        owned_sequence_bounds: &'a [(semantic_vocabulary::StructuralTypeId, u64)],
        profile: target::TargetProfile,
    ) -> Result<Self, Vec<Diagnostic>> {
        let layouts =
            StructuralLayoutResolver::with_owned_sequence_bounds(declarations, owned_sequence_bounds)
                .map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "task storage declarations: {error:?}"
                    ))]
                })?;
        let target = profile.native_target();
        Ok(Self {
            layouts,
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
                calling_conventions::terminal_layout::scalar_shape(scalar).map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "task scalar storage geometry: {error:?}"
                    ))]
                })?
            }
            TerminalTaskValueType::Structural(identity) => {
                self.layouts.shape(identity).map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "task storage geometry: {error:?}"
                    ))]
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
        let identity = nonzero(frame.identity, "task-stack-frame")?;
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
        let calls = frame
            .calls
            .iter()
            .map(|call| nonzero(call.callee, "task-stack-frame"))
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
                    kind: site.kind,
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
