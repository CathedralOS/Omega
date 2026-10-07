use boundary_applications::BoundaryApplicationRealization;
use object_file::{RelocationKind, RelocationOrigin, RelocationRecord, SectionKind};
use sha2::{Digest, Sha256};
use target::{Architecture, NativeTarget};
use target_operations::CallSiteOwner;
use terminal_psi::OperationKind;

pub(super) mod fragment_call;
mod fragment_comparison;

use super::{
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

/// The occurrence's emitted bytes must rejoin exactly one machine/object/
/// final-image span, and `Ok(None)` remains a witnessed gap — it becomes
/// `UnsupportedOperatorSpan`, never an erasure.
///
/// Reachability is upstream of this table, and on the current surface it is
/// narrower than the match arms suggest. A `boundary machine` requirement
/// spelled like an operator emits a `BoundaryCall` operation — the
/// boundary-settlement lane owns it, not the operator lane — while named
/// `boundary operator` calls in local initializers stop at statement-sequence
/// planning (`call: flow call`) and spelled `boundary operator` uses stop at
/// scalar-local admission (`pure initializer`);
/// `tests::boundary_operator_frontier` in stage 05 pins all three outcomes.
/// Of the remaining `ExactCompilerIntrinsic` families, only the float and
/// integer comparisons are producible: they are the only intrinsic
/// realizations with a Terminal operation of their own, and stage 07's
/// occurrence replay rebuilds only them. Float arithmetic, negation,
/// conversion and builtins have no Terminal operation to bind, and Terminal
/// nearest-FMA is refused at comparison-coverage settlement for lack of
/// selected-provider custody. A selected float guard over receiver fields
/// stops earlier still, at stage 05 scalar field observation
/// (`tests::boundary_operator_frontier`). The `Call*` arms and the FMA tail
/// still bind replayed artifacts naming them; extending the intrinsic arms
/// before an occurrence exists would dead-code this lane, so the arm to
/// extend is whichever family the next replay change first makes survivable.
///
/// Production native publication admits physical evidence only through
/// `NativePhysicalEvidenceScope::from_validated_fragment_publication`, so
/// `fragment_publication` is true on every compiler-produced artifact; the
/// non-fragment arms serve the constructed optimized and unoptimized scopes.
/// The driver's `boundary_internal_call_evidence` test witnesses the
/// fragment comparison arm end to end on both canonical architectures.
pub(super) fn derive_operator_physical_span(
    occurrence: &OptimizedOperatorOccurrence,
    realization: &BoundaryApplicationRealization,
    module: &terminal_psi::TerminalModule,
    target: NativeTarget,
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
    fragment_publication: bool,
) -> Result<Option<OperatorPhysicalSpan>, &'static str> {
    let operation = exact_terminal_operation(module, occurrence)?;
    match realization {
        BoundaryApplicationRealization::NongenericCheckedBody { .. }
        | BoundaryApplicationRealization::SpecializedCheckedBody { .. } => {
            if fragment_publication {
                fragment_call::derive(occurrence, operation, target, object, image)
            } else {
                derive_checked_call_span(occurrence, operation, target, object, image)
            }
        }
        BoundaryApplicationRealization::ExactCompilerIntrinsic { execution } => {
            if fragment_publication
                && matches!(operation.kind, OperationKind::IeeeFloatCompare { .. })
            {
                return fragment_comparison::derive(occurrence, object, image);
            }
            if matches!(
                operation.kind,
                OperationKind::IntegerEqual { .. }
                    | OperationKind::IntegerLessThan { .. }
                    | OperationKind::IntegerLessOrEqual { .. }
            ) {
                return derive_integer_comparison_span(occurrence, execution, object, image);
            }
            derive_fma_span(occurrence, operation, object, image)
        }
    }
}

fn exact_terminal_operation<'module>(
    module: &'module terminal_psi::TerminalModule,
    occurrence: &OptimizedOperatorOccurrence,
) -> Result<&'module terminal_psi::Operation, &'static str> {
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
    operation: &terminal_psi::Operation,
    target: NativeTarget,
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
) -> Result<Option<OperatorPhysicalSpan>, &'static str> {
    let expected_callee = match &operation.kind {
        OperationKind::Call { callee, .. }
        | OperationKind::CallUnit { callee, .. }
        | OperationKind::CallStructural { callee, .. }
        | OperationKind::CallStructuralScalar { callee, .. }
        | OperationKind::CallStructuralWithScalarArguments { callee, .. } => *callee,
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
        &[(relocation.offset, relocation.byte_width)],
    )
    .map(Some)
}

#[allow(clippy::too_many_arguments)]
fn validate_checked_call_relocation(
    expected_callee: semantic_vocabulary::MachineId,
    emitted_callee: semantic_vocabulary::MachineId,
    byte_count: usize,
    architecture: Architecture,
    caller_symbol: object_file::ObjectSymbolHandle,
    callee_symbol: object_file::ObjectSymbolHandle,
    operation: semantic_vocabulary::OperationId,
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
    operation: &terminal_psi::Operation,
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
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
        &[],
    )
    .map(Some)
}

