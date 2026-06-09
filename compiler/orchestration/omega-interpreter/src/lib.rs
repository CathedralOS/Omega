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
//! ## Scope (first milestone)
//! Supported: a `Main` machine; self-field assignment; `let` locals; Integer / Bool /
//! Float / Binary (arith + compare + logical) / Unary / Name / Member expressions; a
//! multi-arm value/guard transition (subject and boolean forms); a value-call to another
//! state/machine returning a scalar; `&mut`-aliased argument passing; the Console
//! boundary `exit_process` and `write`/`write_line`. Anything outside this subset
//! returns [`InterpretOutcome::error`] so a differential harness can SKIP (xfail) rather
//! than report a false mismatch. The long tail (slices, arrays beyond the simplest
//! cases, enums in payloads, dyn/traits, casts across widths, recursion-heavy dungeon
//! `&mut` chains) is deferred to follow-on milestones.

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
