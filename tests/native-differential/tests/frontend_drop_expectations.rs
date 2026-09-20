//! Frontend expectations for `drop` (nominal cleanup) machines, pinned through the
//! same compile-and-interpret route the rest of this suite uses. The frontend owns
//! these fences today; each probe below states what the frontend actually does so a
//! later semantic change (e.g. real cleanup execution in the reference interpreter)
//! trips a deliberate update here rather than drifting silently.
//!
//! Findings these tests pin down:
//! - `T::drop(&mut self)` is RESERVED: it may not be called as a method, called
//!   qualified, passed as a static machine argument, or forwarded through a machine
//!   binder. Ordinary machines named `drop` (free, or non-`drop` attached) stay
//!   callable. All four authored spellings are FRONTEND-REJECTED with
//!   `reserved cleanup machine` / `does not refine` diagnostics.
//! - Drop bodies are admitted only in the "executable cleanup slice": empty, or a
//!   finite source-ordered list of ordinary zero-argument calls to mutually
//!   distinct exact-empty attached helpers. Repetition, arguments, and helpers that
//!   themselves have bodies are all FRONTEND-REJECTED.
//! - `drop` `ensures` is a PROVED exit contract like any other machine's: on an
//!   empty body only trivially provable facts (`ensures true`) are admitted;
//!   `ensures self in <Domain>` and the member state-predicate form
//!   `ensures self.<field> <state>` both reject with `cannot prove ensures
//!   contract for exit from ...::drop` when the empty body cannot establish
//!   them, and any ensures on a non-empty body is additionally rejected by the
//!   executable-slice fence.
//! - `omega::language::core::drop`'s explicit `drop(value)` consume is admitted
//!   and interprets. A later read of a scalar field out of the consumed local is
//!   still ADMITTED by the frontend today and the interpreter answers with the
//!   pre-consume value — pinned below so a future move-violation tightening is
//!   a deliberate expectation change.
//! - The reference interpreter currently runs no observable cleanup work: every
//!   admitted drop shape interprets to the same outcome as the drop-free
//!   equivalent (admitted helper calls are exact-empty), so these probes pin
//!   compile-time behavior plus ordinary interpretation only.

use checked_interpreter::InterpretOptions;
use checked_interpreter::{InterpretOutcome, interpret_entry};
use compiler::CheckedCompileRequest;
use compiler::{CheckedCompilation, compile_to_checked};
use std::fs;
use std::path::PathBuf;

fn interpret(checked: &CheckedCompilation, stdin: &[u8]) -> InterpretOutcome {
    interpret_entry(checked, "Main::main", stdin, InterpretOptions::default())
}

/// Write `source` to a fresh temp dir as `main.omg` and return the path. The dir is
/// keyed by test name + pid so parallel tests do not collide. (Same convention as
/// coverage.rs.)
fn write_program(name: &str, source: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "omega-interp-drop-exp-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp program dir");
    let main_path = dir.join("main.omg");
    fs::write(&main_path, source).expect("write probe program");
    main_path
}

fn compile(source_name: &str, source: &str) -> CheckedCompilation {
    let main_path = write_program(source_name, source);
    compile_to_checked(CheckedCompileRequest::new(&main_path, None)).unwrap_or_else(|diagnostics| {
        panic!("{source_name}: expected the frontend to accept this program: {diagnostics:?}")
    })
}

fn frontend_rejects(name: &str, source: &str) {
    let main_path = write_program(name, source);
    let result = compile_to_checked(CheckedCompileRequest::new(&main_path, None));
    assert!(
        result.is_err(),
        "{name}: expected the frontend to reject this program; it compiled"
    );
}

// ---- admitted shapes --------------------------------------------------------

