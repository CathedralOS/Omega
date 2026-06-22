// x86-64 lowering + machine-code emission. One of the per-platform backends; a
// future aarch64.rs would be a sibling consuming the same AST.
//
// Strategy: a stack-machine evaluator (each expr leaves its result pushed; binary
// ops pop two and push the result) over an rbp-based locals frame. Trap-on-
// overflow per the Alpha spec ("trap everything").

use crate::ast::{BinOp, ExprKind, Main, Stmt};
use crate::util::align_up;

fn local_disp(idx: usize) -> i32 {
    -(8 * (idx as i32 + 1)) // local i at [rbp - 8*(i+1)]
}

fn emit_load_local(c: &mut Vec<u8>, idx: usize) {
    let disp = local_disp(idx);
    if disp >= -128 {
        c.extend_from_slice(&[0x8B, 0x45, (disp as i8) as u8]); // mov eax, [rbp+disp8]
    } else {
        c.extend_from_slice(&[0x8B, 0x85]);
        c.extend_from_slice(&disp.to_le_bytes()); // mov eax, [rbp+disp32]
    }
}

fn emit_store_local(c: &mut Vec<u8>, idx: usize) {
    let disp = local_disp(idx);
    if disp >= -128 {
        c.extend_from_slice(&[0x89, 0x45, (disp as i8) as u8]); // mov [rbp+disp8], eax
    } else {
        c.extend_from_slice(&[0x89, 0x85]);
        c.extend_from_slice(&disp.to_le_bytes()); // mov [rbp+disp32], eax
    }
}

fn emit_overflow_trap(c: &mut Vec<u8>) {
    // jno +2 ; ud2   -> trap on signed overflow
    c.extend_from_slice(&[0x71, 0x02, 0x0F, 0x0B]);
}

fn emit_epilogue(c: &mut Vec<u8>) {
    c.extend_from_slice(&[0x48, 0x89, 0xEC]); // mov rsp, rbp
    c.push(0x5D); // pop rbp
    c.push(0xC3); // ret  (eax = process exit code via the Windows thread stub)
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
            c.push(0x59); // pop rcx  (rhs)
            c.push(0x58); // pop rax  (lhs)
            match op {
                BinOp::Add => {
                    c.extend_from_slice(&[0x01, 0xC8]); // add eax, ecx
                    emit_overflow_trap(c);
                }
                BinOp::Sub => {
                    c.extend_from_slice(&[0x29, 0xC8]); // sub eax, ecx
                    emit_overflow_trap(c);
                }
                BinOp::Mul => {
                    c.extend_from_slice(&[0x0F, 0xAF, 0xC1]); // imul eax, ecx
                    emit_overflow_trap(c);
                }
                BinOp::Div => {
                    // cdq ; idiv ecx — hardware traps on /0 and INT_MIN/-1
                    c.push(0x99);
                    c.extend_from_slice(&[0xF7, 0xF9]);
                }
            }
            c.push(0x50); // push rax
        }
    }
}

pub fn lower_main(m: &Main) -> Vec<u8> {
    let mut c = Vec::new();
    // prologue
    c.push(0x55); // push rbp
    c.extend_from_slice(&[0x48, 0x89, 0xE5]); // mov rbp, rsp
    let frame = align_up((m.n_locals as u32) * 8, 16);
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
                c.push(0x58); // pop rax  (exit code in eax)
                emit_epilogue(&mut c);
            }
        }
    }
    // fall-through default: exit 0
    c.extend_from_slice(&[0x31, 0xC0]); // xor eax, eax
    emit_epilogue(&mut c);
    c
}
