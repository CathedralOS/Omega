/*
We derive the exact machine, object, and final-image byte spans for surviving
operator-application occurrences, covering both checked call realizations and
compiler-intrinsic integer and IEEE floating-point comparisons across ordinary
and fragment-published object routes.
*/

use super::derivation::hashing::sha256;
use boundary_applications::BoundaryApplicationRealization;
use object_file::{RelocationKind, RelocationOrigin, RelocationRecord, SectionKind};
use target::{Architecture, NativeTarget};
use target_operations::CallSiteOwner;
use terminal_psi::OperationKind;

pub(super) use super::derivation::evidence::{ranges_overlap, span, text_relocation_overlaps};
use super::{
    NativeByteSpan, OptimizedOperatorOccurrence, PhysicalRelocationDisposition, native_byte_span,
    sole_terminal_operation,
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

pub(super) mod fragment_call {
    use super::{
        Architecture, CallSiteOwner, NativeTarget, OperationKind, OperatorPhysicalSpan,
        OptimizedOperatorOccurrence, PhysicalRelocationDisposition, SectionKind, derive_span,
        ranges_overlap,
    };

    pub(super) fn derive(
        pass: crate::physical::PhysicalEvidencePass,
        occurrence: &OptimizedOperatorOccurrence,
        operation: &terminal_psi::Operation,
        target: NativeTarget,
        object: &image_emission::ObjectArtifact,
        image: &[u8],
    ) -> Result<Option<OperatorPhysicalSpan>, &'static str> {
        let expected_callee = match operation.kind {
            OperationKind::Call { callee, .. }
            | OperationKind::CallUnit { callee, .. }
            | OperationKind::CallStructural { callee, .. }
            | OperationKind::CallStructuralScalar { callee, .. }
            | OperationKind::CallStructuralWithScalarArguments { callee, .. } => callee,
            _ => return Ok(None),
        };
        let function = object
            .function_on_machine(occurrence.machine())
            .ok_or("D29 fragment call names an absent caller")?;
        let mut calls = function
            .unit_call_stacks
            .iter()
            .map(|call| (call.owner, call.target, call.text_offset))
            .chain(
                function
                    .scalar_call_stacks
                    .iter()
                    .map(|call| (call.owner, call.target, call.text_offset)),
            )
            .filter(|(owner, _, _)| *owner == CallSiteOwner::Operation(occurrence.operation()));
        let (_, call_target, call_offset) = calls
            .next()
            .ok_or("D29 fragment call has no exact stack/call record")?;
        if calls.next().is_some() || call_target != expected_callee {
            return Err("D29 fragment call changed its unique owner or callee");
        }
        let callee = object
            .function_on_machine(expected_callee)
            .ok_or("D29 fragment call names an absent callee")?;
        let opcode = match target.architecture {
            Architecture::X86_64 => call_offset.checked_sub(1),
            Architecture::Aarch64 => Some(call_offset),
        }
        .ok_or("D29 fragment call opcode offset underflow")?;
        let end = call_offset
            .checked_add(4)
            .ok_or("D29 fragment call extent overflow")?;
        let bytes = object
            .text_bytes()
            .get(opcode..end)
            .ok_or("D29 fragment call is outside object text")?;
        pass.verify(|| {
            validate_destination(target.architecture, opcode, bytes, callee.text_offset)
        })?;
        let mut attribution = object.semantic_code_attribution().iter().filter(|row| {
            row.machine == function.machine
                && row.attribution.site
                    == machine_code::SemanticCodeSite::Operation(occurrence.operation())
                && row.attribution.operation_ordinal == occurrence.operation_ordinal()
                && row.text_offset <= opcode
                && row
                    .text_offset
                    .checked_add(row.attribution.byte_count)
                    .is_some_and(|limit| end <= limit)
        });
        let row = attribution
            .next()
            .ok_or("D29 fragment call lacks its exact semantic interval")?;
        if attribution.next().is_some()
            || function
                .text_offset
                .checked_add(row.attribution.code_offset)
                != Some(row.text_offset)
            || object.relocations().records().any(|(_, relocation)| {
                relocation.section == SectionKind::Text
                    && ranges_overlap(
                        opcode,
                        end,
                        relocation.offset,
                        relocation.offset.saturating_add(relocation.byte_width),
                    )
            })
        {
            return Err("D29 resolved fragment call has ambiguous attribution or a relocation");
        }
        derive_span(
            pass,
            function,
            row.attribution.code_offset,
            row.attribution.byte_count,
            object,
            image,
            PhysicalRelocationDisposition::ResolvedInternalCall,
            &[],
        )
        .map(Some)
    }

    pub(crate) fn validate_destination(
        architecture: Architecture,
        opcode: usize,
        bytes: &[u8],
        callee: usize,
    ) -> Result<(), &'static str> {
        let destination = match architecture {
            Architecture::X86_64 => {
                let [0xe8, first, second, third, fourth] = bytes else {
                    return Err("resolved internal call is not an x86 relative call");
                };
                opcode as i128
                    + 5
                    + i128::from(i32::from_le_bytes([*first, *second, *third, *fourth]))
            }
            Architecture::Aarch64 => {
                let word = u32::from_le_bytes(
                    bytes
                        .try_into()
                        .map_err(|_| "resolved internal call width changed")?,
                );
                if word & 0xfc00_0000 != 0x9400_0000 || !opcode.is_multiple_of(4) {
                    return Err("resolved internal call is not an aligned Arm64 BL");
                }
                let displacement = ((word << 6) as i32 >> 6) as i64 * 4;
                opcode as i128 + i128::from(displacement)
            }
        };
        if destination != callee as i128 {
            return Err("resolved internal call displacement names a different callee");
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::{Architecture, validate_destination};

        #[test]
        fn resolved_call_destination_is_exact_on_both_targets() {
            assert!(validate_destination(Architecture::X86_64, 8, &[0xe8, 3, 0, 0, 0], 16).is_ok());
            assert!(
                validate_destination(Architecture::X86_64, 8, &[0xe8, 3, 0, 0, 0], 17).is_err()
            );
            assert!(
                validate_destination(Architecture::Aarch64, 8, &0x9400_0002_u32.to_le_bytes(), 16)
                    .is_ok()
            );
            assert!(
                validate_destination(Architecture::Aarch64, 8, &0x9400_0002_u32.to_le_bytes(), 20)
                    .is_err()
            );
            assert!(
                validate_destination(Architecture::Aarch64, 16, &0x97ff_fffe_u32.to_le_bytes(), 8)
                    .is_ok()
            );
            assert!(
                validate_destination(Architecture::Aarch64, 8, &0x1400_0002_u32.to_le_bytes(), 16)
                    .is_err()
            );
        }
    }
}

