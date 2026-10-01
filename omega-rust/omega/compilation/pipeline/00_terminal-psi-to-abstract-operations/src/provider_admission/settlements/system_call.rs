//! Join a kernel entry to its exact selected provider and evaluated ABI.
//! A syscall number is payload, never authority or a service-name recognizer.
//! Unlike imports this entry needs no foreign same-stack reserve. Native
//! lowering must independently match the retained plan to the caller's actual
//! arguments and emit its supervisor instruction and declared clobbers.

use calling_conventions::{CallSignature, CallingPolicy, ExternalBindingKind, ExternalBindingRow};
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan};
use target::NativeTarget;
use target_operations::NormalizedSystemCallBinding;

pub(super) fn rejoin_system_call(
    selected_plan: &ProviderPlan,
    external_rows: &[ExternalBindingRow],
    requirement: &str,
    target: NativeTarget,
) -> Result<NormalizedSystemCallBinding, Vec<Diagnostic>> {
    let invalid = |reason| {
        vec![Diagnostic::error(format!(
            "syscall realization for `{requirement}` {reason}"
        ))]
    };
    let profile = target::TargetProfile::from_canonical_target_name(&selected_plan.target)
        .map_err(|error| vec![error])?;
    if profile.native_target() != target {
        return Err(invalid("does not match the selected native target"));
    }
    let policy = CallingPolicy::native_syscall_for_target(target)
        .ok_or_else(|| invalid("has no supported kernel entry policy"))?;
    let mut selected = selected_plan
        .rows
        .iter()
        .filter(|row| row.requirement_identity == requirement);
    let selected_row = selected
        .next()
        .ok_or_else(|| invalid("has no selected row"))?;
    if selected.next().is_some() {
        return Err(invalid("has duplicate selected rows"));
    }
    let ProviderBinding::Syscall { number } = selected_row.binding else {
        return Err(invalid("does not select a syscall binding"));
    };
    if number < 0 {
        return Err(invalid("has a negative syscall number"));
    }
    let mut retained = external_rows
        .iter()
        .filter(|row| row.requirement_identity == requirement);
    let external = retained
        .next()
        .ok_or_else(|| invalid("has no retained external row"))?;
    if retained.next().is_some() {
        return Err(invalid("has duplicate retained external rows"));
    }
    if external.binding != (ExternalBindingKind::Syscall { number })
        || external.target_name != selected_plan.target
        || external.trait_name != selected_plan.schema.trait_name
        || external.method != selected_row.method
        || external.table_type != selected_plan.provider_type
    {
        return Err(invalid("does not rejoin the exact selected external row"));
    }
    let entry = external
        .boundary_entry_plan
        .as_ref()
        .ok_or_else(|| invalid("has no retained calling plan"))?;
    if entry.call.policy != policy {
        return Err(invalid(
            "does not retain the target's syscall calling policy",
        ));
    }
    // This checks the retained physical promise, not the source signature.
    // Consumer lowering separately rejoins each argument/result's semantic
    // shape; neither a canonical plan nor its mere existence establishes that.
    let signature = CallSignature {
        parameters: entry
            .call
            .parameters
            .iter()
            .map(|parameter| parameter.shape)
            .collect(),
        result: entry.call.result.as_ref().map(|result| result.shape),
    };
    // Generic plan validation permits authored calling-policy refinements;
    // a direct kernel entry instead has one target-defined physical ABI.
    // In particular, a structurally valid plan cannot omit kernel clobbers or
    // replace supervisor entry with an ordinary user-space call.
    let expected = calling_conventions::evaluate_call_plan(policy, &signature)
        .map_err(|_| invalid("has an unsupported syscall signature"))?;
    if entry.call != expected {
        return Err(invalid("does not retain the exact kernel ABI"));
    }
    calling_conventions::validate_boundary_entry_plan(entry.clone(), &signature)
        .map_err(|_| invalid("has a noncanonical syscall calling plan"))?;
    Ok(NormalizedSystemCallBinding {
        target,
        number,
        boundary_entry_plan: entry.clone(),
    })
}

#[cfg(test)]
mod tests;
