//! Object layout, decided before any bytes are emitted: the total text size,
//! the dynamic `.data` size, the symbol capacity to reserve, and the section
//! plan they produce.

use super::same_dynamic_table_application;
use crate::image_emission::object_artifact::ObjectError;
use crate::image_emission::object_artifact::private_functions::ValidatedPrivateFunction;
use crate::image_emission::object_artifact::replay::dynamic::forwarded_descriptor::ValidatedForwardedDynamicApplication;
use crate::object_file::{ObjectPlan, SectionKind, SectionPlan};
use post_allocation_machine_to_selected_form_encoding::machine_code::{
    CompilerPrivateMachineCodeFunction, MachineCodePlan,
};

/// The sized object plan and the figures the emission phases size their
/// buffers from.
pub(super) struct ObjectLayout {
    pub(super) object: ObjectPlan,
    pub(super) dynamic_applications: Vec<terminal_psi::ClosedConformanceApplication>,
    pub(super) text_size: usize,
    pub(super) dynamic_data_size: usize,
    pub(super) foreign_call_count: usize,
}

/// Size the object: validated function text plus private functions and
/// forwarded adapters, the dynamic tables' data, and one symbol per function,
/// foreign call, table and adapter.
pub(super) fn plan_object_layout(
    plan: &MachineCodePlan,
    private_functions: &[CompilerPrivateMachineCodeFunction],
    validated_private_functions: &[ValidatedPrivateFunction<'_>],
    forwarded_dynamic_applications: &[ValidatedForwardedDynamicApplication],
    mut text_size: usize,
) -> Result<ObjectLayout, ObjectError> {
    for private in validated_private_functions {
        text_size = text_size
            .checked_add(private.machine.function.bytes.len())
            .ok_or(ObjectError::TextSizeOverflow)?;
    }
    for application in forwarded_dynamic_applications {
        for adapter in &application.adapters {
            text_size = text_size
                .checked_add(adapter.bytes.len())
                .ok_or(ObjectError::TextSizeOverflow)?;
        }
    }

    let dynamic_applications = collect_dynamic_applications(plan)?;

    let foreign_call_count = plan
        .functions
        .iter()
        .map(|function| function.foreign_calls.len())
        .sum::<usize>();
    let symbol_capacity = plan
        .functions
        .len()
        .saturating_add(private_functions.len())
        .saturating_add(foreign_call_count)
        .saturating_add(dynamic_applications.len())
        .saturating_add(forwarded_dynamic_applications.len())
        .saturating_add(
            forwarded_dynamic_applications
                .iter()
                .map(|application| application.adapters.len())
                .sum::<usize>(),
        );
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
    let dynamic_data_size =
        dynamic_data_section_size(&dynamic_applications, forwarded_dynamic_applications)?;
    if dynamic_data_size != 0 {
        object.layout.sections.insert(SectionPlan {
            kind: SectionKind::Data,
            size: dynamic_data_size,
            alignment: 8,
        });
    }
    Ok(ObjectLayout {
        object,
        dynamic_applications,
        text_size,
        dynamic_data_size,
        foreign_call_count,
    })
}

/// Collect each distinct dynamic-conformance application addressed by a
/// direct or stored dynamic call, rejecting commitment collisions.
fn collect_dynamic_applications(
    plan: &MachineCodePlan,
) -> Result<Vec<terminal_psi::ClosedConformanceApplication>, ObjectError> {
    let mut dynamic_applications = Vec::<terminal_psi::ClosedConformanceApplication>::new();
    for application in plan
        .functions
        .iter()
        .flat_map(|function| &function.dynamic_calls)
        .map(|call| &call.dynamic_dispatch.application)
        .chain(
            plan.functions
                .iter()
                .flat_map(|function| &function.stored_dynamic_calls)
                .map(|call| &call.establishment.stored.application),
        )
    {
        if let Some(existing) = dynamic_applications
            .iter()
            .find(|existing| existing.commitment == application.commitment)
        {
            if !same_dynamic_table_application(existing, application) {
                return Err(ObjectError::DynamicConformanceCommitmentCollision);
            }
        } else {
            dynamic_applications.push(application.clone());
        }
    }
    Ok(dynamic_applications)
}

/// Size of the `.data` section holding the dynamic-conformance and forwarded
/// descriptor tables, eight bytes per slot.
fn dynamic_data_section_size(
    dynamic_applications: &[terminal_psi::ClosedConformanceApplication],
    forwarded_dynamic_applications: &[ValidatedForwardedDynamicApplication],
) -> Result<usize, ObjectError> {
    dynamic_applications
        .iter()
        .try_fold(0usize, |size, application| {
            application
                .rows
                .len()
                .checked_mul(8)
                .and_then(|bytes| size.checked_add(bytes))
                .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)
        })?
        .checked_add(forwarded_dynamic_applications.iter().try_fold(
            0usize,
            |size, application| {
                application
                    .adapters
                    .len()
                    .checked_mul(8)
                    .and_then(|bytes| size.checked_add(bytes))
                    .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)
            },
        )?)
        .ok_or(ObjectError::DynamicConformanceDataSizeOverflow)
}
