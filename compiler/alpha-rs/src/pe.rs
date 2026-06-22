// Windows PE32+ image emission (one of the per-format image writers; a future
// elf.rs / macho.rs would be siblings). Deterministic: TimeDateStamp = 0, no
// timestamps, fixed field order.
//
// Two paths:
//  - no imports (slices 1-2): a single .text section (byte-identical to before).
//  - with imports (slice 3+): .text + .rdata (strings + a kernel32 import table),
//    with RIP-relative relocations in .text patched once RVAs are known.

use crate::util::align_up;
use crate::x64::{ImportFn, Lowered, RelocTarget};

const FILE_ALIGN: u32 = 0x200;
const SECT_ALIGN: u32 = 0x1000;
const IMAGE_BASE: u64 = 0x1_4000_0000;
const TEXT_CHARS: u32 = 0x6000_0020; // CNT_CODE | MEM_EXECUTE | MEM_READ
const RDATA_CHARS: u32 = 0x4000_0040; // CNT_INITIALIZED_DATA | MEM_READ

struct Section {
    name: [u8; 8],
    rva: u32,
    data: Vec<u8>,
    characteristics: u32,
}

fn name8(s: &[u8]) -> [u8; 8] {
    let mut a = [0u8; 8];
    a[..s.len()].copy_from_slice(s);
    a
}
fn put_u32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}
fn put_u64(b: &mut [u8], at: usize, v: u64) {
    b[at..at + 8].copy_from_slice(&v.to_le_bytes());
}
fn align2(v: usize) -> usize {
    (v + 1) & !1
}
fn align8(v: usize) -> usize {
    (v + 7) & !7
}

pub fn build_pe(low: &Lowered) -> Vec<u8> {
    let text_rva = SECT_ALIGN; // 0x1000

    if !low.uses_imports {
        let text = Section {
            name: name8(b".text"),
            rva: text_rva,
            data: low.code.clone(),
            characteristics: TEXT_CHARS,
        };
        return assemble(text_rva, &[text], &[]);
    }

    // Two sections: .text + .rdata (strings followed by the kernel32 import table).
    let mut code = low.code.clone();
    let rdata_rva = text_rva + align_up(code.len() as u32, SECT_ALIGN);
    let rd = build_rdata(&low.rodata, rdata_rva);

    // Patch RIP-relative disp32s now that all RVAs are fixed.
    for r in &low.relocs {
        let target_rva = match &r.target {
            RelocTarget::Rodata(off) => rdata_rva + off,
            RelocTarget::Import(f) => {
                rd.iat_rva
                    + match f {
                        ImportFn::GetStdHandle => 0,
                        ImportFn::WriteFile => 8,
                    }
            }
        };
        let rip = text_rva + r.at + 4; // RVA of the byte after the disp32
        let disp = target_rva as i64 - rip as i64;
        let a = r.at as usize;
        code[a..a + 4].copy_from_slice(&(disp as i32).to_le_bytes());
    }

    let text = Section { name: name8(b".text"), rva: text_rva, data: code, characteristics: TEXT_CHARS };
    let rdata = Section { name: name8(b".rdata"), rva: rdata_rva, data: rd.bytes, characteristics: RDATA_CHARS };
    assemble(
        text_rva,
        &[text, rdata],
        &[(1, rd.idt_rva, rd.idt_size), (12, rd.iat_rva, rd.iat_size)],
    )
}

struct Rdata {
    bytes: Vec<u8>,
    idt_rva: u32,
    idt_size: u32,
    iat_rva: u32,
    iat_size: u32,
}

