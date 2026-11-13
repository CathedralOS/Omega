//! Test policy cannot infer dangerous-service permission from retired facts.
//!
//! Retain the consent request vocabulary. Discovery explicitly refuses until
//! the lowered route publishes the required exact call and service custody.

use compiler::CheckedCompilation;
use semantic_vocabulary::PackageKeyIdentity;

/// The `Console` operations a program resolves against the selected package's
/// provider. Each flag admits the terminal-authority class
/// its methods exercise; an operation the program never resolves is not
/// permitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConsoleUse {
    /// `exit_process`.
    pub exit: bool,
    /// `write`, `write_byte` or `write_line`.
    pub output: bool,
    /// `read_byte` or `read_line`.
    pub input: bool,
}

impl ConsoleUse {
    pub const fn is_empty(self) -> bool {
        !(self.exit || self.output || self.input)
    }
}

/// The dangerous package-owned services one checked program requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RequiredDangerousServices {
    /// The selected entry demands the standard library's `FilesystemHost`
    /// boundary: its reach names it, directly or through a helper such as the
    /// `Filesystem` wrapper.
    pub filesystem: bool,
    /// The selected entry demands the standard library's `TimeHost`
    /// boundary: its reach names it, directly or through a helper such as
    /// `Time::now`.
    pub time: bool,
    /// The operations the program resolves against this package's
    /// selected `Console` provider; `None` when it selected no such provider
    /// or resolves no operation against it.
    pub console: Option<ConsoleUse>,
    /// The program resolves `exit_process` against this package's
    /// selected `ProcessExit` provider.
    pub process_exit: bool,
}

/// Refuse service discovery until the new route supplies its actual call roster.
pub fn required_dangerous_services(
    _checked: &CheckedCompilation,
    _owner: PackageKeyIdentity,
) -> Result<RequiredDangerousServices, Vec<diagnostics::Diagnostic>> {
    Err(vec![diagnostics::Diagnostic::error(
        "no product route yet: fixture dangerous-service discovery requires lowered-route call custody",
    )])
}
