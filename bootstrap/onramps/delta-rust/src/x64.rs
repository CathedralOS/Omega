// x86-64 lowering + machine-code emission. One of the per-platform backends; a
// future aarch64.rs would be a sibling consuming the same AST.
//
// Strategy: a stack-machine evaluator (each expr leaves its result pushed; binary
// ops pop two and push the result) over an rbp-based locals frame. Trap-on-
// overflow per the Alpha spec. Slice 3 adds host calls (write_line) lowered to
// the Win32 ABI with RIP-relative relocations resolved by the PE writer.

use crate::ast::{BinaryOp, Expr, Machine, Pattern, Program, Statement};
use crate::util::align_up;

#[derive(Clone, Copy)]
pub enum ImportFunction {
    GetStdHandle, // IAT slot 0
    WriteFile,    // IAT slot 1
    ReadFile,     // IAT slot 2
}

pub enum RelocationTarget {
    Rodata(u32),            // RIP-relative reference to a byte offset within .rdata strings
    Import(ImportFunction), // RIP-relative indirect call through the IAT slot
    Data(u32), // RIP-relative reference to the provisioned entry receiver in .data
}

pub struct Relocation {
    pub patch_offset: u32, // byte offset in `code` where the little-endian disp32 to patch begins
    pub target: RelocationTarget,
}

pub struct LoweredProgram {
    pub code: Vec<u8>,
    pub relocations: Vec<Relocation>,
    pub rodata: Vec<u8>, // concatenated string payloads; RelocationTarget::Rodata indexes into this
    pub uses_imports: bool,
    pub data_size: u32,  // bytes of the zero-init .data entry receiver, 0 if none
}

fn local_displacement(local_index: usize) -> i32 {
    -(8 * (local_index as i32 + 1)) // local i at [rbp - 8*(i+1)]
}

// Emit `<prefix> ModRM[+disp]` for a memory operand [rbp + displacement], with the
// given ModRM reg field (reg_field). prefix carries REX + opcode.
fn emit_rbp_memory_operand(code: &mut Vec<u8>, prefix: &[u8], reg_field: u8, displacement: i32) {
    code.extend_from_slice(prefix);
    if (-128..=127).contains(&displacement) {
        code.push(0x40 | (reg_field << 3) | 0x05); // mod=01, reg, rm=101 (rbp)
        code.push(displacement as i8 as u8);
    } else {
        code.push(0x80 | (reg_field << 3) | 0x05); // mod=10, reg, rm=101 (rbp+disp32)
        code.extend_from_slice(&displacement.to_le_bytes());
    }
}

fn emit_load_local(code: &mut Vec<u8>, local_index: usize) {
    emit_rbp_memory_operand(code, &[0x8B], 0, local_displacement(local_index)); // mov eax, [rbp+disp]
}

fn emit_store_local(code: &mut Vec<u8>, local_index: usize) {
    emit_rbp_memory_operand(code, &[0x89], 0, local_displacement(local_index)); // mov [rbp+disp], eax
}

// On entry the caller passed the first 4 args in rcx/rdx/r8/r9 (Win64). Spill the
// register at `register_index` into local slot `slot_index`. For a `&mut self`
// method, rcx holds self, so value params start at register_index 1.
fn emit_store_parameter_register(code: &mut Vec<u8>, slot_index: usize, register_index: usize) {
    let displacement = local_displacement(slot_index);
    let (prefix, reg_field): (&[u8], u8) = match register_index {
        0 => (&[0x48, 0x89], 1), // rcx
        1 => (&[0x48, 0x89], 2), // rdx
        2 => (&[0x4C, 0x89], 0), // r8
        _ => (&[0x4C, 0x89], 1), // r9
    };
    emit_rbp_memory_operand(code, prefix, reg_field, displacement);
}

// Emit a memory operand [rax + offset] with the given ModRM reg field, for the
// given opcode (e.g. 0x8B mov-load, 0x89 mov-store). Used for self-field access
// once the self pointer is in rax.
fn emit_rax_indirect(code: &mut Vec<u8>, opcode: u8, reg_field: u8, offset: i32) {
    code.push(opcode);
    if (-128..=127).contains(&offset) {
        code.push(0x40 | (reg_field << 3)); // mod=01, reg, rm=000 (rax+disp8)
        code.push(offset as i8 as u8);
    } else {
        code.push(0x80 | (reg_field << 3)); // mod=10, reg, rm=000 (rax+disp32)
        code.extend_from_slice(&offset.to_le_bytes());
    }
}

