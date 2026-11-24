/*
Windows finds a function's frame through the exception directory (data
directory 3): a sorted `.pdata` table of function ranges, each pointing at
`.xdata` unwind information. Without a row, the unwinder treats the function as
a leaf whose return address sits at the stack pointer, so exception dispatch
and stack walks that reach an Omega frame read the wrong return address.

We write one row per compiled function that moves the stack pointer or saves a
preserved register (`FinalImage::function_unwinds`), from the prologue and
epilogue steps the frame protocol encoder recorded. Both tables live in
`.rdata` after the import and export directories.

x64 rows hold `UNWIND_INFO` version 1. Each prologue instruction that changes
the frame gets an unwind code keyed by its end offset. `SizeOfProlog` and code
offsets are one byte, so a prologue longer than 255 bytes (a large frame with
many stack-commit probes) is split into consecutive rows; each later row is a
chained `UNWIND_INFO` whose parent is the previous row. The unwinder recognizes
`add rsp, N; ret` epilogues by their bytes, and the earlier register reloads
leave the saved values in place, so x64 needs no epilogue description.

ARM64 rows hold `.xdata` records. Omega's AArch64 prologue is `sub sp`, probe
loads, then one `str` per saved register, which never matches the packed
`.pdata` shapes (those save `x29, lr` pairs with `stp`), so every row uses
unwind codes. ARM64 unwind codes count instructions: the unwinder skips the
codes of prologue instructions that have not run yet. We therefore emit exactly
one code per prologue instruction (`nop` for probes and address computations)
and an epilogue scope for every return, whose codes follow the epilogue
instructions in order.
*/

use calling_conventions::MachineRegister;
use calling_conventions::frame_unwind::{
    FrameProtocolEffect, FrameProtocolStep, FunctionFrameUnwind,
};
use diagnostics::Diagnostic;
use image::FinalImage;

use crate::bytes::write_u32;
use crate::constants::TEXT_RVA;
use crate::imports::PeIsa;
use crate::layout::align_to;

/// `.xdata` followed by the sorted `.pdata` table, ready to place at `base_rva`.
pub(crate) struct PeUnwindTables {
    pub(crate) bytes: Vec<u8>,
    pub(crate) directory_offset: usize,
    pub(crate) directory_size: usize,
}

impl PeUnwindTables {
    pub(crate) fn is_empty(&self) -> bool {
        self.directory_size == 0
    }
}

struct FunctionRange<'a> {
    begin: u32,
    end: u32,
    frame: &'a FunctionFrameUnwind,
}

pub(crate) fn build_unwind_tables(
    image: &FinalImage,
    isa: PeIsa,
    base_rva: u32,
) -> Result<PeUnwindTables, Diagnostic> {
    let mut functions = Vec::with_capacity(image.function_unwinds.len());
    for unwind in &image.function_unwinds {
        if unwind.frame.is_frameless() {
            continue;
        }
        if !image.symbol_table.symbols.is_valid(unwind.symbol) {
            return Err(Diagnostic::error("PE unwind row names no image symbol"));
        }
        let symbol = image.symbol_table.symbols.get(unwind.symbol);
        if symbol.section != image::FinalImageSection::Text {
            return Err(Diagnostic::error("PE unwind row names a non-text symbol"));
        }
        let begin = rva(TEXT_RVA, symbol.offset)?;
        let end = rva(begin, symbol.size)?;
        functions.push(FunctionRange {
            begin,
            end,
            frame: &unwind.frame,
        });
    }
    functions.sort_unstable_by_key(|function| function.begin);
    if functions
        .windows(2)
        .any(|pair| pair[0].end > pair[1].begin || pair[0].begin == pair[1].begin)
    {
        return Err(Diagnostic::error("PE unwind rows overlap"));
    }

    let mut xdata = Vec::new();
    let mut rows = Vec::with_capacity(functions.len());
    for function in &functions {
        match isa {
            PeIsa::X86_64 => x64::append_rows(function, base_rva, &mut xdata, &mut rows)?,
            PeIsa::Aarch64 => {
                let info = rva(base_rva, xdata.len())?;
                arm64::append_xdata(function, &mut xdata)?;
                rows.push(PdataRow {
                    begin: function.begin,
                    end: function.end,
                    unwind: info,
                });
            }
        }
    }
    if rows.is_empty() {
        return Ok(PeUnwindTables {
            bytes: Vec::new(),
            directory_offset: 0,
            directory_size: 0,
        });
    }
    let directory_offset = align_to(xdata.len(), 4);
    let mut bytes = xdata;
    bytes.resize(directory_offset, 0);
    for row in &rows {
        write_u32(&mut bytes, row.begin);
        if isa == PeIsa::X86_64 {
            write_u32(&mut bytes, row.end);
        }
        write_u32(&mut bytes, row.unwind);
    }
    let directory_size = bytes.len() - directory_offset;
    Ok(PeUnwindTables {
        bytes,
        directory_offset,
        directory_size,
    })
}

