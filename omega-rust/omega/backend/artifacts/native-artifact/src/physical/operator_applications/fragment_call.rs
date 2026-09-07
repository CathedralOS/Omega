//! D29 joins to already resolved calls in the common physical program.

use super::*;

pub(super) fn derive(
    occurrence: &OptimizedOperatorOccurrence,
    operation: &terminal_psi::Operation,
    target: NativeTarget,
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
) -> Result<Option<OperatorPhysicalSpan>, &'static str> {
    let expected_callee = match operation.kind {
        OperationKind::Call { callee, .. } => callee,
        _ => return Ok(None),
    };
    let function = object
        .functions()
        .iter()
        .find(|function| function.machine == occurrence.machine())
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
        .functions()
        .iter()
        .find(|function| function.machine == expected_callee)
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
    validate_destination(target.architecture, opcode, bytes, callee.text_offset)?;
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
    // No relocation mask: placed internal calls must remain byte-identical in
    // the final image, and their displacement already names the exact callee.
    derive_span(
        function,
        row.attribution.code_offset,
        row.attribution.byte_count,
        object,
        image,
        PhysicalRelocationDisposition::ResolvedInternalCall,
        None,
    )
    .map(Some)
}

fn validate_destination(
    architecture: Architecture,
    opcode: usize,
    bytes: &[u8],
    callee: usize,
) -> Result<(), &'static str> {
    let destination = match architecture {
        Architecture::X86_64 => {
            let [0xe8, first, second, third, fourth] = bytes else {
                return Err("D29 fragment call is not an x86 relative call");
            };
            opcode as i128 + 5 + i128::from(i32::from_le_bytes([*first, *second, *third, *fourth]))
        }
        Architecture::Aarch64 => {
            let word = u32::from_le_bytes(
                bytes
                    .try_into()
                    .map_err(|_| "D29 fragment call width changed")?,
            );
            if word & 0xfc00_0000 != 0x9400_0000 || !opcode.is_multiple_of(4) {
                return Err("D29 fragment call is not an aligned Arm64 BL");
            }
            let displacement = ((word << 6) as i32 >> 6) as i64 * 4;
            opcode as i128 + i128::from(displacement)
        }
    };
    if destination != callee as i128 {
        return Err("D29 fragment call displacement names a different callee");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_call_destination_is_exact_on_both_targets() {
        assert!(validate_destination(Architecture::X86_64, 8, &[0xe8, 3, 0, 0, 0], 16).is_ok());
        assert!(validate_destination(Architecture::X86_64, 8, &[0xe8, 3, 0, 0, 0], 17).is_err());
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
