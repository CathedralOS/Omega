//! Windows' own x64 unwinder reads the rows `emit_pe_executable` writes.
//!
//! We map an emitted image's sections at their RVAs in a buffer, register its
//! exception directory with `RtlAddFunctionTable`, and ask `RtlVirtualUnwind`
//! to undo a frame from a program counter in the prologue, the body and an
//! epilogue. The unwinder only reads the buffer, so nothing has to execute.
//! Each case checks the return address, the caller's stack pointer and the
//! saved registers against values planted in a fake stack.
#![cfg(all(windows, target_arch = "x86_64"))]

use arena::Handle;
use calling_conventions::MachineRegister;
use calling_conventions::frame_unwind::{FrameProtocolEffect, FrameProtocolStep, FunctionFrameUnwind};
use image::{FinalFunctionUnwind, FinalImage, FinalImageMemory, FinalImageSection, FinalImageSymbol};
use object_file::SymbolKind;
use target::NativeTarget;

#[repr(C, align(16))]
struct Context([u8; 0x4d0]);

const RBX: usize = 0x90;
const RSP: usize = 0x98;
const R12: usize = 0xd8;
const RIP: usize = 0xf8;

impl Context {
    fn get(&self, offset: usize) -> u64 {
        u64::from_le_bytes(self.0[offset..offset + 8].try_into().unwrap())
    }
    fn set(&mut self, offset: usize, value: u64) {
        self.0[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn RtlAddFunctionTable(table: *const u8, count: u32, base: u64) -> u8;
    fn RtlDeleteFunctionTable(table: *const u8) -> u8;
    fn RtlLookupFunctionEntry(pc: u64, base: *mut u64, history: *mut u8) -> *const u8;
    fn RtlVirtualUnwind(
        handler_type: u32,
        base: u64,
        pc: u64,
        entry: *const u8,
        context: *mut Context,
        handler_data: *mut *mut u8,
        establisher_frame: *mut u64,
        pointers: *mut u8,
    ) -> *const u8;
}

fn step(end_offset: u32, effect: FrameProtocolEffect) -> FrameProtocolStep {
    FrameProtocolStep { end_offset, effect }
}

fn saved(register: MachineRegister, frame_offset_bytes: u32) -> FrameProtocolEffect {
    FrameProtocolEffect::Register {
        register,
        frame_offset_bytes,
    }
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

/// The image's sections laid out at their RVAs, plus its exception directory.
fn mapped(text: Vec<u8>, frame: FunctionFrameUnwind) -> (Vec<u8>, u32, u32) {
    let size = text.len();
    let mut image = FinalImage::with_capacity(
        NativeTarget::windows_x64(),
        FinalImageMemory {
            text,
            ..Default::default()
        },
        Handle::invalid(),
        1,
        0,
        0,
    );
    let symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "framed".into(),
        section: FinalImageSection::Text,
        size,
        kind: SymbolKind::Function,
        ..Default::default()
    });
    image.symbol_table.entry_symbol = symbol;
    image.function_unwinds.push(FinalFunctionUnwind { symbol, frame });
    let bytes = image_pe::emit_pe_executable(image, 3).expect("emit image").bytes;
    let pe = u32_at(&bytes, 0x3c) as usize;
    let optional = pe + 24;
    let size_of_image = u32_at(&bytes, optional + 56) as usize;
    let mut memory = vec![0_u8; size_of_image];
    let sections = usize::from(u16::from_le_bytes([bytes[pe + 6], bytes[pe + 7]]));
    let optional_size = usize::from(u16::from_le_bytes([bytes[pe + 20], bytes[pe + 21]]));
    for index in 0..sections {
        let header = optional + optional_size + 40 * index;
        let (rva, raw_size, raw) = (
            u32_at(&bytes, header + 12) as usize,
            u32_at(&bytes, header + 16) as usize,
            u32_at(&bytes, header + 20) as usize,
        );
        memory[rva..rva + raw_size].copy_from_slice(&bytes[raw..raw + raw_size]);
    }
    let directory = optional + 112 + 3 * 8;
    (memory, u32_at(&bytes, directory), u32_at(&bytes, directory + 4))
}

struct Unwound {
    rip: u64,
    rsp: u64,
    rbx: u64,
    r12: u64,
}

/// Undo one frame at `pc_offset` into the function, whose stack pointer is
/// `rsp_offset` bytes into `stack`.
fn unwind(memory: &[u8], directory: (u32, u32), pc_offset: u64, stack: &[u64], rsp_offset: usize) -> Unwound {
    let base = memory.as_ptr() as u64;
    let table = memory[directory.0 as usize..].as_ptr();
    let mut context = Context([0; 0x4d0]);
    let rsp = stack.as_ptr() as u64 + rsp_offset as u64;
    context.set(RSP, rsp);
    context.set(RBX, 0xb0b0);
    context.set(R12, 0x1212);
    let pc = base + 0x1000 + pc_offset;
    context.set(RIP, pc);
    unsafe {
        assert_ne!(RtlAddFunctionTable(table, directory.1 / 12, base), 0);
        let mut found_base = 0;
        let entry = RtlLookupFunctionEntry(pc, &mut found_base, std::ptr::null_mut());
        assert!(!entry.is_null(), "no row covers offset {pc_offset:#x}");
        assert_eq!(found_base, base);
        let mut handler_data = std::ptr::null_mut();
        let mut establisher = 0;
        RtlVirtualUnwind(
            0,
            base,
            pc,
            entry,
            &mut context,
            &mut handler_data,
            &mut establisher,
            std::ptr::null_mut(),
        );
        RtlDeleteFunctionTable(table);
    }
    Unwound {
        rip: context.get(RIP),
        rsp: context.get(RSP),
        rbx: context.get(RBX),
        r12: context.get(R12),
    }
}

#[test]
fn windows_unwinder_restores_the_caller_from_prologue_body_and_epilogue() {
    // sub rsp, 40; mov [rsp], rbx; mov [rsp+8], r12; body; epilogue at 0x20.
    let mut text = vec![0x48, 0x83, 0xec, 40, 0x48, 0x89, 0x1c, 0x24, 0x4c, 0x89, 0x64, 0x24, 8];
    text.resize(0x20, 0x90);
    text.extend_from_slice(&[0x4c, 0x8b, 0x64, 0x24, 8, 0x48, 0x8b, 0x1c, 0x24]);
    let epilogue_add = text.len() as u64;
    text.extend_from_slice(&[0x48, 0x83, 0xc4, 40, 0xc3]);
    let frame = FunctionFrameUnwind {
        prologue: vec![
            step(4, FrameProtocolEffect::AdjustStack { bytes: 40 }),
            step(8, saved(MachineRegister::X86Rbx, 0)),
            step(13, saved(MachineRegister::X86R12, 8)),
        ],
        epilogue: Vec::new(),
        epilogue_offsets: vec![0x20],
    };
    let (memory, directory, size) = mapped(text, frame);
    // stack[0..5] is the 40-byte frame; stack[5] is the return address.
    let stack = [0x5b5b_u64, 0x1c1c, 0, 0, 0, 0xcafe, 0];
    let caller = stack.as_ptr() as u64 + 48;

    let body = unwind(&memory, (directory, size), 0x18, &stack, 0);
    assert_eq!((body.rip, body.rsp, body.rbx, body.r12), (0xcafe, caller, 0x5b5b, 0x1c1c));

    // After `sub rsp` only: nothing saved yet, so registers keep their values.
    let early = unwind(&memory, (directory, size), 4, &stack, 0);
    assert_eq!((early.rip, early.rsp, early.rbx, early.r12), (0xcafe, caller, 0xb0b0, 0x1212));

    // At function entry the return address is at the stack pointer.
    let entry = unwind(&memory, (directory, size), 0, &stack, 40);
    assert_eq!((entry.rip, entry.rsp), (0xcafe, caller));

    // At `add rsp, 40` the registers are already reloaded; the unwinder
    // recognizes the epilogue by its bytes and only releases the frame.
    let epilogue = unwind(&memory, (directory, size), epilogue_add, &stack, 0);
    assert_eq!(
        (epilogue.rip, epilogue.rsp, epilogue.rbx, epilogue.r12),
        (0xcafe, caller, 0xb0b0, 0x1212)
    );
}

#[test]
fn windows_unwinder_follows_chained_rows_through_a_long_probed_prologue() {
    // 64 probed pages, then `mov [rsp+0x3fff8], rsi`.
    let mut text = Vec::new();
    let mut prologue = Vec::new();
    for _ in 0..64 {
        text.extend_from_slice(&[0x48, 0x81, 0xec, 0x00, 0x10, 0x00, 0x00]);
        prologue.push(step(text.len() as u32, FrameProtocolEffect::AdjustStack { bytes: 4_096 }));
        text.extend_from_slice(&[0x80, 0x3c, 0x24, 0x00]);
        prologue.push(step(text.len() as u32, FrameProtocolEffect::Unchanged));
    }
    text.extend_from_slice(&[0x48, 0x89, 0xb4, 0x24, 0xf8, 0xff, 0x03, 0x00]);
    prologue.push(step(text.len() as u32, saved(MachineRegister::X86Rsi, 0x3fff8)));
    text.resize(text.len() + 32, 0x90);
    let frame = FunctionFrameUnwind {
        prologue,
        epilogue: Vec::new(),
        epilogue_offsets: Vec::new(),
    };
    let (memory, directory, size) = mapped(text, frame);
    assert_eq!(size, 36, "three chained rows");
    let mut stack = vec![0_u64; 0x40000 / 8 + 2];
    stack[0x40000 / 8] = 0xfeed;
    let caller = stack.as_ptr() as u64 + 0x40008;

    // In the body: every page and the save are undone through the chain.
    let body = unwind(&memory, (directory, size), 0x2d0, &stack, 0);
    assert_eq!((body.rip, body.rsp), (0xfeed, caller));

    // In the second row's prologue after 27 pages: only those are undone.
    let partial = unwind(&memory, (directory, size), 297, &stack, 0x40000 - 27 * 4_096);
    assert_eq!((partial.rip, partial.rsp), (0xfeed, caller));
}
