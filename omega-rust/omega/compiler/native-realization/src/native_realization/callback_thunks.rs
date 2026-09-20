//! Lower each settled callback thunk's canonical Terminal artifact to machine
//! code inside the same realization as the semantic program.
//!
//! `produce_callback_thunk_artifact` deliberately stops at
//! `finalize_terminal_artifact`: the settlement carries the artifact, its
//! lowering receipt, and its boundary entry plan, never bytes. This stage is
//! the consumer half of that contract — every thunk replays the ordinary
//! native pipeline (decode, abstract optimization, target lowering,
//! instruction selection, register homes, fragment emission, frame
//! application, fixed-frame text placement) as a standalone module and
//! materializes as a `CompilerPrivateMachineCodeFunction` the retained
//! object's private-function channel can publish.

use crate::native_realization::input_preparation::lower_realization_input;
use crate::native_realization::realization_diagnostics::realization_error;
use crate::native_realization::realization_request::{
    NativeCallbackThunkSettlement, NativeRealizationRequest,
};
use diagnostics::Diagnostic;
use machine_code::{CompilerPrivateMachineCodeFunction, MachineCodeFunction};
use semantic_vocabulary::MachineId;

/// Realize every settled callback thunk into its compiler-private machine-code
/// record, preserving the request's placement order.
///
/// A thunk is a self-contained module: it carries no boundary settlements,
/// provider installations, FMA custody, or callback arguments of its own, so
/// the shared stages run with empty auxiliary rosters. Any decode, lowering,
/// or shape violation rejects here with the thunk's placement index in the
/// diagnostic rather than surfacing as an unrelated program-stage failure.
pub(crate) fn lower_callback_thunks(
    request: &NativeRealizationRequest<'_>,
) -> Result<Vec<CompilerPrivateMachineCodeFunction>, Vec<Diagnostic>> {
    request
        .callback_thunks
        .iter()
        .map(|settlement| lower_callback_thunk(request, settlement))
        .collect()
}

fn lower_callback_thunk(
    request: &NativeRealizationRequest<'_>,
    settlement: &NativeCallbackThunkSettlement<'_>,
) -> Result<CompilerPrivateMachineCodeFunction, Vec<Diagnostic>> {
    let reject = |reason: &'static str| {
        realization_error(
            "callback thunk materialization",
            format!(
                "callback thunk at placement index {}: {reason}",
                settlement.placement_index
            ),
        )
    };
    let artifact = settlement.artifact;
    artifact
        .validate()
        .map_err(|error| realization_error("callback thunk artifact replay", error))?;
    let input = lower_realization_input(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        request.profile,
    )?;
    let optimized = crate::optimize_verified_abstract_input(
        input.into_optimization_input(),
        crate::compiler_baseline_request_v1(request.optimization_selections.selections()),
    )
    .map_err(|error| realization_error("callback thunk abstract optimization", error))?;
    let optimized_target =
        abstract_operations_to_target_operations::lower_optimized_to_target_operations(
            optimized,
            abstract_operations_to_target_operations::OptimizedTargetLoweringRequest::new(
                request.target,
            ),
        )
        .map_err(|error| realization_error("callback thunk target lowering", error))?;
    // The exact target plan survives as shared custody inside the staged
    // physical product; retain one before emission consumes the stage.
    let target_plan = optimized_target.shared_program();
    let physical = crate::stage_optimized_verified_physical_pipeline(
        optimized_target,
        request.optimization_selections,
    )
    .map_err(|error| realization_error("callback thunk physical pipeline", error))?;
    let emission_source = physical.into_function_fragment_emission_source();
    let emitted = machine_emission::stage_optimized_function_fragment_emission(emission_source)
        .map_err(|error| realization_error("callback thunk fragment emission", error))?;
    let applied = machine_emission::stage_function_fragment_frame_application(emitted)
        .map_err(|error| realization_error("callback thunk frame application", error))?;
    let text = machine_emission::stage_optimized_fixed_frame_text_section(applied)
        .map_err(|error| realization_error("callback thunk text placement", error))?;
    let section = text.text_section();
    let [placed] = section.functions.as_slice() else {
        return Err(reject(
            "a private callback thunk emits exactly one function",
        ));
    };
    if placed.machine != section.semantic_entry
        || !section.resolved_internal_machine_calls.is_empty()
        || !section.unresolved_normalized_foreign_calls.is_empty()
    {
        return Err(reject(
            "a private callback thunk is a leaf function with no call custody",
        ));
    }
    let targeted = target_plan
        .functions
        .iter()
        .find(|function| function.machine == placed.machine)
        .ok_or_else(|| reject("emitted thunk machine is absent from its target operations"))?;
    let offset = usize::try_from(placed.section_offset)
        .map_err(|_| reject("callback thunk text offset exceeds host size"))?;
    let byte_count = usize::try_from(placed.byte_count)
        .map_err(|_| reject("callback thunk byte count exceeds host size"))?;
    let bytes = section
        .bytes
        .get(offset..offset + byte_count)
        .ok_or_else(|| reject("callback thunk byte range exceeds its text section"))?
        .to_vec();
    Ok(CompilerPrivateMachineCodeFunction {
        identity: settlement.callback_function,
        private_symbol: std::sync::Arc::from(settlement.private_symbol),
        source_psi: section.psi,
        function: materialize_function(placed.machine, targeted, bytes),
    })
}

fn materialize_function(
    machine: MachineId,
    targeted: &target_operations::TargetFunction,
    bytes: Vec<u8>,
) -> MachineCodeFunction {
    MachineCodeFunction {
        machine,
        attachment: targeted.attachment,
        scalar_abi: targeted.scalar_abi.clone(),
        mixed_structural_scalar_abi: targeted.mixed_structural_scalar_abi.clone(),
        structural_call_scalar_return: None,
        parameter_abi: None,
        provenance: targeted.provenance.clone(),
        bytes,
        x86_scalar_fma: Vec::new(),
        x86_scalar_fma_occurrences: Vec::new(),
        x86_floating_control: None,
        unit_stack: None,
        unit_parameter_homes: Vec::new(),
        unit_parameters: Vec::new(),
        scalar_stack: None,
        internal_calls: Vec::new(),
        foreign_calls: Vec::new(),
        internal_unit_calls: Vec::new(),
        internal_unit_scalar_calls: Vec::new(),
        installed_provider_unit_scalar_calls: Vec::new(),
        dynamic_calls: Vec::new(),
        stored_dynamic_calls: Vec::new(),
        dynamic_parameter_calls: Vec::new(),
        forwarded_dynamic_parameter_calls: Vec::new(),
        forwarded_dynamic_descriptor_calls: Vec::new(),
        unit_scalar_homes: Vec::new(),
        unit_integer_constants: Vec::new(),
        unit_affine_scalar_records: Vec::new(),
        unit_structural_scalar_field_stores: Vec::new(),
        unit_write_only_primitive_stores: Vec::new(),
        scalar_structural_scalar_field_stores: Vec::new(),
        unit_affine_cleanup: None,
        unit_continuations: Vec::new(),
        scalar_affine_cleanup: None,
        scalar_control_affine_cleanups: Vec::new(),
        scalar_structural_parameters: Vec::new(),
        scalar_structural_parameter_homes: Vec::new(),
        semantic_code_attribution: Vec::new(),
        port_effects: Vec::new(),
        boundary_settlements: Vec::new(),
        structural_return: None,
    }
}