/// Selected integer comparisons emit instruction-only operations with no
/// dedicated roster; the exact span is the provenance-attributed byte
/// interval for the surviving Terminal operation.
fn derive_integer_comparison_span(
    occurrence: &OptimizedOperatorOccurrence,
    execution: &effects::CompilerIntrinsicExecutionIdentity,
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
) -> Result<Option<OperatorPhysicalSpan>, &'static str> {
    if !matches!(
        execution,
        effects::CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison { .. }
    ) {
        return Err("D29 integer comparison rejoined a mismatched intrinsic realization");
    }
    let function = object
        .functions()
        .iter()
        .find(|function| function.machine == occurrence.machine())
        .ok_or("D29 integer comparison names an absent object function")?;
    let mut attributed = object.semantic_code_attribution().iter().filter(|row| {
        row.machine == function.machine
            && row.attribution.site
                == machine_code::SemanticCodeSite::Operation(occurrence.operation())
            && row.attribution.operation_ordinal == occurrence.operation_ordinal()
            && function
                .text_offset
                .checked_add(row.attribution.code_offset)
                == Some(row.text_offset)
    });
    let Some(row) = attributed.next() else {
        return Ok(None);
    };
    if attributed.next().is_some() || row.attribution.byte_count == 0 {
        return Err("D29 integer comparison rejoins ambiguous or empty attribution");
    }
    let object_offset = row.text_offset;
    let object_end = object_offset
        .checked_add(row.attribution.byte_count)
        .ok_or("D29 integer comparison object end overflow")?;
    if object.relocations().records().any(|(_, relocation)| {
        relocation.section == SectionKind::Text
            && ranges_overlap(
                object_offset,
                object_end,
                relocation.offset,
                relocation.offset.saturating_add(relocation.byte_width),
            )
    }) {
        return Err("D29 integer comparison overlaps a relocation");
    }
    derive_span(
        function,
        row.attribution.code_offset,
        row.attribution.byte_count,
        object,
        image,
        PhysicalRelocationDisposition::DirectInstructionBytes,
        &[],
    )
    .map(Some)
}

fn derive_span(
    function: &image_emission::ObjectFunction,
    code_offset: usize,
    byte_count: usize,
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
    disposition: PhysicalRelocationDisposition,
    relocations: &[(usize, usize)],
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
    let final_image_bytes = span(&image.final_text_bytes, final_image)?;
    if machine_bytes != object_bytes {
        return Err("D29 physical child changed before object custody");
    }
    let mut windows = Vec::with_capacity(relocations.len());
    for &(relocation_offset, relocation_width) in relocations {
        let relative_start = relocation_offset
            .checked_sub(object_offset)
            .ok_or("D29 relocation precedes its physical child")?;
        let relative_end = relative_start
            .checked_add(relocation_width)
            .ok_or("D29 relocation span overflow")?;
        if relative_end > byte_count {
            return Err("D29 relocation leaves its physical child");
        }
        windows.push((relative_start, relative_end));
    }
    if object_bytes
        .iter()
        .zip(final_image_bytes)
        .enumerate()
        .any(|(index, (before, after))| {
            before != after
                && !windows
                    .iter()
                    .any(|&(start, end)| index >= start && index < end)
        })
    {
        return Err(if relocations.is_empty() {
            "D29 direct physical child changed before final image custody"
        } else {
            "D29 physical child bytes changed outside its relocation windows"
        });
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
    use super::{
        Architecture, RelocationKind, RelocationOrigin, RelocationRecord, SectionKind,
        validate_checked_call_relocation,
    };

    fn checked_call_relocation() -> RelocationRecord {
        RelocationRecord {
            origin: RelocationOrigin::SemanticOperation {
                function_symbol_handle: object_file::ObjectSymbolHandle::from_arena_index(1),
                operation_identity: 3,
            },
            section: SectionKind::Text,
            offset: 12,
            byte_width: 4,
            symbol_handle: object_file::ObjectSymbolHandle::from_arena_index(2),
            addend: 0,
            kind: RelocationKind::X86_64Relative32,
        }
    }

    fn validate(relocation: &RelocationRecord) -> Result<(), &'static str> {
        validate_checked_call_relocation(
            semantic_vocabulary::MachineId::new(2).expect("callee"),
            semantic_vocabulary::MachineId::new(2).expect("callee"),
            6,
            Architecture::X86_64,
            object_file::ObjectSymbolHandle::from_arena_index(1),
            object_file::ObjectSymbolHandle::from_arena_index(2),
            semantic_vocabulary::OperationId::new(3).expect("operation"),
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
            function_symbol_handle: object_file::ObjectSymbolHandle::from_arena_index(1),
            operation_identity: 4,
        };
        assert!(validate(&relocation).is_err());
    }
}
