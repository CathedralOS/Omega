//! The evaluator's non-local control flow: `Halt` and the constructors every
//! evaluator method halts through (`unsupported`, `trap`, and a filesystem
//! sponsor's refusal).

/// Non-local control flow. An authored crash retains its checked cause and
/// names its site (the machine, state and transition span of the crash edge),
/// so a trapping harness or fixture says where it crashed; generic evaluator
/// traps and unsupported operations are not source crashes. All abnormal
/// outcomes still reject value-returning build-time evaluation.
pub(crate) enum Halt {
    Exit(i32),
    Crash(typed_trees::signature::CrashCause, String),
    Unsupported(String),
    Trap(String),
    Resource(String),
    StaticInputUnavailable(crate::interpreter::UnavailableStaticInput),
}

pub(super) type EvalResult<T> = Result<T, Halt>;

pub(super) fn unsupported<T>(message: impl Into<String>) -> EvalResult<T> {
    Err(Halt::Unsupported(message.into()))
}

pub(super) fn trap<T>(message: impl Into<String>) -> EvalResult<T> {
    Err(Halt::Trap(message.into()))
}

pub(super) fn filesystem_sponsor_halt<T>(error: crate::FilesystemSponsorError) -> EvalResult<T> {
    let message = format!("filesystem staging sponsor rejected operation: {error}");
    if error.is_limit_exceeded() {
        Err(Halt::Resource(message))
    } else {
        Err(Halt::Trap(message))
    }
}
