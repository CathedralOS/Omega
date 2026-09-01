use omega_boundary_applications::BoundaryApplicationRealization;
use omega_object_file::{RelocationKind, RelocationOrigin, RelocationRecord, SectionKind};
use omega_target::{Architecture, NativeTarget};
use omega_target_operations::CallSiteOwner;
use psi_terminal::OperationKind;
use sha2::{Digest, Sha256};

use super::model::{
    NativeByteSpan, OptimizedOperatorOccurrence, PhysicalRelocationDisposition, native_byte_span,
};

pub(super) struct OperatorPhysicalSpan {
    pub machine: NativeByteSpan,
    pub object: NativeByteSpan,
    pub final_image: NativeByteSpan,
    pub machine_bytes_digest: [u8; 32],
    pub object_bytes_digest: [u8; 32],
    pub final_image_bytes_digest: [u8; 32],
    pub relocation: PhysicalRelocationDisposition,
}

pub(super) fn derive_operator_physical_span(
    occurrence: &OptimizedOperatorOccurrence,
    realization: &BoundaryApplicationRealization,
    module: &psi_terminal::TerminalModule,
    target: NativeTarget,
    object: &omega_image_emission::ObjectArtifact,
    image: &omega_image_emission::ExecutableImage,
) -> Result<Option<OperatorPhysicalSpan>, &'static str> {
    let operation = exact_terminal_operation(module, occurrence)?;
    match realization {
        BoundaryApplicationRealization::NongenericCheckedBody { .. }
        | BoundaryApplicationRealization::SpecializedCheckedBody { .. } => {
            derive_checked_call_span(occurrence, operation, target, object, image)
        }
        BoundaryApplicationRealization::ExactCompilerIntrinsic { .. } => {
            derive_fma_span(occurrence, operation, object, image)
        }
    }
}

fn exact_terminal_operation<'module>(
    module: &'module psi_terminal::TerminalModule,
    occurrence: &OptimizedOperatorOccurrence,
) -> Result<&'module psi_terminal::Operation, &'static str> {
    let matching = module
        .machines
        .iter()
        .filter(|machine| machine.id == occurrence.machine())
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == occurrence.operation())
        .collect::<Vec<_>>();
    let [operation] = matching.as_slice() else {
        return Err("D29 physical occurrence does not rejoin one Terminal operation");
    };
    Ok(operation)
}

fn derive_checked_call_span(
    occurrence: &OptimizedOperatorOccurrence,
    operation: &psi_terminal::Operation,
    target: NativeTarget,
    object: &omega_image_emission::ObjectArtifact,
    image: &omega_image_emission::ExecutableImage,
) -> Result<Option<OperatorPhysicalSpan>, &'static str> {
    let expected_callee = match &operation.kind {
        OperationKind::Call { callee, .. }
        | OperationKind::CallUnit { callee, .. }
        | OperationKind::CallStructuralScalar { callee, .. } => *callee,
        _ => return Ok(None),
    };
    let function = object
        .functions()
        .iter()
        .find(|function| function.machine == occurrence.machine())
        .ok_or("D29 checked call names an absent object function")?;
    let calls = function
        .internal_unit_calls
        .iter()
        .filter_map(|call| {
            (call.owner == CallSiteOwner::Operation(occurrence.operation())
                && call.operation_ordinal == occurrence.operation_ordinal())
            .then_some((call.target, call.code_offset, call.byte_count))
        })
        .chain(
            function
                .internal_unit_scalar_calls
                .iter()
                .filter_map(|call| {
                    (call.owner == CallSiteOwner::Operation(occurrence.operation())
                        && call.operation_ordinal == occurrence.operation_ordinal())
                    .then_some((call.target, call.code_offset, call.byte_count))
                }),
        )
        .collect::<Vec<_>>();
    let [(callee, code_offset, byte_count)] = calls.as_slice() else {
        return if calls.is_empty() {
            Ok(None)
        } else {
            Err("D29 checked call rejoins multiple emitted call records")
        };
    };
    let object_offset = function
        .text_offset
        .checked_add(*code_offset)
        .ok_or("D29 checked call object span overflow")?;
    let object_end = object_offset
        .checked_add(*byte_count)
        .ok_or("D29 checked call object end overflow")?;
    let target_function = object
        .functions()
        .iter()
        .find(|candidate| candidate.machine == expected_callee)
        .ok_or("D29 checked call names an absent object callee")?;
    let overlapping = object
        .relocations()
        .records()
        .map(|(_, relocation)| relocation)
        .filter(|relocation| {
            relocation.section == SectionKind::Text
                && ranges_overlap(
                    object_offset,
                    object_end,
                    relocation.offset,
                    relocation.offset.saturating_add(relocation.byte_width),
                )
        })
        .collect::<Vec<_>>();
    let [relocation] = overlapping.as_slice() else {
        return Err("D29 checked call does not contain one exact internal relocation");
    };
    validate_checked_call_relocation(
        expected_callee,
        *callee,
        *byte_count,
        target.architecture,
        function.symbol,
        target_function.symbol,
        occurrence.operation(),
        object_offset,
        object_end,
        relocation,
    )?;
    derive_span(
        function,
        *code_offset,
        *byte_count,
        object,
        image,
        PhysicalRelocationDisposition::ResolvedInternalCall,
        Some((relocation.offset, relocation.byte_width)),
    )
    .map(Some)
}

