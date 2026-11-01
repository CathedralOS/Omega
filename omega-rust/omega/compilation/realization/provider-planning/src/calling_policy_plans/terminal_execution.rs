//! Run one retained calling policy without a source compiler or source trees.

use super::MaterializedBoundarySignature;
use super::build_time_decoding::decode_boundary_plan_result;
use super::plan_computation::{
    build_boundary_signature, validate_materialized_boundary_plan_result,
};
use super::scalar_completion::complete_policy_scalar_inputs;
use super::terminal_values::{input_contents, result_value};
use calling_conventions::ValidatedBoundaryEntryPlan;
use evaluation_accounting::EvaluationInvocationAccount;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_fuel::{TerminalFuelMeter, TerminalFuelUsage};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInstantiation, TerminalProgram,
    TerminalRuntimeSupplyRequest, TerminalStructuralInputs, TerminalStructuralValue,
};
use terminal_psi::TargetMachineCallable;

/// Explicit source-free inputs of a policy activation. The producer's checked
/// invocation admission precedes this request; neither entry selection nor
/// successful interpretation grants provider, callback or native authority.
pub struct TerminalCallingPolicyInvocation<'input> {
    pub artifact: &'input CanonicalTerminalArtifact,
    pub entry: TargetMachineCallable,
    pub target: target::TargetProfile,
    pub signature: &'input MaterializedBoundarySignature,
    pub account: EvaluationInvocationAccount,
}

/// Execute the selected callable on the supplied signature, decode its actual
/// returned plan, and validate every placement against that same signature.
/// Multiple activations borrow the same immutable code. No source elaboration,
/// policy compilation, unchecked storage copying or alternative engine exists
/// in this operation; envelope-loaded artifacts use the identical route.
pub fn evaluate_terminal_calling_policy(
    invocation: TerminalCallingPolicyInvocation<'_>,
) -> Result<(ValidatedBoundaryEntryPlan, TerminalFuelUsage), String> {
    let TerminalCallingPolicyInvocation {
        artifact,
        entry,
        target,
        signature,
        account,
    } = invocation;
    if signature.native_target() != target.native_target() {
        return Err("calling policy target differs from its materialized signature".into());
    }
    let selected = terminal_installation::admit_target_machine_selections(
        artifact.view(),
        target.target_name(),
    );
    let facts = target
        .layout_target_facts()
        .ok_or("calling policy target has no layout facts")?;
    let layouts = terminal_installation::admit_target_layouts(artifact.view(), &selected, &facts)
        .map_err(|error| format!("calling policy target layouts: {error:?}"))?;
    let providers = terminal_installation::admit_provider_installation(artifact.view(), &[], &[])
        .map_err(|error| format!("calling policy providers: {error:?}"))?;
    let program = TerminalProgram::borrow(artifact.view());
    let installation = program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            x86_flags: None,
            memory_ordering: None,
            installation: Some(&providers),
            host_imports: &[],
            entry: Some(entry),
            target_machines: Some(&selected),
            target_layouts: Some(&layouts),
        })
        .map_err(|error| format!("calling policy selection: {error:?}"))?;
    let requirements = program
        .execution_requirements(Some(&installation))
        .map_err(|error| format!("calling policy demands: {error:?}"))?;
    let mut meter = TerminalFuelMeter::with_invocation(account.clone(), None)
        .map_err(|error| format!("calling policy fuel: {error:?}"))?;
    let scalars = complete_policy_scalar_inputs(
        artifact,
        target,
        entry,
        &requirements.scalar_applications,
        &providers,
        &mut meter,
    )?;
    let installation = program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            x86_flags: None,
            memory_ordering: None,
            installation: Some(&providers),
            host_imports: &[],
            entry: Some(entry),
            target_machines: Some(&selected),
            target_layouts: Some(scalars.primary().layouts()),
        })
        .map_err(|error| format!("calling policy scalar installation: {error:?}"))?;
    let entry = program
        .entry_installed(&installation)
        .map_err(|error| format!("calling policy entry: {error:?}"))?;
    if !entry.scalar_parameters.is_empty() {
        return Err("calling policy entry must have one structural signature argument".into());
    }
    let [parameter] = entry.structural_parameters else {
        return Err("calling policy entry must have one structural signature argument".into());
    };
    let result_type = entry
        .result
        .structural()
        .ok_or("calling policy result must be structural")?
        .structural_type;
    let argument = build_boundary_signature(signature)?;
    let contents = [input_contents(
        &program,
        parameter.structural_type,
        &argument,
    )?];
    let arguments = [TerminalStructuralValue::minted(
        1,
        parameter.structural_type,
        Vec::new(),
    )];
    let mut instance = program
        .instantiate(TerminalInstantiation {
            structural_inputs: TerminalStructuralInputs {
                arguments: &arguments,
                contents: &contents,
                ..Default::default()
            },
            installation: Some(&installation),
            account: Some(&account),
            scalar_admission: Some(&scalars),
            ..Default::default()
        })
        .map_err(|error| format!("calling policy arguments: {error:?}"))?;
    let result = instance
        .invoke(&mut meter)
        .map_err(|error| format!("calling policy execution: {error:?}"))?;
    match result {
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Structural(_))
        | TerminalExecutionStatus::Complete(TerminalExecutionResult::ScalarCase(_)) => {}
        status => {
            return Err(format!(
                "calling policy did not return a structural plan: {status:?}"
            ));
        }
    }
    let value = result_value(&instance, result_type)?;
    let cells = value
        .retained_cell_count()
        .ok_or("calling policy result cell count overflowed")?;
    let bytes = value
        .retained_text_byte_count()
        .ok_or("calling policy result payload count overflowed")?;
    account
        .charge_result_custody(cells, bytes)
        .map_err(|error| format!("calling policy result custody: {error}"))?;
    let plan = decode_boundary_plan_result(&value)?;
    let validated = validate_materialized_boundary_plan_result(plan, signature)?;
    instance
        .close()
        .map_err(|error| format!("calling policy close: {error:?}"))?;
    Ok((validated, meter.into_usage()))
}
