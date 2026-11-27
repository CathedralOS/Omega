//! Check the source register containing each scalar assembly result against its calling plan.
mod versions;

use calling_conventions::{
    CallSignature, CallingPolicy, EntryControl, ValueLocation, ValueShape, evaluate_call_plan,
};
use terminal_psi::{
    AssemblyExitConstraint, TerminalMachine, TerminalMachineResult, TerminalModule,
};

pub(super) fn bind(
    profile: target::TargetProfile,
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Result<Option<target_bound_program::AssemblyCallingPlan>, String> {
    let mut returns = machine
        .blocks
        .iter()
        .filter_map(|block| match block.assembly_exit {
            Some(AssemblyExitConstraint::ScalarReturn(view)) => Some(view),
            _ => None,
        });
    let Some(view) = returns.next() else {
        return Ok(None);
    };
    if returns.any(|other| other != view) {
        return Err("return paths disagree on the scalar result register".into());
    }
    if module
        .interfaces
        .provider_candidates
        .iter()
        .any(|row| row.candidate == machine.id)
    {
        return Err(
            "an assembly provider return needs the boundary's selected calling plan".into(),
        );
    }
    if !machine.structural_parameters.is_empty() || !machine.entry_claims.is_empty() {
        return Err("this assembly return requires a fixed scalar internal calling plan".into());
    }
    let TerminalMachineResult::Scalar(result) = &machine.result else {
        return Err("assembly register return requires a scalar result".into());
    };
    let parameters = machine
        .parameters
        .iter()
        .map(|param| ValueShape::for_scalar(param.scalar_type))
        .collect::<Option<Vec<_>>>()
        .ok_or("assembly return has a non-scalar native parameter")?;
    let shape =
        ValueShape::for_scalar(result.scalar_type).ok_or("missing assembly result shape")?;
    let target = profile.native_target();
    let plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters,
            result: Some(shape),
        },
    )
    .map_err(|error| format!("cannot evaluate the scalar entry plan: {error:?}"))?;
    let placement = plan.result.as_ref().ok_or("calling plan has no result")?;
    let [
        ValueLocation::Register {
            register,
            value_byte_offset: 0,
            byte_size,
        },
    ] = placement.locations.as_slice()
    else {
        return Err("scalar assembly return needs one direct result register".into());
    };
    if plan.entry_control != EntryControl::CallReturn
        || !plan.ordinary_clobbers.contains(*register)
        || *byte_size != shape.byte_size
        || placement.shape != shape
    {
        return Err("calling plan does not admit writing this entire scalar result".into());
    }
    let env = register_environment::baseline_target_register_environment(target)
        .map_err(|error| format!("invalid register environment: {error:?}"))?;
    let physical = env.physical().model();
    let result_home = env
        .fixed_register_view(*register)
        .and_then(|id| physical.view(id))
        .ok_or("calling plan has no physical result view")?;
    let authored = physical
        .view_named(view.name())
        .ok_or("assembly return has no view on this target")?;
    // A write to eax or w0 also zeroes the high bits of its full register.
    // The entire write must stay in the result register that the plan lets the callee change.
    if u32::from(authored.bits) != shape.byte_size * 8
        || authored.write_units != result_home.units
        || !result_home.units.starts_with(&authored.units)
    {
        return Err(format!(
            "return value is in {}, but the calling plan requires {} ({} bytes)",
            view.name(),
            result_home.name,
            byte_size
        ));
    }
    Ok(Some(target_bound_program::AssemblyCallingPlan {
        machine: machine.id,
        result: view,
        assembly_register_versions: versions::derive(machine, view)?,
        call: plan,
    }))
}