struct PdataRow {
    begin: u32,
    end: u32,
    unwind: u32,
}

fn rva(base: u32, offset: usize) -> Result<u32, Diagnostic> {
    u32::try_from(offset)
        .ok()
        .and_then(|offset| base.checked_add(offset))
        .ok_or_else(|| Diagnostic::error("PE unwind address exceeds a 32-bit RVA"))
}

fn unrepresentable(what: &str) -> Diagnostic {
    Diagnostic::error(format!("PE unwind codes cannot describe {what}"))
}

mod x64 {
    use super::*;

    const UWOP_ALLOC_LARGE: u8 = 1;
    const UWOP_ALLOC_SMALL: u8 = 2;
    const UWOP_SAVE_NONVOL: u8 = 4;
    const UWOP_SAVE_NONVOL_FAR: u8 = 5;
    const UNW_FLAG_CHAININFO: u8 = 4;
    const MAX_PROLOG_BYTES: u32 = 255;

    pub(super) fn append_rows(
        function: &FunctionRange<'_>,
        base_rva: u32,
        xdata: &mut Vec<u8>,
        rows: &mut Vec<PdataRow>,
    ) -> Result<(), Diagnostic> {
        let mut region_start = 0_u32;
        let mut remaining = function.frame.prologue.as_slice();
        let mut parent: Option<(u32, u32, u32)> = None;
        loop {
            let take = remaining
                .iter()
                .take_while(|step| step.end_offset - region_start <= MAX_PROLOG_BYTES)
                .count();
            if take == 0 && !remaining.is_empty() {
                return Err(unrepresentable(
                    "an x64 prologue instruction longer than 255 bytes",
                ));
            }
            let (region, rest) = remaining.split_at(take);
            let region_end = region.last().map_or(region_start, |step| step.end_offset);
            let begin = function
                .begin
                .checked_add(region_start)
                .ok_or_else(|| unrepresentable("a function beyond 4 GiB"))?;
            let end = if rest.is_empty() {
                function.end
            } else {
                function
                    .begin
                    .checked_add(region_end)
                    .ok_or_else(|| unrepresentable("a function beyond 4 GiB"))?
            };
            if end <= begin {
                return Err(unrepresentable("an empty x64 unwind region"));
            }
            let info = rva(base_rva, xdata.len())?;
            append_unwind_info(region, region_start, region_end, parent, xdata)?;
            rows.push(PdataRow {
                begin,
                end,
                unwind: info,
            });
            if rest.is_empty() {
                return Ok(());
            }
            parent = Some((begin, end, info));
            region_start = region_end;
            remaining = rest;
        }
    }

