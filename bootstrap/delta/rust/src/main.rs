// delta-onramp — a THROWAWAY Delta compiler written in Rust.
//
// Purpose ("action produces information"): discover what Delta actually needs by
// compiling it, then port THIS compiler's structure to Delta so Delta compiles
// itself. The on-ramp's trust lineage does not matter (it is discarded); what
// matters is that it is written in simple, arena/index-based, monomorphic Rust
// that ports 1:1 to Delta, and that its front-end enforces the Delta subset.
//
// Module layout decouples front-end (platform-independent) from per-arch
// lowering and per-format image writing, so the eventual Rust -> Delta port has
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

mod aarch64;
mod ast;
mod discharge;
mod gamma_emit;
mod lex;
mod parse;
mod pe;
mod util;
mod x64;

use std::process::{exit, Command};

fn compile(source: &[u8]) -> Result<Vec<u8>, String> {
    let tokens = lex::lex(source)?;
    let mut parser = parse::Parser::new(&tokens, source);
    let program = parser.parse_program()?;
    let lowered = x64::lower_program(&program);
    Ok(pe::build_pe(&lowered))
}

// aarch64/macOS path: front-end -> ARM64 assembly text -> clang assemble+link ->
// codesign ad-hoc. Returns the runnable binary's path (== `output`). Selected by
// `DELTA_ARCH=aarch64`; this is what makes the rung executable (verifiable) on this
// machine, where the x64 PE backend's output cannot run.
fn compile_aarch64(source: &[u8], output: &str) -> Result<(), String> {
    let tokens = lex::lex(source)?;
    let mut parser = parse::Parser::new(&tokens, source);
    let program = parser.parse_program()?;
    let asm = aarch64::lower_program(&program);

    let asm_path = format!("{}.s", output);
    std::fs::write(&asm_path, asm.as_bytes())
        .map_err(|e| format!("cannot write {}: {}", asm_path, e))?;

    let clang = Command::new("clang")
        .args(["-arch", "arm64", "-Wl,-no_uuid", "-o", output, &asm_path])
        .output()
        .map_err(|e| format!("cannot run clang: {}", e))?;
    if !clang.status.success() {
        return Err(format!(
            "clang failed:\n{}",
            String::from_utf8_lossy(&clang.stderr)
        ));
    }
    // Apple Silicon refuses to exec an unsigned/invalid Mach-O; ad-hoc sign it.
    let sign = Command::new("codesign")
        .args(["-f", "-s", "-", output])
        .output()
        .map_err(|e| format!("cannot run codesign: {}", e))?;
    if !sign.status.success() {
        return Err(format!(
            "codesign failed:\n{}",
            String::from_utf8_lossy(&sign.stderr)
        ));
    }
    Ok(())
}

fn main() {
    // delta <input.alp>... <output.exe>  — Delta is multi-FILE, one translation
    // unit: the inputs are concatenated in order (no module system), so the front
    // end / per-arch / per-format files build as one program. With a single input
    // the output defaults to a.exe.
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: delta <input.alp>... <output.exe>");
        exit(2);
    }
    let (inputs, output): (&[String], String) = if args.len() >= 3 {
        (&args[1..args.len() - 1], args[args.len() - 1].clone())
    } else {
        (&args[1..2], "a.exe".to_string())
    };

    let mut source: Vec<u8> = Vec::new();
    for input in inputs {
        match std::fs::read(input) {
            Ok(contents) => {
                source.extend_from_slice(&contents);
                source.push(b'\n'); // keep a separator so a file ending mid-token can't fuse
            }
            Err(error) => {
                eprintln!("alpha: cannot read {}: {}", input, error);
                exit(1);
            }
        }
    }

    // Static contract discharge: instead of emitting a binary, print one proof certificate
    // per statically-dischargeable `ensures` contract. The trust anchor (check.beta) validates
    // them at build time -- a true postcondition is accepted, a false one rejected. The
    // compiler proves the contract; it is not trusted to.
    if std::env::var("DELTA_EMIT").as_deref() == Ok("contracts") {
        let tokens = match lex::lex(&source) {
            Ok(tokens) => tokens,
            Err(error) => {
                eprintln!("{}", error);
                exit(1);
            }
        };
        let mut parser = parse::Parser::new(&tokens, &source);
        let program = match parser.parse_program() {
            Ok(program) => program,
            Err(error) => {
                eprintln!("{}", error);
                exit(1);
            }
        };
        for certificate in discharge::emit_contracts(&program) {
            println!("{}", certificate);
        }
        return;
    }

    // Delta meaning via gamma: translate the supported subset to a gamma expression the
    // Rust-free reference interpreter evaluates. The delta-meaning diamond checks it against
    // native execution. Prints nothing (and exits 0) for programs outside the supported subset.
    if std::env::var("DELTA_EMIT").as_deref() == Ok("gamma") {
        let tokens = match lex::lex(&source) {
            Ok(tokens) => tokens,
            Err(error) => {
                eprintln!("{}", error);
                exit(1);
            }
        };
        let mut parser = parse::Parser::new(&tokens, &source);
        let program = match parser.parse_program() {
            Ok(program) => program,
            Err(error) => {
                eprintln!("{}", error);
                exit(1);
            }
        };
        // DELTA_GAMMA_INPUT="65 66 67" bakes stdin bytes into the gamma program for read_byte() programs
        // (the diamond feeds the SAME bytes to native stdin); absent/empty -> no input.
        let input: Vec<i32> = std::env::var("DELTA_GAMMA_INPUT")
            .unwrap_or_default()
            .split_whitespace()
            .filter_map(|t| t.parse::<i32>().ok())
            .collect();
        if let Some(expr) = gamma_emit::emit_gamma(&program, &input) {
            println!("{}", expr);
        }
        return;
    }

    if std::env::var("DELTA_ARCH").as_deref() == Ok("aarch64") {
        match compile_aarch64(&source, &output) {
            Ok(()) => eprintln!("delta-onramp: wrote {} (aarch64/macOS, signed)", output),
            Err(error) => {
                eprintln!("{}", error);
                exit(1);
            }
        }
        return;
    }

    match compile(&source) {
        Ok(bytes) => {
            if let Err(error) = std::fs::write(&output, &bytes) {
                eprintln!("delta-onramp: cannot write {}: {}", output, error);
                exit(1);
            }
            eprintln!("delta-onramp: wrote {} ({} bytes)", output, bytes.len());
        }
        Err(error) => {
            eprintln!("{}", error);
            exit(1);
        }
    }
}