// Consume a pushed index off the stack and leave `&self.<array>[index]` in rax:
// bounds-check (trap on out-of-bounds, per the Alpha spec), then
// rax = self_ptr + field_offset + index*element_bytes.
fn emit_self_element_address(
    code: &mut Vec<u8>,
    self_ptr_displacement: i32,
    field_offset: i32,
    count: i32,
    element_bytes: i32,
) {
    code.push(0x58); // pop rax (index)
    code.extend_from_slice(&[0x89, 0xC0]); // mov eax, eax (zero-extend index into rax)
    code.push(0x3D);
    code.extend_from_slice(&count.to_le_bytes()); // cmp eax, count
    code.extend_from_slice(&[0x72, 0x02, 0x0F, 0x0B]); // jb +2 ; ud2 (trap if index >= count, unsigned)
    let shift = match element_bytes {
        1 => 0,
        2 => 1,
        4 => 2,
        _ => 3, // 8
    };
    if shift > 0 {
        code.extend_from_slice(&[0x48, 0xC1, 0xE0, shift]); // shl rax, shift (index * element_bytes)
    }
    if field_offset != 0 {
        code.extend_from_slice(&[0x48, 0x05]);
        code.extend_from_slice(&field_offset.to_le_bytes()); // add rax, field_offset
    }
    emit_rbp_memory_operand(code, &[0x48, 0x03], 0, self_ptr_displacement); // add rax, [rbp+self]
}

// Load a pushed call argument off the stack into its ABI register:
// mov <rcx|rdx|r8|r9>, [rsp + displacement].
fn emit_load_argument_register(code: &mut Vec<u8>, arg_position: usize, displacement: i32) {
    let modrm_bytes: &[u8] = match arg_position {
        0 => &[0x48, 0x8B, 0x4C, 0x24], // rcx, [rsp+disp8]
        1 => &[0x48, 0x8B, 0x54, 0x24], // rdx
        2 => &[0x4C, 0x8B, 0x44, 0x24], // r8
        _ => &[0x4C, 0x8B, 0x4C, 0x24], // r9
    };
    code.extend_from_slice(modrm_bytes);
    code.push(displacement as u8); // disp8 (0..=24, since arg_count <= 4)
}

fn emit_overflow_trap(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x71, 0x02, 0x0F, 0x0B]); // jno +2 ; ud2
}

// eax = (eax <cc> ecx) ? 1 : 0  — signed compare, then setcc al + zero-extend.
fn emit_cmp_set(code: &mut Vec<u8>, setcc: u8) {
    code.extend_from_slice(&[0x39, 0xC8]); // cmp eax, ecx
    code.extend_from_slice(&[0x0F, setcc, 0xC0]); // setcc al
    code.extend_from_slice(&[0x0F, 0xB6, 0xC0]); // movzx eax, al
}

fn emit_epilogue(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x48, 0x89, 0xEC]); // mov rsp, rbp
    code.push(0x5D); // pop rbp
    code.push(0xC3); // ret
}

// call [rip + disp32] (indirect through the IAT) — records a relocation for the disp32.
fn emit_rip_call(code: &mut Vec<u8>, relocations: &mut Vec<Relocation>, target: RelocationTarget) {
    code.extend_from_slice(&[0xFF, 0x15]); // call r/m64, ModRM mod=00 reg=010 rm=101 (RIP)
    let patch_offset = code.len() as u32;
    code.extend_from_slice(&[0, 0, 0, 0]);
    relocations.push(Relocation { patch_offset, target });
}

// lea rdx, [rip + disp32] — records a relocation for the disp32.
fn emit_lea_rdx_rip(code: &mut Vec<u8>, relocations: &mut Vec<Relocation>, target: RelocationTarget) {
    code.extend_from_slice(&[0x48, 0x8D, 0x15]); // lea rdx, [rip+disp32]
    let patch_offset = code.len() as u32;
    code.extend_from_slice(&[0, 0, 0, 0]);
    relocations.push(Relocation { patch_offset, target });
}

