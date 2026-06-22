// The Alpha tape VM — interprets a bytecode tape. This is the SEED: the small,
// auditable thing the whole trust chain bottoms out at. It is written in Rust for
// now ("action produces information"); the trust-root version is a hand-rolled
// assembly port of this same loop, which is why the loop is kept dead simple and
// regular (a uniform fetch/decode/execute over a fixed opcode table).
//
//   vm <tape>           run the tape; stdin/stdout are the program's byte I/O
//
// The tape loads at address 0 and PC starts at 0. Memory is a flat, zero-initialized
// byte array; the program owns everything past the tape for its own buffers.

#[path = "isa.rs"]
mod isa;
use isa::*;

use std::io::{Read, Write};
use std::process::exit;

const MEM_SIZE: usize = 64 * 1024 * 1024;

fn read_i64(mem: &[u8], at: usize) -> i64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&mem[at..at + 8]);
    i64::from_le_bytes(bytes)
}

fn write_i64(mem: &mut [u8], at: usize, value: i64) {
    mem[at..at + 8].copy_from_slice(&value.to_le_bytes());
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: vm <tape>");
        exit(2);
    }
    let tape = match std::fs::read(&args[1]) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("vm: cannot read {}: {}", args[1], error);
            exit(1);
        }
    };

    let mut mem = vec![0u8; MEM_SIZE];
    mem[..tape.len()].copy_from_slice(&tape);

    let mut reg = [0i64; 16];
    let mut pc: usize = 0;

    let stdin = std::io::stdin();
    let mut input = stdin.lock().bytes();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    loop {
        let op = mem[pc];
        // operand offsets within this instruction
        let a = pc + 1; // first operand
        match op {
            OP_HALT => {
                let code = (reg[mem[a] as usize] & 0xff) as i32;
                let _ = out.flush();
                exit(code);
            }
            OP_IMM => {
                reg[mem[a] as usize] = read_i64(&mem, a + 1);
                pc += instr_size(op);
            }
            OP_MOV => { reg[mem[a] as usize] = reg[mem[a + 1] as usize]; pc += instr_size(op); }
            OP_ADD => { reg[mem[a] as usize] = reg[mem[a] as usize].wrapping_add(reg[mem[a + 1] as usize]); pc += instr_size(op); }
            OP_SUB => { reg[mem[a] as usize] = reg[mem[a] as usize].wrapping_sub(reg[mem[a + 1] as usize]); pc += instr_size(op); }
            OP_MUL => { reg[mem[a] as usize] = reg[mem[a] as usize].wrapping_mul(reg[mem[a + 1] as usize]); pc += instr_size(op); }
            OP_DIV => { reg[mem[a] as usize] /= reg[mem[a + 1] as usize]; pc += instr_size(op); }
            OP_MOD => { reg[mem[a] as usize] %= reg[mem[a + 1] as usize]; pc += instr_size(op); }
            OP_LOADB => { reg[mem[a] as usize] = mem[reg[mem[a + 1] as usize] as usize] as i64; pc += instr_size(op); }
            OP_STOREB => { let addr = reg[mem[a] as usize] as usize; mem[addr] = (reg[mem[a + 1] as usize] & 0xff) as u8; pc += instr_size(op); }
            OP_LOAD => { reg[mem[a] as usize] = read_i64(&mem, reg[mem[a + 1] as usize] as usize); pc += instr_size(op); }
            OP_STORE => { let addr = reg[mem[a] as usize] as usize; let v = reg[mem[a + 1] as usize]; write_i64(&mut mem, addr, v); pc += instr_size(op); }
            OP_JMP => { pc = read_i64(&mem, a) as usize; }
            OP_JZ => { if reg[mem[a] as usize] == 0 { pc = read_i64(&mem, a + 1) as usize; } else { pc += instr_size(op); } }
            OP_JNZ => { if reg[mem[a] as usize] != 0 { pc = read_i64(&mem, a + 1) as usize; } else { pc += instr_size(op); } }
            OP_JLT => { if reg[mem[a] as usize] < reg[mem[a + 1] as usize] { pc = read_i64(&mem, a + 2) as usize; } else { pc += instr_size(op); } }
            OP_JEQ => { if reg[mem[a] as usize] == reg[mem[a + 1] as usize] { pc = read_i64(&mem, a + 2) as usize; } else { pc += instr_size(op); } }
            OP_READ => {
                let value = match input.next() {
                    Some(Ok(byte)) => byte as i64,
                    _ => -1,
                };
                reg[mem[a] as usize] = value;
                pc += instr_size(op);
            }
            OP_WRITE => {
                let byte = (reg[mem[a] as usize] & 0xff) as u8;
                let _ = out.write_all(&[byte]);
                pc += instr_size(op);
            }
            _ => {
                eprintln!("vm: bad opcode {} at pc {}", op, pc);
                exit(4);
            }
        }
    }
}