/// The empty-body reserved shape is the baseline: the hook exists for the checker
/// and runs nothing observable.
#[test]
fn empty_drop_machine_compiles_and_interprets() {
    let checked = compile(
        "drop-empty-hook",
        r#"
use omega::language::core::service;
use omega::language::std::console;

data Guard {
    handle: i32;
}

machine Guard::drop(&mut self) {
}

data Main {
    console: Service<Console>;
}

machine Main::main(&mut self) reaches Console {
    let g: Guard = Guard { handle: 7 };
    self.console.exit_process(70);
}
"#,
    );
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "empty drop hook must interpret: {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

/// The executable slice's admitted non-empty body: a source-ordered list of
/// ordinary zero-argument calls to mutually distinct exact-empty attached helpers.
#[test]
fn executable_slice_helper_list_compiles_and_interprets() {
    let checked = compile(
        "drop-slice-helpers",
        r#"
use omega::language::core::service;
use omega::language::std::console;

data First {
}
machine First::touch() {
}
data Second {
}
machine Second::touch() {
}

data Guard {
    handle: i32;
}

machine Guard::drop(&mut self) {
    First::touch();
    Second::touch();
}

data Main {
    console: Service<Console>;
}

machine Main::main(&mut self) reaches Console {
    let g: Guard = Guard { handle: 7 };
    self.console.exit_process(70);
}
"#,
    );
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "slice-admitted drop helpers must interpret: {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

/// `ensures true` on an empty-body drop is trivially provable and admitted.
#[test]
fn drop_ensures_true_compiles_and_interprets() {
    let checked = compile(
        "drop-ensures-true",
        r#"
use omega::language::core::service;
use omega::language::std::console;

data Mutex {
    locked: bool;
}

machine Mutex::drop(&mut self)
ensures
    true
{
}

data Main {
    console: Service<Console>;
}

machine Main::main(&mut self) reaches Console {
    let m: Mutex = Mutex { locked: false };
    self.console.exit_process(70);
}
"#,
    );
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "trivially provable drop ensures must interpret: {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

/// `omega::language::core::drop` consumes a value explicitly: the call is admitted
/// and the owning hook (empty here) is compiler-selected on the consumed edge.
#[test]
fn core_drop_explicit_consume_interprets() {
    let checked = compile(
        "core-drop-consume",
        r#"
use omega::language::core::drop;
use omega::language::core::service;

use omega::language::core::service;
use omega::language::std::console;

data Guard {
    handle: i32;
}

machine Guard::drop(&mut self) {
}

data Main {
    console: Service<Console>;
}

machine Main::main(&mut self) reaches Console {
    let g: Guard = Guard { handle: 7 };
    drop(g);
    self.console.exit_process(70);
}
"#,
    );
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "core::drop consume must interpret: {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

/// Ordinary `drop` spellings are not reserved: a free `machine drop(...)` is
/// callable, and `T::drop_counter` is unrelated to the cleanup hook.
#[test]
fn ordinary_drop_spellings_compile_and_interpret() {
    let checked = compile(
        "ordinary-drop-spellings",
        r#"
use omega::language::core::service;
use omega::language::std::console;

data Resource {
    value: i32;
}

machine Resource::drop_counter(&mut self) {
    self.value = 0;
}

machine drop(resource: &mut Resource) {
    resource.value = 41;
}

data Main {
    console: Service<Console>;
    resource: Resource;
}

machine Main::main(&mut self) reaches Console {
    self.resource = Resource { value: 1 };
    self.resource.drop_counter();
    drop(&mut self.resource);
    self.console.exit_process(self.resource.value);
}
"#,
    );
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "ordinary drop spellings must interpret: {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 41);
}

// ---- rejected shapes ---------------------------------------------------------

/// Authored method selection of the reserved hook is rejected.
#[test]
fn reserved_drop_method_call_is_frontend_rejected() {
    frontend_rejects(
        "drop-method-call",
        r#"
data Resource {
    value: i32;
}

machine Resource::drop(&mut self) {
}

machine misuse(resource: &mut Resource) {
    resource.drop();
}

data Main {
}

machine Main::main(&mut self) {
}
"#,
    );
}

/// Authored qualified selection of the reserved hook is rejected identically.
#[test]
fn reserved_drop_qualified_call_is_frontend_rejected() {
    frontend_rejects(
        "drop-qualified-call",
        r#"
data Resource {
    value: i32;
}

machine Resource::drop(&mut self) {
}

machine misuse(resource: &mut Resource) {
    Resource::drop(resource);
}

data Main {
}

machine Main::main(&mut self) {
}
"#,
    );
}

/// The reserved hook cannot be supplied as a static machine argument.
#[test]
fn reserved_drop_as_machine_argument_is_frontend_rejected() {
    frontend_rejects(
        "drop-machine-argument",
        r#"
data Resource {
    value: i32;
}

machine Resource::drop(&mut self) {
}

machine accept<machine Selected>()
where machine Selected(value: &mut Resource)
{
}

machine misuse() {
    accept<Resource::drop>();
}

data Main {
}

machine Main::main(&mut self) {
}
"#,
    );
}

/// The reservation survives forwarding through a machine binder.
#[test]
fn reserved_drop_forwarded_machine_argument_is_frontend_rejected() {
    frontend_rejects(
        "drop-forwarded-argument",
        r#"
data Resource {
    value: i32;
}

machine Resource::drop(&mut self) {
}

machine sink<machine Selected>()
where machine Selected(value: &mut Resource)
{
}

machine forward<machine Selected>()
where machine Selected(value: &mut Resource)
{
    sink<Selected>();
}

machine misuse() {
    forward<Resource::drop>();
}

data Main {
}

machine Main::main(&mut self) {
}
"#,
    );
}

/// Arbitrary statements are outside the executable cleanup slice.
#[test]
fn drop_body_with_assignment_is_frontend_rejected() {
    frontend_rejects(
        "drop-body-assignment",
        r#"
data Lock {
    held: i32;
}

machine Lock::drop(&mut self) {
    self.held = 0;
}

data Main {
}

machine Main::main(&mut self) {
}
"#,
    );
}

/// Repeating the same helper in a drop body leaves the slice (each admitted helper
/// must be mutually distinct).
#[test]
fn drop_body_repeated_helper_is_frontend_rejected() {
    frontend_rejects(
        "drop-repeated-helper",
        r#"
data Helper {
}
machine Helper::touch() {
}

data Wrapper {
    value: i32;
}

machine Wrapper::drop(&mut self) {
    Helper::touch();
    Helper::touch();
}

data Main {
}

machine Main::main(&mut self) {
}
"#,
    );
}

/// Helper calls carrying arguments are outside the slice.
#[test]
fn drop_body_argumented_helper_is_frontend_rejected() {
    frontend_rejects(
        "drop-argumented-helper",
        r#"
data Helper {
}
machine Helper::touch(value: u8) {
}

data Wrapper {
    value: i32;
}

machine Wrapper::drop(&mut self) {
    Helper::touch(1u8);
}

data Main {
}

machine Main::main(&mut self) {
}
"#,
    );
}

/// Helpers that themselves have bodies leave the slice (helpers must be
/// exact-empty).
#[test]
fn drop_body_nonempty_helper_is_frontend_rejected() {
    frontend_rejects(
        "drop-nonempty-helper",
        r#"
data Leaf {
}
machine Leaf::finish() {
}
data First {
}
machine First::touch() {
}
data Second {
}
machine Second::touch() {
    Leaf::finish();
}

data Wrapper {
    value: i32;
}

machine Wrapper::drop(&mut self) {
    First::touch();
    Second::touch();
}

data Main {
}

machine Main::main(&mut self) {
}
"#,
    );
}

/// The member state-predicate form `ensures self.<field> <state>` is also a proved
/// exit contract — with an empty drop body nothing establishes it.
/// (The older "retained without proof" expectation lived at
/// `tests/omega/pass/drops/drop_ensures_unlocked_predicate`; it now sits in the
/// fail corpus as `tests/omega/fail/drops/drop_ensures_unlocked_predicate`.)
#[test]
fn drop_ensures_member_state_predicate_unprovable_is_frontend_rejected() {
    frontend_rejects(
        "drop-ensures-member-state",
        r#"
data Mutex {
    locked: bool;
}

data MutexGuard {
    mutex: i32;
}

machine MutexGuard::drop(&mut self)
    ensures self.mutex unlocked
{
}

data Main {
}

machine Main::main(&mut self) {
}
"#,
    );
}

/// `ensures self in <Domain>` is a proved exit contract even on `drop`: the empty
/// body cannot establish the membership, so the program is rejected with
/// `cannot prove ensures contract for exit from ...::drop`.
#[test]
fn drop_ensures_self_domain_unprovable_is_frontend_rejected() {
    frontend_rejects(
        "drop-ensures-self-domain",
        r#"
data Lock {
    held: i32;
}

domain Lock::Released
requires
    self.held == 0;

machine Lock::drop(&mut self)
ensures
    self in Lock::Released
{
}

data Main {
}

machine Main::main(&mut self) {
}
"#,
    );
}

/// An `ensures` on a non-empty body is the sharpest expectation the fence has to
/// hold: the body never runs, so it must not contribute a cleanup fact.
#[test]
fn drop_ensures_with_nonempty_body_is_frontend_rejected() {
    frontend_rejects(
        "drop-ensures-nonempty-body",
        r#"
data Lock {
    held: i32;
}

domain Lock::Released
requires
    self.held == 0;

machine Lock::drop(&mut self)
ensures
    self in Lock::Released
{
    self.held = 0;
}

data Main {
}

machine Main::main(&mut self) {
    let l: Lock = Lock { held: 0 };
}
"#,
    );
}

/// A `requires` clause on `drop` is an entry contract on the cleanup edge: the
/// empty body satisfies it trivially and the program is admitted. (The caller
/// never invokes the hook directly, so there is no call site to pre-prove the
/// requirement against — it is retained as part of the cleanup contract.)
#[test]
fn drop_requires_on_empty_body_compiles_and_interprets() {
    let checked = compile(
        "drop-requires-empty-body",
        r#"
use omega::language::core::service;
use omega::language::std::console;

data Mutex {
    locked: bool;
}

machine Mutex::drop(&mut self)
requires
    true
{
}

data Main {
    console: Service<Console>;
}

machine Main::main(&mut self) reaches Console {
    let m: Mutex = Mutex { locked: true };
    self.console.exit_process(70);
}
"#,
    );
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "requires-clause drop must interpret: {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 70);
}

/// `core::drop` moves its argument, but the frontend currently does NOT reject a
/// later read of a scalar field out of the consumed local: `g.handle` below still
/// checks, and the reference interpreter reads the pre-consume value. This pins
/// today's admitted surface, not a normative claim — if consumed-local reads ever
/// start rejecting, this probe becomes a `frontend_rejects` and the header bullet
/// gets updated.
#[test]
fn core_drop_use_after_consume_is_currently_admitted() {
    let checked = compile(
        "core-drop-use-after",
        r#"
use omega::language::core::drop;
use omega::language::core::service;

use omega::language::core::service;
use omega::language::std::console;

data Guard {
    handle: i32;
}

machine Guard::drop(&mut self) {
}

data Main {
    console: Service<Console>;
}

machine Main::main(&mut self) reaches Console {
    let g: Guard = Guard { handle: 7 };
    drop(g);
    self.console.exit_process(g.handle);
}
"#,
    );
    let outcome = interpret(&checked, b"");
    assert!(
        !outcome.is_error(),
        "use-after-drop currently interprets: {:?}",
        outcome.error
    );
    assert_eq!(outcome.exit_code, 7);
}
