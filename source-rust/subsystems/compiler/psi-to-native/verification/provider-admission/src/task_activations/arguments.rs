use super::{TaskPlanDiagnostic, validate_alignment};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TaskArgumentExtent {
    offset: u64,
    bytes: u64,
    alignment: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TaskArgumentLayout {
    bytes: u64, // Total bytes of the marshalled image, including interior and trailing padding.
    alignment: u64, // Widest field alignment, or 1 for an empty bundle.
    fields: Vec<TaskArgumentExtent>, // Per-argument placement in source parameter order.
}

impl TaskArgumentLayout {
    pub(super) fn new(
        arguments: impl Iterator<Item = (u64, u64)>,
    ) -> Result<Self, TaskPlanDiagnostic> {
        let (fields, bytes, alignment) = pack_task_argument_fields(arguments)?;
        Ok(Self {
            bytes,
            alignment,
            fields,
        })
    }

    pub(super) fn is_canonical(&self) -> Result<bool, TaskPlanDiagnostic> {
        Ok(canonical_argument_layout(self)? == *self)
    }
}

const MAX_TASK_ARGUMENT_ALIGNMENT: u64 = 1 << 16;

fn pack_task_argument_fields(
    arguments: impl Iterator<Item = (u64, u64)>,
) -> Result<(Vec<TaskArgumentExtent>, u64, u64), TaskPlanDiagnostic> {
    let mut fields = Vec::new();
    let mut cursor = 0u64;
    let mut image_alignment = 1u64;
    for (index, (bytes, alignment)) in arguments.enumerate() {
        validate_alignment(alignment, &format!("task argument field {index}"))?;
        if alignment > MAX_TASK_ARGUMENT_ALIGNMENT {
            return Err(TaskPlanDiagnostic(format!(
                "task argument field {index} alignment {alignment} exceeds the marshalling \
                 bound {MAX_TASK_ARGUMENT_ALIGNMENT}"
            )));
        }
        image_alignment = image_alignment.max(alignment);
        let offset = cursor
            .checked_add(alignment - 1)
            .map(|padded| padded & !(alignment - 1))
            .ok_or_else(|| {
                TaskPlanDiagnostic(format!(
                    "task argument field {index} offset overflows the marshalling image"
                ))
            })?;
        fields.push(TaskArgumentExtent {
            offset,
            bytes,
            alignment,
        });
        cursor = offset.checked_add(bytes).ok_or_else(|| {
            TaskPlanDiagnostic(format!(
                "task argument field {index} extent overflows the marshalling image"
            ))
        })?;
    }
    let bytes = cursor
        .checked_add(image_alignment - 1)
        .map(|padded| padded & !(image_alignment - 1))
        .ok_or_else(|| TaskPlanDiagnostic("task argument image extent overflows".into()))?;
    Ok((fields, bytes, image_alignment))
}

fn canonical_argument_layout(
    layout: &TaskArgumentLayout,
) -> Result<TaskArgumentLayout, TaskPlanDiagnostic> {
    let (fields, bytes, alignment) = pack_task_argument_fields(
        layout
            .fields
            .iter()
            .map(|field| (field.bytes, field.alignment)),
    )?;
    Ok(TaskArgumentLayout {
        bytes,
        alignment,
        fields,
    })
}