fn lower_expression(
    node: usize,
    context: &LoweringContext,
    code: &mut Vec<u8>,
    relocations: &mut Vec<Relocation>,
    call_fixups: &mut Vec<(u32, usize)>,
) {
    match context.program.expressions[node] {
        Expr::Int(value) => {
            code.push(0xB8); // mov eax, imm32
            code.extend_from_slice(&value.to_le_bytes());
            code.push(0x50); // push rax
        }
        Expr::Local(local_index) => {
            emit_load_local(code, local_index);
            code.push(0x50); // push rax
        }
        Expr::SelfField(offset) => {
            emit_rbp_memory_operand(code, &[0x48, 0x8B], 0, context.self_ptr_displacement); // mov rax, [rbp+self]
            emit_rax_indirect(code, 0x8B, 0, offset); // mov eax, [rax+offset]
            code.push(0x50); // push rax
        }
        Expr::SelfIndex(field_offset, count, element_bytes, index) => {
            lower_expression(index, context, code, relocations, call_fixups); // push index
            emit_self_element_address(code, context.self_ptr_displacement, field_offset, count, element_bytes);
            if element_bytes == 1 {
                code.extend_from_slice(&[0x0F, 0xB6, 0x00]); // movzx eax, byte [rax]
            } else {
                code.extend_from_slice(&[0x8B, 0x00]); // mov eax, [rax]
            }
            code.push(0x50); // push rax
        }
        Expr::ReadByte => {
            // handle = GetStdHandle(STD_INPUT_HANDLE = -10)
            code.push(0xB9);
            code.extend_from_slice(&(-10i32).to_le_bytes()); // mov ecx, -10
            emit_rip_call(code, relocations, RelocationTarget::Import(ImportFunction::GetStdHandle));
            emit_rbp_memory_operand(code, &[0x48, 0x89], 0, context.handle_displacement); // mov [rbp+handle], rax
            // ReadFile(handle, &io_byte, 1, &count, NULL)
            emit_rbp_memory_operand(code, &[0x48, 0x8B], 1, context.handle_displacement); // mov rcx, [rbp+handle]
            emit_rbp_memory_operand(code, &[0x48, 0x8D], 2, context.io_byte_displacement); // lea rdx, [rbp+io_byte]
            code.extend_from_slice(&[0x41, 0xB8, 0x01, 0, 0, 0]); // mov r8d, 1
            emit_rbp_memory_operand(code, &[0x4C, 0x8D], 1, context.written_displacement); // lea r9, [rbp+count]
            code.extend_from_slice(&[0x48, 0xC7, 0x44, 0x24, 0x20, 0, 0, 0, 0]); // mov qword [rsp+0x20], 0
            emit_rip_call(code, relocations, RelocationTarget::Import(ImportFunction::ReadFile));
            // result = (bytesRead != 0) ? io_byte : -1   (branchless via cmove)
            emit_rbp_memory_operand(code, &[0x0F, 0xB6], 0, context.io_byte_displacement); // movzx eax, byte [rbp+io_byte]
            code.push(0xB9);
            code.extend_from_slice(&(-1i32).to_le_bytes()); // mov ecx, -1
            emit_rbp_memory_operand(code, &[0x83], 7, context.written_displacement); // cmp dword [rbp+count], imm8
            code.push(0x00);
            code.extend_from_slice(&[0x0F, 0x44, 0xC1]); // cmove eax, ecx
            code.push(0x50); // push rax
        }
        Expr::Call(machine_index, args_start, arg_count) => {
            // Evaluate args left-to-right (each leaves its result pushed), then load
            // them into the ABI registers and discard them so rsp returns to the
            // (16-aligned) frame base before the call. arg i sits at the deeper slot.
            for index in 0..arg_count {
                let arg_node = context.program.call_args[args_start + index];
                lower_expression(arg_node, context, code, relocations, call_fixups);
            }
            for index in 0..arg_count {
                let displacement = (8 * (arg_count - 1 - index)) as i32;
                emit_load_argument_register(code, index, displacement);
            }
            if arg_count > 0 {
                code.extend_from_slice(&[0x48, 0x83, 0xC4, (8 * arg_count) as u8]); // add rsp, imm8
            }
            code.push(0xE8); // call rel32
            let patch_offset = code.len() as u32;
            code.extend_from_slice(&[0, 0, 0, 0]);
            call_fixups.push((patch_offset, machine_index));
            code.push(0x50); // push rax (return value)
        }
        Expr::SelfCall(machine_index, args_start, arg_count) => {
            // Like Call, but self goes in rcx and the args shift to rdx/r8/r9.
            for index in 0..arg_count {
                let arg_node = context.program.call_args[args_start + index];
                lower_expression(arg_node, context, code, relocations, call_fixups);
            }
            for index in 0..arg_count {
                let displacement = (8 * (arg_count - 1 - index)) as i32;
                emit_load_argument_register(code, index + 1, displacement); // args -> rdx/r8/r9
            }
            if arg_count > 0 {
                code.extend_from_slice(&[0x48, 0x83, 0xC4, (8 * arg_count) as u8]); // add rsp, imm8
            }
            emit_rbp_memory_operand(code, &[0x48, 0x8B], 1, context.self_ptr_displacement); // mov rcx, [rbp+self]
            code.push(0xE8); // call rel32
            let patch_offset = code.len() as u32;
            code.extend_from_slice(&[0, 0, 0, 0]);
            call_fixups.push((patch_offset, machine_index));
            code.push(0x50); // push rax (return value)
        }
        Expr::Binary(op, lhs, rhs) => {
            lower_expression(lhs, context, code, relocations, call_fixups);
            lower_expression(rhs, context, code, relocations, call_fixups);
            code.push(0x59); // pop rcx (rhs)
            code.push(0x58); // pop rax (lhs)
            match op {
                BinaryOp::Add => {
                    code.extend_from_slice(&[0x01, 0xC8]);
                    emit_overflow_trap(code);
                }
                BinaryOp::Sub => {
                    code.extend_from_slice(&[0x29, 0xC8]);
                    emit_overflow_trap(code);
                }
                BinaryOp::Mul => {
                    code.extend_from_slice(&[0x0F, 0xAF, 0xC1]);
                    emit_overflow_trap(code);
                }
                BinaryOp::Div => {
                    code.push(0x99); // cdq
                    code.extend_from_slice(&[0xF7, 0xF9]); // idiv ecx (traps on /0, INT_MIN/-1)
                }
                BinaryOp::Rem => {
                    code.push(0x99); // cdq
                    code.extend_from_slice(&[0xF7, 0xF9]); // idiv ecx (same /0 trap as Div)
                    code.extend_from_slice(&[0x89, 0xD0]); // mov eax, edx (remainder)
                }
                // bitwise ops: no overflow possible, so no trap (mirror Add's ModRM)
                BinaryOp::BitAnd => code.extend_from_slice(&[0x21, 0xC8]), // and eax, ecx
                BinaryOp::BitOr => code.extend_from_slice(&[0x09, 0xC8]),  // or  eax, ecx
                BinaryOp::BitXor => code.extend_from_slice(&[0x31, 0xC8]), // xor eax, ecx
                // shifts: count in cl (rhs is in rcx); hardware masks it to 0-31.
                BinaryOp::Shl => code.extend_from_slice(&[0xD3, 0xE0]), // shl eax, cl
                BinaryOp::Shr => code.extend_from_slice(&[0xD3, 0xF8]), // sar eax, cl (arithmetic)
                BinaryOp::Lt => emit_cmp_set(code, 0x9C),   // setl
                BinaryOp::Gt => emit_cmp_set(code, 0x9F),   // setg
                BinaryOp::Le => emit_cmp_set(code, 0x9E),   // setle
                BinaryOp::Ge => emit_cmp_set(code, 0x9D),   // setge
                BinaryOp::EqEq => emit_cmp_set(code, 0x94), // sete
                BinaryOp::Ne => emit_cmp_set(code, 0x95),   // setne
            }
            code.push(0x50); // push rax
        }
        Expr::Min(lhs, rhs) | Expr::Max(lhs, rhs) => {
            lower_expression(lhs, context, code, relocations, call_fixups);
            lower_expression(rhs, context, code, relocations, call_fixups);
            code.push(0x59); // pop rcx (rhs = b)
            code.push(0x58); // pop rax (lhs = a)
            code.extend_from_slice(&[0x39, 0xC8]); // cmp eax, ecx
            // min: keep the smaller — if a>b move b in (cmovg); max: keep the larger — if a<b move b in (cmovl).
            let cmov = if matches!(context.program.expressions[node], Expr::Min(..)) { 0x4F } else { 0x4C };
            code.extend_from_slice(&[0x0F, cmov, 0xC1]); // cmovg/cmovl eax, ecx
            code.push(0x50); // push rax
        }
    }
}