    fn append_unwind_info(
        steps: &[FrameProtocolStep],
        region_start: u32,
        region_end: u32,
        parent: Option<(u32, u32, u32)>,
        xdata: &mut Vec<u8>,
    ) -> Result<(), Diagnostic> {
        // Codes run in reverse prologue order: the unwinder undoes the last
        // instruction first and skips codes whose offset it has not reached.
        let mut slots: Vec<u16> = Vec::new();
        for step in steps.iter().rev() {
            let offset = u16::from((step.end_offset - region_start) as u8);
            match step.effect {
                FrameProtocolEffect::Unchanged => {}
                FrameProtocolEffect::AdjustStack { bytes } => {
                    if bytes == 0 {
                        return Err(unrepresentable("a zero-byte x64 stack adjustment"));
                    }
                    if bytes.is_multiple_of(8) && bytes <= 128 {
                        slots.push(code(offset, UWOP_ALLOC_SMALL, (bytes / 8 - 1) as u8));
                    } else if bytes.is_multiple_of(8) && bytes / 8 <= u32::from(u16::MAX) {
                        slots.push(code(offset, UWOP_ALLOC_LARGE, 0));
                        slots.push((bytes / 8) as u16);
                    } else {
                        slots.push(code(offset, UWOP_ALLOC_LARGE, 1));
                        slots.push(bytes as u16);
                        slots.push((bytes >> 16) as u16);
                    }
                }
                FrameProtocolEffect::Register {
                    register,
                    frame_offset_bytes,
                } => {
                    let number = general_register(register)?;
                    if frame_offset_bytes.is_multiple_of(8)
                        && frame_offset_bytes / 8 <= u32::from(u16::MAX)
                    {
                        slots.push(code(offset, UWOP_SAVE_NONVOL, number));
                        slots.push((frame_offset_bytes / 8) as u16);
                    } else {
                        slots.push(code(offset, UWOP_SAVE_NONVOL_FAR, number));
                        slots.push(frame_offset_bytes as u16);
                        slots.push((frame_offset_bytes >> 16) as u16);
                    }
                }
            }
        }
        let count = u8::try_from(slots.len())
            .map_err(|_| unrepresentable("more than 255 x64 unwind code slots"))?;
        let flags = if parent.is_some() {
            UNW_FLAG_CHAININFO
        } else {
            0
        };
        xdata.push(1 | (flags << 3));
        xdata.push((region_end - region_start) as u8);
        xdata.push(count);
        xdata.push(0);
        for slot in &slots {
            xdata.extend_from_slice(&slot.to_le_bytes());
        }
        if slots.len() % 2 == 1 {
            xdata.extend_from_slice(&[0, 0]);
        }
        if let Some((begin, end, info)) = parent {
            write_u32(xdata, begin);
            write_u32(xdata, end);
            write_u32(xdata, info);
        }
        Ok(())
    }

    fn code(offset: u16, operation: u8, info: u8) -> u16 {
        offset | (u16::from(operation | (info << 4)) << 8)
    }

    /// x64 unwind codes number registers by their instruction encoding.
    fn general_register(register: MachineRegister) -> Result<u8, Diagnostic> {
        use MachineRegister as R;
        Ok(match register {
            R::X86Rax => 0,
            R::X86Rcx => 1,
            R::X86Rdx => 2,
            R::X86Rbx => 3,
            R::X86Rbp => 5,
            R::X86Rsi => 6,
            R::X86Rdi => 7,
            R::X86R8 => 8,
            R::X86R9 => 9,
            R::X86R10 => 10,
            R::X86R11 => 11,
            R::X86R12 => 12,
            R::X86R13 => 13,
            R::X86R14 => 14,
            R::X86R15 => 15,
            _ => {
                return Err(unrepresentable(
                    "a saved register outside the x64 integer file",
                ));
            }
        })
    }
}

mod arm64 {
    use super::*;

    const NOP: u8 = 0xe3;
    const END: u8 = 0xe4;
    const MAX_SCALED_OFFSET: u32 = (1 << 18) - 1;

