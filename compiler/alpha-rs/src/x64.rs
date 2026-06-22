// x86-64 lowering + machine-code emission. One of the per-platform backends; a
// future aarch64.rs would be a sibling consuming the same AST.
//
// Strategy: a stack-machine evaluator (each expr leaves its result pushed; binary
// ops pop two and push the result) over an rbp-based locals frame. Trap-on-
// overflow per the Alpha spec. Slice 3 adds host calls (write_line) lowered to
// the Win32 ABI with RIP-relative relocations resolved by the PE writer.

use crate::ast::{BinOp, ExprKind, Main, Stmt};
use crate::util::align_up;

#[derive(Clone, Copy)]
pub enum ImportFn {
    GetStdHandle, // IAT slot 0
    WriteFile,    // IAT slot 1
}

pub enum RelocTarget {
    Rodata(u32),      // RIP-relative reference to a byte offset within .rdata strings
    Import(ImportFn), // RIP-relative indirect call through the IAT slot
}

pub struct Reloc {
    pub at: u32, // byte offset in `code` where the little-endian disp32 to patch begins
    pub target: RelocTarget,
}

pub struct Lowered {
    pub code: Vec<u8>,
    pub relocs: Vec<Reloc>,
    pub rodata: Vec<u8>, // concatenated string payloads; Reloc::Rodata indexes into this
    pub uses_imports: bool,
}

fn local_disp(idx: usize) -> i32 {
    -(8 * (idx as i32 + 1)) // local i at [rbp - 8*(i+1)]
}

// Emit `<prefix> ModRM[+disp]` for a memory operand [rbp + disp], with the given
// ModRM reg field (reg3). prefix carries REX + opcode.
fn emit_rbp_mem(c: &mut Vec<u8>, prefix: &[u8], reg3: u8, disp: i32) {
    c.extend_from_slice(prefix);
    if (-128..=127).contains(&disp) {
        c.push(0x40 | (reg3 << 3) | 0x05); // mod=01, reg, rm=101 (rbp)
        c.push(disp as i8 as u8);
    } else {
        c.push(0x80 | (reg3 << 3) | 0x05); // mod=10, reg, rm=101 (rbp+disp32)
        c.extend_from_slice(&disp.to_le_bytes());
    }
}

fn emit_load_local(c: &mut Vec<u8>, idx: usize) {
    emit_rbp_mem(c, &[0x8B], 0, local_disp(idx)); // mov eax, [rbp+disp]
}

fn emit_store_local(c: &mut Vec<u8>, idx: usize) {
    emit_rbp_mem(c, &[0x89], 0, local_disp(idx)); // mov [rbp+disp], eax
}

fn emit_overflow_trap(c: &mut Vec<u8>) {
    c.extend_from_slice(&[0x71, 0x02, 0x0F, 0x0B]); // jno +2 ; ud2
}

// eax = (eax <cc> ecx) ? 1 : 0  — signed compare, then setcc al + zero-extend.
fn emit_cmp_set(c: &mut Vec<u8>, setcc: u8) {
    c.extend_from_slice(&[0x39, 0xC8]); // cmp eax, ecx
    c.extend_from_slice(&[0x0F, setcc, 0xC0]); // setcc al
    c.extend_from_slice(&[0x0F, 0xB6, 0xC0]); // movzx eax, al
}

fn emit_epilogue(c: &mut Vec<u8>) {
    c.extend_from_slice(&[0x48, 0x89, 0xEC]); // mov rsp, rbp
    c.push(0x5D); // pop rbp
    c.push(0xC3); // ret
}

// call [rip + disp32] (indirect through the IAT) — records a reloc for the disp32.
fn emit_rip_call(c: &mut Vec<u8>, relocs: &mut Vec<Reloc>, target: RelocTarget) {
    c.extend_from_slice(&[0xFF, 0x15]); // call r/m64, ModRM mod=00 reg=010 rm=101 (RIP)
    let at = c.len() as u32;
    c.extend_from_slice(&[0, 0, 0, 0]);
    relocs.push(Reloc { at, target });
}

// lea rdx, [rip + disp32] — records a reloc for the disp32.
fn emit_lea_rdx_rip(c: &mut Vec<u8>, relocs: &mut Vec<Reloc>, target: RelocTarget) {
    c.extend_from_slice(&[0x48, 0x8D, 0x15]); // lea rdx, [rip+disp32]
    let at = c.len() as u32;
    c.extend_from_slice(&[0, 0, 0, 0]);
    relocs.push(Reloc { at, target });
}