#[allow(clippy::too_many_arguments)]
fn validate_checked_call_relocation(
    expected_callee: psi_core::MachineId,
    emitted_callee: psi_core::MachineId,
    byte_count: usize,
    architecture: Architecture,
    caller_symbol: omega_object_file::ObjectSymbolHandle,
    callee_symbol: omega_object_file::ObjectSymbolHandle,
    operation: psi_core::OperationId,
    object_offset: usize,
    object_end: usize,
    relocation: &RelocationRecord,
) -> Result<(), &'static str> {
    let expected_kind = match architecture {
        Architecture::Aarch64 => RelocationKind::Aarch64Branch26,
        Architecture::X86_64 => RelocationKind::X86_64Relative32,
    };
    if emitted_callee != expected_callee
        || byte_count == 0
        || relocation.origin
            != (RelocationOrigin::SemanticOperation {
                function_symbol_handle: caller_symbol,
                operation_identity: operation.get(),
            })
        || relocation.section != SectionKind::Text
        || relocation.symbol_handle != callee_symbol
        || relocation.addend != 0
        || relocation.kind != expected_kind
        || relocation.byte_width != 4
        || relocation.offset < object_offset
        || relocation.offset.saturating_add(relocation.byte_width) > object_end
    {
        return Err(
            "D29 checked call relocation changed callee, owner, target, addend, kind, or span",
        );
    }
    Ok(())
}

fn derive_fma_span(
    occurrence: &OptimizedOperatorOccurrence,
    operation: &psi_terminal::Operation,
    object: &omega_image_emission::ObjectArtifact,
    image: &omega_image_emission::ExecutableImage,
) -> Result<Option<OperatorPhysicalSpan>, &'static str> {
    if !matches!(
        operation.kind,
        OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
    ) {
        return Ok(None);
    }
    let function = object
        .functions()
        .iter()
        .find(|function| function.machine == occurrence.machine())
        .ok_or("D29 FMA names an absent object function")?;
    let matching = function
        .x86_scalar_fma_occurrences
        .iter()
        .zip(&function.x86_scalar_fma)
        .filter(|(retained, _)| {
            retained.terminal_operation == occurrence.operation()
                && retained.operation_ordinal == occurrence.operation_ordinal()
        })
        .collect::<Vec<_>>();
    let [(retained, fragment)] = matching.as_slice() else {
        return if matching.is_empty() {
            Ok(None)
        } else {
            Err("D29 FMA rejoins multiple emitted fragments")
        };
    };
    if retained.fragment_identity != fragment.identity || fragment.byte_count == 0 {
        return Err("D29 FMA changed its exact fragment or emitted an empty span");
    }
    derive_span(
        function,
        fragment.code_offset,
        fragment.byte_count,
        object,
        image,
        PhysicalRelocationDisposition::DirectInstructionBytes,
        None,
    )
    .map(Some)
}

