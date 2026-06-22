// The Alpha assembler — assembles Alpha-assembly text into a bytecode tape. This is
// the throwaway on-ramp; the self-hosting target is the same assembler written in
// Alpha-assembly and run on the VM.
//
//   assembler [in.alp] [out.tape]      (default: stdin -> stdout)
//
// Syntax (one item per line; `;` starts a comment):
//   label:                a label, bound to the current tape address
//   mnemonic op, op, ...   an instruction; operands are rN, a decimal int, or a label
// Two passes: pass 1 assigns label addresses; pass 2 emits bytes with labels resolved.

#[path = "isa.rs"]
mod isa;
use isa::*;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::process::exit;

enum Item {
    Label(String),
    Instr { op: u8, args: Vec<String> },
}

fn fail(message: String) -> ! {
    eprintln!("asm: {}", message);
    exit(1);
}

fn parse(source: &str) -> Vec<Item> {
    let mut items = Vec::new();
    for (line_number, raw) in source.lines().enumerate() {
        let line = raw.split(';').next().unwrap_or("");
        // tokens: whitespace- or comma-separated
        let mut tokens: Vec<&str> = line
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|t| !t.is_empty())
            .collect();
        if tokens.is_empty() {
            continue;
        }
        // leading label(s)
        while let Some(first) = tokens.first() {
            if let Some(name) = first.strip_suffix(':') {
                items.push(Item::Label(name.to_string()));
                tokens.remove(0);
            } else {
                break;
            }
        }
        if tokens.is_empty() {
            continue;
        }
        let mnemonic_text = tokens[0];
        // accept either a mnemonic ("imm") or a bare opcode number ("1"); the
        // numeric form is what the self-hosting assembler (assembler.alp) reads + emits.
        let op = match mnemonic(mnemonic_text) {
            Some(op) => op,
            None => match mnemonic_text.parse::<u8>() {
                Ok(n) if n < OP_COUNT => n,
                _ => fail(format!("line {}: unknown mnemonic '{}'", line_number + 1, mnemonic_text)),
            },
        };
        let args: Vec<String> = tokens[1..].iter().map(|t| t.to_string()).collect();
        let expected = operands(op).len();
        if args.len() != expected {
            fail(format!(
                "line {}: '{}' takes {} operands, got {}",
                line_number + 1, mnemonic_text, expected, args.len()
            ));
        }
        items.push(Item::Instr { op, args });
    }
    items
}

fn register(token: &str) -> u8 {
    // accept "r5" (mnemonic form) or plain "5" (numeric form the asm-in-asm uses)
    let digits = token.strip_prefix('r').unwrap_or(token);
    let index = digits
        .parse::<u8>()
        .unwrap_or_else(|_| fail(format!("bad register '{}'", token)));
    if index > 15 {
        fail(format!("register out of range '{}'", token));
    }
    index
}

fn resolve_value(token: &str, labels: &HashMap<String, i64>) -> i64 {
    if let Ok(value) = token.parse::<i64>() {
        return value;
    }
    match labels.get(token) {
        Some(&address) => address,
        None => fail(format!("undefined label '{}'", token)),
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().collect();

    // --num: re-emit the source with mnemonics replaced by opcode numbers (labels and
    // operands preserved). This is the numeric form the self-hosting assembler.alp
    // reads; it lets assembler.alp be authored in mnemonics yet self-host on numbers.
    let numericize = args.len() >= 2 && args[1] == "--num";
    if numericize {
        args.remove(1);
    }

    let mut source = String::new();
    if args.len() >= 2 {
        source = std::fs::read_to_string(&args[1])
            .unwrap_or_else(|e| fail(format!("cannot read {}: {}", args[1], e)));
    } else {
        std::io::stdin().read_to_string(&mut source).ok();
    }

    let items = parse(&source);

    if numericize {
        let mut out = String::new();
        for item in &items {
            match item {
                Item::Label(name) => out.push_str(&format!("{}:\n", name)),
                Item::Instr { op, args } => {
                    out.push_str(&op.to_string());
                    // emit register operands as plain numbers (rN -> N); leave
                    // immediates and label/addr operands as written.
                    for (arg, kind) in args.iter().zip(operands(*op)) {
                        out.push(' ');
                        if *kind == Operand::Reg {
                            out.push_str(&register(arg).to_string());
                        } else {
                            out.push_str(arg);
                        }
                    }
                    out.push('\n');
                }
            }
        }
        print!("{}", out);
        return;
    }

    // pass 1: assign label addresses
    let mut labels: HashMap<String, i64> = HashMap::new();
    let mut address: i64 = 0;
    for item in &items {
        match item {
            Item::Label(name) => {
                labels.insert(name.clone(), address);
            }
            Item::Instr { op, .. } => address += instr_size(*op) as i64,
        }
    }

    // pass 2: emit
    let mut tape: Vec<u8> = Vec::new();
    for item in &items {
        if let Item::Instr { op, args } = item {
            tape.push(*op);
            for (operand, kind) in args.iter().zip(operands(*op)) {
                match kind {
                    Operand::Reg => tape.push(register(operand)),
                    Operand::Imm => tape.extend_from_slice(&resolve_value(operand, &labels).to_le_bytes()),
                    Operand::Addr => tape.extend_from_slice(&resolve_value(operand, &labels).to_le_bytes()),
                }
            }
        }
    }

    if args.len() >= 3 {
        std::fs::write(&args[2], &tape).unwrap_or_else(|e| fail(format!("cannot write {}: {}", args[2], e)));
    } else {
        std::io::stdout().write_all(&tape).ok();
    }
}
