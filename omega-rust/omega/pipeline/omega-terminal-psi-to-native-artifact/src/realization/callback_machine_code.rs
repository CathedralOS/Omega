//! Machine-code emission and assignment replay for compiler-private callback thunks.

use super::diagnostics::realization_error;
use super::model::NativeCallbackThunkSettlement;
use omega_machine_code::CompilerPrivateMachineCodeFunction;
use psi_diagnostics::Diagnostic;

pub(super) fn emit_callback_thunks(
    thunks: &[NativeCallbackThunkSettlement<'_>],
    target: omega_target::NativeTarget,
    profile: &psi_proof_admission::AdmissionProfile,
) -> Result<Vec<CompilerPrivateMachineCodeFunction>, Vec<Diagnostic>> {
    if thunks.len() > 1 {
        return Err(realization_error(
            "native callback thunk emission",
            "ordinary native realization currently admits exactly one callback thunk",
        ));
    }
    let mut emitted = Vec::with_capacity(thunks.len());
    for thunk in thunks {
        if thunk.private_symbol.is_empty()
            || thunk.callback_function.callback_thunk_placement_index()
                != Some(thunk.placement_index)
        {
            return Err(realization_error(
                "native callback thunk emission",
                "callback thunk has invalid private identity custody",
            ));
        }
        thunk
            .artifact
            .validate()
            .map_err(|error| realization_error("callback thunk artifact replay", error))?;
        let lowered =
            omega_psi_to_abstract_operations::lower_artifact_sections_for_native_realization(
                thunk.artifact.semantic_bytes(),
                thunk.artifact.proof_bytes(),
                profile,
            )
            .map_err(|error| realization_error("callback thunk abstract lowering", error))?;
        let omega_psi_to_abstract_operations::NativeArtifactOperationPlan::Ordinary(abstract_plan) =
            lowered
        else {
            return Err(realization_error(
                "native callback thunk emission",
                "callback thunk cannot use ranked native authority",
            ));
        };
        let target_plan =
            omega_abstract_operations_to_target_operations::lower_to_target_operations(
                &abstract_plan,
                target,
            )
            .map_err(|error| realization_error("callback thunk target lowering", error))?;
        let assigned =
            omega_target_operations_to_assigned_target_operations::assign_registers(&target_plan)
                .map_err(|error| realization_error("callback thunk physical assignment", error))?;
        let machine_code = omega_machine_emission::emit_machine_code(&assigned)
            .map_err(|error| realization_error("callback thunk machine-code emission", error))?;
        let [function] = machine_code.functions.as_slice() else {
            return Err(realization_error(
                "native callback thunk emission",
                "bounded callback thunk must emit exactly one native function",
            ));
        };
        let Some(abi) = function.fixed_integer_scalar_abi.as_ref() else {
            return Err(realization_error(
                "native callback thunk emission",
                "bounded callback thunk did not retain its fixed-integer scalar ABI",
            ));
        };
        if machine_code.target != target
            || machine_code.entry != thunk.lowering_receipt.terminal_machine
            || function.machine != thunk.lowering_receipt.terminal_machine
            || abi.call_plan != thunk.boundary_entry_plan.call
        {
            return Err(realization_error(
                "native callback thunk emission",
                "callback thunk machine, target, or inbound ABI drifted from its lowering receipt",
            ));
        }
        emitted.push(CompilerPrivateMachineCodeFunction {
            identity: thunk.callback_function,
            private_symbol: std::sync::Arc::from(thunk.private_symbol),
            source_psi: machine_code.psi,
            function: function.clone(),
        });
    }
    Ok(emitted)
}

pub(super) fn validate_callback_thunk_assignments(
    thunks: &[NativeCallbackThunkSettlement<'_>],
    assigned: &[omega_assigned_target_operations::AssignedNativeCallbackArgument],
) -> Result<(), Vec<Diagnostic>> {
    if thunks.len() != assigned.len() {
        return Err(realization_error(
            "native callback thunk assignment",
            "callback body and assigned argument rosters differ",
        ));
    }
    for thunk in thunks {
        let matching = assigned
            .iter()
            .filter(|argument| argument.target.placement_index == thunk.placement_index)
            .collect::<Vec<_>>();
        let [argument] = matching.as_slice() else {
            return Err(realization_error(
                "native callback thunk assignment",
                "callback body does not rejoin exactly one assigned registrar argument",
            ));
        };
        if argument.target.terminal_operation != thunk.terminal_operation
            || argument.target.callback_function != thunk.callback_function
        {
            return Err(realization_error(
                "native callback thunk assignment",
                "callback body identity drifted from its assigned registrar argument",
            ));
        }
    }
    Ok(())
}
