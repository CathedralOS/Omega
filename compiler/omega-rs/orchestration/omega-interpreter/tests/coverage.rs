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
//! - Case PAYLOADS (declaration, brace construction, transition-arm binding, tag-only
//!   equality) interpret end-to-end; the native backend gates construction after checked
//!   trees, so this coverage is interpreter-only until payload codegen lands. A
//!   parenthesized construction `E::A(5)` is NOT the construction spelling (braces are);
//!   it still parses as a call expression that resolves to nothing, and the interpreter
//!   declines it rather than guessing a value.
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

/// Construct a payload-carrying case (`Command::Move { steps: 70 }`), dispatch on it
/// with a case-pattern arm, and BIND the payload into the target state's argument.
/// The whole chain -- construction, tag-compare guard, `steps` rewritten to the
/// subject's payload read -- must deliver 70 to `exit_process`.
#[test]
fn case_payload_construction_and_binding_deliver_payload() {
    let main_path = write_program(
        "case-payload-bind",
        r#"
boundary trait Console {
    machine exit_process(return_code: i32);
}

data Command {
    case None;
    case Quit;
    case Move(steps: i32);
}

data Main {
    console: Console;
    cmd: Command;
}

machine Main::main(&mut self) {
    self.cmd = Command::Move { steps: 70 };

    transition self.cmd {
        Command::Move { steps } -> done(steps)
        _ -> bad()
    }

    state done(&mut self, steps: i32) {
        self.console.exit_process(steps);
    }

    state bad(&mut self) {
        self.console.exit_process(71);
    }
}
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|d| panic!("payload program should reach checked trees: {d:?}"));
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "interpreter should support payload construction + binding, got {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70, "bound payload should reach the target state");
}

