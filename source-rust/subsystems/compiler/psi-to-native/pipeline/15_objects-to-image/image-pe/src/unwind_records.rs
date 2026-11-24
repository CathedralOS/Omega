/*
We read a finished PE image's exception directory back into the frame each
row describes, without the writer's code or the compiler's prologue steps.
Validators compare the result with the frame layout plan: every function that
moves the stack pointer or saves a register needs a row, and undoing the row's
codes must release the planned frame and restore each planned register from its
planned slot. Neither dumpbin nor llvm-readobj has to be installed for this.

x64 rows that chain to a parent (`UNW_FLAG_CHAININFO`) describe the rest of one
function's prologue, so we fold a chain into the function it starts.
*/

use calling_conventions::MachineRegister;

/// One function's frame as its unwind records describe it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeFunctionFrame {
    pub begin_rva: u32,
    pub end_rva: u32,
    /// Bytes the unwinder adds back to the stack pointer.
    pub frame_bytes: u64,
    /// Registers the unwinder reloads, with their offset above the
    /// post-prologue stack pointer, in ascending register order.
    pub saved: Vec<(MachineRegister, u64)>,
    /// ARM64 epilogue starts, function-relative. Empty on x64.
    pub epilogue_offsets: Vec<u32>,
}

pub fn read_pe_function_frames(image: &[u8]) -> Result<Vec<PeFunctionFrame>, String> {
    let pe = PeView::parse(image)?;
    let (directory_rva, directory_size) = pe.directory(3)?;
    if directory_size == 0 {
        return Ok(Vec::new());
    }
    match pe.machine {
        0x8664 => read_x64(&pe, directory_rva, directory_size),
        0xaa64 => read_arm64(&pe, directory_rva, directory_size),
        machine => Err(format!(
            "unwind records for machine {machine:#x} are unknown"
        )),
    }
}

struct PeView<'a> {
    bytes: &'a [u8],
    machine: u16,
    optional_header: usize,
    sections: Vec<(u32, u32, usize, usize)>,
}

impl<'a> PeView<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        let u16_at = |offset: usize| read_u16(bytes, offset);
        let u32_at = |offset: usize| read_u32(bytes, offset);
        let pe = u32_at(0x3c)? as usize;
        if bytes.get(pe..pe + 4) != Some(b"PE\0\0") {
            return Err("image has no PE signature".into());
        }
        let machine = u16_at(pe + 4)?;
        let section_count = usize::from(u16_at(pe + 6)?);
        let optional_size = usize::from(u16_at(pe + 20)?);
        let optional_header = pe + 24;
        let mut sections = Vec::with_capacity(section_count);
        for index in 0..section_count {
            let header = optional_header + optional_size + 40 * index;
            sections.push((
                u32_at(header + 12)?,
                u32_at(header + 8)?,
                u32_at(header + 20)? as usize,
                u32_at(header + 16)? as usize,
            ));
        }
        Ok(Self {
            bytes,
            machine,
            optional_header,
            sections,
        })
    }

    fn directory(&self, index: usize) -> Result<(u32, u32), String> {
        let entry = self.optional_header + 112 + 8 * index;
        Ok((
            read_u32(self.bytes, entry)?,
            read_u32(self.bytes, entry + 4)?,
        ))
    }

    /// The file bytes backing `length` bytes at `rva`, inside one section.
    fn at(&self, rva: u32, length: usize) -> Result<&'a [u8], String> {
        for &(address, virtual_size, raw, raw_size) in &self.sections {
            let Some(offset) = rva.checked_sub(address) else {
                continue;
            };
            let offset = offset as usize;
            if offset + length <= virtual_size as usize && offset + length <= raw_size {
                return Ok(&self.bytes[raw + offset..raw + offset + length]);
            }
        }
        Err(format!("RVA {rva:#x}+{length} is outside every section"))
    }

    fn u32_at(&self, rva: u32) -> Result<u32, String> {
        read_u32(self.at(rva, 4)?, 0)
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    bytes
        .get(offset..offset + 2)
        .map(|word| u16::from_le_bytes([word[0], word[1]]))
        .ok_or_else(|| format!("read past the image at {offset:#x}"))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    bytes
        .get(offset..offset + 4)
        .map(|word| u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
        .ok_or_else(|| format!("read past the image at {offset:#x}"))
}

fn sorted_rows(
    pe: &PeView<'_>,
    rva: u32,
    size: u32,
    row_size: u32,
) -> Result<Vec<(u32, u32)>, String> {
    if !size.is_multiple_of(row_size) {
        return Err("exception directory is not a whole number of rows".into());
    }
    let mut rows = Vec::with_capacity((size / row_size) as usize);
    for index in 0..size / row_size {
        let row = rva + index * row_size;
        rows.push((row, pe.u32_at(row)?));
    }
    if rows.windows(2).any(|pair| pair[0].1 >= pair[1].1) {
        return Err("exception directory rows are not sorted by function start".into());
    }
    Ok(rows)
}

fn read_x64(pe: &PeView<'_>, rva: u32, size: u32) -> Result<Vec<PeFunctionFrame>, String> {
    let mut frames: Vec<PeFunctionFrame> = Vec::new();
    let mut previous_end = 0;
    for (row, begin) in sorted_rows(pe, rva, size, 12)? {
        let end = pe.u32_at(row + 4)?;
        let info = pe.u32_at(row + 8)?;
        if end <= begin || begin < previous_end {
            return Err(format!("x64 row at {begin:#x} is empty or overlaps"));
        }
        previous_end = end;
        let decoded = decode_x64_info(pe, info)?;
        match decoded.parent {
            None => frames.push(PeFunctionFrame {
                begin_rva: begin,
                end_rva: end,
                frame_bytes: decoded.frame_bytes,
                saved: decoded.saved,
                epilogue_offsets: Vec::new(),
            }),
            Some((parent_begin, parent_end)) => {
                // A chained row continues the function its parent ends.
                let function = frames
                    .last_mut()
                    .filter(|function| function.end_rva == parent_end && parent_end == begin)
                    .filter(|function| function.begin_rva <= parent_begin)
                    .ok_or_else(|| {
                        format!("x64 chained row at {begin:#x} has no adjacent parent")
                    })?;
                function.end_rva = end;
                function.frame_bytes += decoded.frame_bytes;
                function.saved.extend(decoded.saved);
            }
        }
    }
    for frame in &mut frames {
        frame.saved.sort_unstable();
        if frame.saved.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(format!(
                "x64 function at {:#x} restores one register twice",
                frame.begin_rva
            ));
        }
    }
    Ok(frames)
}