struct LoweringContext<'a> {
    program: &'a Program,
    string_offsets: &'a [u32],
    handle_displacement: i32,
    written_displacement: i32,    // also the byte-count slot for read/write_byte
    io_byte_displacement: i32,    // 1-byte scratch buffer for read_byte/write_byte
    self_ptr_displacement: i32,   // frame slot holding the `self` pointer (if has_self)
    state_arg_base: usize,        // local index of state-arg slot 0 (= param_count)
}

fn lower_statement(
    statement: &Statement,
    context: &LoweringContext,
    code: &mut Vec<u8>,
    relocations: &mut Vec<Relocation>,
    state_fixups: &mut Vec<(u32, usize)>,
    call_fixups: &mut Vec<(u32, usize)>,
) {
    match statement {
        Statement::Block(statements) => {
            for inner in statements {
                lower_statement(inner, context, code, relocations, state_fixups, call_fixups);
            }
        }
        Statement::Assert(expression) => {
            lower_expression(*expression, context, code, relocations, call_fixups);
            code.push(0x58); // pop rax (the condition)
            code.extend_from_slice(&[0x85, 0xC0]); // test eax, eax
            code.extend_from_slice(&[0x75, 0x02, 0x0F, 0x0B]); // jnz +2 ; ud2 (trap if false/zero)
        }
        Statement::Let(local_index, expression, _wrapping) => {
            // x64 keeps trapping arithmetic regardless of domain (the aarch64 macOS target is the gated
            // path for the Wrapping slice; x64 Wrapping is a follow-on).
            lower_expression(*expression, context, code, relocations, call_fixups);
            code.push(0x58); // pop rax
            emit_store_local(code, *local_index);
        }
        Statement::Assign(local_index, expression) => {
            lower_expression(*expression, context, code, relocations, call_fixups);
            code.push(0x58); // pop rax
            emit_store_local(code, *local_index);
        }
        Statement::Eval(expression) => {
            lower_expression(*expression, context, code, relocations, call_fixups);
            code.push(0x58); // pop rax (discard the result)
        }
        Statement::StoreSelfField(offset, expression, _domain) => {
            lower_expression(*expression, context, code, relocations, call_fixups);
            code.push(0x59); // pop rcx (value)
            emit_rbp_memory_operand(code, &[0x48, 0x8B], 0, context.self_ptr_displacement); // mov rax, [rbp+self]
            emit_rax_indirect(code, 0x89, 1, *offset); // mov [rax+offset], ecx
        }
        Statement::StoreSelfIndex(field_offset, count, element_bytes, index, value) => {
            lower_expression(*value, context, code, relocations, call_fixups); // push value
            lower_expression(*index, context, code, relocations, call_fixups); // push index (popped first)
            emit_self_element_address(code, context.self_ptr_displacement, *field_offset, *count, *element_bytes);
            code.push(0x59); // pop rcx (value)
            if *element_bytes == 1 {
                code.extend_from_slice(&[0x88, 0x08]); // mov [rax], cl
            } else {
                code.extend_from_slice(&[0x89, 0x08]); // mov [rax], ecx
            }
        }
        Statement::Return(expression) => {
            lower_expression(*expression, context, code, relocations, call_fixups);
            code.push(0x58); // pop rax (return value -> caller)
            emit_epilogue(code);
        }
        Statement::Exit(expression) => {
            lower_expression(*expression, context, code, relocations, call_fixups);
            code.push(0x58); // pop rax (exit code)
            emit_epilogue(code);
        }
        Statement::WriteByte(expression) => {
            lower_expression(*expression, context, code, relocations, call_fixups);
            code.push(0x58); // pop rax (value)
            emit_rbp_memory_operand(code, &[0x88], 0, context.io_byte_displacement); // mov [rbp+io_byte], al
            // handle = GetStdHandle(STD_OUTPUT_HANDLE = -11)
            code.push(0xB9);
            code.extend_from_slice(&(-11i32).to_le_bytes()); // mov ecx, -11
            emit_rip_call(code, relocations, RelocationTarget::Import(ImportFunction::GetStdHandle));
            emit_rbp_memory_operand(code, &[0x48, 0x89], 0, context.handle_displacement); // mov [rbp+handle], rax
            // WriteFile(handle, &io_byte, 1, &written, NULL)
            emit_rbp_memory_operand(code, &[0x48, 0x8B], 1, context.handle_displacement); // mov rcx, [rbp+handle]
            emit_rbp_memory_operand(code, &[0x48, 0x8D], 2, context.io_byte_displacement); // lea rdx, [rbp+io_byte]
            code.extend_from_slice(&[0x41, 0xB8, 0x01, 0, 0, 0]); // mov r8d, 1
            emit_rbp_memory_operand(code, &[0x4C, 0x8D], 1, context.written_displacement); // lea r9, [rbp+written]
            code.extend_from_slice(&[0x48, 0xC7, 0x44, 0x24, 0x20, 0, 0, 0, 0]); // mov qword [rsp+0x20], 0
            emit_rip_call(code, relocations, RelocationTarget::Import(ImportFunction::WriteFile));
        }
        Statement::WriteLine(string_index) => {
            let offset = context.string_offsets[*string_index];
            let len = context.program.strings[*string_index].len() as u32;
            // handle = GetStdHandle(STD_OUTPUT_HANDLE = -11)
            code.push(0xB9);
            code.extend_from_slice(&(-11i32).to_le_bytes()); // mov ecx, -11
            emit_rip_call(code, relocations, RelocationTarget::Import(ImportFunction::GetStdHandle));
            emit_rbp_memory_operand(code, &[0x48, 0x89], 0, context.handle_displacement); // mov [rbp+handle], rax
            // WriteFile(handle, buf, len, &written, NULL)
            emit_rbp_memory_operand(code, &[0x48, 0x8B], 1, context.handle_displacement); // mov rcx, [rbp+handle]
            emit_lea_rdx_rip(code, relocations, RelocationTarget::Rodata(offset)); // lea rdx, [rip+str]
            code.extend_from_slice(&[0x41, 0xB8]);
            code.extend_from_slice(&len.to_le_bytes()); // mov r8d, len
            emit_rbp_memory_operand(code, &[0x4C, 0x8D], 1, context.written_displacement); // lea r9, [rbp+written]
            code.extend_from_slice(&[0x48, 0xC7, 0x44, 0x24, 0x20, 0, 0, 0, 0]); // mov qword [rsp+0x20], 0
            emit_rip_call(code, relocations, RelocationTarget::Import(ImportFunction::WriteFile));
        }
        Statement::Transition(subject, arms) => {
            lower_expression(*subject, context, code, relocations, call_fixups);
            code.push(0x58); // pop rax (subject value)
            for arm in arms {
                if arm.args.is_empty() {
                    // argless arm — byte-identical to the pre-state-params lowering
                    match arm.pattern {
                        Pattern::Int(value) => {
                            code.push(0x3D);
                            code.extend_from_slice(&value.to_le_bytes()); // cmp eax, imm32
                            code.extend_from_slice(&[0x0F, 0x84]); // je rel32
                            let patch_offset = code.len() as u32;
                            code.extend_from_slice(&[0, 0, 0, 0]);
                            state_fixups.push((patch_offset, arm.target));
                        }
                        Pattern::Wild => {
                            code.push(0xE9); // jmp rel32
                            let patch_offset = code.len() as u32;
                            code.extend_from_slice(&[0, 0, 0, 0]);
                            state_fixups.push((patch_offset, arm.target));
                        }
                    }
                } else {
                    // arg-passing arm: on a match, evaluate all args (left→right onto the
                    // stack), pop them into the shared state-arg slots, then jump. The eval
                    // runs only on the matching (jump-away) path, so eax (the subject) is
                    // preserved for the remaining arms' comparisons.
                    let mut skip_patch: Option<usize> = None;
                    if let Pattern::Int(value) = arm.pattern {
                        code.push(0x3D);
                        code.extend_from_slice(&value.to_le_bytes()); // cmp eax, imm32
                        code.extend_from_slice(&[0x0F, 0x85]);         // jne rel32 (skip)
                        skip_patch = Some(code.len());
                        code.extend_from_slice(&[0, 0, 0, 0]);
                    }
                    for arg in &arm.args {
                        lower_expression(*arg, context, code, relocations, call_fixups);
                    }
                    for i in (0..arm.args.len()).rev() {
                        code.push(0x58); // pop rax (last-pushed arg first)
                        emit_store_local(code, context.state_arg_base + i);
                    }
                    code.push(0xE9); // jmp target rel32
                    let patch_offset = code.len() as u32;
                    code.extend_from_slice(&[0, 0, 0, 0]);
                    state_fixups.push((patch_offset, arm.target));
                    if let Some(p) = skip_patch {
                        let rel = (code.len() - (p + 4)) as i32;
                        code[p..p + 4].copy_from_slice(&rel.to_le_bytes());
                    }
                }
            }
        }
    }
}

