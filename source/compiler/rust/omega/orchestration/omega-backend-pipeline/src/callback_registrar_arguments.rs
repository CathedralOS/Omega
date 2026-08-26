use omega_abstract_operations::{AbstractBoundarySummary, AbstractHostCallSourceSite};
use omega_backend_plan::{
    BoundNominalCallbackPlacement, CallbackPrivateRelocationDemand,
    CallbackRegistrarArgumentBinding, CallbackThunkPlan,
    replay_callback_registrar_argument_bindings,
};
use omega_calling_conventions::{NativeParameterId, NativePlace};
use omega_platform_interface::HostCallPlan;
use psi_arena::{Handle, HandleSpan};
use psi_checked_trees::NominalMachineUseSite;
use psi_diagnostics::Diagnostic;
use std::sync::Arc;

pub(super) fn plan_callback_registrar_arguments(
    placements: &[BoundNominalCallbackPlacement],
    thunks: &[CallbackThunkPlan],
    demands: &[CallbackPrivateRelocationDemand],
    host_calls: &HostCallPlan,
    boundaries: &AbstractBoundarySummary,
) -> Result<Arc<[CallbackRegistrarArgumentBinding]>, Diagnostic> {
    let mut bindings = Vec::with_capacity(demands.len());
    for (demand_index, demand) in demands.iter().enumerate() {
        let matching_occurrences = boundaries
            .host_calls
            .iter()
            .filter(|(_, occurrence)| {
                sites_match(occurrence.source_site, demand.placement_identity.site)
                    && occurrence.registration_operation
                        == demand.placement_identity.registration_operation
            })
            .collect::<Vec<_>>();
        let [(host_call, occurrence)] = matching_occurrences.as_slice() else {
            return Err(Diagnostic::error(format!(
                "callback private relocation demand {demand_index} resolves to {} registrar host-call occurrences; exactly one is required",
                matching_occurrences.len()
            )));
        };
        let root_parameter = root_parameter(&demand.destination).ok_or_else(|| {
            Diagnostic::error(format!(
                "callback private relocation demand {demand_index} retained an empty nominal field path"
            ))
        })?;
        let arguments = boundaries
            .host_call_arguments
            .span(occurrence.arguments)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "callback registrar occurrence {demand_index} retained an invalid native-argument span"
                ))
            })?;
        let matching_arguments = arguments
            .iter()
            .enumerate()
            .filter(|(_, argument)| argument.native_parameter == Some(root_parameter))
            .collect::<Vec<_>>();
        let [(argument_offset, _)] = matching_arguments.as_slice() else {
            return Err(Diagnostic::error(format!(
                "callback private relocation demand {demand_index} resolves to {} registrar native arguments; exactly one is required",
                matching_arguments.len()
            )));
        };
        let native_argument =
            span_handle(occurrence.arguments, *argument_offset).ok_or_else(|| {
                Diagnostic::error(format!(
                    "callback registrar occurrence {demand_index} native-argument handle overflowed"
                ))
            })?;
        bindings.push(CallbackRegistrarArgumentBinding {
            demand_index,
            demand: demand.clone(),
            host_call: *host_call,
            native_argument,
        });
    }

    replay_callback_registrar_argument_bindings(
        placements, thunks, demands, host_calls, boundaries, &bindings,
    )
    .map_err(|error| {
        Diagnostic::error(format!(
            "callback registrar argument binding replay failed: {error}"
        ))
    })?;
    Ok(Arc::from(bindings))
}

fn sites_match(left: AbstractHostCallSourceSite, right: NominalMachineUseSite) -> bool {
    matches!(
        (left, right),
        (
            AbstractHostCallSourceSite::Statement(left),
            NominalMachineUseSite::Statement(right)
        ) if left == right
    ) || matches!(
        (left, right),
        (
            AbstractHostCallSourceSite::Expression(left),
            NominalMachineUseSite::Expression(right)
        ) if left == right
    )
}

fn root_parameter(destination: &NativePlace) -> Option<NativeParameterId> {
    match destination {
        NativePlace::Parameter(parameter) => Some(*parameter),
        NativePlace::Field {
            parameter,
            field_path,
            ..
        } if !field_path.is_empty() => Some(*parameter),
        NativePlace::Field { .. } => None,
    }
}

fn span_handle<T>(span: HandleSpan<T>, offset: usize) -> Option<Handle<T>> {
    let offset = u32::try_from(offset).ok()?;
    if offset >= span.count() {
        return None;
    }
    Some(Handle::from_parts(
        span.start().arena_index().checked_add(offset)?,
        span.start().generation(),
    ))
}

#[cfg(test)]
mod tests;
