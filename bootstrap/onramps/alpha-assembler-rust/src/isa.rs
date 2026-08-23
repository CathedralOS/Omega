// Alpha tape-VM instruction set — shared by the assembler (assembler.rs) and the VM
// (vm.rs) so they can never disagree on encoding.
//
// A program is a flat bytecode tape, loaded at address 0; PC starts at 0. There are
// 16 registers (r0..r15), each a signed 64-bit integer, and a flat byte-addressable
// memory. Each instruction is one opcode byte followed by its operands:
//   Reg  = 1 byte  (register index 0..15)
//   Imm  = 8 bytes (little-endian i64)
//   Addr = 8 bytes (little-endian absolute byte offset into the tape)

pub const OP_HALT: u8 = 0; //  halt rS            exit(rS & 0xff)
pub const OP_IMM: u8 = 1; //   imm  rD, imm       rD = imm
pub const OP_MOV: u8 = 2; //   mov  rD, rS        rD = rS
pub const OP_ADD: u8 = 3; //   add  rD, rS        rD = rD + rS
pub const OP_SUB: u8 = 4; //   sub  rD, rS        rD = rD - rS
pub const OP_MUL: u8 = 5; //   mul  rD, rS        rD = rD * rS
pub const OP_DIV: u8 = 6; //   div  rD, rS        rD = rD / rS  (signed, truncating)
pub const OP_MOD: u8 = 7; //   mod  rD, rS        rD = rD % rS
pub const OP_LOADB: u8 = 8; // loadb rD, rS       rD = mem[rS]            (byte, zero-extended)
pub const OP_STOREB: u8 = 9; // storeb rD, rS     mem[rD] = rS & 0xff
pub const OP_LOAD: u8 = 10; // load rD, rS        rD = i64 at mem[rS..rS+8]
pub const OP_STORE: u8 = 11; // store rD, rS      i64 at mem[rD..rD+8] = rS
pub const OP_JMP: u8 = 12; //  jmp  addr          PC = addr
pub const OP_JZ: u8 = 13; //   jz   rS, addr      if rS == 0 PC = addr
pub const OP_JNZ: u8 = 14; //  jnz  rS, addr      if rS != 0 PC = addr
pub const OP_JLT: u8 = 15; //  jlt  rA, rB, addr  if rA <  rB (signed) PC = addr
pub const OP_JEQ: u8 = 16; //  jeq  rA, rB, addr  if rA == rB PC = addr
pub const OP_READ: u8 = 17; // read rD            rD = next stdin byte, or -1 at EOF
pub const OP_WRITE: u8 = 18; // write rS          write (rS & 0xff) to stdout
pub const OP_CALL: u8 = 19; // call addr          push return addr; PC = addr
pub const OP_RET: u8 = 20; //  ret                PC = popped return addr

pub const OP_COUNT: u8 = 21;

#[derive(Clone, Copy, PartialEq)]
pub enum Operand {
    Reg,
    Imm,
    Addr,
}

impl Operand {
    pub fn size(self) -> usize {
        match self {
            Operand::Reg => 1,
            Operand::Imm | Operand::Addr => 8,
        }
    }
}

pub fn operands(op: u8) -> &'static [Operand] {
    use Operand::*;
    match op {
        OP_HALT | OP_READ | OP_WRITE => &[Reg],
        OP_IMM => &[Reg, Imm],
        OP_MOV | OP_ADD | OP_SUB | OP_MUL | OP_DIV | OP_MOD | OP_LOADB | OP_STOREB | OP_LOAD
        | OP_STORE => &[Reg, Reg],
        OP_JMP | OP_CALL => &[Addr],
        OP_JZ | OP_JNZ => &[Reg, Addr],
        OP_JLT | OP_JEQ => &[Reg, Reg, Addr],
        OP_RET => &[],
        _ => &[],
    }
}

pub fn instr_size(op: u8) -> usize {
    1 + operands(op).iter().map(|o| o.size()).sum::<usize>()
}

// Assembly mnemonic -> opcode. Mnemonics are the lowercase op names above.
pub fn mnemonic(name: &str) -> Option<u8> {
    Some(match name {
        "halt" => OP_HALT,
        "imm" => OP_IMM,
        "mov" => OP_MOV,
        "add" => OP_ADD,
        "sub" => OP_SUB,
        "mul" => OP_MUL,
        "div" => OP_DIV,
        "mod" => OP_MOD,
        "loadb" => OP_LOADB,
        "storeb" => OP_STOREB,
        "load" => OP_LOAD,
        "store" => OP_STORE,
        "jmp" => OP_JMP,
        "jz" => OP_JZ,
        "jnz" => OP_JNZ,
        "jlt" => OP_JLT,
        "jeq" => OP_JEQ,
        "read" => OP_READ,
        "write" => OP_WRITE,
        "call" => OP_CALL,
        "ret" => OP_RET,
        _ => return None,
    })
}