struct X64Info {
    frame_bytes: u64,
    saved: Vec<(MachineRegister, u64)>,
    parent: Option<(u32, u32)>,
}

fn decode_x64_info(pe: &PeView<'_>, info: u32) -> Result<X64Info, String> {
    let header = pe.at(info, 4)?;
    let (version, flags) = (header[0] & 7, header[0] >> 3);
    let prolog = header[1];
    let count = usize::from(header[2]);
    if version != 1 || header[3] != 0 || flags & !4 != 0 {
        return Err(format!(
            "x64 UNWIND_INFO at {info:#x} has an unexpected header"
        ));
    }
    let slots_bytes = pe.at(info + 4, 2 * count)?;
    let slot =
        |index: usize| u16::from_le_bytes([slots_bytes[2 * index], slots_bytes[2 * index + 1]]);
    let mut frame_bytes = 0_u64;
    let mut saved = Vec::new();
    let mut index = 0;
    let mut previous_offset = u8::MAX;
    while index < count {
        let code = slot(index);
        let offset = code as u8;
        let (operation, operand) = ((code >> 8) as u8 & 15, (code >> 12) as u8);
        if offset > prolog || offset > previous_offset {
            return Err(format!(
                "x64 unwind code at {info:#x} is out of prologue order"
            ));
        }
        previous_offset = offset;
        let extra = |slots: usize| -> Result<u32, String> {
            if index + slots >= count {
                return Err("x64 unwind code runs past its slots".into());
            }
            Ok(if slots == 1 {
                u32::from(slot(index + 1))
            } else {
                u32::from(slot(index + 1)) | u32::from(slot(index + 2)) << 16
            })
        };
        index += match operation {
            2 => {
                frame_bytes += u64::from(operand) * 8 + 8;
                1
            }
            1 if operand == 0 => {
                frame_bytes += u64::from(extra(1)?) * 8;
                2
            }
            1 if operand == 1 => {
                frame_bytes += u64::from(extra(2)?);
                3
            }
            4 => {
                saved.push((x64_register(operand)?, u64::from(extra(1)?) * 8));
                2
            }
            5 => {
                saved.push((x64_register(operand)?, u64::from(extra(2)?)));
                3
            }
            _ => return Err(format!("x64 unwind operation {operation} is not expected")),
        };
    }
    let parent = if flags & 4 != 0 {
        let parent = info + 4 + 2 * count.next_multiple_of(2) as u32;
        Some((pe.u32_at(parent)?, pe.u32_at(parent + 4)?))
    } else {
        None
    };
    Ok(X64Info {
        frame_bytes,
        saved,
        parent,
    })
}

fn x64_register(number: u8) -> Result<MachineRegister, String> {
    use MachineRegister as R;
    Ok([
        R::X86Rax,
        R::X86Rcx,
        R::X86Rdx,
        R::X86Rbx,
        R::X86Rsp,
        R::X86Rbp,
        R::X86Rsi,
        R::X86Rdi,
        R::X86R8,
        R::X86R9,
        R::X86R10,
        R::X86R11,
        R::X86R12,
        R::X86R13,
        R::X86R14,
        R::X86R15,
    ]
    .get(usize::from(number))
    .copied()
    .filter(|register| *register != R::X86Rsp)
    .ok_or("x64 unwind code saves the stack pointer")?)
}

