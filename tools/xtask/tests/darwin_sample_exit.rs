//! Check the signed Darwin exit notification against the kernel's ordinary wait status.
#![cfg(target_os = "macos")]

use std::fs;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, ExitStatus};

#[test]
fn observes_one_child_including_signed_status_and_signals() {
    const CHILD_CASE: &str = "OMEGA_TEST_DARWIN_CHILD_TERMINATION";
    if let Ok(case) = std::env::var(CHILD_CASE) {
        if case == "signal" {
            // This test binary is the child the observer started.
            unsafe { libc::raise(libc::SIGTERM) };
            panic!("SIGTERM did not terminate the child");
        }
        std::process::exit(case.parse().unwrap());
    }

    let directory = std::env::temp_dir().join(format!("omega-darwin-exit-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let executable = std::env::current_exe().unwrap();
    for case in [
        "0",
        "255",
        "256",
        "-1",
        "305419896",
        "-2147483648",
        "2147483647",
        "signal",
    ] {
        let record = directory.join(case);
        let observed = Command::new(env!("CARGO_BIN_EXE_xtask"))
            .args(["sample-gate", "--observe-darwin-exit"])
            .arg(&executable)
            .arg(&record)
            .env(CHILD_CASE, case)
            .output()
            .unwrap();
        assert!(
            observed.status.success(),
            "{}",
            String::from_utf8_lossy(&observed.stderr)
        );
        let (raw, signed): (i32, Option<i32>) =
            serde_json::from_slice(&fs::read(record).unwrap()).unwrap();
        let status = ExitStatus::from_raw(raw);
        if case == "signal" {
            assert_eq!((status.signal(), signed), (Some(libc::SIGTERM), None));
        } else {
            let expected: i32 = case.parse().unwrap();
            assert_eq!(
                (status.code(), signed),
                (Some(expected & 255), Some(expected))
            );
        }
    }
    fs::remove_dir_all(directory).unwrap();
}
