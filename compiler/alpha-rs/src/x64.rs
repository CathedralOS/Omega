// x86-64 lowering + machine-code emission. One of the per-platform backends; a
// future aarch64.rs would be a sibling consuming the same AST.
//
// Strategy: a stack-machine evaluator (each expr leaves its result pushed; binary
// ops pop two and push the result) over an rbp-based locals frame. Trap-on-
// overflow per the Alpha spec. Slice 3 adds host calls (write_line) lowered to
// the Win32 ABI with RIP-relative relocations resolved by the PE writer.

use crate::ast::{BinaryOp, Expr, Pattern, Program, Statement};
use crate::util::align_up;

#[derive(Clone, Copy)]
pub enum ImportFunction {
    GetStdHandle, // IAT slot 0
    WriteFile,    // IAT slot 1
}

pub enum RelocationTarget {
    Rodata(u32),              // RIP-relative reference to a byte offset within .rdata strings
    Import(ImportFunction),   // RIP-relative indirect call through the IAT slot
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

fn lower_expression(node: usize, expressions: &[Expr], code: &mut Vec<u8>) {
    match expressions[node] {
        Expr::Int(value) => {
            code.push(0xB8); // mov eax, imm32
            code.extend_from_slice(&value.to_le_bytes());
            code.push(0x50); // push rax
        }
        Expr::Local(local_index) => {
            emit_load_local(code, local_index);
            code.push(0x50); // push rax
        }
        Expr::Binary(op, lhs, rhs) => {
            lower_expression(lhs, expressions, code);
            lower_expression(rhs, expressions, code);
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
                BinaryOp::Lt => emit_cmp_set(code, 0x9C),   // setl
                BinaryOp::Gt => emit_cmp_set(code, 0x9F),   // setg
                BinaryOp::Le => emit_cmp_set(code, 0x9E),   // setle
                BinaryOp::Ge => emit_cmp_set(code, 0x9D),   // setge
                BinaryOp::EqEq => emit_cmp_set(code, 0x94), // sete
                BinaryOp::Ne => emit_cmp_set(code, 0x95),   // setne
            }
            code.push(0x50); // push rax
        }
    }
}

struct LoweringContext<'a> {
    program: &'a Program,
    string_offsets: &'a [u32],
    handle_displacement: i32,
    written_displacement: i32,
}

fn lower_statement(
    statement: &Statement,
    context: &LoweringContext,
    code: &mut Vec<u8>,
    relocations: &mut Vec<Relocation>,
    fixups: &mut Vec<(u32, usize)>,
) {
    match statement {
        Statement::Let(local_index, expression) => {
            lower_expression(*expression, &context.program.expressions, code);
            code.push(0x58); // pop rax
            emit_store_local(code, *local_index);
        }
        Statement::Exit(expression) => {
            lower_expression(*expression, &context.program.expressions, code);
            code.push(0x58); // pop rax (exit code)
            emit_epilogue(code);
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
            lower_expression(*subject, &context.program.expressions, code);
            code.push(0x58); // pop rax (subject value)
            for arm in arms {
                match arm.pattern {
                    Pattern::Int(value) => {
                        code.push(0x3D);
                        code.extend_from_slice(&value.to_le_bytes()); // cmp eax, imm32
                        code.extend_from_slice(&[0x0F, 0x84]); // je rel32
                        let patch_offset = code.len() as u32;
                        code.extend_from_slice(&[0, 0, 0, 0]);
                        fixups.push((patch_offset, arm.target));
                    }
                    Pattern::Wild => {
                        code.push(0xE9); // jmp rel32
                        let patch_offset = code.len() as u32;
                        code.extend_from_slice(&[0, 0, 0, 0]);
                        fixups.push((patch_offset, arm.target));
                    }
                }
            }
        }
    }
}

pub fn lower_program(program: &Program) -> LoweredProgram {
    // .rdata strings + per-string offsets
    let mut rodata = Vec::new();
    let mut string_offsets = Vec::with_capacity(program.strings.len());
    for string in &program.strings {
        string_offsets.push(rodata.len() as u32);
        rodata.extend_from_slice(string);
    }

    let local_count = program.local_count as u32;
    let has_calls = program
        .entry
        .iter()
        .chain(program.states.iter().flatten())
        .any(|statement| matches!(statement, Statement::WriteLine(_)));
    // frame = locals + (when calling: 16 scratch [written,handle] + 32 shadow + 8 fifth-arg slot)
    let extra = if has_calls { 16 + 40 } else { 0 };
    let frame = align_up(local_count * 8 + extra, 16);
    let context = LoweringContext {
        program,
        string_offsets: &string_offsets,
        handle_displacement: -((local_count * 8 + 16) as i32),
        written_displacement: -((local_count * 8 + 8) as i32),
    };

    let mut code = Vec::new();
    let mut relocations = Vec::new();
    let mut fixups: Vec<(u32, usize)> = Vec::new();

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

    for statement in &program.entry {
        lower_statement(statement, &context, &mut code, &mut relocations, &mut fixups);
    }
    let mut labels = vec![0u32; program.states.len()];
    for (state_index, state_statements) in program.states.iter().enumerate() {
        labels[state_index] = code.len() as u32;
        for statement in state_statements {
            lower_statement(statement, &context, &mut code, &mut relocations, &mut fixups);
        }
    }
    // trailing default: exit 0 (reached only by fall-through)
    code.extend_from_slice(&[0x31, 0xC0]); // xor eax, eax
    emit_epilogue(&mut code);

    // patch intra-text jumps to state labels
    for (patch_offset, target) in &fixups {
        let relative = labels[*target] as i64 - (*patch_offset as i64 + 4);
        let offset = *patch_offset as usize;
        code[offset..offset + 4].copy_from_slice(&(relative as i32).to_le_bytes());
    }

    LoweredProgram { code, relocations, rodata, uses_imports: has_calls }
}