mod fragment_comparison {
    use super::{
        OperatorPhysicalSpan, OptimizedOperatorOccurrence, PhysicalRelocationDisposition,
        SectionKind, derive_span, ranges_overlap,
    };
    use image_emission::ObjectCodeAttribution;
    use machine_code::SemanticCodeSite;
    use semantic_vocabulary::{MachineId, OperationId};

    pub(super) fn derive(
        pass: crate::physical::PhysicalEvidencePass,
        occurrence: &OptimizedOperatorOccurrence,
        object: &image_emission::ObjectArtifact,
        image: &[u8],
    ) -> Result<Option<OperatorPhysicalSpan>, &'static str> {
        let mut functions = object
            .functions()
            .iter()
            .filter(|function| function.machine == occurrence.machine());
        let function = functions
            .next()
            .ok_or("D29 comparison names an absent object function")?;
        if functions.next().is_some() {
            return Err("D29 comparison names duplicate object functions");
        }
        let (offset, length) = comparison_interval(
            occurrence.machine(),
            occurrence.operation(),
            occurrence.operation_ordinal(),
            function.text_offset,
            function.byte_count,
            object.semantic_code_attribution(),
        )?;
        let start = function
            .text_offset
            .checked_add(offset)
            .ok_or("D29 comparison text offset overflow")?;
        let end = start
            .checked_add(length)
            .ok_or("D29 comparison text end overflow")?;
        if object.relocations().records().any(|(_, relocation)| {
            relocation.section == SectionKind::Text
                && ranges_overlap(
                    start,
                    end,
                    relocation.offset,
                    relocation.offset.saturating_add(relocation.byte_width),
                )
        }) {
            return Err("D29 comparison direct bytes contain a relocation");
        }
        derive_span(
            pass,
            function,
            offset,
            length,
            object,
            image,
            PhysicalRelocationDisposition::DirectInstructionBytes,
            &[],
        )
        .map(Some)
    }

    fn comparison_interval(
        machine: MachineId,
        operation: OperationId,
        ordinal: usize,
        function_offset: usize,
        function_length: usize,
        rows: &[ObjectCodeAttribution],
    ) -> Result<(usize, usize), &'static str> {
        let site = SemanticCodeSite::Operation(operation);
        let mut first = None;
        let mut end = 0usize;
        for row in rows
            .iter()
            .filter(|row| row.machine == machine && row.attribution.site == site)
        {
            let attribution = &row.attribution;
            let row_end = attribution
                .code_offset
                .checked_add(attribution.byte_count)
                .ok_or("D29 comparison interval overflow")?;
            if attribution.operation_ordinal != ordinal
                || attribution.byte_count == 0
                || row_end > function_length
                || function_offset.checked_add(attribution.code_offset) != Some(row.text_offset)
                || first.is_some() && attribution.code_offset < end
            {
                return Err("D29 comparison changed its exact interval coordinates");
            }
            first.get_or_insert(attribution.code_offset);
            end = row_end;
        }
        let first = first.ok_or("D29 comparison lacks semantic code attribution")?;
        let start = function_offset
            .checked_add(first)
            .ok_or("D29 comparison start overflow")?;
        let limit = function_offset
            .checked_add(end)
            .ok_or("D29 comparison end overflow")?;
        for row in rows {
            if row.machine == machine && row.attribution.site == site {
                continue;
            }
            let row_end = row
                .text_offset
                .checked_add(row.attribution.byte_count)
                .ok_or("D29 neighboring attribution overflow")?;
            if ranges_overlap(start, limit, row.text_offset, row_end)
                || row.attribution.byte_count == 0
                    && start < row.text_offset
                    && row.text_offset < limit
            {
                return Err("D29 comparison interval absorbs another semantic site");
            }
        }
        Ok((first, end - first))
    }

    #[cfg(test)]
    mod tests {
        use super::{
            MachineId, ObjectCodeAttribution, OperationId, SemanticCodeSite, comparison_interval,
        };

        fn row(offset: usize, length: usize) -> ObjectCodeAttribution {
            ObjectCodeAttribution {
                machine: MachineId::new(1).unwrap(),
                attribution: machine_code::SemanticCodeAttribution {
                    site: SemanticCodeSite::Operation(OperationId::new(2).unwrap()),
                    operation_ordinal: 3,
                    code_offset: offset,
                    byte_count: length,
                },
                text_offset: 100 + offset,
            }
        }

        fn check(rows: &[ObjectCodeAttribution]) -> Result<(usize, usize), &'static str> {
            comparison_interval(
                MachineId::new(1).unwrap(),
                OperationId::new(2).unwrap(),
                3,
                100,
                64,
                rows,
            )
        }

        #[test]
        fn comparison_child_covers_all_intervals_and_private_spill_gaps() {
            assert_eq!(check(&[row(8, 12)]), Ok((8, 12)));
            assert_eq!(check(&[row(8, 12), row(24, 16)]), Ok((8, 32)));
            assert_eq!(check(&[row(8, 12), row(20, 16)]), Ok((8, 28)));
        }

        #[test]
        fn comparison_child_rejects_hostile_coordinates_and_foreign_sites() {
            assert!(check(&[]).is_err());
            for mutation in 0..8 {
                let mut rows = vec![row(8, 12), row(24, 16)];
                match mutation {
                    0 => rows[0].attribution.operation_ordinal += 1,
                    1 => rows[0].text_offset += 1,
                    2 => rows[0].attribution.byte_count = 0,
                    3 => rows[1] = row(16, 16),
                    4 => rows[1] = row(60, 16),
                    5 => rows.swap(0, 1),
                    6 => rows[1].attribution.code_offset = usize::MAX,
                    _ => rows
                        .iter_mut()
                        .for_each(|row| row.machine = MachineId::new(9).unwrap()),
                }
                assert!(check(&rows).is_err(), "coordinate mutation {mutation}");
            }
            for (site, width) in [
                (SemanticCodeSite::Operation(OperationId::new(7).unwrap()), 4),
                (
                    SemanticCodeSite::Edge(semantic_vocabulary::EdgeId::new(7).unwrap()),
                    0,
                ),
            ] {
                let mut foreign = row(21, width);
                foreign.attribution.site = site;
                assert!(check(&[row(8, 12), row(24, 16), foreign]).is_err());
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn derive_operator_physical_span(
    pass: crate::physical::PhysicalEvidencePass,
    occurrence: &OptimizedOperatorOccurrence,
    realization: &BoundaryApplicationRealization,
    module: &terminal_psi::TerminalModule,
    target: NativeTarget,
    object: &image_emission::ObjectArtifact,
    image: &[u8],
    fragment_publication: bool,
) -> Result<Option<OperatorPhysicalSpan>, &'static str> {
    let operation = exact_terminal_operation(module, occurrence)?;
    match realization {
        BoundaryApplicationRealization::NongenericCheckedBody { .. }
        | BoundaryApplicationRealization::SpecializedCheckedBody { .. } => {
            if fragment_publication {
                fragment_call::derive(pass, occurrence, operation, target, object, image)
            } else {
                derive_checked_call_span(pass, occurrence, operation, target, object, image)
            }
        }
        BoundaryApplicationRealization::ExactCompilerIntrinsic { execution } => {
            if fragment_publication
                && matches!(operation.kind, OperationKind::IeeeFloatCompare { .. })
            {
                return fragment_comparison::derive(pass, occurrence, object, image);
            }
            if matches!(
                operation.kind,
                OperationKind::IntegerEqual { .. }
                    | terminal_psi::integer_less_or_equal_pair_pattern!(..)
            ) {
                return derive_integer_comparison_span(pass, occurrence, execution, object, image);
            }
            Ok(None)
        }
    }
}

fn exact_terminal_operation<'module>(
    module: &'module terminal_psi::TerminalModule,
    occurrence: &OptimizedOperatorOccurrence,
) -> Result<&'module terminal_psi::Operation, &'static str> {
    let operation = sole_terminal_operation(module, occurrence.machine(), occurrence.operation())
        .ok_or("D29 physical occurrence does not rejoin one Terminal operation")?;
    Ok(operation)
}

