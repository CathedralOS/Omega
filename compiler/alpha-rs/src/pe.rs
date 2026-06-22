// Windows PE32+ image emission (one of the per-format image writers; a future
// elf.rs / macho.rs would be siblings). Deterministic: TimeDateStamp = 0, no
// timestamps, fixed field order.

use crate::util::align_up;

pub fn build_pe(code: &[u8]) -> Vec<u8> {
    const FILE_ALIGN: u32 = 0x200;
    const SECT_ALIGN: u32 = 0x1000;
    const IMAGE_BASE: u64 = 0x1_4000_0000;
    const ENTRY_RVA: u32 = 0x1000;

    let size_of_headers = FILE_ALIGN;
    let code_raw = align_up(code.len() as u32, FILE_ALIGN);
    let size_of_image = SECT_ALIGN + align_up(code.len() as u32, SECT_ALIGN);

    let mut o: Vec<u8> = Vec::new();

    // DOS header (64 bytes): MZ + e_lfanew -> 0x40
    o.extend_from_slice(b"MZ");
    o.resize(0x3C, 0);
    o.extend_from_slice(&0x40u32.to_le_bytes());
    assert_eq!(o.len(), 0x40);

    o.extend_from_slice(b"PE\0\0");

    // COFF file header (20 bytes)
    o.extend_from_slice(&0x8664u16.to_le_bytes());
    o.extend_from_slice(&1u16.to_le_bytes());
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
    o.extend_from_slice(&code_raw.to_le_bytes());
    o.extend_from_slice(&0u32.to_le_bytes());
    o.extend_from_slice(&0u32.to_le_bytes());
    o.extend_from_slice(&ENTRY_RVA.to_le_bytes());
    o.extend_from_slice(&ENTRY_RVA.to_le_bytes());
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
    for _ in 0..16 {
        o.extend_from_slice(&0u64.to_le_bytes());
    }
    assert_eq!(o.len() - opt, 240);

    // Section header: .text
    o.extend_from_slice(b".text\0\0\0");
    o.extend_from_slice(&(code.len() as u32).to_le_bytes());
    o.extend_from_slice(&ENTRY_RVA.to_le_bytes());
    o.extend_from_slice(&code_raw.to_le_bytes());
    o.extend_from_slice(&size_of_headers.to_le_bytes());
    o.extend_from_slice(&0u32.to_le_bytes());
    o.extend_from_slice(&0u32.to_le_bytes());
    o.extend_from_slice(&0u16.to_le_bytes());
    o.extend_from_slice(&0u16.to_le_bytes());
    o.extend_from_slice(&0x6000_0020u32.to_le_bytes());

    o.resize(size_of_headers as usize, 0);
    o.extend_from_slice(code);
    o.resize((size_of_headers + code_raw) as usize, 0);
    o
}
