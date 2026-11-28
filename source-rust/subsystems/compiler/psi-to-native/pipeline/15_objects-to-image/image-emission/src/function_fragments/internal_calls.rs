//! Join closed internal-call plans to current instruction spans.
use super::{Error, host};
use crate::object_input::ObjectEmissionInput;
use machine_code::internal_call_plans::*;
use machine_code::{
    InternalUnitCallArgumentRecord, InternalUnitCallRecord, InternalUnitScalarCallArgumentRecord,
    SemanticCodeAttribution,
};
use semantic_vocabulary::MachineId;

pub(super) fn error(error: InternalCallPlanError) -> Error {
    match error {
        InternalCallPlanError::Mismatch(reason) => Error::Mismatch(reason),
        InternalCallPlanError::Unsupported(reason) => Error::Unsupported(reason),
        InternalCallPlanError::Overflow => Error::Overflow,
    }
}

pub(super) fn plan(
    source: &ObjectEmissionInput,
    machine: MachineId,
) -> Result<&PhysicalFunctionInternalCalls, Error> {
    source
        .emission_facts()
        .physical
        .internal_calls
        .function(machine)
        .map_err(error)
}

pub(super) fn publication(
    source: &ObjectEmissionInput,
    machine: MachineId,
) -> Result<&InternalCallPublication, Error> {
    plan(source, machine)?
        .publication
        .as_ref()
        .map_err(|reason| error(*reason))
}

fn instruction(
    fragment: &machine_code::FunctionFragment,
    id: selected_instructions::SelectedInstructionId,
) -> Result<&machine_code::FunctionFragmentInstructionSpan, Error> {
    let mut rows = fragment.instructions().filter(|row| row.instruction == id);
    let row = rows
        .next()
        .ok_or(Error::Mismatch("Unit call has no instruction span"))?;
    if rows.next().is_some() {
        return Err(Error::Mismatch("Unit call has duplicate instruction spans"));
    }
    Ok(row)
}

fn argument_extent(
    fragment: &machine_code::FunctionFragment,
    plan: &PhysicalArgumentSpan,
    call: &machine_code::FunctionFragmentInstructionSpan,
) -> Result<(usize, usize), Error> {
    let PhysicalArgumentSpan::Copy { extent, transfers } = plan else {
        let start = host(call.offset)?;
        let end = start.checked_add(call.bytes.len()).ok_or(Error::Overflow)?;
        if fragment.bytes.get(start..end) != Some(call.bytes.as_slice()) {
            return Err(Error::Mismatch(
                "call instruction span differs from function bytes",
            ));
        }
        return Ok((start, call.bytes.len()));
    };
    let invalid =
        || Error::Mismatch("owned argument copy differs from physical memory obligations");
    let mut spans = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for id in extent {
        if !seen.insert(*id) {
            return Err(invalid());
        }
        let span = instruction(fragment, *id)?;
        spans.push((host(span.offset)?, span.bytes.len()));
    }
    spans.sort_unstable();
    let start = spans.first().ok_or_else(invalid)?.0;
    let mut end = start;
    for (offset, size) in spans {
        if offset < end {
            return Err(invalid());
        }
        end = offset.checked_add(size).ok_or(Error::Overflow)?;
    }
    // Read/write transports independently delimit the publication interval.
    let mut seen = std::collections::BTreeSet::new();
    let mut first = None;
    let mut transfer_end = 0;
    for span in fragment.instructions() {
        let count = transfers
            .iter()
            .filter(|id| **id == span.instruction)
            .count();
        if count == 0 {
            continue;
        }
        if count != 1 || !seen.insert(span.instruction) || span.bytes.is_empty() {
            return Err(invalid());
        }
        let offset = host(span.offset)?;
        if first.is_some() && offset < transfer_end {
            return Err(invalid());
        }
        first.get_or_insert(offset);
        transfer_end = offset
            .checked_add(span.bytes.len())
            .ok_or(Error::Overflow)?;
    }
    if seen.len() != transfers.len() || first != Some(start) || transfer_end != end {
        return Err(invalid());
    }
    Ok((start, end.checked_sub(start).ok_or(Error::Overflow)?))
}

