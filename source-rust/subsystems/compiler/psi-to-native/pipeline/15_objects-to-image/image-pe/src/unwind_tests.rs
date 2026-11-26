use crate::emit_pe_executable;
use crate::unwind_records::{PeFunctionFrame, read_pe_function_frames};
use arena::Handle;
use calling_conventions::MachineRegister;
use calling_conventions::frame_unwind::{
    FrameProtocolEffect, FrameProtocolStep, FunctionFrameUnwind,
};
use image::{
    FinalFunctionUnwind, FinalImage, FinalImageMemory, FinalImageSection, FinalImageSymbol,
};
use object_file::SymbolKind;
use target::NativeTarget;

fn step(end_offset: u32, effect: FrameProtocolEffect) -> FrameProtocolStep {
    FrameProtocolStep { end_offset, effect }
}

fn saved(register: MachineRegister, frame_offset_bytes: u32) -> FrameProtocolEffect {
    FrameProtocolEffect::Register {
        register,
        frame_offset_bytes,
    }
}

/// An image whose entry is a 16-byte frameless function followed by one
/// framed function of `function_bytes`.
fn image_with(
    target: NativeTarget,
    function_bytes: usize,
    frame: FunctionFrameUnwind,
) -> FinalImage {
    let mut image = FinalImage::with_capacity(
        target,
        FinalImageMemory {
            text: vec![0; 16 + function_bytes],
            ..Default::default()
        },
        Handle::invalid(),
        2,
        0,
        0,
    );
    image.symbol_table.entry_symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "entry".into(),
        section: FinalImageSection::Text,
        size: 16,
        kind: SymbolKind::Function,
        ..Default::default()
    });
    let symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "framed".into(),
        section: FinalImageSection::Text,
        offset: 16,
        size: function_bytes,
        kind: SymbolKind::Function,
    });
    image
        .function_unwinds
        .push(FinalFunctionUnwind { symbol, frame });
    image
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

/// The file offset of `rva` and the exception directory's (rva, size).
fn exception_directory(bytes: &[u8]) -> (u32, u32) {
    let pe = u32_at(bytes, 0x3c) as usize;
    let entry = pe + 24 + 112 + 3 * 8;
    (u32_at(bytes, entry), u32_at(bytes, entry + 4))
}

fn file_offset(bytes: &[u8], rva: u32) -> usize {
    let pe = u32_at(bytes, 0x3c) as usize;
    let optional_size = u16::from_le_bytes(bytes[pe + 20..pe + 22].try_into().unwrap()) as usize;
    let sections = usize::from(u16::from_le_bytes(
        bytes[pe + 6..pe + 8].try_into().unwrap(),
    ));
    (0..sections)
        .map(|index| pe + 24 + optional_size + 40 * index)
        .find_map(|header| {
            let start = u32_at(bytes, header + 12);
            (rva >= start && rva < start + u32_at(bytes, header + 8))
                .then(|| (rva - start) as usize + u32_at(bytes, header + 20) as usize)
        })
        .unwrap()
}