// Layout of .rdata: [strings][IDT][ILT][IAT][hint/name entries][dll name].
fn build_rdata(rodata: &[u8], rdata_rva: u32) -> Rdata {
    let gsh = b"GetStdHandle";
    let wf = b"WriteFile";
    let dll = b"kernel32.dll";

    let str_len = rodata.len();
    let mut o = align8(str_len);
    let idt_off = o;
    o += 40; // IMAGE_IMPORT_DESCRIPTOR (kernel32) + null terminator (2 * 20)
    let ilt_off = o;
    o += 24; // 2 thunks + null (3 * u64)
    let iat_off = o;
    o += 24;
    let hn_gsh_off = o;
    o += align2(2 + gsh.len() + 1);
    let hn_wf_off = o;
    o += align2(2 + wf.len() + 1);
    let dll_off = o;
    o += align2(dll.len() + 1);
    let total = o;

    let ilt_rva = rdata_rva + ilt_off as u32;
    let iat_rva = rdata_rva + iat_off as u32;
    let dll_rva = rdata_rva + dll_off as u32;
    let hn_gsh_rva = rdata_rva + hn_gsh_off as u32;
    let hn_wf_rva = rdata_rva + hn_wf_off as u32;
    let idt_rva = rdata_rva + idt_off as u32;

    let mut b = vec![0u8; total];
    b[..str_len].copy_from_slice(rodata);

    // IDT[0] = kernel32 (then a zero null-descriptor follows automatically)
    put_u32(&mut b, idt_off, ilt_rva); // OriginalFirstThunk
    put_u32(&mut b, idt_off + 12, dll_rva); // Name
    put_u32(&mut b, idt_off + 16, iat_rva); // FirstThunk

    // ILT and IAT both hold RVAs of the hint/name entries (import-by-name).
    put_u64(&mut b, ilt_off, hn_gsh_rva as u64);
    put_u64(&mut b, ilt_off + 8, hn_wf_rva as u64);
    put_u64(&mut b, iat_off, hn_gsh_rva as u64);
    put_u64(&mut b, iat_off + 8, hn_wf_rva as u64);

    // hint/name entries: u16 hint (0) + name + NUL
    b[hn_gsh_off + 2..hn_gsh_off + 2 + gsh.len()].copy_from_slice(gsh);
    b[hn_wf_off + 2..hn_wf_off + 2 + wf.len()].copy_from_slice(wf);
    // dll name + NUL
    b[dll_off..dll_off + dll.len()].copy_from_slice(dll);

    Rdata { bytes: b, idt_rva, idt_size: 40, iat_rva, iat_size: 24 }
}

