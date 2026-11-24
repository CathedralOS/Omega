//! Check a Windows image's unwind records against the frame layout plan.
//!
//! The writer builds `.pdata`/`.xdata` from the prologue steps that frame
//! encoding recorded. This check reads the finished bytes back with
//! `image_pe::unwind_records` and compares them with stage 09's frame layout
//! instead: every compiled function whose frame releases bytes or restores a
//! register must have exactly one record over its whole span, and undoing that
//! record must release the planned extent and reload each planned register
//! from its planned slot. A record for any other range is refused.

use crate::ObjectArtifact;
use calling_conventions::MachineRegister;
use diagnostics::Diagnostic;
use image::EmittedImageOutput;
use image_pe::unwind_records::{PeFunctionFrame, read_pe_function_frames};

pub(crate) fn validate(
    artifact: &ObjectArtifact,
    output: &EmittedImageOutput,
) -> Result<(), Diagnostic> {
    let failure = |detail: String| Diagnostic::error(format!("PE unwind records: {detail}"));
    let image_base =
        image_base(&output.bytes).ok_or_else(|| failure("image has no PE32+ image base".into()))?;
    let text_rva = output
        .final_image_layout
        .text_address
        .checked_sub(image_base)
        .and_then(|rva| u32::try_from(rva).ok())
        .ok_or_else(|| failure("text lies below the image base".into()))?;
    let mut expected = Vec::new();
    planned_frames(artifact, 0, text_rva, &mut expected).map_err(failure)?;
    for private in artifact.private_functions() {
        planned_frames(&private.body, private.text_offset, text_rva, &mut expected)
            .map_err(failure)?;
    }
    expected.sort_unstable_by_key(|frame| frame.begin_rva);
    let mut found = read_pe_function_frames(&output.bytes).map_err(failure)?;
    for frame in &mut found {
        frame.epilogue_offsets.clear();
    }
    if found != expected {
        let missing = expected.iter().find(|frame| !found.contains(frame));
        let extra = found.iter().find(|frame| !expected.contains(frame));
        return Err(failure(format!(
            "records differ from the frame plan; planned but not recorded: {missing:?}; recorded but not planned: {extra:?}"
        )));
    }
    Ok(())
}

fn image_base(bytes: &[u8]) -> Option<u64> {
    // ImageBase sits 24 bytes into the PE32+ optional header, which follows
    // the 4-byte signature and the 20-byte COFF header.
    let pe = crate::codec_bytes::u32_at(bytes, 0x3c)? as usize;
    crate::codec_bytes::u64_at(bytes, pe + 24 + 24)
}

/// The frames stage 09 planned for one body, placed at `body_offset` in `.text`.
fn planned_frames(
    body: &ObjectArtifact,
    body_offset: usize,
    text_rva: u32,
    frames: &mut Vec<PeFunctionFrame>,
) -> Result<(), String> {
    let readback = body.emission_input.readback();
    let model = readback.physical.model();
    for layout in &readback.frame.functions {
        if layout.unwind.released_bytes == 0 && layout.unwind.restores.is_empty() {
            continue;
        }
        let function = body.function_on_machine(layout.machine).ok_or_else(|| {
            format!(
                "framed machine {} has no placed function",
                layout.machine.get()
            )
        })?;
        let offset = body_offset
            .checked_add(function.location.text_offset)
            .ok_or("function offset overflows")?;
        let begin_rva = u32::try_from(offset)
            .ok()
            .and_then(|offset| text_rva.checked_add(offset))
            .ok_or("function lies beyond a 32-bit RVA")?;
        let end_rva = u32::try_from(function.location.byte_count)
            .ok()
            .and_then(|size| begin_rva.checked_add(size))
            .ok_or("function lies beyond a 32-bit RVA")?;
        let mut saved = Vec::with_capacity(layout.unwind.restores.len());
        for restore in &layout.unwind.restores {
            let name = model
                .view(restore.view)
                .map(|view| view.name.as_str())
                .ok_or("frame plan restores an unknown register")?;
            saved.push((machine_register(name)?, restore.frame_offset_bytes));
        }
        saved.sort_unstable();
        frames.push(PeFunctionFrame {
            begin_rva,
            end_rva,
            frame_bytes: layout.unwind.released_bytes,
            saved,
            epilogue_offsets: Vec::new(),
        });
    }
    Ok(())
}

fn machine_register(name: &str) -> Result<MachineRegister, String> {
    use MachineRegister as R;
    let numbered = |prefix: &str| {
        name.strip_prefix(prefix)
            .and_then(|number| number.parse::<u8>().ok())
    };
    Ok(match name {
        "rax" => R::X86Rax,
        "rcx" => R::X86Rcx,
        "rdx" => R::X86Rdx,
        "rbx" => R::X86Rbx,
        "rbp" => R::X86Rbp,
        "rsi" => R::X86Rsi,
        "rdi" => R::X86Rdi,
        "r8" => R::X86R8,
        "r9" => R::X86R9,
        "r10" => R::X86R10,
        "r11" => R::X86R11,
        "r12" => R::X86R12,
        "r13" => R::X86R13,
        "r14" => R::X86R14,
        "r15" => R::X86R15,
        _ => match (numbered("x"), numbered("d")) {
            (Some(number), _) if number < 31 => R::Aarch64X(number),
            (_, Some(number)) if number < 32 => R::Aarch64V(number),
            _ => {
                return Err(format!(
                    "frame plan restores `{name}`, which no unwind record names"
                ));
            }
        },
    })
}
