//! Host wait-status presentation, separate from exact Terminal completion.
//!
//! The emitted architectural crash leaf is BRK on ARM64 and UD2 on x86-64.
//! Hosted systems present these as SIGTRAP, SIGILL, or a Windows illegal-
//! instruction status. A segfault is not a trap just because both lack an exit
//! code. This mapping belongs to the host observer, not the portable interpreter.
//! Agreement cannot identify a crash site or establish semantic trace equality;
//! Windows also cannot distinguish that exception status from an explicitly
//! supplied exit code with the same bits. Abort has no admitted mapping here.

use target::TargetProfile;
use terminal_psi::CrashCause;

/// Portable observation of one child process's termination. Signal death and
/// missing host observations must not collapse into a numeric exit fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessExitObservation {
    /// The host supplied a numeric process termination status.
    Exited(i32),
    /// Unix signal death without a numeric process exit status.
    Signaled(i32),
    /// No numeric or signal status was available.
    Terminated,
}

impl ProcessExitObservation {
    pub fn observe(status: &std::process::ExitStatus) -> Self {
        if let Some(code) = status.code() {
            return Self::Exited(code);
        }
        #[cfg(unix)]
        if let Some(signal) = std::os::unix::process::ExitStatusExt::signal(status) {
            return Self::Signaled(signal);
        }
        Self::Terminated
    }

    /// The reported code, without a numeric fallback for signal death.
    pub const fn code(self) -> Option<i32> {
        match self {
            Self::Exited(code) => Some(code),
            Self::Signaled(_) | Self::Terminated => None,
        }
    }

    /// Unix wait status retains only eight bits; Windows retains the i32 bit
    /// pattern. Never truncate the interpreter's exact semantic status itself.
    pub(super) fn matches_host_presentation(self, semantic_status: i32) -> bool {
        let expected_status = if cfg!(unix) {
            semantic_status & 255
        } else {
            semantic_status
        };
        self.code() == Some(expected_status)
    }

    /// `run` executes only the current host profile. Explicit foreign targets
    /// are publication-only, so their status must never enter this comparison.
    pub(super) fn matches_crash_presentation(self, cause: CrashCause) -> Option<bool> {
        if cause != CrashCause::Trap {
            return None;
        }
        let expected = trap_presentation(TargetProfile::host_if_supported()?)?;
        Some(self == expected)
    }

    pub fn describe(self) -> String {
        match self {
            Self::Exited(code) => format!("exit {code}"),
            Self::Signaled(signal) => format!("signal {signal}"),
            Self::Terminated => "terminated without an exit code".to_owned(),
        }
    }
}

fn trap_presentation(profile: TargetProfile) -> Option<ProcessExitObservation> {
    match profile {
        TargetProfile::MacosArm64 | TargetProfile::LinuxArm64 => {
            Some(ProcessExitObservation::Signaled(5)) // BRK -> SIGTRAP
        }
        TargetProfile::MacosX64 | TargetProfile::LinuxX64 => {
            Some(ProcessExitObservation::Signaled(4)) // UD2 -> SIGILL
        }
        TargetProfile::WindowsX64 => {
            Some(ProcessExitObservation::Exited(-1073741795)) // STATUS_ILLEGAL_INSTRUCTION
        }
        TargetProfile::UefiX64
        | TargetProfile::CrossPlatformCli
        | TargetProfile::LocalUnchecked
        | TargetProfile::AlphaBootstrap => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{CrashCause, ProcessExitObservation, TargetProfile, trap_presentation};

    #[test]
    fn typed_trap_does_not_accept_unrelated_termination() {
        let Some(expected) = TargetProfile::host_if_supported().and_then(trap_presentation) else {
            return;
        };
        assert_eq!(
            expected.matches_crash_presentation(CrashCause::Trap),
            Some(true)
        );
        for actual in [
            ProcessExitObservation::Exited(0),
            ProcessExitObservation::Exited(5),
            ProcessExitObservation::Signaled(11),
            ProcessExitObservation::Terminated,
        ] {
            assert_eq!(
                actual.matches_crash_presentation(CrashCause::Trap),
                Some(false)
            );
        }
        assert_eq!(expected.matches_crash_presentation(CrashCause::Abort), None);
    }

    #[test]
    fn numeric_completion_never_accepts_a_signal() {
        assert!(!ProcessExitObservation::Signaled(5).matches_host_presentation(5));
        assert!(!ProcessExitObservation::Terminated.matches_host_presentation(-1));
        assert!(ProcessExitObservation::Exited(70).matches_host_presentation(70));
        assert_eq!(
            ProcessExitObservation::Exited(212).matches_host_presentation(-44),
            cfg!(unix)
        );
    }
}
