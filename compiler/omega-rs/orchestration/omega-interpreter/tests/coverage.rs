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
    machine set_permissions(path: &[u8] in Path, mode: u32) -> i32;
    machine rename(from: &[u8] in Path, to: &[u8] in Path) -> i32;
    machine hard_link(original: &[u8] in Path, link: &[u8] in Path) -> i32;
    machine read_metadata(path: &[u8] in Path, buffer: &mut [u8]) -> i32;
    machine set_len(fd: i32, length: i64) -> i32;
    machine sync(fd: i32) -> i32;
    machine errno() -> i32;
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


/// The ERGONOMIC `Filesystem` wrapper (Rust-like): value-RETURNING machines
/// (`create(path) -> OpenResult`) hide flags/mode/fd behind `File`/result enums
/// over the raw `FilesystemHost` seam. `Main` touches only the clean API. Uses
/// the SAME human method names on both layers (create/open/read/write/close/
/// remove) — the receiver-typed call-resolution fix (D7) makes that work. Full
/// CRUD round-trip, exit 70.
#[test]
fn filesystem_ergonomic_wrapper_crud() {
    interpret_fs(
        "fs-ergonomic",
        r#"
data File [copy, zero_init] { fd: i32; }
data OpenResult { case Error; case Ok(file: File); }
data IoResult { case Error; case Ok(count: usize); }
data UnitResult { case Error; case Ok; }

data Filesystem {
    host: FilesystemHost;
    create_mode: i32;
    read_flags: i32;
}
machine Filesystem::create(&mut self, path: &[u8] in Path) -> OpenResult {
    self.create_mode = 420;
    let fd: i32 = self.host.create(path, self.create_mode);
    transition fd >= 0 { true -> ok(fd) _ -> err() }
    state ok(&mut self, fd: i32) -> OpenResult { OpenResult::Ok { file: File { fd: fd } } }
    state err(&mut self) -> OpenResult { OpenResult::Error }
}
machine Filesystem::open(&mut self, path: &[u8] in Path) -> OpenResult {
    self.read_flags = 0;
    let fd: i32 = self.host.open(path, self.read_flags);
    transition fd >= 0 { true -> ok(fd) _ -> err() }
    state ok(&mut self, fd: i32) -> OpenResult { OpenResult::Ok { file: File { fd: fd } } }
    state err(&mut self) -> OpenResult { OpenResult::Error }
}
machine Filesystem::write(&mut self, file: File, bytes: &[u8]) -> IoResult {
    let n: i64 = self.host.write(file.fd, bytes);
    transition n >= 0 { true -> ok(n) _ -> err() }
    state ok(&mut self, n: i64) -> IoResult { IoResult::Ok { count: n as usize } }
    state err(&mut self) -> IoResult { IoResult::Error }
}
machine Filesystem::read(&mut self, file: File, buffer: &mut [u8], count: usize) -> IoResult {
    let n: i64 = self.host.read(file.fd, buffer, count);
    transition n >= 0 { true -> ok(n) _ -> err() }
    state ok(&mut self, n: i64) -> IoResult { IoResult::Ok { count: n as usize } }
    state err(&mut self) -> IoResult { IoResult::Error }
}
machine Filesystem::close(&mut self, file: File) -> i32 {
    self.host.close(file.fd)
}
machine Filesystem::remove(&mut self, path: &[u8] in Path) -> UnitResult {
    let rc: i32 = self.host.remove(path);
    transition rc == 0 { true -> ok() _ -> err() }
    state ok(&mut self) -> UnitResult { UnitResult::Ok }
    state err(&mut self) -> UnitResult { UnitResult::Error }
}

data Main {
    fs: Filesystem;
    console: Console;
    open_result: OpenResult;
    io_result: IoResult;
    unit_result: UnitResult;
    close_rc: i32;
    cap: usize;
    buffer: [u8; 64];
}
machine Main::main(&mut self) {
    self.cap = 64;
    self.open_result = self.fs.create("/erg.txt");
    transition self.open_result {
        OpenResult::Ok { file } -> wrote(file)
        _ -> fail()
    }
    state wrote(&mut self, file: File) {
        self.io_result = self.fs.write(file, "ergonomic omega fs\n");
        self.close_rc = self.fs.close(file);
        self.open_result = self.fs.open("/erg.txt");
        transition self.open_result {
            OpenResult::Ok { file } -> rd(file)
            _ -> fail()
        }
    }
    state rd(&mut self, file: File) {
        self.io_result = self.fs.read(file, &mut self.buffer, self.cap);
        self.close_rc = self.fs.close(file);
        transition self.io_result {
            IoResult::Ok { count } -> verify(count)
            _ -> fail()
        }
    }
    state verify(&mut self, count: usize) {
        transition count == 19 { true -> cleanup() _ -> fail() }
    }
    state cleanup(&mut self) {
        self.unit_result = self.fs.remove("/erg.txt");
        self.console.exit_process(70);
    }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
        70,
        "ergonomic value-returning File API: create/write/close/open/read(19)/remove",
    );
}

/// The REAL shipped std module `omega/language/std/filesystem.omg`, imported via
/// `use` and driven through its ergonomic `Filesystem` API — a full CRUD
/// round-trip in the interpreter. Proves the canonical std::fs surface (not an
/// inline copy) works end-to-end.
#[test]
fn filesystem_std_module_ergonomic_crud() {
    let main_path = write_program(
        "fs-std-module",
        r#"
use omega::language::std::filesystem;
use omega::language::std::console;

data Main {
    fs: Filesystem;
    console: Console;
    open_result: OpenResult;
    io_result: IoResult;
    unit_result: UnitResult;
    close_rc: i32;
    cap: usize;
    buffer: [u8; 64];
}
machine Main::main(&mut self) {
    self.cap = 64;
    self.open_result = self.fs.create("/std.txt");
    transition self.open_result {
        OpenResult::Ok { file } -> wrote(file)
        _ -> fail()
    }
    state wrote(&mut self, file: File) {
        self.io_result = self.fs.write(file, "std module fs\n");
        self.close_rc = self.fs.close(file);
        self.open_result = self.fs.open("/std.txt");
        transition self.open_result {
            OpenResult::Ok { file } -> rd(file)
            _ -> fail()
        }
    }
    state rd(&mut self, file: File) {
        self.io_result = self.fs.read(file, &mut self.buffer, self.cap);
        self.close_rc = self.fs.close(file);
        transition self.io_result {
            IoResult::Ok { count } -> verify(count)
            _ -> fail()
        }
    }
    state verify(&mut self, count: usize) {
        transition count == 14 { true -> cleanup() _ -> fail() }
    }
    state cleanup(&mut self) {
        self.unit_result = self.fs.remove("/std.txt");
        self.console.exit_process(70);
    }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|d| panic!("std::fs module program should reach checked trees: {d:?}"));
    let outcome = interpret(&checked, b"");
    assert!(!outcome.is_error(), "std::fs module: {:?}", outcome.error);
    assert_eq!(
        outcome.exit_code, 70,
        "the shipped std::fs Filesystem API must round-trip create/write/read(14)/remove"
    );
}

/// `set_len` (Rust `File::set_len`) truncates: write 20 bytes, set_len to 5,
/// seek-to-end reports 5.
#[test]
fn filesystem_value_returning_set_len() {
    interpret_fs(
        "fs-vr-setlen",
        r#"
data Main {
    fs: FilesystemHost;
    console: Console;
    mode: i32;
    new_len: i64;
    zero: i64;
    seek_end: i32;
    fd: i32;
    n: i64;
    rc: i32;
    size: i64;
}
machine Main::main(&mut self) {
    self.mode = 420;
    self.new_len = 5;
    self.zero = 0;
    self.seek_end = 2;
    self.fd = self.fs.create("/t.txt", self.mode);
    transition self.fd >= 0 { true -> wrote() _ -> fail() }
    state wrote(&mut self) {
        self.n = self.fs.write(self.fd, "twenty-byte content!");
        self.rc = self.fs.set_len(self.fd, self.new_len);
        self.size = self.fs.seek(self.fd, self.zero, self.seek_end);
        self.n = self.fs.close(self.fd);
        self.n = self.fs.remove("/t.txt");
        transition self.size == 5 { true -> ok() _ -> fail() }
    }
    state ok(&mut self) { self.console.exit_process(70); }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
        70,
        "set_len must truncate a 20-byte file to 5 (seek-to-end == 5)",
    );
}

/// `File::metadata().len` via the std module — composed from the raw `seek` op
/// (save/measure/restore). Verifies the size (17) AND that metadata is
/// non-destructive: a following read from a freshly-opened file still gets all
/// 17 bytes (the cursor was preserved).
#[test]
fn filesystem_std_module_metadata_len() {
    let main_path = write_program(
        "fs-metadata",
        r#"
use omega::language::std::filesystem;
use omega::language::std::console;

data Main {
    fs: Filesystem;
    console: Console;
    open_result: OpenResult;
    io_result: IoResult;
    meta_result: MetadataResult;
    unit_result: UnitResult;
    close_rc: i32;
    cap: usize;
    buffer: [u8; 64];
}
machine Main::main(&mut self) {
    self.cap = 64;
    self.open_result = self.fs.create("/m.txt");
    transition self.open_result { OpenResult::Ok { file } -> wrote(file) _ -> fail() }
    state wrote(&mut self, file: File) {
        self.io_result = self.fs.write(file, "omega end to end\n");
        self.close_rc = self.fs.close(file);
        self.open_result = self.fs.open("/m.txt");
        transition self.open_result { OpenResult::Ok { file } -> meta(file) _ -> fail() }
    }
    state meta(&mut self, file: File) {
        self.meta_result = self.fs.metadata(file);
        transition self.meta_result { MetadataResult::Ok { meta } -> checklen(file, meta) _ -> fail() }
    }
    state checklen(&mut self, file: File, meta: Metadata) {
        transition meta.len == 17 { true -> rd(file) _ -> fail() }
    }
    state rd(&mut self, file: File) {
        self.io_result = self.fs.read(file, &mut self.buffer, self.cap);
        self.close_rc = self.fs.close(file);
        self.unit_result = self.fs.remove("/m.txt");
        transition self.io_result { IoResult::Ok { count } -> verify(count) _ -> fail() }
    }
    state verify(&mut self, count: usize) {
        transition count == 17 { true -> ok() _ -> fail() }
    }
    state ok(&mut self) { self.console.exit_process(70); }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|d| panic!("metadata program should reach checked trees: {d:?}"));
    let outcome = interpret(&checked, b"");
    assert!(!outcome.is_error(), "metadata: {:?}", outcome.error);
    assert_eq!(
        outcome.exit_code, 70,
        "metadata().len must be 17 and metadata must preserve the cursor (read gets 17)"
    );
}

/// `File::sync_all` via the std module: create → write → `sync` returns
/// `UnitResult::Ok` → the file's bytes survive the flush (metadata().len still
/// reports the written size). Exercises the shipped `Filesystem::sync` wrapper
/// over the raw `sync` seam.
#[test]
fn filesystem_std_module_sync() {
    let main_path = write_program(
        "fs-sync",
        r#"
use omega::language::std::filesystem;
use omega::language::std::console;

data Main {
    fs: Filesystem;
    console: Console;
    open_result: OpenResult;
    io_result: IoResult;
    unit_result: UnitResult;
    meta_result: MetadataResult;
    close_rc: i32;
}
machine Main::main(&mut self) {
    self.open_result = self.fs.create("/s.txt");
    transition self.open_result { OpenResult::Ok { file } -> wrote(file) _ -> fail() }
    state wrote(&mut self, file: File) {
        self.io_result = self.fs.write(file, "durable payload!!");
        self.unit_result = self.fs.sync(file);
        transition self.unit_result { UnitResult::Ok -> synced(file) _ -> fail() }
    }
    state synced(&mut self, file: File) {
        self.meta_result = self.fs.metadata(file);
        transition self.meta_result { MetadataResult::Ok { meta } -> checklen(file, meta) _ -> fail() }
    }
    state checklen(&mut self, file: File, meta: Metadata) {
        self.close_rc = self.fs.close(file);
        self.unit_result = self.fs.remove("/s.txt");
        transition meta.len == 17 { true -> ok() _ -> fail() }
    }
    state ok(&mut self) { self.console.exit_process(70); }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|d| panic!("sync program should reach checked trees: {d:?}"));
    let outcome = interpret(&checked, b"");
    assert!(!outcome.is_error(), "sync: {:?}", outcome.error);
    assert_eq!(
        outcome.exit_code, 70,
        "sync must return UnitResult::Ok and leave the 17 written bytes intact"
    );
}

/// `OpenOptions` via the std module (Rust `OpenOptions::open`): the wrapper
/// composes POSIX flags from the option bools. Exercises the append flag
/// (O_APPEND: writes land at end -> file grows 11 -> 14) and the truncate flag
/// (O_TRUNC: reopening empties the file -> len 0), proving the computed
/// `access | append | truncate` bits drive real IO.
#[test]
fn filesystem_std_module_open_options() {
    let main_path = write_program(
        "fs-openopts",
        r#"
use omega::language::std::filesystem;
use omega::language::std::console;

data Main {
    fs: Filesystem;
    console: Console;
    open_result: OpenResult;
    io_result: IoResult;
    meta_result: MetadataResult;
    unit_result: UnitResult;
    close_rc: i32;
    append_opts: OpenOptions;
    trunc_opts: OpenOptions;
}
machine Main::main(&mut self) {
    self.append_opts = OpenOptions { read: false, write: false, append: true, truncate: false };
    self.trunc_opts = OpenOptions { read: false, write: true, append: false, truncate: true };
    self.open_result = self.fs.create("/o.txt");
    transition self.open_result { OpenResult::Ok { file } -> wrote(file) _ -> fail() }
    state wrote(&mut self, file: File) {
        self.io_result = self.fs.write(file, "hello world");
        self.close_rc = self.fs.close(file);
        self.open_result = self.fs.open_with("/o.txt", self.append_opts);
        transition self.open_result { OpenResult::Ok { file } -> appended(file) _ -> fail() }
    }
    state appended(&mut self, file: File) {
        self.io_result = self.fs.write(file, "!!!");
        self.close_rc = self.fs.close(file);
        self.open_result = self.fs.open("/o.txt");
        transition self.open_result { OpenResult::Ok { file } -> checkgrown(file) _ -> fail() }
    }
    state checkgrown(&mut self, file: File) {
        self.meta_result = self.fs.metadata(file);
        self.close_rc = self.fs.close(file);
        transition self.meta_result { MetadataResult::Ok { meta } -> grownlen(meta) _ -> fail() }
    }
    state grownlen(&mut self, meta: Metadata) {
        transition meta.len == 14 { true -> dotrunc() _ -> fail() }
    }
    state dotrunc(&mut self) {
        self.open_result = self.fs.open_with("/o.txt", self.trunc_opts);
        transition self.open_result { OpenResult::Ok { file } -> truncated(file) _ -> fail() }
    }
    state truncated(&mut self, file: File) {
        self.close_rc = self.fs.close(file);
        self.open_result = self.fs.open("/o.txt");
        transition self.open_result { OpenResult::Ok { file } -> checkempty(file) _ -> fail() }
    }
    state checkempty(&mut self, file: File) {
        self.meta_result = self.fs.metadata(file);
        self.close_rc = self.fs.close(file);
        self.unit_result = self.fs.remove("/o.txt");
        transition self.meta_result { MetadataResult::Ok { meta } -> emptylen(meta) _ -> fail() }
    }
    state emptylen(&mut self, meta: Metadata) {
        transition meta.len == 0 { true -> ok() _ -> fail() }
    }
    state ok(&mut self) { self.console.exit_process(70); }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|d| panic!("open_options program should reach checked trees: {d:?}"));
    let outcome = interpret(&checked, b"");
    assert!(!outcome.is_error(), "open_options: {:?}", outcome.error);
    assert_eq!(
        outcome.exit_code, 70,
        "open_with(append) must grow 11->14 and open_with(truncate) must empty to 0"
    );
}

/// One-shot whole-file helpers via the std module: `write_all` (Rust
/// `fs::write`) then `read_all` (Rust `fs::read`) round-trip the same 15 bytes
/// with no `File` handle leaking to the caller; the first byte reads back 'o'
/// (111). Also asserts the failure path: `read_all` on a missing path returns
/// `IoResult::Error` (the entry's open fails and the `n >= 0` guard reports it).
#[test]
fn filesystem_std_module_whole_file_helpers() {
    let main_path = write_program(
        "fs-wholefile",
        r#"
use omega::language::std::filesystem;
use omega::language::std::console;

data Main {
    fs: Filesystem;
    console: Console;
    unit_result: UnitResult;
    io_result: IoResult;
    cap: usize;
    first: u8;
    buffer: [u8; 64];
}
machine Main::main(&mut self) {
    self.cap = 64;
    self.unit_result = self.fs.write_all("/w.txt", "one-shot bytes!");
    transition self.unit_result { UnitResult::Ok -> readback() _ -> fail() }
    state readback(&mut self) {
        self.io_result = self.fs.read_all("/w.txt", &mut self.buffer, self.cap);
        transition self.io_result { IoResult::Ok { count } -> checkcount(count) _ -> fail() }
    }
    state checkcount(&mut self, count: usize) {
        transition count == 15 { true -> checkbyte() _ -> fail() }
    }
    state checkbyte(&mut self) {
        self.first = self.buffer[0];
        transition self.first == 111 { true -> missing() _ -> fail() }
    }
    state missing(&mut self) {
        // read_all on a path that does not exist must be Error, not Ok.
        self.unit_result = self.fs.remove("/w.txt");
        self.io_result = self.fs.read_all("/gone.txt", &mut self.buffer, self.cap);
        transition self.io_result { IoResult::Error { kind } -> ok() _ -> fail() }
    }
    state ok(&mut self) { self.console.exit_process(70); }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|d| panic!("whole-file program should reach checked trees: {d:?}"));
    let outcome = interpret(&checked, b"");
    assert!(!outcome.is_error(), "whole_file: {:?}", outcome.error);
    assert_eq!(
        outcome.exit_code, 70,
        "write_all/read_all must round-trip 15 bytes and read_all(missing) must be Error"
    );
}

/// Raw `errno` seam: opening a missing path fails and `errno()` reports ENOENT
/// (2); creating a directory twice reports EEXIST (17). Exercises the
/// value-returning-with-deref op on the interpreter side (mirrors the native
/// `___error()` deref).
#[test]
fn filesystem_value_returning_errno() {
    interpret_fs(
        "fs-vr-errno",
        r#"
data Main {
    fs: FilesystemHost;
    console: Console;
    rdonly: i32;
    mode: i32;
    fd: i32;
    rc: i32;
    code: i32;
}
machine Main::main(&mut self) {
    self.rdonly = 0;
    self.mode = 493;
    self.fd = self.fs.open("/nope.txt", self.rdonly);
    transition self.fd < 0 { true -> checkenoent() _ -> fail() }
    state checkenoent(&mut self) {
        self.code = self.fs.errno();
        transition self.code == 2 { true -> makedir() _ -> fail() }
    }
    state makedir(&mut self) {
        self.rc = self.fs.create_dir("/d", self.mode);
        self.rc = self.fs.create_dir("/d", self.mode);
        transition self.rc < 0 { true -> checkeexist() _ -> fail() }
    }
    state checkeexist(&mut self) {
        self.code = self.fs.errno();
        transition self.code == 17 { true -> ok() _ -> fail() }
    }
    state ok(&mut self) { self.console.exit_process(70); }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
        70,
        "errno must report ENOENT(2) for a missing open and EEXIST(17) for a duplicate mkdir",
    );
}

/// The typed error model via the std module (Rust `io::Error::kind`): failures
/// self-describe — the `ErrorKind` is embedded in the `Error` case, classified
/// at the point of failure. Proves the kind VARIES per cause: `open`(missing) ->
/// `NotFound` (ENOENT), and `create_dir` on an existing dir -> `AlreadyExists`
/// (EEXIST). If the kind were hard-wired, the second check would fail.
#[test]
fn filesystem_std_module_error_kind() {
    let main_path = write_program(
        "fs-errorkind",
        r#"
use omega::language::std::filesystem;
use omega::language::std::console;

data Main {
    fs: Filesystem;
    console: Console;
    open_result: OpenResult;
    unit_result: UnitResult;
}
machine Main::main(&mut self) {
    self.open_result = self.fs.open("/absent.txt");
    // The failure self-describes: the kind is embedded in the Error case.
    transition self.open_result { OpenResult::Error { kind } -> not_found(kind) _ -> fail() }
    state not_found(&mut self, kind: ErrorKind) {
        transition kind { ErrorKind::NotFound -> make_dir() _ -> fail() }
    }
    state make_dir(&mut self) {
        self.unit_result = self.fs.create_dir("/d");
        transition self.unit_result { UnitResult::Ok -> make_dir_again() _ -> fail() }
    }
    state make_dir_again(&mut self) {
        self.unit_result = self.fs.create_dir("/d");
        transition self.unit_result { UnitResult::Error { kind } -> already_exists(kind) _ -> fail() }
    }
    state already_exists(&mut self, kind: ErrorKind) {
        self.unit_result = self.fs.remove_dir("/d");
        transition kind { ErrorKind::AlreadyExists -> ok() _ -> fail() }
    }
    state ok(&mut self) { self.console.exit_process(70); }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|d| panic!("error-kind program should reach checked trees: {d:?}"));
    let outcome = interpret(&checked, b"");
    assert!(!outcome.is_error(), "error_kind: {:?}", outcome.error);
    assert_eq!(
        outcome.exit_code, 70,
        "open(missing) -> NotFound and create_dir(existing) -> AlreadyExists (kind varies per cause)"
    );
}

/// Path-query helpers via the std module: `exists` (Rust `Path::exists`) is
/// false before a write and true after; `metadata_path` (Rust `fs::metadata`)
/// reports the byte length of a path without a `File` handle. Then `remove`
/// makes `exists` false again.
#[test]
fn filesystem_std_module_path_queries() {
    let main_path = write_program(
        "fs-pathquery",
        r#"
use omega::language::std::filesystem;
use omega::language::std::console;

data Main {
    fs: Filesystem;
    console: Console;
    unit_result: UnitResult;
    meta_result: MetadataResult;
    present: bool;
}
machine Main::main(&mut self) {
    self.present = self.fs.exists("/q.txt");
    transition self.present { true -> fail() _ -> makeit() }
    state makeit(&mut self) {
        self.unit_result = self.fs.write_all("/q.txt", "twelve bytes");
        transition self.unit_result { UnitResult::Ok -> checkpresent() _ -> fail() }
    }
    state checkpresent(&mut self) {
        self.present = self.fs.exists("/q.txt");
        transition self.present { true -> checklen() _ -> fail() }
    }
    state checklen(&mut self) {
        self.meta_result = self.fs.metadata_path("/q.txt");
        transition self.meta_result { MetadataResult::Ok { meta } -> verifylen(meta) _ -> fail() }
    }
    state verifylen(&mut self, meta: Metadata) {
        transition meta.len == 12 { true -> removeit() _ -> fail() }
    }
    state removeit(&mut self) {
        self.unit_result = self.fs.remove("/q.txt");
        self.present = self.fs.exists("/q.txt");
        transition self.present { true -> fail() _ -> ok() }
    }
    state ok(&mut self) { self.console.exit_process(70); }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|d| panic!("path-query program should reach checked trees: {d:?}"));
    let outcome = interpret(&checked, b"");
    assert!(!outcome.is_error(), "path_queries: {:?}", outcome.error);
    assert_eq!(
        outcome.exit_code, 70,
        "exists: false->true->false around write/remove; metadata_path.len == 12"
    );
}

/// `copy` (Rust `std::fs::copy`) via the std module: write 13 bytes to a source,
/// copy it to a destination, then read the destination back and confirm both the
/// byte COUNT (13, not the buffer capacity 64) and the first byte match. Also
/// exercises the new `eval_fs_bytes` byte-array path (the copy writes a buffer,
/// not a string literal).
#[test]
fn filesystem_std_module_copy() {
    let main_path = write_program(
        "fs-copy",
        r#"
use omega::language::std::filesystem;
use omega::language::std::console;

data Main {
    fs: Filesystem;
    console: Console;
    unit_result: UnitResult;
    io_result: IoResult;
    cap: usize;
    first: u8;
    buffer: [u8; 64];
    verify: [u8; 64];
}
machine Main::main(&mut self) {
    self.cap = 64;
    self.unit_result = self.fs.write_all("/src.txt", "copy me please");
    transition self.unit_result { UnitResult::Ok -> docopy() _ -> fail() }
    state docopy(&mut self) {
        self.io_result = self.fs.copy("/src.txt", "/dst.txt", &mut self.buffer, self.cap);
        transition self.io_result { IoResult::Ok { count } -> checkcount(count) _ -> fail() }
    }
    state checkcount(&mut self, count: usize) {
        // 14 bytes ("copy me please"), NOT the 64-byte buffer capacity.
        transition count == 14 { true -> readback() _ -> fail() }
    }
    state readback(&mut self) {
        self.io_result = self.fs.read_all("/dst.txt", &mut self.verify, self.cap);
        transition self.io_result { IoResult::Ok { count } -> checklen(count) _ -> fail() }
    }
    state checklen(&mut self, count: usize) {
        transition count == 14 { true -> checkbyte() _ -> fail() }
    }
    state checkbyte(&mut self) {
        self.first = self.verify[0];
        self.unit_result = self.fs.remove("/src.txt");
        self.unit_result = self.fs.remove("/dst.txt");
        transition self.first == 99 { true -> ok() _ -> fail() }
    }
    state ok(&mut self) { self.console.exit_process(70); }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|d| panic!("copy program should reach checked trees: {d:?}"));
    let outcome = interpret(&checked, b"");
    assert!(!outcome.is_error(), "copy: {:?}", outcome.error);
    assert_eq!(
        outcome.exit_code, 70,
        "copy must transfer exactly 14 bytes (truncated to n, not buffer cap) and match content"
    );
}

/// Opening a directory for writing is classified as `ErrorKind::IsADirectory`
/// (Rust `io::ErrorKind::IsADirectory`, EISDIR). Create a dir, then `open_with`
/// write on that path -> Error whose embedded kind is IsADirectory.
#[test]
fn filesystem_std_module_is_a_directory() {
    let main_path = write_program(
        "fs-isdir",
        r#"
use omega::language::std::filesystem;
use omega::language::std::console;

data Main {
    fs: Filesystem;
    console: Console;
    unit_result: UnitResult;
    open_result: OpenResult;
    write_opts: OpenOptions;
}
machine Main::main(&mut self) {
    self.write_opts = OpenOptions { read: false, write: true, append: false, truncate: false };
    self.unit_result = self.fs.create_dir("/d");
    transition self.unit_result { UnitResult::Ok -> openit() _ -> fail() }
    state openit(&mut self) {
        self.open_result = self.fs.open_with("/d", self.write_opts);
        transition self.open_result { OpenResult::Error { kind } -> classify(kind) _ -> fail() }
    }
    state classify(&mut self, kind: ErrorKind) {
        self.unit_result = self.fs.remove_dir("/d");
        transition kind { ErrorKind::IsADirectory -> ok() _ -> fail() }
    }
    state ok(&mut self) { self.console.exit_process(70); }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|d| panic!("is-a-directory program should reach checked trees: {d:?}"));
    let outcome = interpret(&checked, b"");
    assert!(!outcome.is_error(), "is_a_directory: {:?}", outcome.error);
    assert_eq!(
        outcome.exit_code, 70,
        "open_with(write) on a directory must be Error with kind IsADirectory"
    );
}

/// `set_permissions` (Rust `std::fs::set_permissions`): after chmod'ing a file to
/// read-only (0o444 = 292), a write-open fails and its embedded kind is
/// `PermissionDenied` (EACCES). Proves both the chmod op and the permission
/// enforcement it drives.
#[test]
fn filesystem_std_module_set_permissions() {
    let main_path = write_program(
        "fs-perms",
        r#"
use omega::language::std::filesystem;
use omega::language::std::console;

data Main {
    fs: Filesystem;
    console: Console;
    unit_result: UnitResult;
    open_result: OpenResult;
    read_only: Permissions;
    write_opts: OpenOptions;
}
machine Main::main(&mut self) {
    self.read_only = Permissions { mode: 292 };
    self.write_opts = OpenOptions { read: false, write: true, append: false, truncate: false };
    self.unit_result = self.fs.write_all("/p.txt", "content");
    transition self.unit_result { UnitResult::Ok -> lockit() _ -> fail() }
    state lockit(&mut self) {
        self.unit_result = self.fs.set_permissions("/p.txt", self.read_only);
        transition self.unit_result { UnitResult::Ok -> trywrite() _ -> fail() }
    }
    state trywrite(&mut self) {
        self.open_result = self.fs.open_with("/p.txt", self.write_opts);
        transition self.open_result { OpenResult::Error { kind } -> classify(kind) _ -> fail() }
    }
    state classify(&mut self, kind: ErrorKind) {
        self.unit_result = self.fs.remove("/p.txt");
        transition kind { ErrorKind::PermissionDenied -> ok() _ -> fail() }
    }
    state ok(&mut self) { self.console.exit_process(70); }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|d| panic!("set-permissions program should reach checked trees: {d:?}"));
    let outcome = interpret(&checked, b"");
    assert!(!outcome.is_error(), "set_permissions: {:?}", outcome.error);
    assert_eq!(
        outcome.exit_code, 70,
        "chmod read-only then write-open must be Error with kind PermissionDenied"
    );
}

/// `hard_link` (Rust `std::fs::hard_link`): after linking, the new name reads
/// back the original's bytes, AND the link survives removal of the original.
/// Also asserts the failure path: linking onto an existing name is
/// `AlreadyExists`.
#[test]
fn filesystem_std_module_hard_link() {
    let main_path = write_program(
        "fs-hardlink",
        r#"
use omega::language::std::filesystem;
use omega::language::std::console;

data Main {
    fs: Filesystem;
    console: Console;
    unit_result: UnitResult;
    io_result: IoResult;
    cap: usize;
    first: u8;
    buffer: [u8; 32];
}
machine Main::main(&mut self) {
    self.cap = 32;
    self.unit_result = self.fs.write_all("/orig.txt", "linked bytes");
    transition self.unit_result { UnitResult::Ok -> linkit() _ -> fail() }
    state linkit(&mut self) {
        self.unit_result = self.fs.hard_link("/orig.txt", "/alias.txt");
        transition self.unit_result { UnitResult::Ok -> dupfails() _ -> fail() }
    }
    state dupfails(&mut self) {
        // linking onto an existing name is AlreadyExists
        self.unit_result = self.fs.hard_link("/orig.txt", "/alias.txt");
        transition self.unit_result { UnitResult::Error { kind } -> checkdup(kind) _ -> fail() }
    }
    state checkdup(&mut self, kind: ErrorKind) {
        transition kind { ErrorKind::AlreadyExists -> dropsrc() _ -> fail() }
    }
    state dropsrc(&mut self) {
        // remove the original; the link must still read back the content
        self.unit_result = self.fs.remove("/orig.txt");
        self.io_result = self.fs.read_all("/alias.txt", &mut self.buffer, self.cap);
        transition self.io_result { IoResult::Ok { count } -> checklen(count) _ -> fail() }
    }
    state checklen(&mut self, count: usize) {
        transition count == 12 { true -> checkbyte() _ -> fail() }
    }
    state checkbyte(&mut self) {
        self.first = self.buffer[0];
        self.unit_result = self.fs.remove("/alias.txt");
        transition self.first == 108 { true -> ok() _ -> fail() }
    }
    state ok(&mut self) { self.console.exit_process(70); }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|d| panic!("hard-link program should reach checked trees: {d:?}"));
    let outcome = interpret(&checked, b"");
    assert!(!outcome.is_error(), "hard_link: {:?}", outcome.error);
    assert_eq!(
        outcome.exit_code, 70,
        "hard_link: alias reads 12 bytes after original removed; relink is AlreadyExists"
    );
}

/// `metadata_path` now decodes `st_mode` (Rust `Metadata::is_dir`/`is_file`) via
/// `stat`: a regular file reports `is_dir == false` with the right `len`, and a
/// directory reports `is_dir == true`. Proves the byte-assembly extraction of
/// both `st_size` (off 96) and `st_mode` (off 4) from the stat record.
#[test]
fn filesystem_std_module_metadata_is_dir() {
    let main_path = write_program(
        "fs-isdir-meta",
        r#"
use omega::language::std::filesystem;
use omega::language::std::console;

data Main {
    fs: Filesystem;
    console: Console;
    unit_result: UnitResult;
    meta_result: MetadataResult;
}
machine Main::main(&mut self) {
    self.unit_result = self.fs.write_all("/f.txt", "abc");
    transition self.unit_result { UnitResult::Ok -> statfile() _ -> fail() }
    state statfile(&mut self) {
        self.meta_result = self.fs.metadata_path("/f.txt");
        transition self.meta_result { MetadataResult::Ok { meta } -> checkfile(meta) _ -> fail() }
    }
    state checkfile(&mut self, meta: Metadata) {
        // a regular file: len 3, not a directory
        transition meta.len == 3 { true -> checkfilekind(meta) _ -> fail() }
    }
    state checkfilekind(&mut self, meta: Metadata) {
        transition meta.is_dir { true -> fail() _ -> makedir() }
    }
    state makedir(&mut self) {
        self.unit_result = self.fs.create_dir("/dd");
        transition self.unit_result { UnitResult::Ok -> statdir() _ -> fail() }
    }
    state statdir(&mut self) {
        self.meta_result = self.fs.metadata_path("/dd");
        transition self.meta_result { MetadataResult::Ok { meta } -> checkdir(meta) _ -> fail() }
    }
    state checkdir(&mut self, meta: Metadata) {
        self.unit_result = self.fs.remove_dir("/dd");
        self.unit_result = self.fs.remove("/f.txt");
        transition meta.is_dir { true -> ok() _ -> fail() }
    }
    state ok(&mut self) { self.console.exit_process(70); }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|d| panic!("metadata-is-dir program should reach checked trees: {d:?}"));
    let outcome = interpret(&checked, b"");
    assert!(!outcome.is_error(), "metadata_is_dir: {:?}", outcome.error);
    assert_eq!(
        outcome.exit_code, 70,
        "metadata_path: file -> is_dir false (len 3); directory -> is_dir true"
    );
}

/// `Metadata::is_file`/`readonly`/`permissions` (Rust `Metadata::is_file` +
/// `Metadata::permissions().readonly()`), decoded from `st_mode`: a fresh file is
/// a writable regular file (is_file, !readonly); after chmod to 0o444 the same
/// path reports readonly and `permissions().mode == 292` (0o444).
#[test]
fn filesystem_std_module_metadata_permissions() {
    let main_path = write_program(
        "fs-meta-perms",
        r#"
use omega::language::std::filesystem;
use omega::language::std::console;

data Main {
    fs: Filesystem;
    console: Console;
    unit_result: UnitResult;
    meta_result: MetadataResult;
    read_only: Permissions;
    perms: Permissions;
}
machine Main::main(&mut self) {
    self.read_only = Permissions { mode: 292 };
    self.unit_result = self.fs.write_all("/rw.txt", "data");
    transition self.unit_result { UnitResult::Ok -> statfresh() _ -> fail() }
    state statfresh(&mut self) {
        self.meta_result = self.fs.metadata_path("/rw.txt");
        transition self.meta_result { MetadataResult::Ok { meta } -> checkfresh(meta) _ -> fail() }
    }
    state checkfresh(&mut self, meta: Metadata) {
        // a fresh regular file: is_file, and NOT read-only
        transition meta.is_file() { true -> checkwritable(meta) _ -> fail() }
    }
    state checkwritable(&mut self, meta: Metadata) {
        transition meta.readonly() { true -> fail() _ -> lockit() }
    }
    state lockit(&mut self) {
        self.unit_result = self.fs.set_permissions("/rw.txt", self.read_only);
        transition self.unit_result { UnitResult::Ok -> statlocked() _ -> fail() }
    }
    state statlocked(&mut self) {
        self.meta_result = self.fs.metadata_path("/rw.txt");
        transition self.meta_result { MetadataResult::Ok { meta } -> checklocked(meta) _ -> fail() }
    }
    state checklocked(&mut self, meta: Metadata) {
        self.perms = meta.permissions();
        transition meta.readonly() { true -> checkperm() _ -> fail() }
    }
    state checkperm(&mut self) {
        self.unit_result = self.fs.remove("/rw.txt");
        transition self.perms.mode == 292 { true -> ok() _ -> fail() }
    }
    state ok(&mut self) { self.console.exit_process(70); }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|d| panic!("metadata-permissions program should reach checked trees: {d:?}"));
    let outcome = interpret(&checked, b"");
    assert!(!outcome.is_error(), "metadata_permissions: {:?}", outcome.error);
    assert_eq!(
        outcome.exit_code, 70,
        "fresh file: is_file & writable; after chmod 0o444: readonly & permissions().mode == 292"
    );
}

/// `Metadata::modified` (Rust `Metadata::modified()`), decoded from the stat
/// record's `st_mtimespec.tv_sec` (i64 @48). The hermetic FS reports a fixed
/// modeled epoch (1_000_000_000), so this asserts that exact value; native
/// `stat` returns the real time (see the `native_stat` canary family).
#[test]
fn filesystem_std_module_metadata_modified() {
    let main_path = write_program(
        "fs-meta-mtime",
        r#"
use omega::language::std::filesystem;
use omega::language::std::console;

data Main {
    fs: Filesystem;
    console: Console;
    unit_result: UnitResult;
    meta_result: MetadataResult;
}
machine Main::main(&mut self) {
    self.unit_result = self.fs.write_all("/t.txt", "when");
    transition self.unit_result { UnitResult::Ok -> statit() _ -> fail() }
    state statit(&mut self) {
        self.meta_result = self.fs.metadata_path("/t.txt");
        transition self.meta_result { MetadataResult::Ok { meta } -> checkmtime(meta) _ -> fail() }
    }
    state checkmtime(&mut self, meta: Metadata) {
        self.unit_result = self.fs.remove("/t.txt");
        transition meta.modified() == 1000000000 { true -> ok() _ -> fail() }
    }
    state ok(&mut self) { self.console.exit_process(70); }
    state fail(&mut self) { self.console.exit_process(71); }
}
"#,
    );
    let checked = compile_to_checked(&main_path, None)
        .unwrap_or_else(|d| panic!("metadata-modified program should reach checked trees: {d:?}"));
    let outcome = interpret(&checked, b"");
    assert!(!outcome.is_error(), "metadata_modified: {:?}", outcome.error);
    assert_eq!(
        outcome.exit_code, 70,
        "metadata_path.modified() must decode the modeled st_mtime (1_000_000_000)"
    );
}
