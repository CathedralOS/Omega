//! A reference INTERPRETER for the Omega checked/typed trees, used as a DIFFERENTIAL
//! ORACLE for the native backend.
//!
//! The interpreter evaluates the program at the level of the typed/checked trees
//! (`omega_checked_trees::CheckedTrees`, which derefs to `omega_typed_trees::TypedTrees`)
//! -- the source-of-truth semantics, ABOVE all backend lowering. It is therefore
//! independent of the backend bugs it must catch: if `interpret()` and the native
//! binary disagree on exit code or stdout for the same program, the backend is wrong.
//!
//! ## Value & store model (the crux: aliasing)
//! Every storage place -- local, struct field, machine instance -- is an
//! `Rc<RefCell<Value>>` ([`value::Cell`]). A `&mut place` argument evaluates to a
//! [`Value::Ref`] holding a CLONE of the *same* `Rc`, so a write through the reference
//! mutates the original cell. Multi-level `&mut` aliasing is therefore correct by
//! construction -- this is exactly the property the native backend is known to get
//! wrong (an `&mut`-write through a call chain that does not persist). Once the
//! interpreter's coverage reaches such a program, `interpret() != native` localizes the
//! bug instantly.
//!
//! ## Execution model
//! The entry is the `Main::main` machine / `main` state (mirroring the backend's entry
//! selection). A machine instance is a [`Value::Struct`] with default-initialized
//! fields. A state has parameters + a sequence of statements + guarded transitions; the
//! first transition whose guard holds determines the next state (or the returned value /
//! terminal). Host-boundary calls (`exit_process`, `write`, `write_line`) on a
//! `boundary trait` machine drive exit code / stdout.
//!
//! ## Scope
//! Supported: multiple machines with per-instance contained sub-objects; symbol/group-based
//! machine + sibling-state resolution; self-field assignment; `let` locals; Integer / Bool /
//! Float / Binary (arith + compare + logical) / Unary / Name / Member / Indexed / Cast /
//! ArrayLiteral / StructLiteral expressions; fixed arrays and `.as_slice()`/`.as_mut_slice()`
//! slice views (a slice shares the array's element cells, preserving `&mut` aliasing);
//! width/signedness-aware `as` casts (int<->float, integer narrow/widen); multi-arm
//! value/guard transitions (subject, tuple, and boolean forms); value-calls returning a
//! scalar/struct; method calls on `&mut Data` reference params; `&mut`-aliased argument
//! passing, including MULTI-HOP forwarding (a `&mut` param passed onward as a bare name --
//! to a nested call or a transition-target state -- stays aliased, hop after hop);
//! `dyn Trait` dispatch by the receiver's RUNTIME type (works for any number of
//! impls -- AHEAD of the native backend, which only devirtualizes single-impl traits); the
//! entry machine's value as the exit code; the Console boundary `exit_process`,
//! `write`/`write_line`, and `read_line` (consuming `stdin`), including the imported std
//! `console`. The full `dungeon_crawler_cli` sample interprets end-to-end with
//! depth-correct room rendering. Anything outside this subset returns
//! [`InterpretOutcome::error`] so a differential harness SKIPS (xfail) rather than reporting
//! a false mismatch.
//!
//! Two formerly-deferred constructs turn out to be FRONTEND-REJECTED today (probed in
//! `tests/coverage.rs`), so there is nothing to interpret:
//! - General/open range expressions outside the index position (`let r: i32 = 1..5;`,
//!   `f(1..5)`) are parse errors; `ExpressionNode::Range` only ever appears under
//!   `collection[...]`, which the subslice support already covers.
//! - Enum variant payload declarations (`enum E { A(i32) }`) are parse errors (the typed
//!   `DataVariant` has no payload slot); `Value::Enum::payload` stays empty until the
//!   language grows the syntax. A paren'd construction against a payload-less variant
//!   (`E::A(5)`) parses as a CALL but resolves to nothing; the interpreter declines it.
//!
//! Still deferred: the stderr host capability (`write_error`/`write_error_line`).

mod evaluator;
mod value;

pub use value::{Cell, Value};

use omega_checked_trees::CheckedTrees;

/// The result of interpreting a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterpretOutcome {
    /// The process exit code (from `exit_process`, or 0 if the program ran to a terminal
    /// transition without exiting).
    pub exit_code: i32,
    /// Bytes written to stdout via `write` / `write_line`.
    pub stdout: Vec<u8>,
    /// `Some` when the interpreter hit an UNSUPPORTED construct (so a harness can skip),
    /// or a genuine trap. `None` on a clean run.
    pub error: Option<String>,
}

impl InterpretOutcome {
    fn exited(exit_code: i32, stdout: Vec<u8>) -> Self {
        Self {
            exit_code,
            stdout,
            error: None,
        }
    }

    fn error(message: impl Into<String>, stdout: Vec<u8>) -> Self {
        Self {
            exit_code: 0,
            stdout,
            error: Some(message.into()),
        }
    }

    /// Whether the interpreter declined to evaluate the program (unsupported construct or
    /// trap). Differential harnesses skip these rather than treat them as a mismatch.
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }
}

/// Interpret a checked program, returning its exit code and stdout. `stdin` provides the
/// bytes a `read_line` host call would consume (unused in the first milestone).
pub fn interpret(checked: &CheckedTrees, stdin: &[u8]) -> InterpretOutcome {
    evaluator::run(checked, stdin)
}