/// A case-pattern arm whose tag does NOT match must fall through; equality between case
/// values compares the TAG ONLY (payloads never participate in `==`, mirroring the
/// native backend's constant tag compare). Two differently-constructed `Move` values
/// compare equal; `Quit` selects the second arm.
#[test]
fn case_equality_is_tag_only_and_mismatched_tag_falls_through() {
    let main_path = write_program(
        "case-payload-tag-equality",
        r#"
boundary trait Console {
    machine exit_process(return_code: i32);
}

data Command {
    case None;
    case Quit;
    case Move(steps: i32);
}

data Main {
    console: Console;
    cmd: Command;
    other: Command;
}

machine Main::main(&mut self) {
    self.cmd = Command::Quit;
    self.other = Command::Move { steps: 9 };

    transition self.cmd {
        Command::Move { steps } -> bad()
        Command::Quit -> check()
        _ -> bad()
    }

    state check(&mut self) {
        // Tag-only equality: a constructed payload value equals the bare case name.
        transition self.other {
            Command::Move -> good()
            _ -> bad()
        }
    }

    state good(&mut self) {
        self.console.exit_process(70);
    }

    state bad(&mut self) {
        self.console.exit_process(71);
    }
}
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|d| panic!("tag-equality program should reach checked trees: {d:?}"));
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "interpreter should support tag-only case dispatch, got {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

/// A case pattern can bind MULTIPLE payload fields and use them in an `if` guard as
/// well as the target arguments: `Command::Walk { dx, dy } if dx > dy -> move(dx, dy)`.
#[test]
fn case_payload_multi_field_binding_with_guard() {
    let main_path = write_program(
        "case-payload-multi-bind",
        r#"
boundary trait Console {
    machine exit_process(return_code: i32);
}

data Command {
    case None;
    case Walk(dx: i32, dy: i32);
}

data Main {
    console: Console;
    cmd: Command;
}

machine Main::main(&mut self) {
    self.cmd = Command::Walk { dx: 60, dy: 10 };

    transition self.cmd {
        Command::Walk { dx, dy } if dx > dy -> done(dx, dy)
        _ -> bad()
    }

    state done(&mut self, dx: i32, dy: i32) {
        self.console.exit_process(dx + dy);
    }

    state bad(&mut self) {
        self.console.exit_process(71);
    }
}
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|d| panic!("multi-field payload program should reach checked trees: {d:?}"));
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "interpreter should support multi-field payload binding, got {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

/// Under `[zero_init]` (zero means empty, frozen decision 8) the ZERO case
/// (first case) declaring a payload is rejected by validation: the
/// zero-initialized value must be the empty value, so tag 0 carries no
/// payload. Without the property the same shape is legal.
#[test]
fn zero_case_payload_is_rejected() {
    frontend_rejects(
        "case-zero-payload",
        r#"
boundary trait Console {
    machine exit_process(return_code: i32);
}

data Token [zero_init] {
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
    let a: i32 in Wrapping = self.dispatch(&mut self.c);
    let b: i32 in Wrapping = self.dispatch(&mut self.q);
    let n: i32 in Wrapping = a * 10 + b;
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
    let a: i32 in Wrapping = self.dispatch(&mut self.q);
    let b: i32 in Wrapping = self.dispatch(&mut self.c);
    let n: i32 in Wrapping = a * 10 + b;
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

// ---- straight-line `main` terminal expressions --------------------------------

/// A no-transition `main -> i32` whose terminal expression is a LOCAL read must deliver
/// that local's value as the exit code, exactly like a bare literal terminal does.
/// (Native-backend probe 2026-06-11: literal `70` exits 70, `let exit_code: i32 = 70;
/// exit_code` exits 1 -- this test is the interpreter half of the parity check.)
#[test]
fn straight_line_terminal_local_delivers_exit_code() {
    let main_path = write_program(
        "straight-line-terminal-local",
        r#"
data Main { }

machine Main::main(&mut self) -> i32 {
    let exit_code: i32 = 70;
    exit_code
}
"#,
    );
    let checked = compile_to_checked(&main_path, None).expect("terminal-local program compiles");
    let outcome = interpret(&checked, b"");
    assert_eq!(outcome.error, None, "interpreter declined the program");
    assert_eq!(outcome.exit_code, 70, "terminal local read must become the exit code");
}

/// Same shape, terminal FIELD read-back after a straight-line field write.
#[test]
fn straight_line_terminal_field_readback_delivers_exit_code() {
    let main_path = write_program(
        "straight-line-terminal-field",
        r#"
data Main {
    count: i32;
}

machine Main::main(&mut self) -> i32 {
    self.count = 70;
    self.count
}
"#,
    );
    let checked = compile_to_checked(&main_path, None).expect("terminal-field program compiles");
    let outcome = interpret(&checked, b"");
    assert_eq!(outcome.error, None, "interpreter declined the program");
    assert_eq!(outcome.exit_code, 70, "terminal field read-back must become the exit code");
}

/// Same shape, terminal ARITHMETIC over a local.
#[test]
fn straight_line_terminal_arithmetic_delivers_exit_code() {
    let main_path = write_program(
        "straight-line-terminal-arithmetic",
        r#"
data Main { }

machine Main::main(&mut self) -> i32 {
    let x: i32 = 1 + 69;
    x
}
"#,
    );
    let checked = compile_to_checked(&main_path, None).expect("terminal-arithmetic program compiles");
    let outcome = interpret(&checked, b"");
    assert_eq!(outcome.error, None, "interpreter declined the program");
    assert_eq!(outcome.exit_code, 70, "terminal arithmetic must become the exit code");
}

// ---- wire zero-copy `&[u8]` borrowed-bytes field (#43) -----------------------

/// A borrowed byte slice `&[u8]` wire field round-trips: `encode` frames it
/// as RAW bytes (length varint + the bytes) and `decode` reads it back as a
/// buffer VIEW. This is the honest borrowed bytes/text field that replaced the
/// retired `&string` -- a `&[u8]` is already a fat slice, and the raw-byte
/// encoding is distinct from a `[u8; N]` repeated field (packed per-element
/// varints). Native byte-slice wire decode landed (#46/#47, `ReadWireByteSlice`
/// on both x86_64 and aarch64), so the round trip now also runs natively
/// (canaries/pass/wire/runtime_wire_decode_byte_slice_exit, oracle-matched);
/// this interpreter coverage pins the reference semantics.
///
/// `{ bytes: [72, 105] }` encodes to 5 bytes: era 0 (0x00), tag 0 (0x00),
/// length 2 (0x02), then `0x48 0x69`. The decoder consumes the same 5 bytes;
/// the canary checks the framing (written/read/ok) in-language and exits 70.
#[test]
fn wire_borrowed_byte_slice_field_round_trips() {
    let main_path = write_program(
        "wire-borrowed-byte-slice-roundtrip",
        r#"
boundary trait Console {
    machine exit_process(return_code: i32);
}

data Blob {
    0: bytes: &[u8];
}

data BlobSample {
    bytes: &[u8];
}

data WireVerdict {
    case Invalid;
    case Sound;
}

data Main {
    console: Console;
    source: [u8; 4];
    buffer: [u8; 64];
    written: usize;
    read: usize;
    verdict: WireVerdict;
}

machine Main::main(&mut self) {
    self.source[0] = 72;
    self.source[1] = 105;
    let sample: BlobSample = BlobSample { bytes: self.source[0..2] };

    Blob::encode(&sample, &mut self.buffer, &mut self.written);

    let decoded: BlobSample = BlobSample { bytes: self.source[0..2] };
    Blob::decode(&mut decoded, &self.buffer, &mut self.read, &mut self.verdict);

    let matches: bool = self.verdict == WireVerdict::Sound && self.written == 5 && self.read == 5;
    transition matches {
        true -> good()
        false -> bad()
    }

    state good(&mut self) {
        self.console.exit_process(70);
    }

    state bad(&mut self) {
        self.console.exit_process(71);
    }
}
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|d| panic!("borrowed-byte-slice wire program should reach checked trees: {d:?}"));
    let outcome = interpret(&checked, b"");
    assert_eq!(
        outcome.error, None,
        "interpreter should round-trip a borrowed `&[u8]` wire field"
    );
    assert_eq!(
        outcome.exit_code, 70,
        "encode+decode of a `&[u8]` field must consume 5 bytes (length + raw bytes)"
    );
}

// ---- std::fs (value-returning FilesystemHost raw seam; matches native) -------

/// The raw `FilesystemHost` boundary (value-returning ints) + a Console for
/// exit codes. Same surface the native backend lowers, so the interpreter is a
/// faithful differential model of the on-disk syscalls.
const FS_PRELUDE: &str = r#"
domain [u8]::Path when no_nul(self) {
}

boundary trait FilesystemHost {
    machine create(path: &[u8] in Path, mode: i32) -> i32;
    machine open(path: &[u8] in Path, flags: i32) -> i32;
    machine read(fd: i32, buffer: &mut [u8], count: usize) -> i64;
    machine write(fd: i32, bytes: &[u8]) -> i64;
    machine close(fd: i32) -> i32;
    machine remove(path: &[u8] in Path) -> i32;
    machine seek(fd: i32, offset: i64, whence: i32) -> i64;
    machine create_dir(path: &[u8] in Path, mode: i32) -> i32;
    machine remove_dir(path: &[u8] in Path) -> i32;
    machine rename(from: &[u8] in Path, to: &[u8] in Path) -> i32;
}

boundary trait Console {
    machine exit_process(return_code: i32);
}
"#;

/// Run `FS_PRELUDE ++ body` through the frontend + interpreter, asserting a
/// clean run and a specific exit code.
fn interpret_fs(name: &str, body: &str, expected_exit: i32, why: &str) {
    let main_path = write_program(name, &(FS_PRELUDE.to_owned() + body));
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|d| panic!("{name}: fs program should reach checked trees: {d:?}"));
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "{name}: interpreter should run the fs program, got {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, expected_exit, "{name}: {why}");
}

/// Full CRUD round-trip over the value-returning seam: create -> write 17B ->
/// close -> open -> read (17) -> close -> remove. Exit 70 only if read returns
/// exactly the 17 bytes written.
#[test]
fn filesystem_value_returning_crud_round_trip() {
    interpret_fs(
        "fs-vr-crud",
        r#"
data Main {
    fs: FilesystemHost;
    console: Console;
    mode: i32;
    read_flags: i32;
    cap: usize;
    fd: i32;
    n: i64;
    rn: i64;
    buffer: [u8; 64];
}

machine Main::main(&mut self) {
    self.mode = 420;
    self.read_flags = 0;
    self.cap = 64;
    self.fd = self.fs.create("/crud.txt", self.mode);
    transition self.fd >= 0 { true -> wrote() _ -> fail() }
    state wrote(&mut self) {
        self.n = self.fs.write(self.fd, "omega end to end\n");
        self.n = self.fs.close(self.fd);
        self.fd = self.fs.open("/crud.txt", self.read_flags);
        transition self.fd >= 0 { true -> rd() _ -> fail() }
    }
    state rd(&mut self) {
        self.rn = self.fs.read(self.fd, &mut self.buffer, self.cap);
        self.n = self.fs.close(self.fd);
        self.n = self.fs.remove("/crud.txt");
        transition self.rn == 17 { true -> ok() _ -> fail() }
    }
    state ok(&mut self) { self.console.exit_process(70); }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
        70,
        "create->write(17)->close->open->read must read back 17 bytes",
    );
}

/// Append (O_WRONLY|O_APPEND=9) grows the file: write "AAA", reopen append,
/// write "BBB", read back 6 bytes.
#[test]
fn filesystem_value_returning_append() {
    interpret_fs(
        "fs-vr-append",
        r#"
data Main {
    fs: FilesystemHost;
    console: Console;
    mode: i32;
    append_flags: i32;
    read_flags: i32;
    cap: usize;
    fd: i32;
    n: i64;
    rn: i64;
    buffer: [u8; 64];
}

machine Main::main(&mut self) {
    self.mode = 420;
    self.append_flags = 9;
    self.read_flags = 0;
    self.cap = 64;
    self.fd = self.fs.create("/app.txt", self.mode);
    transition self.fd >= 0 { true -> first() _ -> fail() }
    state first(&mut self) {
        self.n = self.fs.write(self.fd, "AAA");
        self.n = self.fs.close(self.fd);
        self.fd = self.fs.open("/app.txt", self.append_flags);
        transition self.fd >= 0 { true -> second() _ -> fail() }
    }
    state second(&mut self) {
        self.n = self.fs.write(self.fd, "BBB");
        self.n = self.fs.close(self.fd);
        self.fd = self.fs.open("/app.txt", self.read_flags);
        transition self.fd >= 0 { true -> rd() _ -> fail() }
    }
    state rd(&mut self) {
        self.rn = self.fs.read(self.fd, &mut self.buffer, self.cap);
        transition self.rn == 6 { true -> ok() _ -> fail() }
    }
    state ok(&mut self) { self.console.exit_process(70); }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
        70,
        "append must grow the file to 6 bytes",
    );
}

/// `seek(fd, 0, SEEK_END)` reports the file size; `open` of a missing path
/// returns a negative fd.
#[test]
fn filesystem_value_returning_seek_and_missing() {
    interpret_fs(
        "fs-vr-seek",
        r#"
data Main {
    fs: FilesystemHost;
    console: Console;
    mode: i32;
    zero: i64;
    seek_end: i32;
    fd: i32;
    n: i64;
    size: i64;
}

machine Main::main(&mut self) {
    self.mode = 420;
    self.zero = 0;
    self.seek_end = 2;
    self.fd = self.fs.open("/missing.txt", self.seek_end);
    transition self.fd >= 0 { true -> fail() _ -> make() }
    state make(&mut self) {
        self.fd = self.fs.create("/sz.txt", self.mode);
        transition self.fd >= 0 { true -> wrote() _ -> fail() }
    }
    state wrote(&mut self) {
        self.n = self.fs.write(self.fd, "omega end to end\n");
        self.size = self.fs.seek(self.fd, self.zero, self.seek_end);
        self.n = self.fs.close(self.fd);
        self.n = self.fs.remove("/sz.txt");
        transition self.size == 17 { true -> ok() _ -> fail() }
    }
    state ok(&mut self) { self.console.exit_process(70); }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
        70,
        "open-missing must return <0 and seek-to-end must report 17",
    );
}

/// Directory ops and rename over the value-returning seam.
#[test]
fn filesystem_value_returning_dirs_and_rename() {
    interpret_fs(
        "fs-vr-dirs",
        r#"
data Main {
    fs: FilesystemHost;
    console: Console;
    mode: i32;
    read_flags: i32;
    cap: usize;
    fd: i32;
    n: i64;
    rn: i64;
    rc: i32;
    buffer: [u8; 64];
}

machine Main::main(&mut self) {
    self.mode = 420;
    self.read_flags = 0;
    self.cap = 64;
    self.rc = self.fs.create_dir("/d", self.mode);
    transition self.rc == 0 { true -> made() _ -> fail() }
    state made(&mut self) {
        self.fd = self.fs.create("/a.txt", self.mode);
        self.n = self.fs.write(self.fd, "hello");
        self.n = self.fs.close(self.fd);
        self.rc = self.fs.rename("/a.txt", "/b.txt");
        transition self.rc == 0 { true -> moved() _ -> fail() }
    }
    state moved(&mut self) {
        self.fd = self.fs.open("/b.txt", self.read_flags);
        transition self.fd >= 0 { true -> rd() _ -> fail() }
    }
    state rd(&mut self) {
        self.rn = self.fs.read(self.fd, &mut self.buffer, self.cap);
        self.n = self.fs.close(self.fd);
        self.n = self.fs.remove("/b.txt");
        self.rc = self.fs.remove_dir("/d");
        transition self.rn == 5 { true -> checkdir() _ -> fail() }
    }
    state checkdir(&mut self) {
        transition self.rc == 0 { true -> ok() _ -> fail() }
    }
    state ok(&mut self) { self.console.exit_process(70); }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
        70,
        "create_dir + rename A->B (read 5) + remove_dir must all succeed",
    );
}

