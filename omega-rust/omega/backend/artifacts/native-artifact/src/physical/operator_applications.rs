use boundary_applications::BoundaryApplicationRealization;
use object_file::{RelocationKind, RelocationOrigin, RelocationRecord, SectionKind};
use sha2::{Digest, Sha256};
use target::{Architecture, NativeTarget};
use target_operations::CallSiteOwner;
use terminal_psi::OperationKind;

mod fragment_call;
mod fragment_comparison;

use super::derivation::hashing::{
    canonical_usize, hash_object_symbol, hash_relocation_origin, relocation_kind_tag,
};
use super::{
    NativeByteSpan, OptimizedOperatorOccurrence, PhysicalRelocationDisposition,
    dynamic_call_relocation_custody, native_byte_span,
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
        // Dynamic call kinds carry descriptor/parameter ordinals rather than a
        // static callee; they join their own dispatch record families through
        // `derive_dynamic_call_span`, not the resolved call rows here.
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

/// One surviving `CallDynamic*` occurrence's complete emitted custody. Every
/// dispatch family — rebound, stored, parameter, and the forwarded forms —
/// names its Terminal operation and plan ordinal on the record, so the
/// occurrence joins whichever family emitted it; the record's outer
/// `code_offset`/`byte_count` commits the entire emitted sequence rather than
/// the call instruction alone. Descriptor-materializing records additionally
/// carry relocation custody: every Text relocation inside the span is one of
/// the record's validated windows — each table-address materialization joined
/// to its emitted conformance or descriptor table symbol, and a forwarded
/// record's resolved direct call bound to its emitted callee — committed as
/// an ordered digest inside `DynamicCallCustody`.
pub(super) fn derive_dynamic_call_span(
    occurrence: &OptimizedOperatorOccurrence,
    target: NativeTarget,
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
) -> Result<Option<OperatorPhysicalSpan>, &'static str> {
    let function = object
        .functions()
        .iter()
        .find(|function| function.machine == occurrence.machine())
        .ok_or("dynamic call names an absent object function")?;
    let calls = function
        .dynamic_calls
        .iter()
        .map(DynamicCallEmission::Rebound)
        .chain(
            function
                .stored_dynamic_calls
                .iter()
                .map(DynamicCallEmission::Stored),
        )
        .chain(
            function
                .dynamic_parameter_calls
                .iter()
                .map(DynamicCallEmission::Parameter),
        )
        .chain(
            function
                .forwarded_dynamic_parameter_calls
                .iter()
                .map(DynamicCallEmission::ForwardedParameter),
        )
        .chain(
            function
                .forwarded_dynamic_descriptor_calls
                .iter()
                .map(DynamicCallEmission::ForwardedDescriptor),
        )
        .filter(|emission| {
            emission.psi_operation() == occurrence.operation()
                && emission.operation_ordinal() == occurrence.operation_ordinal()
        })
        .collect::<Vec<_>>();
    let [emission] = calls.as_slice() else {
        return if calls.is_empty() {
            Ok(None)
        } else {
            Err("dynamic call rejoins multiple emitted dispatch records")
        };
    };
    let code_offset = emission.code_offset();
    let byte_count = emission.byte_count();
    if byte_count == 0 {
        return Err("dynamic call emitted an empty span");
    }
    let object_offset = function
        .text_offset
        .checked_add(code_offset)
        .ok_or("dynamic call object span overflow")?;
    let object_end = object_offset
        .checked_add(byte_count)
        .ok_or("dynamic call object end overflow")?;
    // The windows the record's own materializations must carry inside the
    // span, plus the resolved direct call forwarded forms end in.
    let mut expected = Vec::new();
    let mut forwarded_call = None;
    match emission {
        DynamicCallEmission::Rebound(call) => {
            let table = object
                .dynamic_conformance_tables()
                .iter()
                .find(|table| table.application == call.dynamic_dispatch.application)
                .ok_or("dynamic call materialization names an absent conformance table")?;
            expected.extend(table_address_relocation_windows(
                function.text_offset,
                &call.table_address,
                table.symbol,
            ));
        }
        DynamicCallEmission::Stored(_) | DynamicCallEmission::Parameter(_) => {}
        DynamicCallEmission::ForwardedParameter(call) => {
            forwarded_call = Some((
                call.callee,
                call.direct_call_offset,
                call.direct_call_byte_count,
            ));
        }
        DynamicCallEmission::ForwardedDescriptor(call) => {
            for argument in &call.dynamic_arguments {
                let application = match &argument.custody.source {
                    abstract_operations::AbstractDynamicDescriptorSource::Selection {
                        application,
                        ..
                    }
                    | abstract_operations::AbstractDynamicDescriptorSource::Rebound {
                        application,
                        ..
                    } => application,
                    abstract_operations::AbstractDynamicDescriptorSource::Parameter(_) => {
                        return Err(
                            "forwarded dynamic descriptor argument carries a parameter source",
                        );
                    }
                };
                let table = object
                    .forwarded_dynamic_descriptor_tables()
                    .iter()
                    .find(|table| table.application == *application)
                    .ok_or("forwarded dynamic descriptor materialization names an absent table")?;
                expected.extend(table_address_relocation_windows(
                    function.text_offset,
                    &argument.table_address,
                    table.symbol,
                ));
            }
            forwarded_call = Some((
                call.callee,
                call.direct_call_offset,
                call.direct_call_byte_count,
            ));
        }
    }
    let mut overlapping = object
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
    let mut windows = match_expected_relocation_windows(
        &mut overlapping,
        &expected,
        function.symbol,
        occurrence.operation(),
    )?;
    match forwarded_call {
        None => {
            if !overlapping.is_empty() {
                return Err("dynamic call emitted sequence carries an unexpected relocation");
            }
        }
        Some((callee, call_offset, call_byte_count)) => {
            let [relocation] = overlapping.as_slice() else {
                return Err("forwarded dynamic call does not contain one exact relocation");
            };
            let target_function = object
                .functions()
                .iter()
                .find(|candidate| candidate.machine == callee)
                .ok_or("forwarded dynamic call names an absent object callee")?;
            let call_start = function
                .text_offset
                .checked_add(call_offset)
                .ok_or("forwarded dynamic call object span overflow")?;
            let call_end = call_start
                .checked_add(call_byte_count)
                .ok_or("forwarded dynamic call object end overflow")?;
            if call_start < object_offset || call_end > object_end {
                return Err("forwarded dynamic call leaves its record's emitted span");
            }
            validate_checked_call_relocation(
                callee,
                callee,
                call_byte_count,
                target.architecture,
                function.symbol,
                target_function.symbol,
                occurrence.operation(),
                call_start,
                call_end,
                relocation,
            )?;
            windows.push((
                relocation.offset,
                relocation.byte_width,
                relocation.kind,
                relocation.symbol_handle,
            ));
        }
    }
    let disposition = match emission {
        DynamicCallEmission::Rebound(_) | DynamicCallEmission::ForwardedDescriptor(_) => {
            PhysicalRelocationDisposition::DynamicCallCustody(dynamic_call_relocation_custody(
                dynamic_call_windows_digest(&windows, function.symbol, occurrence.operation()),
                u32::try_from(windows.len())
                    .map_err(|_| "dynamic call window count overflows custody")?,
            ))
        }
        DynamicCallEmission::Stored(_) | DynamicCallEmission::Parameter(_) => {
            PhysicalRelocationDisposition::DirectInstructionBytes
        }
        DynamicCallEmission::ForwardedParameter(_) => {
            PhysicalRelocationDisposition::ResolvedInternalCall
        }
    };
    let drift_windows = windows
        .iter()
        .map(|&(offset, width, _, _)| (offset, width))
        .collect::<Vec<_>>();
    derive_span(
        function,
        code_offset,
        byte_count,
        object,
        image,
        disposition,
        &drift_windows,
    )
    .map(Some)
}

