/*
Execute a retained Terminal calling-policy machine on a canonical boundary
signature (`evaluate_terminal_calling_policy`).

`provider_planning` prepares the canonical `CallingPolicySignature`, the selected
callable entry, and the invocation work/memory account, then invokes
`evaluate_terminal_calling_policy` through the evaluator function pointer stored
on its `CallingProgram`.

Why this lives in `build-settings` without touching source trees:
1. Calling policies are compiled once into an immutable Terminal Psi artifact (or
   loaded from a verified package envelope) and shared across many boundary
   signature evaluations.
2. For each boundary signature, we admit target machine selections and target
   layouts for `target`, verify that no boundary provider requirement is
   unsatisfied (reporting the missing provider's display path from the retained
   module if any requirement is unfulfilled), complete any demanded scalar
   applications (`complete_calling_scalar_inputs`), and instantiate a fresh
   `TerminalInstance` with the encoded structural signature argument
   (`terminal_values::input_contents`).
3. After `instance.invoke(&mut meter)` returns normally, we read the structural
   result out of the instance (`terminal_values::result_value`), charge result custody to
   the invocation account, and hand the `BuildTimeValue` back to
   `provider_planning` (`decode_boundary_plan_result` +
   `validate_calling_policy_result`) so every register/stack placement is checked
   against the exact input signature.
*/

use build_time_evaluation::terminal_values::{input_contents, result_value};
use calling_conventions::ValidatedBoundaryEntryPlan;
use provider_planning::calling_policy_plans::{
    CallingScalarScope, TerminalCallingPolicyInvocation, complete_calling_scalar_inputs,
    decode_boundary_plan_result, encode_calling_policy_signature, validate_calling_policy_result,
};
use psi_work::{WorkMeter, WorkUsage};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInstantiation, TerminalProgram,
    TerminalRuntimeSupplyRequest, TerminalStructuralInputs, TerminalStructuralValue,
};
use terminal_psi::TargetMachineCallable;

pub fn evaluate_terminal_calling_policy(
    invocation: TerminalCallingPolicyInvocation<'_>,
) -> Result<(ValidatedBoundaryEntryPlan, WorkUsage), String> {
    let TerminalCallingPolicyInvocation {
        artifact,
        entry,
        target,
        signature,
        account,
        admission,
    } = invocation;
    if signature.target != target.native_target() {
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
    let selected_entry = selected
        .entry(selected.terminal_psi(), artifact.semantic_module(), entry)
        .map_err(|error| format!("calling policy canonical selection: {error}"))?;
    admission.require_invocation(
        artifact.view(),
        target.target_name(),
        TargetMachineCallable::Machine(selected_entry),
    )?;
    let providers = admission.providers();
    let program = TerminalProgram::borrow(artifact.view());
    let installation = program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            native_code_layouts: None,
            x86_flags: None,
            memory_ordering: None,
            installation: Some(providers),
            host_imports: &[],
            entry: Some(entry),
            target_machines: Some(&selected),
            target_layouts: Some(&layouts),
        })
        .map_err(|error| format!("calling policy selection: {error:?}"))?;
    let requirements = program
        .execution_requirements(Some(&installation))
        .map_err(|error| format!("calling policy demands: {error:?}"))?;
    if let Some(boundary) = requirements.missing_providers.first() {
        let declaration = artifact
            .semantic_module()
            .boundary_machine(*boundary)
            .ok_or("calling policy's missing provider has no boundary declaration")?;
        let identity = artifact
            .semantic_module()
            .identities
            .text(declaration.identity)
            .ok_or("calling policy's missing provider has no callable identity")?;
        return Err(format!(
            "calling policy calls `{}` without a selected provider for target `{}`",
            language_semantics::type_identity::callable_display_path(identity),
            target.target_name(),
        ));
    }
    let mut demanded = requirements.scalar_applications;
    for application in requirements.scalar_entry_conditions {
        if !demanded.contains(&application) {
            demanded.push(application);
        }
    }
    let mut meter = WorkMeter::with_invocation(account.clone(), None)
        .map_err(|error| format!("calling policy work: {error:?}"))?;
    let declaration = artifact
        .semantic_module()
        .scalar_applications
        .proof_scope
        .callable_declaration(TargetMachineCallable::Machine(selected_entry))
        .ok_or("calling policy entry has no exact checked proof declaration")?;
    let scalars = complete_calling_scalar_inputs(
        artifact,
        target,
        CallingScalarScope::Invocation {
            declaration,
            applications: &demanded,
        },
        providers,
        &mut meter,
    )?;
    let installation = program
        .admit_runtime_supplies(TerminalRuntimeSupplyRequest {
            native_code_layouts: None,
            x86_flags: None,
            memory_ordering: None,
            installation: Some(providers),
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
    let argument = encode_calling_policy_signature(signature)?;
    let contents = [input_contents(
        &program,
        0,
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
    let validated = validate_calling_policy_result(plan, signature)?;
    instance
        .close()
        .map_err(|error| format!("calling policy close: {error:?}"))?;
    Ok((validated, meter.into_usage()))
}