fn derive_checked_call_span(
    pass: crate::physical::PhysicalEvidencePass,
    occurrence: &OptimizedOperatorOccurrence,
    operation: &terminal_psi::Operation,
    target: NativeTarget,
    object: &image_emission::ObjectArtifact,
    image: &[u8],
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
        .function_on_machine(occurrence.machine())
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
        pass,
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

fn derive_integer_comparison_span(
    pass: crate::physical::PhysicalEvidencePass,
    occurrence: &OptimizedOperatorOccurrence,
    execution: &effects::CompilerIntrinsicExecutionIdentity,
    object: &image_emission::ObjectArtifact,
    image: &[u8],
) -> Result<Option<OperatorPhysicalSpan>, &'static str> {
    if !matches!(
        execution,
        effects::CompilerIntrinsicExecutionIdentity::PrimitiveIntegerComparison { .. }
    ) {
        return Err("D29 integer comparison rejoined a mismatched intrinsic realization");
    }
    let function = object
        .function_on_machine(occurrence.machine())
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
    if text_relocation_overlaps(object, (object_offset, object_end)) {
        return Err("D29 integer comparison overlaps a relocation");
    }
    derive_span(
        pass,
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

#[allow(clippy::too_many_arguments)]
fn derive_span(
    pass: crate::physical::PhysicalEvidencePass,
    function: &image_emission::ObjectFunction,
    code_offset: usize,
    byte_count: usize,
    object: &image_emission::ObjectArtifact,
    image: &[u8],
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
    let final_image_bytes = span(image, final_image)?;
    if pass.is_receiver() && machine_bytes != object_bytes {
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
    if pass.is_receiver()
        && object_bytes
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