fn derive_span(
    function: &omega_image_emission::ObjectFunction,
    code_offset: usize,
    byte_count: usize,
    object: &omega_image_emission::ObjectArtifact,
    image: &omega_image_emission::ExecutableImage,
    disposition: PhysicalRelocationDisposition,
    relocation: Option<(usize, usize)>,
) -> Result<OperatorPhysicalSpan, &'static str> {
    let object_offset = function
        .text_offset
        .checked_add(code_offset)
        .ok_or("D29 physical child object span overflow")?;
    let machine = native_byte_span(code_offset, byte_count);
    let object_span = native_byte_span(object_offset, byte_count);
    let final_image = object_span;
    let machine_bytes = span(function.bytes(object), machine)?;
    let object_bytes = span(object.text_bytes(), object_span)?;
    let final_image_bytes = span(&image.output().final_text_bytes, final_image)?;
    if machine_bytes != object_bytes {
        return Err("D29 physical child changed before object custody");
    }
    match relocation {
        None if object_bytes != final_image_bytes => {
            return Err("D29 direct physical child changed before final image custody");
        }
        Some((relocation_offset, relocation_width)) => {
            let relative_start = relocation_offset
                .checked_sub(object_offset)
                .ok_or("D29 relocation precedes its physical child")?;
            let relative_end = relative_start
                .checked_add(relocation_width)
                .ok_or("D29 relocation span overflow")?;
            if relative_end > byte_count
                || object_bytes.iter().zip(final_image_bytes).enumerate().any(
                    |(index, (before, after))| {
                        (index < relative_start || index >= relative_end) && before != after
                    },
                )
            {
                return Err("D29 internal-call bytes changed outside the exact relocation");
            }
        }
        None => {}
    }
    Ok(OperatorPhysicalSpan {
        machine,
        object: object_span,
        final_image,
        machine_bytes_digest: sha256(machine_bytes),
        object_bytes_digest: sha256(object_bytes),
        final_image_bytes_digest: sha256(final_image_bytes),
        relocation: disposition,
    })
}

fn span(bytes: &[u8], span: NativeByteSpan) -> Result<&[u8], &'static str> {
    let end = span
        .offset()
        .checked_add(span.byte_count())
        .ok_or("D29 physical child byte span overflow")?;
    bytes
        .get(span.offset()..end)
        .ok_or("D29 physical child byte span is out of bounds")
}

fn ranges_overlap(
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
) -> bool {
    left_start < right_end && right_start < left_end
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checked_call_relocation() -> RelocationRecord {
        RelocationRecord {
            origin: RelocationOrigin::SemanticOperation {
                function_symbol_handle: omega_object_file::ObjectSymbolHandle::from_arena_index(1),
                operation_identity: 3,
            },
            section: SectionKind::Text,
            offset: 12,
            byte_width: 4,
            symbol_handle: omega_object_file::ObjectSymbolHandle::from_arena_index(2),
            addend: 0,
            kind: RelocationKind::X86_64Relative32,
        }
    }

    fn validate(relocation: &RelocationRecord) -> Result<(), &'static str> {
        validate_checked_call_relocation(
            psi_core::MachineId::new(2).expect("callee"),
            psi_core::MachineId::new(2).expect("callee"),
            6,
            Architecture::X86_64,
            omega_object_file::ObjectSymbolHandle::from_arena_index(1),
            omega_object_file::ObjectSymbolHandle::from_arena_index(2),
            psi_core::OperationId::new(3).expect("operation"),
            10,
            16,
            relocation,
        )
    }

    #[test]
    fn checked_call_relocation_binds_exact_semantic_call() {
        validate(&checked_call_relocation()).expect("exact relocation");
    }

    #[test]
    fn checked_call_relocation_rejects_substituted_addend_and_owner() {
        let mut relocation = checked_call_relocation();
        relocation.addend = 1;
        assert!(validate(&relocation).is_err());

        relocation = checked_call_relocation();
        relocation.origin = RelocationOrigin::SemanticOperation {
            function_symbol_handle: omega_object_file::ObjectSymbolHandle::from_arena_index(1),
            operation_identity: 4,
        };
        assert!(validate(&relocation).is_err());
    }
}