fn assemble(entry_rva: u32, sections: &[Section], data_dirs: &[(usize, u32, u32)]) -> Vec<u8> {
    let nsec = sections.len() as u32;
    let headers_unpadded = 0x40 + 4 + 20 + 240 + 40 * nsec;
    let size_of_headers = align_up(headers_unpadded, FILE_ALIGN);

    let mut raw_offsets = Vec::with_capacity(sections.len());
    let mut raw_sizes = Vec::with_capacity(sections.len());
    let mut file_off = size_of_headers;
    for s in sections {
        let raw = align_up(s.data.len() as u32, FILE_ALIGN);
        raw_offsets.push(file_off);
        raw_sizes.push(raw);
        file_off += raw;
    }
    let total_file = file_off;

    let size_of_image = sections
        .iter()
        .map(|s| s.rva + align_up(s.data.len() as u32, SECT_ALIGN))
        .max()
        .unwrap();
    let size_of_code: u32 = sections
        .iter()
        .zip(&raw_sizes)
        .filter(|(s, _)| s.characteristics & 0x20 != 0)
        .map(|(_, &r)| r)
        .sum();
    let size_of_init: u32 = sections
        .iter()
        .zip(&raw_sizes)
        .filter(|(s, _)| s.characteristics & 0x40 != 0)
        .map(|(_, &r)| r)
        .sum();

    let mut o: Vec<u8> = Vec::new();

    // DOS header
    o.extend_from_slice(b"MZ");
    o.resize(0x3C, 0);
    o.extend_from_slice(&0x40u32.to_le_bytes());

    // PE signature + COFF header
    o.extend_from_slice(b"PE\0\0");
    o.extend_from_slice(&0x8664u16.to_le_bytes());
    o.extend_from_slice(&(nsec as u16).to_le_bytes());
    o.extend_from_slice(&0u32.to_le_bytes()); // TimeDateStamp (deterministic)
    o.extend_from_slice(&0u32.to_le_bytes());
    o.extend_from_slice(&0u32.to_le_bytes());
    o.extend_from_slice(&240u16.to_le_bytes());
    o.extend_from_slice(&0x0022u16.to_le_bytes());

    // Optional header (PE32+, 240 bytes)
    let opt = o.len();
    o.extend_from_slice(&0x20Bu16.to_le_bytes());
    o.push(0);
    o.push(0);
    o.extend_from_slice(&size_of_code.to_le_bytes());
    o.extend_from_slice(&size_of_init.to_le_bytes());
    o.extend_from_slice(&0u32.to_le_bytes());
    o.extend_from_slice(&entry_rva.to_le_bytes());
    o.extend_from_slice(&entry_rva.to_le_bytes());
    o.extend_from_slice(&IMAGE_BASE.to_le_bytes());
    o.extend_from_slice(&SECT_ALIGN.to_le_bytes());
    o.extend_from_slice(&FILE_ALIGN.to_le_bytes());
    o.extend_from_slice(&6u16.to_le_bytes());
    o.extend_from_slice(&0u16.to_le_bytes());
    o.extend_from_slice(&0u16.to_le_bytes());
    o.extend_from_slice(&0u16.to_le_bytes());
    o.extend_from_slice(&6u16.to_le_bytes());
    o.extend_from_slice(&0u16.to_le_bytes());
    o.extend_from_slice(&0u32.to_le_bytes());
    o.extend_from_slice(&size_of_image.to_le_bytes());
    o.extend_from_slice(&size_of_headers.to_le_bytes());
    o.extend_from_slice(&0u32.to_le_bytes());
    o.extend_from_slice(&3u16.to_le_bytes()); // Subsystem = console
    o.extend_from_slice(&0u16.to_le_bytes());
    o.extend_from_slice(&0x100000u64.to_le_bytes());
    o.extend_from_slice(&0x1000u64.to_le_bytes());
    o.extend_from_slice(&0x100000u64.to_le_bytes());
    o.extend_from_slice(&0x1000u64.to_le_bytes());
    o.extend_from_slice(&0u32.to_le_bytes());
    o.extend_from_slice(&16u32.to_le_bytes());
    let mut dirs = [(0u32, 0u32); 16];
    for &(idx, rva, size) in data_dirs {
        dirs[idx] = (rva, size);
    }
    for (rva, size) in dirs {
        o.extend_from_slice(&rva.to_le_bytes());
        o.extend_from_slice(&size.to_le_bytes());
    }
    assert_eq!(o.len() - opt, 240);

    // Section headers
    for (i, s) in sections.iter().enumerate() {
        o.extend_from_slice(&s.name);
        o.extend_from_slice(&(s.data.len() as u32).to_le_bytes()); // VirtualSize
        o.extend_from_slice(&s.rva.to_le_bytes());
        o.extend_from_slice(&raw_sizes[i].to_le_bytes());
        o.extend_from_slice(&raw_offsets[i].to_le_bytes());
        o.extend_from_slice(&0u32.to_le_bytes());
        o.extend_from_slice(&0u32.to_le_bytes());
        o.extend_from_slice(&0u16.to_le_bytes());
        o.extend_from_slice(&0u16.to_le_bytes());
        o.extend_from_slice(&s.characteristics.to_le_bytes());
    }

    o.resize(size_of_headers as usize, 0);
    for (i, s) in sections.iter().enumerate() {
        debug_assert_eq!(o.len(), raw_offsets[i] as usize);
        o.extend_from_slice(&s.data);
        o.resize((raw_offsets[i] + raw_sizes[i]) as usize, 0);
    }
    debug_assert_eq!(o.len(), total_file as usize);
    o
}
