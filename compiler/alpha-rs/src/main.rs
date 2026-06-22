// alpha-onramp — a THROWAWAY Alpha compiler written in Rust.
//
// Purpose ("action produces information"): discover what Alpha actually needs by
// compiling it, then port THIS compiler's structure to Alpha so Alpha compiles
// itself. The on-ramp's trust lineage does not matter (it is discarded); what
// matters is that it is written in simple, arena/index-based, monomorphic Rust
// that ports 1:1 to Alpha, and that its front-end enforces the Alpha subset.
//
// Module layout decouples front-end (platform-independent) from per-arch
// lowering and per-format image writing, so the eventual Rust -> Alpha port has
// clean per-file boundaries and adding a target (aarch64.rs / elf.rs) is a new
// sibling file, not an edit to a monolith:
//
//   lex.rs   source bytes -> tokens          (front-end)
//   ast.rs   index-based AST node types       (front-end)
//   parse.rs tokens -> AST, enforces subset   (front-end)
//   x64.rs   AST -> x86-64 machine code       (per-arch backend)
//   pe.rs    code -> Windows PE32+ image      (per-format image)
//   util.rs  shared helpers
//
// Slices done:
//   1. exit_process(N) end-to-end -> a runnable Windows x64 PE.
//   2. expressions + locals: `let x: i32 = 3 + 4 * 2; exit_process(x)` with
//      trap-on-overflow (see x64.rs).

mod ast;
mod lex;
mod parse;
mod pe;
mod util;
mod x64;

use std::process::exit;

fn compile(source: &[u8]) -> Result<Vec<u8>, String> {
    let tokens = lex::lex(source)?;
    let mut parser = parse::Parser::new(&tokens, source);
    let program = parser.parse_program()?;
    let lowered = x64::lower_program(&program);
    Ok(pe::build_pe(&lowered))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: alpha-onramp <input.alp> [output.exe]");
        exit(2);
    }
    let input = &args[1];
    let output = if args.len() >= 3 {
        args[2].clone()
    } else {
        "a.exe".to_string()
    };

    let source = match std::fs::read(input) {
        Ok(contents) => contents,
        Err(error) => {
            eprintln!("alpha-onramp: cannot read {}: {}", input, error);
            exit(1);
        }
    };

    match compile(&source) {
        Ok(bytes) => {
            if let Err(error) = std::fs::write(&output, &bytes) {
                eprintln!("alpha-onramp: cannot write {}: {}", output, error);
                exit(1);
            }
            eprintln!("alpha-onramp: wrote {} ({} bytes)", output, bytes.len());
        }
        Err(error) => {
            eprintln!("{}", error);
            exit(1);
        }
    }
}
