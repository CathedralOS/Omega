//! Object layout, decided before any bytes are emitted: the total text size,
//! the symbol capacity to reserve, and the section plan they produce.

use crate::object_artifact::ObjectError;
use crate::object_artifact::private_functions::ValidatedPrivateFunction;
use machine_code::{CompilerPrivateMachineCodeFunction, MachineCodePlan};
use object_file::{ObjectPlan, SectionKind, SectionPlan};

/// The sized object plan and the figures the emission phases size their
/// buffers from.
pub(super) struct ObjectLayout {
    pub(super) object: ObjectPlan,
    pub(super) text_size: usize,
    pub(super) foreign_call_count: usize,
}

/// Size the object: validated function text plus private functions, and one
/// symbol per function and foreign call.
pub(super) fn plan_object_layout(
    plan: &MachineCodePlan,
    private_functions: &[CompilerPrivateMachineCodeFunction],
    validated_private_functions: &[ValidatedPrivateFunction<'_>],
    mut text_size: usize,
) -> Result<ObjectLayout, ObjectError> {
    for private in validated_private_functions {
        text_size = text_size
            .checked_add(private.machine.function.bytes.len())
            .ok_or(ObjectError::TextSizeOverflow)?;
    }

    let foreign_call_count = plan
        .functions
        .iter()
        .map(|function| function.foreign_calls.len())
        .sum::<usize>();
    let symbol_capacity = plan
        .functions
        .len()
        .saturating_add(private_functions.len())
        .saturating_add(foreign_call_count);
    let mut object = if private_functions.is_empty() {
        ObjectPlan::with_capacity(plan.target, 1, symbol_capacity)
    } else {
        ObjectPlan::with_capacities(plan.target, 1, symbol_capacity, private_functions.len())
    };
    object.layout.sections.insert(SectionPlan {
        kind: SectionKind::Text,
        size: text_size,
        alignment: 16,
    });
    Ok(ObjectLayout {
        object,
        text_size,
        foreign_call_count,
    })
}