    pub(super) fn append_xdata(
        function: &FunctionRange<'_>,
        xdata: &mut Vec<u8>,
    ) -> Result<(), Diagnostic> {
        let frame = function.frame;
        let length = function.end - function.begin;
        if !length.is_multiple_of(4) || length / 4 > MAX_SCALED_OFFSET {
            return Err(unrepresentable(
                "an ARM64 function that is not a whole number of words below 1 MiB",
            ));
        }
        let mut codes = Vec::new();
        for step in frame.prologue.iter().rev() {
            append_code(step.effect, &mut codes)?;
        }
        codes.push(END);
        let epilogue_index = codes.len();
        if !frame.epilogue_offsets.is_empty() {
            for step in &frame.epilogue {
                append_code(step.effect, &mut codes)?;
            }
            codes.push(END);
        }
        if epilogue_index > 0x3ff {
            return Err(unrepresentable("ARM64 prologue codes beyond index 1023"));
        }
        let code_words = codes.len().div_ceil(4);
        codes.resize(code_words * 4, NOP);
        let epilogue_count = frame.epilogue_offsets.len();
        let extended = epilogue_count > 31 || code_words > 31;
        let mut header = length / 4;
        if !extended {
            header |= (epilogue_count as u32) << 22 | (code_words as u32) << 27;
        }
        write_u32(xdata, header);
        if extended {
            let epilogues = u16::try_from(epilogue_count)
                .map_err(|_| unrepresentable("more than 65535 ARM64 epilogues"))?;
            let words = u8::try_from(code_words)
                .map_err(|_| unrepresentable("more than 255 ARM64 unwind code words"))?;
            write_u32(xdata, u32::from(epilogues) | u32::from(words) << 16);
        }
        for offset in &frame.epilogue_offsets {
            if !offset.is_multiple_of(4) || offset / 4 > MAX_SCALED_OFFSET {
                return Err(unrepresentable("an ARM64 epilogue beyond 1 MiB"));
            }
            write_u32(xdata, (offset / 4) | ((epilogue_index as u32) << 22));
        }
        xdata.extend_from_slice(&codes);
        Ok(())
    }

    fn append_code(effect: FrameProtocolEffect, codes: &mut Vec<u8>) -> Result<(), Diagnostic> {
        match effect {
            FrameProtocolEffect::Unchanged => codes.push(NOP),
            FrameProtocolEffect::AdjustStack { bytes } => {
                if bytes == 0 || !bytes.is_multiple_of(16) {
                    return Err(unrepresentable("an ARM64 stack adjustment off 16 bytes"));
                }
                let units = bytes / 16;
                if units < 32 {
                    codes.push(units as u8);
                } else if units < 1 << 11 {
                    codes.extend_from_slice(&[0xc0 | (units >> 8) as u8, units as u8]);
                } else if units < 1 << 24 {
                    codes.extend_from_slice(&[
                        0xe0,
                        (units >> 16) as u8,
                        (units >> 8) as u8,
                        units as u8,
                    ]);
                } else {
                    return Err(unrepresentable(
                        "an ARM64 stack adjustment of 256 MiB or more",
                    ));
                }
            }
            FrameProtocolEffect::Register {
                register,
                frame_offset_bytes,
            } => {
                if !frame_offset_bytes.is_multiple_of(8) || frame_offset_bytes > 504 {
                    return Err(unrepresentable(
                        "an ARM64 register saved more than 504 bytes above the stack pointer",
                    ));
                }
                let scaled = (frame_offset_bytes / 8) as u8;
                match register {
                    // save_reg: 110100xx'xxzzzzzz, x(19 + X) at [sp + Z * 8].
                    MachineRegister::Aarch64X(number @ 19..=30) => {
                        let index = number - 19;
                        codes.extend_from_slice(&[0xd0 | (index >> 2), (index & 3) << 6 | scaled]);
                    }
                    // save_freg: 1101110x'xxzzzzzz, d(8 + X) at [sp + Z * 8].
                    MachineRegister::Aarch64V(number @ 8..=15) => {
                        let index = number - 8;
                        codes.extend_from_slice(&[0xdc | (index >> 2), (index & 3) << 6 | scaled]);
                    }
                    _ => {
                        return Err(unrepresentable(
                            "an ARM64 saved register outside x19-x30 and d8-d15",
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "unwind_tests.rs"]
mod tests;
