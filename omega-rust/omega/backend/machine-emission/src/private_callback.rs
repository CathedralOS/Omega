//! Private callback materialization from the ordinary placed-text route.

use crate::text_placement::StagedOptimizedFixedFrameTextSection;
use calling_conventions::BoundaryEntryPlan;
use diagnostics::Diagnostic;
use function_identity::MachineFunctionIdentity;
use machine_code::{CompilerPrivateMachineCodeFunction, MachineCodeFunction};
use semantic_vocabulary::MachineId;

/// Identity and boundary ABI for one already-settled private callback.
pub struct PrivateCallbackMaterializationRequest<'request> {
    pub callback_function: MachineFunctionIdentity,
    pub private_symbol: &'request str,
    pub boundary_entry_plan: &'request BoundaryEntryPlan,
    /// Original placement order, retained for diagnostics.
    pub placement_index: usize,
}

/// Consume placed callback text and project its compiler-private function record.
///
/// The section must contain only its entry function, with no external call
/// custody, and its retained target ABI must match the settled boundary entry.
/// Object and image publication still perform their independent admission.
pub fn materialize_private_callback_function(
    text: StagedOptimizedFixedFrameTextSection,
    request: PrivateCallbackMaterializationRequest<'_>,
) -> Result<CompilerPrivateMachineCodeFunction, Vec<Diagnostic>> {
    let reject = |reason: &'static str| {
        vec![Diagnostic::error(format!(
            "native artifact callback thunk materialization failed: callback thunk at placement index {}: {reason}",
            request.placement_index
        ))]
    };
    let target_plan = text
        .source()
        .source()
        .source()
        .optimized_target()
        .target_operations();
    let section = text.text_section();
    let [placed] = section.functions.as_slice() else {
        return Err(reject(
            "a private callback thunk emits exactly one function",
        ));
    };
    if placed.machine != section.semantic_entry {
        return Err(reject("emitted thunk machine is not its section's entry"));
    }
    // Call-custody check, not a shape check: internal calls the section
    // resolved are bound custody and admissible; unresolved foreign calls are
    // not — the compiler-private channel carries no import custody, so a thunk
    // that imports has no path to published custody and rejects here.
    if !section.unresolved_normalized_foreign_calls.is_empty() {
        return Err(reject(
            "a private callback thunk cannot carry foreign-call custody",
        ));
    }
    if section
        .resolved_internal_machine_calls
        .iter()
        .any(|call| call.caller == placed.machine && call.callee != placed.machine)
    {
        return Err(reject(
            "a private callback thunk cannot call outside its own machine",
        ));
    }
    let targeted = target_plan
        .functions
        .iter()
        .find(|function| function.machine == placed.machine)
        .ok_or_else(|| reject("emitted thunk machine is absent from its target operations"))?;
    let boundary_signature = &request.boundary_entry_plan.call;
    let Some(emitted_signature) = targeted.scalar_abi.as_ref() else {
        return Err(reject(
            "callback thunk emits no scalar ABI for its boundary entry signature",
        ));
    };
    if emitted_signature.call_plan.policy != boundary_signature.policy
        || emitted_signature.parameters.len() != boundary_signature.parameters.len()
        || emitted_signature
            .parameters
            .iter()
            .zip(boundary_signature.parameters.iter())
            .any(|(emitted, declared)| emitted.placement != *declared)
        || boundary_signature.result.as_ref() != Some(&emitted_signature.result.placement)
    {
        return Err(reject(
            "callback thunk signature drifts from its boundary entry plan",
        ));
    }
    let bytes = callback_text_bytes(&section.bytes, placed.section_offset, placed.byte_count)
        .map_err(reject)?
        .to_vec();
    Ok(CompilerPrivateMachineCodeFunction {
        identity: request.callback_function,
        private_symbol: std::sync::Arc::from(request.private_symbol),
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
        // Thunks never touch ports; the general producer is still missing
        // (wiki/drafts/designs/evaluated_foreign_bindings.md).
        port_effects: Vec::new(),
        boundary_settlements: Vec::new(),
        structural_return: None,
    }
}

fn callback_text_bytes(
    section_bytes: &[u8],
    section_offset: u64,
    byte_count: u64,
) -> Result<&[u8], &'static str> {
    let section_offset = usize::try_from(section_offset)
        .map_err(|_| "callback thunk text offset exceeds host size")?;
    let byte_count =
        usize::try_from(byte_count).map_err(|_| "callback thunk byte count exceeds host size")?;
    let section_end = section_offset
        .checked_add(byte_count)
        .ok_or("callback thunk byte range exceeds its text section")?;
    section_bytes
        .get(section_offset..section_end)
        .ok_or("callback thunk byte range exceeds its text section")
}

#[cfg(test)]
mod tests {
    use super::callback_text_bytes;

    #[test]
    fn callback_text_range_rejects_out_of_bounds_and_overflow() {
        let section_bytes = [10, 20, 30];
        assert_eq!(
            callback_text_bytes(&section_bytes, 1, 2),
            Ok(&section_bytes[1..3])
        );
        assert_eq!(
            callback_text_bytes(&section_bytes, 3, 0),
            Ok(&section_bytes[3..3])
        );
        assert_eq!(
            callback_text_bytes(&section_bytes, 2, 2),
            Err("callback thunk byte range exceeds its text section")
        );
        assert!(callback_text_bytes(&section_bytes, u64::MAX, 2).is_err());
        assert!(callback_text_bytes(&section_bytes, 2, u64::MAX).is_err());
    }
}