fn lower_expr(node: usize, exprs: &[ExprKind], c: &mut Vec<u8>) {
    match exprs[node] {
        ExprKind::Int(v) => {
            c.push(0xB8); // mov eax, imm32
            c.extend_from_slice(&v.to_le_bytes());
            c.push(0x50); // push rax
        }
        ExprKind::Local(i) => {
            emit_load_local(c, i);
            c.push(0x50); // push rax
        }
        ExprKind::Bin(op, l, r) => {
            lower_expr(l, exprs, c);
            lower_expr(r, exprs, c);
            c.push(0x59); // pop rcx (rhs)
            c.push(0x58); // pop rax (lhs)
            match op {
                BinOp::Add => {
                    c.extend_from_slice(&[0x01, 0xC8]);
                    emit_overflow_trap(c);
                }
                BinOp::Sub => {
                    c.extend_from_slice(&[0x29, 0xC8]);
                    emit_overflow_trap(c);
                }
                BinOp::Mul => {
                    c.extend_from_slice(&[0x0F, 0xAF, 0xC1]);
                    emit_overflow_trap(c);
                }
                BinOp::Div => {
                    c.push(0x99); // cdq
                    c.extend_from_slice(&[0xF7, 0xF9]); // idiv ecx (traps on /0, INT_MIN/-1)
                }
                BinOp::Lt => emit_cmp_set(c, 0x9C),   // setl
                BinOp::Gt => emit_cmp_set(c, 0x9F),   // setg
                BinOp::Le => emit_cmp_set(c, 0x9E),   // setle
                BinOp::Ge => emit_cmp_set(c, 0x9D),   // setge
                BinOp::EqEq => emit_cmp_set(c, 0x94), // sete
                BinOp::Ne => emit_cmp_set(c, 0x95),   // setne
            }
            c.push(0x50); // push rax
        }
    }
}

pub fn lower_main(m: &Main) -> Lowered {
    // .rdata strings + per-string offsets
    let mut rodata = Vec::new();
    let mut offsets = Vec::with_capacity(m.strings.len());
    for s in &m.strings {
        offsets.push(rodata.len() as u32);
        rodata.extend_from_slice(s);
    }

    let n = m.n_locals as u32;
    let has_calls = m.stmts.iter().any(|s| matches!(s, Stmt::WriteLine(_)));
    // frame = locals + (when calling: 16 scratch [written,handle] + 32 shadow + 8 fifth-arg slot)
    let extra = if has_calls { 16 + 40 } else { 0 };
    let frame = align_up(n * 8 + extra, 16);
    let handle_disp = -((n * 8 + 16) as i32); // [rbp+handle_disp] : saved stdout HANDLE
    let written_disp = -((n * 8 + 8) as i32); // [rbp+written_disp]: WriteFile's out DWORD

    let mut c = Vec::new();
    let mut relocs = Vec::new();

    // prologue
    c.push(0x55); // push rbp
    c.extend_from_slice(&[0x48, 0x89, 0xE5]); // mov rbp, rsp
    if frame > 0 {
        if frame <= 127 {
            c.extend_from_slice(&[0x48, 0x83, 0xEC, frame as u8]); // sub rsp, imm8
        } else {
            c.extend_from_slice(&[0x48, 0x81, 0xEC]);
            c.extend_from_slice(&frame.to_le_bytes()); // sub rsp, imm32
        }
    }

    for s in &m.stmts {
        match s {
            Stmt::Let(idx, e) => {
                lower_expr(*e, &m.exprs, &mut c);
                c.push(0x58); // pop rax
                emit_store_local(&mut c, *idx);
            }
            Stmt::Exit(e) => {
                lower_expr(*e, &m.exprs, &mut c);
                c.push(0x58); // pop rax (exit code)
                emit_epilogue(&mut c);
            }
            Stmt::WriteLine(i) => {
                let off = offsets[*i];
                let len = m.strings[*i].len() as u32;
                // handle = GetStdHandle(STD_OUTPUT_HANDLE = -11)
                c.push(0xB9);
                c.extend_from_slice(&(-11i32).to_le_bytes()); // mov ecx, -11
                emit_rip_call(&mut c, &mut relocs, RelocTarget::Import(ImportFn::GetStdHandle));
                emit_rbp_mem(&mut c, &[0x48, 0x89], 0, handle_disp); // mov [rbp+handle], rax
                // WriteFile(handle, buf, len, &written, NULL)
                emit_rbp_mem(&mut c, &[0x48, 0x8B], 1, handle_disp); // mov rcx, [rbp+handle]
                emit_lea_rdx_rip(&mut c, &mut relocs, RelocTarget::Rodata(off)); // lea rdx, [rip+str]
                c.extend_from_slice(&[0x41, 0xB8]);
                c.extend_from_slice(&len.to_le_bytes()); // mov r8d, len
                emit_rbp_mem(&mut c, &[0x4C, 0x8D], 1, written_disp); // lea r9, [rbp+written]
                c.extend_from_slice(&[0x48, 0xC7, 0x44, 0x24, 0x20, 0, 0, 0, 0]); // mov qword [rsp+0x20], 0
                emit_rip_call(&mut c, &mut relocs, RelocTarget::Import(ImportFn::WriteFile));
            }
        }
    }

    // fall-through default: exit 0
    c.extend_from_slice(&[0x31, 0xC0]); // xor eax, eax
    emit_epilogue(&mut c);

    Lowered { code: c, relocs, rodata, uses_imports: has_calls }
}
