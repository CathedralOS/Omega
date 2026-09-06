//! Scalar-leaf admission and exact publication coordinates.
//!
//! ABI mechanics are reconstructed from the signature. Instruction and stack
//! replay remains owned by image emission; neither check re-emits a function.

use calling_conventions::{CallSignature, CallingPolicy, ValueShape, evaluate_call_plan};
use machine_code::{
    MachineCodeFunction, ScalarControlFlowEvidence, SemanticCodeAttribution, SemanticCodeSite,
};
use semantic_vocabulary::{IntegerCarrier, IntegerSign, IntegerType};
use sha2::{Digest, Sha256};
use target::NativeTarget;
use target_operations::{FixedIntegerScalarFunctionAbi, TerminalPsiProvenance};

const INVALID: &str = "selected-lowering scalar function is outside the scalar-leaf cohort";

// These fields have the same meaning in the machine and object records. Keep
// the exclusions shared so publication cannot admit an unbound side record.
macro_rules! unsupported_records {
    ($function:expr) => {
        $function.attachment.is_some()
            || $function.mixed_structural_scalar_abi.is_some()
            || $function.structural_call_scalar_return.is_some()
            || $function.unit_scalar_abi.is_some()
            || !$function.x86_scalar_fma.is_empty()
            || !$function.x86_scalar_fma_occurrences.is_empty()
            || $function.x86_floating_control.is_some()
            || $function.unit_stack.is_some()
            || !$function.unit_parameters.is_empty()
            || !$function.unit_parameter_homes.is_empty()
            || !$function.internal_unit_calls.is_empty()
            || !$function.internal_unit_scalar_calls.is_empty()
            || !$function.installed_provider_unit_scalar_calls.is_empty()
            || !$function.dynamic_calls.is_empty()
            || !$function.stored_dynamic_calls.is_empty()
            || !$function.dynamic_parameter_calls.is_empty()
            || !$function.forwarded_dynamic_parameter_calls.is_empty()
            || !$function.forwarded_dynamic_descriptor_calls.is_empty()
            || !$function.unit_scalar_homes.is_empty()
            || !$function.unit_integer_constants.is_empty()
            || !$function.unit_affine_scalar_records.is_empty()
            || !$function.unit_structural_scalar_field_stores.is_empty()
            || !$function.unit_write_only_primitive_stores.is_empty()
            || !$function.scalar_structural_scalar_field_stores.is_empty()
            || $function.unit_affine_cleanup.is_some()
            || $function.scalar_affine_cleanup.is_some()
            || !$function.scalar_control_affine_cleanups.is_empty()
            || !$function.scalar_structural_parameters.is_empty()
            || !$function.scalar_structural_parameter_homes.is_empty()
            || $function.ranked_u32_countdown.is_some()
            || $function.structural_return.is_some()
    };
}

pub(super) fn validate_machine_function(
    function: &MachineCodeFunction,
    target: NativeTarget,
) -> Result<(), &'static str> {
    if unsupported_records!(function)
        || !function.internal_calls.is_empty()
        || !function.foreign_calls.is_empty()
        || !function.port_effects.is_empty()
        || !function.boundary_settlements.is_empty()
    {
        return Err(INVALID);
    }
    let stack = function.scalar_stack.as_ref().ok_or(INVALID)?;
    if stack.stack_alignment != 16
        || stack.control_flow != ScalarControlFlowEvidence::Linear
        || stack.cleanup_preservation.is_some()
    {
        return Err(INVALID);
    }
    validate_abi(
        function.fixed_integer_scalar_abi.as_ref().ok_or(INVALID)?,
        target,
    )?;
    validate_attribution(
        &function.provenance,
        function.bytes.len(),
        function.semantic_code_attribution.iter(),
    )
}

pub(super) fn validate_object_function(
    object: &image_emission::ObjectArtifact,
    function: &image_emission::ObjectFunction,
) -> Result<(), &'static str> {
    if unsupported_records!(function)
        || !function.unit_call_stacks.is_empty()
        || !function.scalar_call_stacks.is_empty()
        || function
            .scalar_stack
            .as_ref()
            .is_none_or(|stack| stack.stack_alignment != 16)
    {
        return Err(INVALID);
    }
    validate_abi(
        function.fixed_integer_scalar_abi.as_ref().ok_or(INVALID)?,
        object.target(),
    )?;
    let mut attributions = Vec::new();
    for row in object
        .semantic_code_attribution()
        .iter()
        .filter(|row| row.machine == function.machine)
    {
        if function
            .text_offset
            .checked_add(row.attribution.code_offset)
            != Some(row.text_offset)
        {
            return Err(INVALID);
        }
        attributions.push(&row.attribution);
    }
    validate_attribution(
        &function.provenance,
        function.byte_count,
        attributions.into_iter(),
    )
}