#[test]
fn x64_rows_release_the_frame_and_reload_each_saved_register() {
    let frame = FunctionFrameUnwind {
        prologue: vec![
            step(7, FrameProtocolEffect::AdjustStack { bytes: 4_096 }),
            step(11, FrameProtocolEffect::Unchanged),
            step(15, FrameProtocolEffect::AdjustStack { bytes: 40 }),
            step(19, saved(MachineRegister::X86Rbx, 0)),
            step(24, saved(MachineRegister::X86R12, 8)),
        ],
        epilogue: Vec::new(),
        epilogue_offsets: vec![40],
    };
    let output = emit_pe_executable(image_with(NativeTarget::windows_x64(), 64, frame), 3)
        .expect("emit image");
    assert_eq!(
        read_pe_function_frames(&output.bytes).unwrap(),
        [PeFunctionFrame {
            begin_rva: 0x1010,
            end_rva: 0x1050,
            frame_bytes: 4_136,
            saved: vec![(MachineRegister::X86Rbx, 0), (MachineRegister::X86R12, 8)],
            epilogue_offsets: Vec::new(),
        }]
    );
    // UNWIND_INFO v1, 24-byte prologue, codes last instruction first:
    // SAVE_NONVOL r12 @24 (slot 1), SAVE_NONVOL rbx @19 (slot 0),
    // ALLOC_SMALL 40 @15, ALLOC_LARGE 4096 @7 (512 eightbytes).
    let (directory, _) = exception_directory(&output.bytes);
    let row = file_offset(&output.bytes, directory);
    let info = file_offset(&output.bytes, u32_at(&output.bytes, row + 8));
    assert_eq!(
        &output.bytes[info..info + 4 + 2 * 7],
        &[
            0x01, 24, 7, 0, 24, 0xc4, 1, 0, 19, 0x34, 0, 0, 15, 0x42, 7, 0x01, 0, 2,
        ]
    );
}

#[test]
fn x64_prologue_past_255_bytes_continues_in_chained_rows() {
    // 64 probed pages: each `sub rsp, 4096` (7 bytes) then `cmp byte [rsp], 0` (4 bytes).
    let mut prologue = Vec::new();
    for page in 0..64 {
        prologue.push(step(
            11 * page + 7,
            FrameProtocolEffect::AdjustStack { bytes: 4_096 },
        ));
        prologue.push(step(11 * page + 11, FrameProtocolEffect::Unchanged));
    }
    prologue.push(step(712, saved(MachineRegister::X86Rsi, 262_136)));
    let frame = FunctionFrameUnwind {
        prologue,
        epilogue: Vec::new(),
        epilogue_offsets: Vec::new(),
    };
    let output = emit_pe_executable(image_with(NativeTarget::windows_x64(), 800, frame), 3)
        .expect("emit image");
    assert_eq!(
        exception_directory(&output.bytes).1,
        3 * 12,
        "712 prologue bytes need three rows of at most 255"
    );
    assert_eq!(
        read_pe_function_frames(&output.bytes).unwrap(),
        [PeFunctionFrame {
            begin_rva: 0x1010,
            end_rva: 0x1010 + 800,
            frame_bytes: 262_144,
            saved: vec![(MachineRegister::X86Rsi, 262_136)],
            epilogue_offsets: Vec::new(),
        }]
    );
}