pub(super) fn place_calls(
    source: &ObjectEmissionInput,
    machine: MachineId,
    rows: &[SemanticCodeAttribution],
) -> Result<Vec<InternalUnitCallRecord>, Error> {
    let plan = publication(source, machine)?;
    let fragment = super::structural::fragment(source, machine)?;
    plan.calls
        .iter()
        .map(|plan| {
            let site = match plan.owner {
                target_vocabulary::CallSiteOwner::Operation(operation) => {
                    machine_code::SemanticCodeSite::Operation(operation)
                }
                target_vocabulary::CallSiteOwner::InvokeNoNormal(edge)
                | target_vocabulary::CallSiteOwner::CleanupAction { edge, .. } => {
                    machine_code::SemanticCodeSite::Edge(edge)
                }
                _ => return Err(Error::Mismatch("Unit call has no attributed span")),
            };
            let attribution = rows
                .iter()
                .find(|row| row.site == site)
                .ok_or(Error::Mismatch("Unit call has no attributed span"))?;
            let call = instruction(fragment, plan.instruction)?;
            if call
                .internal_machine_fixup
                .as_ref()
                .is_none_or(|fixup| fixup.callee != plan.target)
            {
                return Err(Error::Mismatch(
                    "Unit call fixup differs from current target",
                ));
            }
            let code_offset = host(call.offset)?;
            let scalar_arguments = plan
                .scalar_arguments
                .iter()
                .map(|argument| InternalUnitScalarCallArgumentRecord {
                    parameter_index: argument.parameter_index,
                    source: argument.source,
                    destination: argument.destination.clone(),
                    code_offset,
                    byte_count: call.bytes.len(),
                })
                .collect();
            let arguments = plan
                .arguments
                .iter()
                .map(|argument| {
                    let (code_offset, byte_count) =
                        argument_extent(fragment, &argument.span, call)?;
                    let end = code_offset.checked_add(byte_count).ok_or(Error::Overflow)?;
                    let bytes = fragment
                        .bytes
                        .get(code_offset..end)
                        .ok_or(Error::Mismatch("argument extent exceeds function"))?
                        .to_vec();
                    Ok(InternalUnitCallArgumentRecord {
                        place: argument.place,
                        access: argument.access,
                        path: argument.path.clone(),
                        root_structural_type: argument.root_structural_type,
                        structural_type: argument.structural_type,
                        shape: argument.shape,
                        source_byte_offset: argument.source_byte_offset,
                        source_location: argument.source_location,
                        call_stack_bytes: argument.call_stack_bytes,
                        fixed_array_length: argument.fixed_array_length,
                        element_stride: argument.element_stride,
                        source: argument.source.clone(),
                        destination: argument.destination.clone(),
                        code_offset,
                        byte_count,
                        bytes,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            Ok(InternalUnitCallRecord {
                source: plan.source.clone(),
                owner: plan.owner,
                target: plan.target,
                result: plan.semantic_result.map(|result| result.scalar_type),
                semantic_result: plan.semantic_result,
                structural_result: None,
                scalar_arguments,
                arguments,
                claim_transfers: plan.claim_transfers.clone(),
                operation_ordinal: attribution.operation_ordinal,
                code_offset,
                byte_count: call.bytes.len(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use machine_code::{
        FunctionFragment, FunctionFragmentBlockSpan, FunctionFragmentControlProvenance,
        FunctionFragmentInstructionSpan,
    };
    use selected_instructions::{
        MachineAlternativeFamily, MachineAlternativeKey, SelectedBlockId, SelectedInstructionId,
    };
    fn fragment() -> FunctionFragment {
        FunctionFragment {
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            provenance: target_vocabulary::TerminalPsiProvenance {
                operations: vec![],
                edges: vec![],
            },
            byte_count: 12,
            bytes: vec![0x90; 12],
            blocks: vec![FunctionFragmentBlockSpan {
                block: SelectedBlockId(0),
                offset: 0,
                byte_count: 12,
                instructions: [0, 4, 8]
                    .into_iter()
                    .enumerate()
                    .map(|(id, offset)| FunctionFragmentInstructionSpan {
                        instruction: SelectedInstructionId(id as u32),
                        offset,
                        bytes: vec![0x90; 2],
                        alternative: MachineAlternativeKey {
                            family: MachineAlternativeFamily::MaterializeI64,
                            variant: 0,
                        },
                        branch: None,
                        internal_machine_fixup: None,
                        normalized_foreign_call_fixup: None,
                        image_address_fixup: None,
                        provenance: Default::default(),
                        control: FunctionFragmentControlProvenance::None,
                    })
                    .collect(),
            }],
        }
    }
    fn copies() -> PhysicalArgumentSpan {
        PhysicalArgumentSpan::Copy {
            extent: vec![SelectedInstructionId(0), SelectedInstructionId(1)],
            transfers: vec![SelectedInstructionId(0), SelectedInstructionId(1)],
        }
    }
    #[test]
    fn current_internal_copy_spans_preserve_gaps_and_follow_layout() {
        let mut fragment = fragment();
        assert_eq!(
            argument_extent(&fragment, &copies(), &fragment.blocks[0].instructions[2]).unwrap(),
            (0, 6)
        );
        fragment.blocks[0].instructions[1].offset = 6;
        assert_eq!(
            argument_extent(&fragment, &copies(), &fragment.blocks[0].instructions[2]).unwrap(),
            (0, 8)
        );
        assert_eq!(
            argument_extent(
                &fragment,
                &PhysicalArgumentSpan::Call,
                &fragment.blocks[0].instructions[2]
            )
            .unwrap(),
            (8, 2)
        );
    }
    #[test]
    fn current_internal_copy_spans_refuse_missing_duplicate_overlapping_and_empty_transfers() {
        for mutation in 0..6 {
            let mut fragment = fragment();
            match mutation {
                0 => {
                    fragment.blocks[0].instructions.remove(0);
                }
                1 => {
                    let row = fragment.blocks[0].instructions[0].clone();
                    fragment.blocks[0].instructions.push(row);
                }
                2 => fragment.blocks[0].instructions[1].offset = 1,
                3 => fragment.blocks[0].instructions[1].bytes.clear(),
                4 => fragment.blocks[0].instructions[1].offset = u64::MAX,
                _ => fragment.blocks[0].instructions.swap(0, 1),
            }
            let call = fragment.blocks[0]
                .instructions
                .iter()
                .find(|row| row.instruction == SelectedInstructionId(2))
                .unwrap();
            assert!(
                argument_extent(&fragment, &copies(), call).is_err(),
                "mutation {mutation}"
            );
        }
        let fragment = fragment();
        let incomplete = PhysicalArgumentSpan::Copy {
            extent: vec![SelectedInstructionId(0), SelectedInstructionId(1)],
            transfers: vec![SelectedInstructionId(0)],
        };
        assert!(
            argument_extent(&fragment, &incomplete, &fragment.blocks[0].instructions[2]).is_err()
        );
    }
}