// Lower one machine as a function: prologue, establish `self`, spill register
// params, entry block, then each state block (recording its label), a trailing
// return-0, and finally patch this machine's intra-machine state jumps.
//
// Frame layout (high → low address, all rbp-relative): locals · self-ptr slot ·
// write_line/io scratch · shadow space. The entry machine's `self` data instance
// lives in a zero-init .data section (RIP-relative), not the frame, so the frame
// stays small regardless of how big the arenas are.
fn lower_machine(
    machine: &Machine,
    is_entry: bool,
    string_offsets: &[u32],
    program: &Program,
    code: &mut Vec<u8>,
    relocations: &mut Vec<Relocation>,
    call_fixups: &mut Vec<(u32, usize)>,
) {
    let local_count = machine.local_count as i32;
    let self_slots = if machine.has_self { 1 } else { 0 };
    let named_bytes = 8 * (local_count + self_slots); // locals + the self-pointer slot
    // scratch when calling out: written/count(8) + handle(8) + io_byte(8) + shadow(32) + 5th arg(8)
    let scratch = if machine.makes_call { 24 + 40 } else { 0 };
    let frame = align_up((named_bytes + scratch) as u32, 16);

    let self_ptr_displacement = -(8 * (local_count + 1)); // first slot past the locals
    let context = LoweringContext {
        program,
        string_offsets,
        written_displacement: -(named_bytes + 8),
        handle_displacement: -(named_bytes + 16),
        io_byte_displacement: -(named_bytes + 24),
        self_ptr_displacement,
        state_arg_base: machine.param_count,
    };

    // prologue
    code.push(0x55); // push rbp
    code.extend_from_slice(&[0x48, 0x89, 0xE5]); // mov rbp, rsp
    if frame > 0 {
        if frame <= 127 {
            code.extend_from_slice(&[0x48, 0x83, 0xEC, frame as u8]); // sub rsp, imm8
        } else {
            code.extend_from_slice(&[0x48, 0x81, 0xEC]);
            code.extend_from_slice(&frame.to_le_bytes()); // sub rsp, imm32
        }
    }
    // spill incoming register params into their frame slots (a `&mut self` method
    // takes self in rcx, so its value params start at register index 1).
    let self_register_offset = if machine.has_self { 1 } else { 0 };
    for param_position in 0..machine.param_count {
        emit_store_parameter_register(code, param_position, param_position + self_register_offset);
    }
    // establish the self pointer
    if machine.has_self {
        if is_entry {
            // This target provisions the entry receiver as a zero-init .data instance.
            code.extend_from_slice(&[0x48, 0x8D, 0x05]); // lea rax, [rip+disp32]
            let patch_offset = code.len() as u32;
            code.extend_from_slice(&[0, 0, 0, 0]);
            relocations.push(Relocation { patch_offset, target: RelocationTarget::Data(0) });
            emit_rbp_memory_operand(code, &[0x48, 0x89], 0, self_ptr_displacement); // mov [rbp+self], rax
        } else {
            emit_rbp_memory_operand(code, &[0x48, 0x89], 1, self_ptr_displacement); // mov [rbp+self], rcx
        }
    }

    let mut state_fixups: Vec<(u32, usize)> = Vec::new();
    for statement in &machine.entry {
        lower_statement(statement, &context, code, relocations, &mut state_fixups, call_fixups);
    }
    if !machine.states.is_empty() && block_falls_through(&machine.entry) {
        emit_implicit_return(code);
    }
    let mut labels = vec![0u32; machine.states.len()];
    for (state_index, state_statements) in machine.states.iter().enumerate() {
        labels[state_index] = code.len() as u32;
        for statement in state_statements {
            lower_statement(statement, &context, code, relocations, &mut state_fixups, call_fixups);
        }
        if state_index + 1 < machine.states.len() && block_falls_through(state_statements) {
            emit_implicit_return(code);
        }
    }
    // Preserve the existing trailing default for the final state (or entry when
    // there are no states). Explicit control flow makes it unreachable.
    emit_implicit_return(code);

    // patch intra-machine jumps to state labels
    for (patch_offset, target) in &state_fixups {
        let relative = labels[*target] as i64 - (*patch_offset as i64 + 4);
        let offset = *patch_offset as usize;
        code[offset..offset + 4].copy_from_slice(&(relative as i32).to_le_bytes());
    }
}

