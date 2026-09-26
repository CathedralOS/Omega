//! The evaluator's non-local control flow: `Halt` and the constructors every
//! evaluator method halts through (`unsupported`, `trap`, and a filesystem
//! sponsor's refusal).

/// A non-local control-flow signal. `Exit` halts cleanly with a code; the others abort
/// the run and surface as `InterpretOutcome.error` (so a harness skips rather than
/// reports a false mismatch).
pub(crate) enum Halt {
    Exit(i32),
    Unsupported(String),
    Trap(String),
    Resource(String),
}

pub(super) type EvalResult<T> = Result<T, Halt>;

pub(super) fn unsupported<T>(message: impl Into<String>) -> EvalResult<T> {
    Err(Halt::Unsupported(message.into()))
}

pub(super) fn trap<T>(message: impl Into<String>) -> EvalResult<T> {
    Err(Halt::Trap(message.into()))
}

pub(super) fn filesystem_sponsor_halt<T>(
    error: crate::checked_interpreter::FilesystemSponsorError,
) -> EvalResult<T> {
    let message = format!("filesystem staging sponsor rejected operation: {error}");
    if error.is_limit_exceeded() {
        Err(Halt::Resource(message))
    } else {
        Err(Halt::Trap(message))
    }
}