#[test]
fn arm64_rows_give_one_code_per_prologue_word_and_scope_each_epilogue() {
    let prologue = vec![
        step(4, FrameProtocolEffect::AdjustStack { bytes: 4_096 }),
        step(8, FrameProtocolEffect::Unchanged),
        step(12, FrameProtocolEffect::AdjustStack { bytes: 32 }),
        step(16, saved(MachineRegister::Aarch64X(19), 8)),
        step(20, saved(MachineRegister::Aarch64V(8), 16)),
        step(24, saved(MachineRegister::Aarch64X(30), 24)),
    ];
    let epilogue = vec![
        step(4, saved(MachineRegister::Aarch64X(30), 24)),
        step(8, saved(MachineRegister::Aarch64V(8), 16)),
        step(12, saved(MachineRegister::Aarch64X(19), 8)),
        step(16, FrameProtocolEffect::AdjustStack { bytes: 4_096 }),
        step(20, FrameProtocolEffect::AdjustStack { bytes: 32 }),
    ];
    let frame = FunctionFrameUnwind {
        prologue,
        epilogue,
        epilogue_offsets: vec![40, 76],
    };
    let output = emit_pe_executable(image_with(NativeTarget::windows_arm64(), 100, frame), 3)
        .expect("emit image");
    assert_eq!(
        read_pe_function_frames(&output.bytes).unwrap(),
        [PeFunctionFrame {
            begin_rva: 0x1010,
            end_rva: 0x1010 + 100,
            frame_bytes: 4_128,
            saved: vec![
                (MachineRegister::Aarch64X(19), 8),
                (MachineRegister::Aarch64X(30), 24),
                (MachineRegister::Aarch64V(8), 16),
            ],
            epilogue_offsets: vec![40, 76],
        }]
    );
    let (directory, size) = exception_directory(&output.bytes);
    assert_eq!(size, 8, "one ARM64 row is a start RVA and an .xdata RVA");
    let row = file_offset(&output.bytes, directory);
    assert_eq!(u32_at(&output.bytes, row), 0x1010);
    let xdata = file_offset(&output.bytes, u32_at(&output.bytes, row + 4));
    // 25 function words, two epilogue scopes, six code words.
    assert_eq!(u32_at(&output.bytes, xdata), 25 | 2 << 22 | 6 << 27);
    assert_eq!(u32_at(&output.bytes, xdata + 4), 10 | 11 << 22);
    assert_eq!(u32_at(&output.bytes, xdata + 8), 19 | 11 << 22);
    // Prologue codes, last instruction first: save_reg lr @24, save_freg d8 @16,
    // save_reg x19 @8, alloc_s 32, nop (probe), alloc_m 4096, end. The epilogue
    // codes follow its instructions in order and end with `end` for the `ret`.
    assert_eq!(
        &output.bytes[xdata + 12..xdata + 12 + 20],
        &[
            0xd2, 0xc3, 0xdc, 0x02, 0xd0, 0x01, 0x02, 0xe3, 0xc1, 0x00, 0xe4, 0xd2, 0xc3, 0xdc,
            0x02, 0xd0, 0x01, 0xc1, 0x00, 0x02,
        ]
    );
    assert_eq!(output.bytes[xdata + 32], 0xe4);
}

#[test]
fn arm64_refuses_a_save_its_codes_cannot_reach() {
    let frame = FunctionFrameUnwind {
        prologue: vec![
            step(4, FrameProtocolEffect::AdjustStack { bytes: 528 }),
            step(8, saved(MachineRegister::Aarch64X(30), 520)),
        ],
        epilogue: Vec::new(),
        epilogue_offsets: Vec::new(),
    };
    let error =
        emit_pe_executable(image_with(NativeTarget::windows_arm64(), 16, frame), 3).unwrap_err();
    assert!(format!("{error:?}").contains("504 bytes"), "{error:?}");
}

#[test]
fn frameless_functions_get_no_row() {
    let output = emit_pe_executable(
        image_with(
            NativeTarget::windows_x64(),
            8,
            FunctionFrameUnwind::default(),
        ),
        3,
    )
    .expect("emit image");
    assert_eq!(exception_directory(&output.bytes), (0, 0));
    assert_eq!(read_pe_function_frames(&output.bytes).unwrap(), []);
}

#[test]
fn x64_rows_encode_and_decode_save_xmm128_and_far() {
    let frame = FunctionFrameUnwind {
        prologue: vec![
            step(4, FrameProtocolEffect::AdjustStack { bytes: 64 }),
            step(12, saved(MachineRegister::X86Xmm(6), 16)),
            step(21, saved(MachineRegister::X86Xmm(12), 0x10_0000)),
        ],
        epilogue: Vec::new(),
        epilogue_offsets: vec![40],
    };
    let output = emit_pe_executable(image_with(NativeTarget::windows_x64(), 64, frame), 3)
        .expect("emit image");
    assert_eq!(
        read_pe_function_frames(&output.bytes).unwrap(),
        [PeFunctionFrame {
            begin_rva: 0x1010,
            end_rva: 0x1050,
            frame_bytes: 64,
            saved: vec![
                (MachineRegister::X86Xmm(6), 16),
                (MachineRegister::X86Xmm(12), 0x10_0000),
            ],
            epilogue_offsets: Vec::new(),
        }]
    );
}
