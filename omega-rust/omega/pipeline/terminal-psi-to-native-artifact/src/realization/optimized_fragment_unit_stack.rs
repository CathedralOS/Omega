//! Object-boundary Unit stack evidence for optimized function fragments.
//!
//! `image-emission` requires every AArch64 Unit function to carry a
//! function-lifetime frame and a saved link register, and admits an x86-64
//! Unit leaf with neither. This module derives that evidence from the applied
//! frame bytes and the target-owned geometry that produced them; it emits no
//! instructions and admits no frame of its own.

use machine_code::{
    Aarch64ReturnLinkEvidence, FunctionAppliedFrameProtocol, StackAdjustmentPair, UnitStackEvidence,
};
use post_allocation_machine_to_frame_layout::ReturnAddressFrameCustody;

/// Derive the object boundary's Unit stack evidence from the applied frame
/// bytes and the target-owned geometry that produced them. A frameless route
/// carries neither, which is the x86-64 Unit leaf shape.
pub(super) fn unit_stack_evidence(
    fragment: &machine_code::FunctionFragment,
    return_bytes: &[u8],
    applied_frame: Option<&FunctionAppliedFrameProtocol>,
    frame_layout: Option<&post_allocation_machine_to_frame_layout::ValidatedTargetFrameLayout>,
) -> Result<UnitStackEvidence, &'static str> {
    // A zero-byte protocol adds no prologue or epilogue, which is the shape a
    // frameless x86-64 Unit leaf keeps and the object boundary admits there.
    let applied = applied_frame.filter(|row| {
        row.prologue_byte_count != 0 || row.epilogues.iter().any(|row| row.byte_count != 0)
    });
    let Some(applied) = applied else {
        return Ok(UnitStackEvidence {
            frame: None,
            aarch64_return_link: None,
            stack_alignment: 16,
        });
    };
    let layout = frame_layout
        .ok_or("optimized framed Unit fragment has no target frame layout")?
        .plan()
        .functions
        .iter()
        .find(|row| row.machine == fragment.machine)
        .ok_or("optimized framed Unit fragment has no target frame layout row")?;
    let ReturnAddressFrameCustody::SavedLinkRegister {
        frame_offset_bytes, ..
    } = layout.return_address
    else {
        return Err("optimized framed Unit fragment does not save its return address");
    };
    saved_link_unit_stack_evidence(
        layout.frame_size_bytes,
        frame_offset_bytes,
        applied,
        fragment.byte_count,
        return_bytes.len(),
    )
}

/// The AAPCS64 saved-link protocol allocates the frame and stores the link in
/// the prologue, then reloads and releases in the epilogue that precedes the
/// return. An empty Unit body puts the epilogue immediately after the
/// prologue, so the function is prologue, epilogue, return.
fn saved_link_unit_stack_evidence(
    frame_size_bytes: u64,
    link_offset_bytes: u64,
    applied: &FunctionAppliedFrameProtocol,
    function_byte_count: u64,
    return_byte_count: usize,
) -> Result<UnitStackEvidence, &'static str> {
    const ADJUSTMENT_BYTES: u64 = 4;
    const LINK_BYTES: u64 = 4;
    let [epilogue] = applied.epilogues.as_slice() else {
        return Err("optimized framed Unit fragment requires one applied epilogue");
    };
    if applied.prologue_function_offset != 0
        || applied.prologue_byte_count != ADJUSTMENT_BYTES + LINK_BYTES
        || epilogue.byte_count != LINK_BYTES + ADJUSTMENT_BYTES
        || epilogue.function_offset != applied.prologue_byte_count
        || epilogue.function_offset + epilogue.byte_count + return_byte_count as u64
            != function_byte_count
    {
        return Err("optimized framed Unit fragment is not the exact saved-link protocol shape");
    }
    let byte_size = u32::try_from(frame_size_bytes)
        .map_err(|_| "optimized framed Unit frame size is not encodable")?;
    let frame_byte_offset = u32::try_from(link_offset_bytes)
        .map_err(|_| "optimized framed Unit link offset is not encodable")?;
    Ok(UnitStackEvidence {
        frame: Some(StackAdjustmentPair {
            byte_size,
            allocation_offset: 0,
            allocation_byte_count: ADJUSTMENT_BYTES as usize,
            release_offset: (epilogue.function_offset + LINK_BYTES) as usize,
            release_byte_count: ADJUSTMENT_BYTES as usize,
        }),
        aarch64_return_link: Some(Aarch64ReturnLinkEvidence {
            frame_byte_offset,
            store_offset: ADJUSTMENT_BYTES as usize,
            load_offset: epilogue.function_offset as usize,
        }),
        stack_alignment: 16,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use machine_code::FunctionAppliedFrameEpilogue;
    use selected_instructions::{SelectedBlockId, SelectedInstructionId};
    use semantic_vocabulary::{EdgeId, MachineId};

    const AARCH64_RETURN: &[u8] = &[0xc0, 0x03, 0x5f, 0xd6];

    /// The exact framed AArch64 Unit function the optimized route emits: a
    /// sixteen-byte frame whose link slot sits at offset zero, the epilogue
    /// immediately after the prologue, and the return last.
    /// `image-emission` pins the matching bytes and evidence at its
    /// object boundary.
    #[test]
    fn saved_link_evidence_matches_the_object_boundary_geometry() {
        let applied = FunctionAppliedFrameProtocol {
            machine: MachineId::new(1).expect("nonzero machine"),
            prologue_function_offset: 0,
            prologue_byte_count: 8,
            epilogues: vec![FunctionAppliedFrameEpilogue {
                block: SelectedBlockId(1),
                return_instruction: SelectedInstructionId(1),
                psi_return_edge: EdgeId::new(7).expect("nonzero edge"),
                function_offset: 8,
                byte_count: 8,
            }],
        };
        let evidence = saved_link_unit_stack_evidence(16, 0, &applied, 20, AARCH64_RETURN.len())
            .expect("exact saved-link protocol");
        assert_eq!(
            evidence.frame,
            Some(StackAdjustmentPair {
                byte_size: 16,
                allocation_offset: 0,
                allocation_byte_count: 4,
                release_offset: 12,
                release_byte_count: 4,
            })
        );
        assert_eq!(
            evidence.aarch64_return_link,
            Some(Aarch64ReturnLinkEvidence {
                frame_byte_offset: 0,
                store_offset: 4,
                load_offset: 8,
            })
        );
        assert_eq!(evidence.stack_alignment, 16);

        let mut short_epilogue = applied.clone();
        short_epilogue.epilogues[0].byte_count = 4;
        assert!(saved_link_unit_stack_evidence(16, 0, &short_epilogue, 20, 4).is_err());

        let mut detached_epilogue = applied;
        detached_epilogue.epilogues[0].function_offset = 12;
        assert!(saved_link_unit_stack_evidence(16, 0, &detached_epilogue, 20, 4).is_err());
    }
}
