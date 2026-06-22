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
        Expr::Call(machine_index, args_start, arg_count) => {
            // Evaluate args left-to-right (each leaves its result pushed), then load
            // them into the ABI registers and discard them so rsp returns to the
            // (16-aligned) frame base before the call. arg i sits at the deeper slot.
            for index in 0..arg_count {
                let arg_node = context.program.call_args[args_start + index];
                lower_expression(arg_node, context, code, call_fixups);
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
        Expr::Binary(op, lhs, rhs) => {
            lower_expression(lhs, context, code, call_fixups);
            lower_expression(rhs, context, code, call_fixups);
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
    self_ptr_displacement: i32, // frame slot holding the `self` pointer (if has_self)
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
        Statement::Let(local_index, expression) => {
            lower_expression(*expression, context, code, call_fixups);
            code.push(0x58); // pop rax
            emit_store_local(code, *local_index);
        }
        Statement::Assign(local_index, expression) => {
            lower_expression(*expression, context, code, call_fixups);
            code.push(0x58); // pop rax
            emit_store_local(code, *local_index);
        }
        Statement::StoreSelfField(offset, expression) => {
            lower_expression(*expression, context, code, call_fixups);
            code.push(0x59); // pop rcx (value)
            emit_rbp_memory_operand(code, &[0x48, 0x8B], 0, context.self_ptr_displacement); // mov rax, [rbp+self]
            emit_rax_indirect(code, 0x89, 1, *offset); // mov [rax+offset], ecx
        }
        Statement::Return(expression) => {
            lower_expression(*expression, context, code, call_fixups);
            code.push(0x58); // pop rax (return value -> caller)
            emit_epilogue(code);
        }
        Statement::Exit(expression) => {
            lower_expression(*expression, context, code, call_fixups);
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
            lower_expression(*subject, context, code, call_fixups);
            code.push(0x58); // pop rax (subject value)
            for arm in arms {
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
            }
        }
    }
}

// Lower one machine as a function: prologue, establish `self`, spill register
// params, entry block, then each state block (recording its label), a trailing
// return-0, and finally patch this machine's intra-machine state jumps.
//
// Frame layout (high → low address, all rbp-relative): locals · self-ptr slot ·
// [entry only] the self data instance · write_line scratch · shadow space.
fn lower_machine(
    machine: &Machine,
    is_entry: bool,
    entry_data_size: i32,
    string_offsets: &[u32],
    program: &Program,
    code: &mut Vec<u8>,
    relocations: &mut Vec<Relocation>,
    call_fixups: &mut Vec<(u32, usize)>,
) {
    let local_count = machine.local_count as i32;
    let self_slots = if machine.has_self { 1 } else { 0 };
    let named_bytes = 8 * (local_count + self_slots); // locals + the self-pointer slot
    // The entry machine's `self` instance lives in its own (long-lived) frame, ZII.
    let region_bytes = if is_entry && machine.has_self {
        (entry_data_size + 7) & !7 // round up to 8
    } else {
        0
    };
    let scratch = if machine.makes_call { 16 + 40 } else { 0 }; // handle/written + shadow + 5th arg
    let frame = align_up((named_bytes + region_bytes + scratch) as u32, 16);

    let self_ptr_displacement = -(8 * (local_count + 1)); // first slot past the locals
    let region_displacement = -(named_bytes + region_bytes); // self instance base (field offset 0)
    let context = LoweringContext {
        program,
        string_offsets,
        handle_displacement: -(named_bytes + region_bytes + 16),
        written_displacement: -(named_bytes + region_bytes + 8),
        self_ptr_displacement,
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
            // zero the frame-resident self instance (ZII), then point self at it
            for slot in 0..(region_bytes / 8) {
                emit_rbp_memory_operand(code, &[0x48, 0xC7], 0, region_displacement + 8 * slot);
                code.extend_from_slice(&[0, 0, 0, 0]); // mov qword [rbp+region+8*slot], 0
            }
            emit_rbp_memory_operand(code, &[0x48, 0x8D], 0, region_displacement); // lea rax, [rbp+region]
            emit_rbp_memory_operand(code, &[0x48, 0x89], 0, self_ptr_displacement); // mov [rbp+self], rax
        } else {
            emit_rbp_memory_operand(code, &[0x48, 0x89], 1, self_ptr_displacement); // mov [rbp+self], rcx
        }
    }

    let mut state_fixups: Vec<(u32, usize)> = Vec::new();
    for statement in &machine.entry {
        lower_statement(statement, &context, code, relocations, &mut state_fixups, call_fixups);
    }
    let mut labels = vec![0u32; machine.states.len()];
    for (state_index, state_statements) in machine.states.iter().enumerate() {
        labels[state_index] = code.len() as u32;
        for statement in state_statements {
            lower_statement(statement, &context, code, relocations, &mut state_fixups, call_fixups);
        }
    }
    // trailing default: return 0 (reached only by fall-through off the end)
    code.extend_from_slice(&[0x31, 0xC0]); // xor eax, eax
    emit_epilogue(code);

    // patch intra-machine jumps to state labels
    for (patch_offset, target) in &state_fixups {
        let relative = labels[*target] as i64 - (*patch_offset as i64 + 4);
        let offset = *patch_offset as usize;
        code[offset..offset + 4].copy_from_slice(&(relative as i32).to_le_bytes());
    }
}

pub fn lower_program(program: &Program) -> LoweredProgram {
    // .rdata strings + per-string offsets (global across machines)
    let mut rodata = Vec::new();
    let mut string_offsets = Vec::with_capacity(program.strings.len());
    for string in &program.strings {
        string_offsets.push(rodata.len() as u32);
        rodata.extend_from_slice(string);
    }
    let uses_imports = program.machines.iter().any(|machine| {
        machine
            .entry
            .iter()
            .chain(machine.states.iter().flatten())
            .any(|statement| matches!(statement, Statement::WriteLine(_)))
    });

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
            program.entry_data_size,
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

    LoweredProgram { code, relocations, rodata, uses_imports }
}