/// The dispatch record whose `(psi_operation, operation_ordinal)` one
/// surviving `CallDynamic*` occurrence rejoins; the outer
/// `code_offset`/`byte_count` of each record is the operation's complete
/// emitted custody.
enum DynamicCallEmission<'artifact> {
    Rebound(&'artifact machine_code::DynamicCallRecord),
    Stored(&'artifact machine_code::StoredDynamicCallRecord),
    Parameter(&'artifact machine_code::DynamicParameterCallRecord),
    ForwardedParameter(&'artifact machine_code::ForwardedDynamicParameterCallRecord),
    ForwardedDescriptor(&'artifact machine_code::ForwardedDynamicDescriptorCallRecord),
}

impl DynamicCallEmission<'_> {
    fn psi_operation(&self) -> semantic_vocabulary::OperationId {
        match self {
            Self::Rebound(call) => call.psi_operation,
            Self::Stored(call) => call.psi_operation,
            Self::Parameter(call) => call.psi_operation,
            Self::ForwardedParameter(call) => call.psi_operation,
            Self::ForwardedDescriptor(call) => call.psi_operation,
        }
    }

    fn operation_ordinal(&self) -> usize {
        match self {
            Self::Rebound(call) => call.operation_ordinal,
            Self::Stored(call) => call.operation_ordinal,
            Self::Parameter(call) => call.operation_ordinal,
            Self::ForwardedParameter(call) => call.operation_ordinal,
            Self::ForwardedDescriptor(call) => call.operation_ordinal,
        }
    }

    fn code_offset(&self) -> usize {
        match self {
            Self::Rebound(call) => call.code_offset,
            Self::Stored(call) => call.code_offset,
            Self::Parameter(call) => call.code_offset,
            Self::ForwardedParameter(call) => call.code_offset,
            Self::ForwardedDescriptor(call) => call.code_offset,
        }
    }

    fn byte_count(&self) -> usize {
        match self {
            Self::Rebound(call) => call.byte_count,
            Self::Stored(call) => call.byte_count,
            Self::Parameter(call) => call.byte_count,
            Self::ForwardedParameter(call) => call.byte_count,
            Self::ForwardedDescriptor(call) => call.byte_count,
        }
    }
}

/// One relocation a dynamic-call record's emitted sequence must carry at an
/// exact object offset, bound to its emitted table or callee symbol.
#[derive(Clone, Copy)]
struct ExpectedRelocationWindow {
    offset: usize,
    kind: RelocationKind,
    symbol: object_file::ObjectSymbolHandle,
}

/// The relocation windows one table-address materialization owns inside the
/// emitted span: a single relative-32 window on x86-64, or the page and
/// page-offset pair AArch64 addresses the table through.
fn table_address_relocation_windows(
    text_offset: usize,
    materialization: &machine_code::DynamicTableAddressMaterialization,
    table_symbol: object_file::ObjectSymbolHandle,
) -> Vec<ExpectedRelocationWindow> {
    match materialization.encoding {
        machine_code::DynamicTableAddressEncoding::X86_64Relative32 { relocation_offset } => {
            vec![ExpectedRelocationWindow {
                offset: text_offset.saturating_add(relocation_offset),
                kind: RelocationKind::X86_64Relative32,
                symbol: table_symbol,
            }]
        }
        machine_code::DynamicTableAddressEncoding::Aarch64PageAddress {
            page_relocation_offset,
            page_offset_relocation_offset,
        } => vec![
            ExpectedRelocationWindow {
                offset: text_offset.saturating_add(page_relocation_offset),
                kind: RelocationKind::Aarch64Page21,
                symbol: table_symbol,
            },
            ExpectedRelocationWindow {
                offset: text_offset.saturating_add(page_offset_relocation_offset),
                kind: RelocationKind::Aarch64PageOffset12,
                symbol: table_symbol,
            },
        ],
    }
}

/// Match each expected window against one of the relocations overlapping the
/// record's emitted span. The windows are consumed positionally so each
/// materialization binds its own relocation rather than a sibling's; any
/// leftover overlap is the caller's forwarded-call window or an unexpected
/// relocation it must reject.
fn match_expected_relocation_windows(
    overlapping: &mut Vec<&RelocationRecord>,
    expected: &[ExpectedRelocationWindow],
    function_symbol: object_file::ObjectSymbolHandle,
    operation: semantic_vocabulary::OperationId,
) -> Result<
    Vec<(
        usize,
        usize,
        RelocationKind,
        object_file::ObjectSymbolHandle,
    )>,
    &'static str,
> {
    let mut windows = Vec::with_capacity(expected.len());
    for window in expected {
        let Some(index) = overlapping
            .iter()
            .position(|relocation| relocation.offset == window.offset)
        else {
            return Err("dynamic call materialization lacks its relocation window");
        };
        let relocation = overlapping.remove(index);
        validate_dynamic_call_window(*window, function_symbol, operation, relocation)?;
        windows.push((
            relocation.offset,
            relocation.byte_width,
            relocation.kind,
            relocation.symbol_handle,
        ));
    }
    Ok(windows)
}

/// A table materialization's window must be the exact relocation construction
/// emitted for it: this operation's semantic origin, the joined table symbol,
/// the encoding's kind, width four, and addend zero.
fn validate_dynamic_call_window(
    window: ExpectedRelocationWindow,
    function_symbol: object_file::ObjectSymbolHandle,
    operation: semantic_vocabulary::OperationId,
    relocation: &RelocationRecord,
) -> Result<(), &'static str> {
    if relocation.origin
        != (RelocationOrigin::SemanticOperation {
            function_symbol_handle: function_symbol,
            operation_identity: operation.get(),
        })
        || relocation.section != SectionKind::Text
        || relocation.symbol_handle != window.symbol
        || relocation.addend != 0
        || relocation.kind != window.kind
        || relocation.byte_width != 4
    {
        return Err("dynamic call materialization carries an inexact relocation window");
    }
    Ok(())
}

/// Ordered digest of a dynamic-call record's validated relocation windows.
/// The windows share the operation's `SemanticOperation` origin, so the
/// digest commits it once and then each window's exact object fields.
fn dynamic_call_windows_digest(
    windows: &[(
        usize,
        usize,
        RelocationKind,
        object_file::ObjectSymbolHandle,
    )],
    function_symbol: object_file::ObjectSymbolHandle,
    operation: semantic_vocabulary::OperationId,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.native-physical.dynamic-call-windows.sha256.v1\0");
    digest.update(canonical_usize(windows.len()));
    hash_relocation_origin(
        &mut digest,
        RelocationOrigin::SemanticOperation {
            function_symbol_handle: function_symbol,
            operation_identity: operation.get(),
        },
    );
    for &(offset, byte_width, kind, symbol) in windows {
        digest.update(canonical_usize(offset));
        digest.update(canonical_usize(byte_width));
        digest.update([relocation_kind_tag(kind)]);
        hash_object_symbol(&mut digest, symbol);
    }
    digest.finalize().into()
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
        Architecture, ExpectedRelocationWindow, RelocationKind, RelocationOrigin, RelocationRecord,
        SectionKind, dynamic_call_windows_digest, match_expected_relocation_windows,
        table_address_relocation_windows, validate_checked_call_relocation,
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

    fn caller_symbol() -> object_file::ObjectSymbolHandle {
        object_file::ObjectSymbolHandle::from_arena_index(1)
    }

    fn operation() -> semantic_vocabulary::OperationId {
        semantic_vocabulary::OperationId::new(3).expect("operation")
    }

    fn table_window(offset: usize, kind: RelocationKind) -> ExpectedRelocationWindow {
        ExpectedRelocationWindow {
            offset,
            kind,
            symbol: object_file::ObjectSymbolHandle::from_arena_index(9),
        }
    }

    fn table_relocation(offset: usize, kind: RelocationKind) -> RelocationRecord {
        RelocationRecord {
            origin: RelocationOrigin::SemanticOperation {
                function_symbol_handle: caller_symbol(),
                operation_identity: operation().get(),
            },
            section: SectionKind::Text,
            offset,
            byte_width: 4,
            symbol_handle: object_file::ObjectSymbolHandle::from_arena_index(9),
            addend: 0,
            kind,
        }
    }

    #[test]
    fn table_address_windows_follow_the_encoding() {
        let x86 = machine_code::DynamicTableAddressMaterialization {
            code_offset: 8,
            byte_count: 4,
            encoding: machine_code::DynamicTableAddressEncoding::X86_64Relative32 {
                relocation_offset: 11,
            },
        };
        let windows = table_address_relocation_windows(
            100,
            &x86,
            table_window(0, RelocationKind::X86_64Relative32).symbol,
        );
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].offset, 111);
        assert_eq!(windows[0].kind, RelocationKind::X86_64Relative32);

        let aarch64 = machine_code::DynamicTableAddressMaterialization {
            code_offset: 8,
            byte_count: 8,
            encoding: machine_code::DynamicTableAddressEncoding::Aarch64PageAddress {
                page_relocation_offset: 12,
                page_offset_relocation_offset: 16,
            },
        };
        let windows = table_address_relocation_windows(
            100,
            &aarch64,
            table_window(0, RelocationKind::X86_64Relative32).symbol,
        );
        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].offset, 112);
        assert_eq!(windows[0].kind, RelocationKind::Aarch64Page21);
        assert_eq!(windows[1].offset, 116);
        assert_eq!(windows[1].kind, RelocationKind::Aarch64PageOffset12);
    }

    #[test]
    fn expected_windows_match_relocated_records_positionally() {
        let expected = [
            table_window(112, RelocationKind::Aarch64Page21),
            table_window(116, RelocationKind::Aarch64PageOffset12),
        ];
        let page = table_relocation(112, RelocationKind::Aarch64Page21);
        let page_offset = table_relocation(116, RelocationKind::Aarch64PageOffset12);
        // The object carries them in emission order reversed relative to the
        // expected encoding order — matching is by exact offset, not position.
        let mut overlapping = vec![&page_offset, &page];
        let windows = match_expected_relocation_windows(
            &mut overlapping,
            &expected,
            caller_symbol(),
            operation(),
        )
        .expect("both windows match their relocations");
        assert_eq!(
            windows,
            vec![
                (112, 4, RelocationKind::Aarch64Page21, expected[0].symbol),
                (
                    116,
                    4,
                    RelocationKind::Aarch64PageOffset12,
                    expected[0].symbol
                )
            ]
        );
        assert!(overlapping.is_empty());
    }

    #[test]
    fn expected_windows_reject_missing_or_substituted_relocations() {
        let expected = [table_window(112, RelocationKind::Aarch64Page21)];
        // No overlapping relocation at the expected offset.
        let stray = table_relocation(200, RelocationKind::Aarch64Page21);
        let mut overlapping = vec![&stray];
        assert!(
            match_expected_relocation_windows(
                &mut overlapping,
                &expected,
                caller_symbol(),
                operation()
            )
            .is_err()
        );

        // A relocation at the right offset but bound to another symbol rejects.
        let mut wrong_symbol = table_relocation(112, RelocationKind::Aarch64Page21);
        wrong_symbol.symbol_handle = object_file::ObjectSymbolHandle::from_arena_index(10);
        let mut overlapping = vec![&wrong_symbol];
        assert!(
            match_expected_relocation_windows(
                &mut overlapping,
                &expected,
                caller_symbol(),
                operation()
            )
            .is_err()
        );

        // Same for an unexpected addend or width.
        let mut wrong_addend = table_relocation(112, RelocationKind::Aarch64Page21);
        wrong_addend.addend = 4;
        let mut overlapping = vec![&wrong_addend];
        assert!(
            match_expected_relocation_windows(
                &mut overlapping,
                &expected,
                caller_symbol(),
                operation()
            )
            .is_err()
        );

        let mut wrong_origin = table_relocation(112, RelocationKind::Aarch64Page21);
        wrong_origin.origin = RelocationOrigin::Materialization {
            object_symbol_handle: caller_symbol(),
        };
        let mut overlapping = vec![&wrong_origin];
        assert!(
            match_expected_relocation_windows(
                &mut overlapping,
                &expected,
                caller_symbol(),
                operation()
            )
            .is_err()
        );
    }

    #[test]
    fn windows_digest_commits_the_ordered_windows() {
        let windows = [
            (
                112usize,
                4usize,
                RelocationKind::Aarch64Page21,
                object_file::ObjectSymbolHandle::from_arena_index(9),
            ),
            (
                116usize,
                4usize,
                RelocationKind::Aarch64PageOffset12,
                object_file::ObjectSymbolHandle::from_arena_index(9),
            ),
        ];
        let digest = dynamic_call_windows_digest(&windows, caller_symbol(), operation());
        assert_eq!(
            digest,
            dynamic_call_windows_digest(&windows, caller_symbol(), operation())
        );
        // Order is materialization order, so a swapped pair differs.
        let reordered = [windows[1], windows[0]];
        assert_ne!(
            digest,
            dynamic_call_windows_digest(&reordered, caller_symbol(), operation())
        );
        // A different bound table symbol or a different owning operation differs.
        let other_symbol = [
            (
                112,
                4,
                RelocationKind::Aarch64Page21,
                object_file::ObjectSymbolHandle::from_arena_index(10),
            ),
            windows[1],
        ];
        assert_ne!(
            digest,
            dynamic_call_windows_digest(&other_symbol, caller_symbol(), operation())
        );
        let other_operation = semantic_vocabulary::OperationId::new(4).expect("operation");
        assert_ne!(
            digest,
            dynamic_call_windows_digest(&windows, caller_symbol(), other_operation)
        );
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
