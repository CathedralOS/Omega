//! Targeted coverage tests for interpreter constructs that have no RUN canary yet, plus
//! PROBES that pin down what the frontend actually accepts (so lib.rs's scope notes stay
//! honest). Each test writes a tiny program to a temp dir, compiles it to checked trees,
//! and interprets it against a hand-computed expectation.
//!
//! Findings these tests pin down:
//! - General/open RANGE expressions outside the index position (`let r: i32 = 1..5;`,
//!   `f(1..5)`) are FRONTEND-REJECTED (parse errors) -- the parser only produces
//!   `ExpressionNode::Range` inside `collection[...]`. The interpreter's subslice support
//!   therefore covers every Range the frontend can emit.
//! - Case PAYLOAD declarations (`data E { case A(value: i32); }`) are FRONTEND-REJECTED.
//!   A parenthesized construction `E::A(5)` against a payload-less case still parses (as
//!   a call expression), but resolves to nothing; the interpreter declines it rather than
//!   guessing a value.
//! - Multi-impl `dyn Trait` dispatch DOES reach checked trees. The interpreter dispatches
//!   by the receiver's runtime type and is AHEAD of the native backend here (the backend
//!   only devirtualizes single-impl traits; as of this writing it emits a crashing binary
//!   for the two-impl program below).

use omega_compiler::compile_to_checked;
use omega_interpreter::interpret;
use std::fs;
use std::path::PathBuf;

/// Write `source` to a fresh temp dir as `main.omg` and return the path. The dir is keyed
/// by test name + pid so parallel tests do not collide.
fn write_program(name: &str, source: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "omega-interp-coverage-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp program dir");
    let main_path = dir.join("main.omg");
    fs::write(&main_path, source).expect("write probe program");
    main_path
}

fn frontend_rejects(name: &str, source: &str) {
    let main_path = write_program(name, source);
    let result = compile_to_checked(&main_path, None);
    assert!(
        result.is_err(),
        "{name}: expected the frontend to reject this program; it compiled"
    );
}

// ---- ranges ------------------------------------------------------------------

/// A range as a `let` initializer does not parse: the parser only recognizes `..`/`..=`
/// inside an index expression. (If this ever starts compiling, the interpreter's
/// `ExpressionNode::Range => unsupported` arm needs a real value representation.)
#[test]
fn range_as_let_initializer_is_frontend_rejected() {
    frontend_rejects(
        "range-let",
        r#"
boundary trait Console {
    machine exit_process(return_code: i32);
}

data Main {
    console: Console;
}

machine Main::main(&mut self) {
    let r: i32 = 1..5;
    self.console.exit_process(0);
}
"#,
    );
}

/// A range as a call argument does not parse either (`expected ')', found '..'`).
#[test]
fn range_as_call_argument_is_frontend_rejected() {
    frontend_rejects(
        "range-arg",
        r#"
boundary trait Console {
    machine exit_process(return_code: i32);
}

data Main {
    console: Console;
}

machine Main::main(&mut self) {
    let n: i32 = self.take(1..5);
    self.console.exit_process(n);
}

machine Main::take(&mut self, r: i32) -> i32 {
    r
}
"#,
    );
}

// ---- case payloads -----------------------------------------------------------

/// A payload-carrying case declaration does not parse ("case payloads are not
/// implemented yet"); the typed-tree `DataVariant` has no payload slot at all. (If this
/// ever starts compiling, `Value::Enum`'s `payload` cells need real construction
/// support.)
#[test]
fn case_payload_declaration_is_frontend_rejected() {
    frontend_rejects(
        "case-payload-decl",
        r#"
boundary trait Console {
    machine exit_process(return_code: i32);
}

data Token {
    case Number(value: i32);
    case End;
}

data Main {
    console: Console;
}

machine Main::main(&mut self) {
    self.console.exit_process(0);
}
"#,
    );
}

