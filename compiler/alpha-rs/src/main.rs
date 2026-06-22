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

fn compile(src: &[u8]) -> Result<Vec<u8>, String> {
    let toks = lex::lex(src)?;
    let mut parser = parse::Parser::new(&toks, src);
    let program = parser.parse_program()?;
    let code = x64::lower_main(&program);
    Ok(pe::build_pe(&code))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: alpha-onramp <input.alpha> [output.exe]");
        exit(2);
    }
    let input = &args[1];
    let output = if args.len() >= 3 {
        args[2].clone()
    } else {
        "a.exe".to_string()
    };

    let src = match std::fs::read(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("alpha-onramp: cannot read {}: {}", input, e);
            exit(1);
        }
    };

    match compile(&src) {
        Ok(bytes) => {
            if let Err(e) = std::fs::write(&output, &bytes) {
                eprintln!("alpha-onramp: cannot write {}: {}", output, e);
                exit(1);
            }
            eprintln!("alpha-onramp: wrote {} ({} bytes)", output, bytes.len());
        }
        Err(e) => {
            eprintln!("{}", e);
            exit(1);
        }
    }
}
