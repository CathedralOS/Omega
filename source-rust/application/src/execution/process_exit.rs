/*

Portable host wait-status observation and comparison (`ProcessExitObservation`).

We keep numeric exit codes (`Exited`), Unix signal deaths (`Signaled`), and
unclassified terminations (`Terminated`) distinct rather than collapsing
non-exit deaths into a synthetic numeric code like `-1`:
- `matches_host_presentation` compares a numeric exit code against the host OS's
  wait-status width: Unix `waitpid` retains only the low 8 bits (`status & 255`),
  whereas Windows retains the full 32-bit `i32` pattern. A `Signaled` or
  `Terminated` process never matches a numeric completion. When Unix keeps only
  the low byte of a semantic status outside 0..=255, a matching low byte proves
  nothing about the rest of the argument, so the answer is `None` (unverified)
  rather than agreement: the process-exit spec compares the exact `i32` and
  forbids letting fewer host status bits weaken that comparison.
- `matches_crash_presentation` checks whether a native child died with the exact
  architectural trap emitted by our backend (`BRK` -> `SIGTRAP` on ARM64
  Unix, `UD2` -> `SIGILL` on x86-64 Unix, and `STATUS_ILLEGAL_INSTRUCTION`
  `-1073741795` on x86-64 Windows). Unrelated signals such as `SIGSEGV` (`11`)
  are rejected so a wild pointer crash is never mistaken for an intentional
  `CrashCause::Trap`. `CrashCause::Abort` has no admitted trap mapping here.

*/

use target::TargetProfile;
use terminal_psi::CrashCause;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessExitObservation {
    Exited(i32),
    Signaled(i32), // Unix signal death without a numeric process exit status.
    Terminated,    // Neither a numeric exit status nor a Unix signal was reported.
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

    pub const fn code(self) -> Option<i32> {
        match self {
            Self::Exited(code) => Some(code),
            Self::Signaled(_) | Self::Terminated => None,
        }
    }

    /// `Some(true)` when the host shows the whole semantic status and it
    /// matches, `Some(false)` when what the host shows differs, and `None`
    /// when the host truncated the status and the visible bits match.
    pub(super) fn matches_host_presentation(self, semantic_status: i32) -> Option<bool> {
        let presented_status = if cfg!(unix) {
            semantic_status & 255
        } else {
            semantic_status
        };
        if self.code() != Some(presented_status) {
            return Some(false);
        }
        (presented_status == semantic_status).then_some(true)
    }

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
        assert_eq!(
            ProcessExitObservation::Signaled(5).matches_host_presentation(5),
            Some(false)
        );
        assert_eq!(
            ProcessExitObservation::Terminated.matches_host_presentation(-1),
            Some(false)
        );
        assert_eq!(
            ProcessExitObservation::Exited(70).matches_host_presentation(70),
            Some(true)
        );
    }

    // Unix shows only the low byte, so Psi `exit_process(-44)` and a native
    // exit of 212 look alike there; that is unverified, never agreement.
    #[test]
    fn truncated_status_is_unverified_not_agreement() {
        let unix_low_byte = if cfg!(unix) { None } else { Some(false) };
        assert_eq!(
            ProcessExitObservation::Exited(212).matches_host_presentation(-44),
            unix_low_byte
        );
        assert_eq!(
            ProcessExitObservation::Exited(0).matches_host_presentation(256),
            unix_low_byte
        );
        assert_eq!(
            ProcessExitObservation::Exited(1).matches_host_presentation(256),
            Some(false)
        );
        assert_eq!(
            ProcessExitObservation::Exited(-44).matches_host_presentation(-44),
            if cfg!(unix) { Some(false) } else { Some(true) }
        );
    }
}