/// `Token::Number(5)` against a payload-LESS case parses as a call expression and reaches
/// checked trees, but resolves to no machine/state. The interpreter must DECLINE (skip)
/// rather than invent a value -- skip-don't-lie.
#[test]
fn paren_variant_construction_is_declined_not_guessed() {
    let main_path = write_program(
        "case-paren-construct",
        r#"
boundary trait Console {
    machine exit_process(return_code: i32);
}

data Token {
    case Number;
    case End;
}

data Main {
    console: Console;
}

machine Main::main(&mut self) {
    let t: Token = Token::Number(5);
    self.console.exit_process(0);
}
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .expect("paren variant construction parses (as a call)");
    let outcome = interpret(&checked, b"");
    assert!(
        outcome.is_error(),
        "expected the interpreter to decline the unresolvable variant-call, got exit {}",
        outcome.exit_code
    );
}

// ---- multi-impl dyn dispatch ---------------------------------------------------

/// TWO data types satisfy `Shape`; a `&mut dyn Shape` parameter must dispatch by the
/// RECEIVER'S RUNTIME TYPE: Circle::code() == 9, Square::code() == 4, so
/// 9 * 10 + 4 == 94. The native backend now matches via call-site monomorphization
/// (one resolved candidate per impl; each call site's receiver type selects one) --
/// see the run canary traits/runtime_dyn_two_impl_dispatch_exit, which the
/// differential harness checks against this same semantics.
#[test]
fn dyn_two_impl_dispatch_selects_impl_by_runtime_type() {
    let main_path = write_program(
        "dyn-two-impls",
        r#"
boundary trait Console {
    machine exit_process(return_code: i32);
}

trait Shape {
    machine code(&mut self) -> i32;
}

data Circle {}

machine Circle::code(&mut self) -> i32 {
    transition {
        _ -> 9
    }
}

data Square {}

machine Square::code(&mut self) -> i32 {
    transition {
        _ -> 4
    }
}

data Main {
    console: Console;
    c: Circle;
    q: Square;
}

machine Main::main(&mut self) {
    let a: i32 = self.dispatch(&mut self.c);
    let b: i32 = self.dispatch(&mut self.q);
    let n: i32 = a * 10 + b;
    self.console.exit_process(n);
}

machine Main::dispatch(&mut self, s: &mut dyn Shape) -> i32 {
    s.code()
}
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|diagnostics| {
            panic!(
                "two-impl dyn program should compile to checked trees:\n{}",
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
    let outcome = interpret(&checked, b"");
    assert_eq!(outcome.error, None, "interpreter declined the program");
    assert_eq!(
        outcome.exit_code, 94,
        "dyn dispatch must pick Circle (9) then Square (4): 9*10+4 == 94"
    );
}

/// The same two-impl shape, but with the CALL ORDER swapped, so a dispatcher that always
/// picks the lexically-first impl (the pre-fix behavior: 99) or the most-recently-seen one
/// cannot pass both tests: Square first gives 4 * 10 + 9 == 49.
#[test]
fn dyn_two_impl_dispatch_swapped_order() {
    let main_path = write_program(
        "dyn-two-impls-swapped",
        r#"
boundary trait Console {
    machine exit_process(return_code: i32);
}

trait Shape {
    machine code(&mut self) -> i32;
}

data Circle {}

machine Circle::code(&mut self) -> i32 {
    transition {
        _ -> 9
    }
}

data Square {}

machine Square::code(&mut self) -> i32 {
    transition {
        _ -> 4
    }
}

data Main {
    console: Console;
    c: Circle;
    q: Square;
}

machine Main::main(&mut self) {
    let a: i32 = self.dispatch(&mut self.q);
    let b: i32 = self.dispatch(&mut self.c);
    let n: i32 = a * 10 + b;
    self.console.exit_process(n);
}

machine Main::dispatch(&mut self, s: &mut dyn Shape) -> i32 {
    s.code()
}
"#,
    );
    let checked = compile_to_checked(&main_path, None).expect("swapped two-impl dyn compiles");
    let outcome = interpret(&checked, b"");
    assert_eq!(outcome.error, None, "interpreter declined the program");
    assert_eq!(
        outcome.exit_code, 49,
        "dyn dispatch must pick Square (4) then Circle (9): 4*10+9 == 49"
    );
}