fn read_arm64(pe: &PeView<'_>, rva: u32, size: u32) -> Result<Vec<PeFunctionFrame>, String> {
    let mut frames: Vec<PeFunctionFrame> = Vec::new();
    for (row, begin) in sorted_rows(pe, rva, size, 8)? {
        let data = pe.u32_at(row + 4)?;
        if data & 3 != 0 {
            return Err(format!("ARM64 row at {begin:#x} uses packed unwind data"));
        }
        let header = pe.u32_at(data)?;
        let length = (header & 0x3_ffff) * 4;
        if (header >> 18) & 3 != 0 || (header >> 20) & 3 != 0 {
            return Err(format!("ARM64 row at {begin:#x} has an unexpected header"));
        }
        let (mut epilogues, mut words) = ((header >> 22) & 31, (header >> 27) & 31);
        let mut cursor = data + 4;
        if epilogues == 0 && words == 0 {
            let extension = pe.u32_at(cursor)?;
            epilogues = extension & 0xffff;
            words = (extension >> 16) & 0xff;
            cursor += 4;
        }
        let mut scopes = Vec::with_capacity(epilogues as usize);
        for _ in 0..epilogues {
            let scope = pe.u32_at(cursor)?;
            scopes.push(((scope & 0x3_ffff) * 4, (scope >> 22) as usize));
            cursor += 4;
        }
        let codes = pe.at(cursor, 4 * words as usize)?;
        let end = begin + length;
        if frames
            .last()
            .is_some_and(|previous| previous.end_rva > begin)
        {
            return Err(format!("ARM64 row at {begin:#x} overlaps its neighbor"));
        }
        let prologue = arm64_codes(codes, 0)?;
        let mut epilogue_offsets = Vec::with_capacity(scopes.len());
        for (offset, index) in scopes {
            if offset >= length {
                return Err(format!(
                    "ARM64 epilogue at {offset:#x} lies outside its function"
                ));
            }
            // An epilogue undoes the same frame the prologue built.
            let epilogue = arm64_codes(codes, index)?;
            let mut restored = epilogue.saved.clone();
            restored.sort_unstable();
            let mut built = prologue.saved.clone();
            built.sort_unstable();
            if epilogue.frame_bytes != prologue.frame_bytes || restored != built {
                return Err(format!(
                    "ARM64 epilogue at {:#x} undoes a different frame than the prologue",
                    begin + offset
                ));
            }
            epilogue_offsets.push(offset);
        }
        let mut saved = prologue.saved;
        saved.sort_unstable();
        frames.push(PeFunctionFrame {
            begin_rva: begin,
            end_rva: end,
            frame_bytes: prologue.frame_bytes,
            saved,
            epilogue_offsets,
        });
    }
    Ok(frames)
}

struct Arm64Frame {
    frame_bytes: u64,
    saved: Vec<(MachineRegister, u64)>,
}

/// Undo codes from `start` to the next `end`, as the unwinder does from the body.
fn arm64_codes(codes: &[u8], start: usize) -> Result<Arm64Frame, String> {
    let mut frame_bytes = 0_u64;
    let mut saved = Vec::new();
    let mut index = start;
    loop {
        let byte = *codes.get(index).ok_or("ARM64 unwind codes have no end")?;
        let next = |count: usize| -> Result<&[u8], String> {
            codes
                .get(index + 1..index + 1 + count)
                .ok_or_else(|| "ARM64 unwind code runs past its words".to_owned())
        };
        index += match byte {
            0x00..=0x1f => {
                frame_bytes += u64::from(byte) * 16;
                1
            }
            0xc0..=0xc7 => {
                let low = next(1)?[0];
                frame_bytes += (u64::from(byte & 7) << 8 | u64::from(low)) * 16;
                2
            }
            0xd0..=0xd3 | 0xdc..=0xdd => {
                let low = next(1)?[0];
                let offset = u64::from(low & 63) * 8;
                let register = if byte <= 0xd3 {
                    MachineRegister::Aarch64X(19 + ((byte & 3) << 2 | low >> 6))
                } else {
                    MachineRegister::Aarch64V(8 + ((byte & 1) << 2 | low >> 6))
                };
                saved.push((register, offset));
                2
            }
            0xe0 => {
                let size = next(3)?;
                frame_bytes +=
                    (u64::from(size[0]) << 16 | u64::from(size[1]) << 8 | u64::from(size[2])) * 16;
                4
            }
            0xe3 => 1,
            0xe4 => break,
            _ => return Err(format!("ARM64 unwind code {byte:#04x} is not expected")),
        };
    }
    Ok(Arm64Frame { frame_bytes, saved })
}