fn integer_shape(integer: IntegerType) -> Result<ValueShape, &'static str> {
    if integer.carrier() != IntegerCarrier::Fixed || !matches!(integer.bits(), 8 | 16 | 32 | 64) {
        return Err(INVALID);
    }
    let bytes = integer.bits().div_ceil(8);
    Ok(ValueShape::integer(bytes, bytes))
}

fn validate_abi(
    abi: &FixedIntegerScalarFunctionAbi,
    target: NativeTarget,
) -> Result<(), &'static str> {
    let signature = CallSignature {
        parameters: abi
            .parameters
            .iter()
            .map(|parameter| integer_shape(parameter.scalar_type))
            .collect::<Result<_, _>>()?,
        result: Some(integer_shape(abi.result.scalar_type)?),
    };
    let expected = evaluate_call_plan(CallingPolicy::native_for_target(target), &signature)
        .map_err(|_| INVALID)?;
    let mut parameters = std::collections::BTreeSet::new();
    if abi.call_plan != expected
        || abi.parameters.len() != expected.parameters.len()
        || abi
            .parameters
            .iter()
            .zip(&expected.parameters)
            .any(|(parameter, placement)| {
                parameter.placement != *placement || !parameters.insert(parameter.value)
            })
        || Some(&abi.result.placement) != expected.result.as_ref()
    {
        return Err(INVALID);
    }
    Ok(())
}

fn validate_attribution<'row>(
    provenance: &TerminalPsiProvenance,
    byte_count: usize,
    attributions: impl Iterator<Item = &'row SemanticCodeAttribution>,
) -> Result<(), &'static str> {
    let [return_edge] = provenance.edges.as_slice() else {
        return Err(INVALID);
    };
    let mut sites = std::collections::BTreeSet::new();
    let mut previous = None;
    let mut return_count = 0;
    for row in attributions {
        let coordinates = (row.operation_ordinal, row.code_offset);
        let end = row.code_offset.checked_add(row.byte_count).ok_or(INVALID)?;
        if previous.is_some_and(|previous| previous >= coordinates)
            || end > byte_count
            || !sites.insert(row.site)
        {
            return Err(INVALID);
        }
        previous = Some(coordinates);
        match row.site {
            SemanticCodeSite::Operation(operation)
                if provenance.operations.contains(&operation) => {}
            SemanticCodeSite::Edge(edge)
                if edge == *return_edge && row.byte_count != 0 && end == byte_count =>
            {
                return_count += 1
            }
            _ => return Err(INVALID),
        }
    }
    if return_count != 1 {
        return Err(INVALID);
    }
    Ok(())
}

/// Unit rows retain their original digest verbatim. Scalar rows append an
/// explicit role and all variable publication facts. Canonical ABI validation
/// makes target plus ordered types uniquely determine every placement/call-plan
/// field; value identities remain separate, exact coordinates here.
pub(super) fn hash_projection<'row>(
    digest: &mut Sha256,
    abi: &FixedIntegerScalarFunctionAbi,
    attributions: impl Iterator<Item = &'row SemanticCodeAttribution>,
) {
    digest.update(b"omega.native-artifact.fixed-integer-scalar-leaf.v1\0");
    digest.update(super::canonical_usize(abi.parameters.len()));
    for value in abi.parameters.iter().chain(std::iter::once(&abi.result)) {
        digest.update(value.value.get().to_le_bytes());
        digest.update(value.scalar_type.bits().to_le_bytes());
        digest.update([match value.scalar_type.sign() {
            IntegerSign::Signed => 1,
            IntegerSign::Unsigned => 2,
        }]);
    }
    let rows = attributions.collect::<Vec<_>>();
    digest.update(super::canonical_usize(rows.len()));
    for row in rows {
        match row.site {
            SemanticCodeSite::Operation(operation) => {
                digest.update([1]);
                digest.update(operation.get().to_le_bytes());
            }
            SemanticCodeSite::Edge(edge) => {
                digest.update([2]);
                digest.update(edge.get().to_le_bytes());
            }
        }
        digest.update(super::canonical_usize(row.operation_ordinal));
        digest.update(super::canonical_usize(row.code_offset));
        digest.update(super::canonical_usize(row.byte_count));
    }
}
