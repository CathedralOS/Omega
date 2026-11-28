//! Darwin sends the signed exit code to the parent in SIGCHLD and truncates
//! the status returned by waitpid. A separate, single-child observer keeps
//! concurrent sample runs from coalescing their notifications.

use super::application::process_exit::ExitEvidence;
use crate::execution;
use std::fs::File;
use std::io::{self, Write};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Command, ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicI32, Ordering};

pub(super) const OBSERVER: &str = "--observe-darwin-exit";

pub(super) fn run(
    artifact: &Path,
    scratch: &Path,
    stdin: Stdio,
    evidence: &Path,
) -> Result<(Output, Option<ExitEvidence>), String> {
    let record = evidence.join("native-child-status.json");
    let mut output = execution::output(
        Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
            .args(["sample-gate", OBSERVER])
            .arg(artifact)
            .arg(&record)
            .current_dir(scratch)
            .stdin(stdin),
        evidence,
        "run-native",
    )?;
    if !output.status.success() {
        return Err(format!(
            "Darwin exit observer failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let (raw_wait, full): (i32, Option<i32>) =
        serde_json::from_slice(&execution::read_bounded(&record)?).map_err(|e| e.to_string())?;
    output.status = ExitStatus::from_raw(raw_wait);
    // Both a unit return and an explicit exit can produce zero. A nonzero
    // full status, including one with a zero low byte, excludes a unit return.
    let evidence = full.map(|code| {
        if code == 0 {
            ExitEvidence::SignedZero
        } else {
            ExitEvidence::Explicit(code)
        }
    });
    Ok((output, evidence))
}

static CHILD_PID: AtomicI32 = AtomicI32::new(0);
static CHILD_REASON: AtomicI32 = AtomicI32::new(0);
static CHILD_STATUS: AtomicI32 = AtomicI32::new(0);

extern "C" fn child_exited(_: libc::c_int, info: *mut libc::siginfo_t, _: *mut libc::c_void) {
    // SA_SIGINFO supplies this record for the duration of the handler.
    let info = unsafe { &*info };
    CHILD_REASON.store(info.si_code, Ordering::Relaxed);
    CHILD_STATUS.store(info.si_status, Ordering::Relaxed);
    CHILD_PID.store(info.si_pid, Ordering::Release);
}

pub(super) fn observe(artifact: &Path, record: &Path) -> io::Result<()> {
    let mut receipt = File::create_new(record)?;
    // Only this observer changes its signal disposition. Block SIGCHLD before
    // spawning so even an immediate exit is pending when sigsuspend begins.
    let (original, waiting) = unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = child_exited as *const () as libc::sighandler_t;
        action.sa_flags = libc::SA_SIGINFO | libc::SA_NOCLDSTOP;
        if libc::sigemptyset(&mut action.sa_mask) == -1
            || libc::sigaction(libc::SIGCHLD, &action, std::ptr::null_mut()) == -1
        {
            return Err(io::Error::last_os_error());
        }
        let mut blocked = 0;
        let mut original = 0;
        if libc::sigemptyset(&mut blocked) == -1
            || libc::sigaddset(&mut blocked, libc::SIGCHLD) == -1
            || libc::sigprocmask(libc::SIG_BLOCK, &blocked, &mut original) == -1
        {
            return Err(io::Error::last_os_error());
        }
        let mut waiting = original;
        if libc::sigdelset(&mut waiting, libc::SIGCHLD) == -1 {
            return Err(io::Error::last_os_error());
        }
        (original, waiting)
    };
    let mut command = Command::new(artifact);
    // Restore the caller's mask before exec. sigprocmask is async-signal-safe.
    unsafe {
        command.pre_exec(move || {
            if libc::sigprocmask(libc::SIG_SETMASK, &original, std::ptr::null_mut()) == -1 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    let mut child = command.spawn()?;
    while CHILD_PID.load(Ordering::Acquire) != child.id() as i32 {
        // Temporarily unblock SIGCHLD, then restore the mask before inspecting
        // the three fields. An unrelated signal can also wake this call.
        unsafe { libc::sigsuspend(&waiting) };
    }
    let status = child.wait()?;
    let reason = CHILD_REASON.load(Ordering::Relaxed);
    let reported = CHILD_STATUS.load(Ordering::Relaxed);
    let full = match (reason, status.code(), status.signal()) {
        (libc::CLD_EXITED, Some(low), None) if low == (reported & 255) => Some(reported),
        (libc::CLD_KILLED | libc::CLD_DUMPED, None, Some(signal)) if signal == reported => None,
        _ => {
            return Err(io::Error::other(
                "SIGCHLD and waitpid disagree about native termination",
            ));
        }
    };
    serde_json::to_writer(&mut receipt, &(status.into_raw(), full))?;
    receipt.write_all(b"\n")
}