fn block_falls_through(statements: &[Statement]) -> bool {
    match statements.last() {
        Some(Statement::Transition(_, _) | Statement::Return(_) | Statement::Exit(_)) => false,
        Some(Statement::Block(inner)) => block_falls_through(inner),
        _ => true,
    }
}

fn emit_implicit_return(code: &mut Vec<u8>) {
    code.extend_from_slice(&[0x31, 0xC0]); // xor eax, eax
    emit_epilogue(code);
}

pub fn lower_program(program: &Program) -> LoweredProgram {
    // .rdata strings + per-string offsets (global across machines)
    let mut rodata = Vec::new();
    let mut string_offsets = Vec::with_capacity(program.strings.len());
    for string in &program.strings {
        string_offsets.push(rodata.len() as u32);
        rodata.extend_from_slice(string);
    }
    let uses_imports = program.uses_imports;

    let mut code = Vec::new();
    let mut relocations = Vec::new();
    let mut call_fixups: Vec<(u32, usize)> = Vec::new();
    let mut machine_offsets = vec![0u32; program.machines.len()];

    // Lower the entry machine FIRST so the PE entry point stays at text offset 0,
    // then the rest; record each machine's start offset for the call relocations.
    let mut order = vec![program.entry_machine];
    for index in 0..program.machines.len() {
        if index != program.entry_machine {
            order.push(index);
        }
    }
    for &machine_index in &order {
        let machine = &program.machines[machine_index];
        machine_offsets[machine_index] = code.len() as u32;
        lower_machine(
            machine,
            machine_index == program.entry_machine,
            &string_offsets,
            program,
            &mut code,
            &mut relocations,
            &mut call_fixups,
        );
    }

    // patch cross-machine call rel32s now that every machine's offset is known
    for (patch_offset, target_machine) in &call_fixups {
        let relative = machine_offsets[*target_machine] as i64 - (*patch_offset as i64 + 4);
        let offset = *patch_offset as usize;
        code[offset..offset + 4].copy_from_slice(&(relative as i32).to_le_bytes());
    }

    // the entry machine's `self` instance lives in a zero-init .data section
    let data_size = if program.machines[program.entry_machine].has_self {
        align_up(program.entry_data_size as u32, 8)
    } else {
        0
    };

    LoweredProgram { code, relocations, rodata, uses_imports, data_size }
}
