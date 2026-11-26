//! A process re-enters this test executable as the stock namespace supervisor.
//! Both the approved program bytes and the scratch filesystem are real.
#[cfg(target_os = "linux")]
fn main() {
    use bounded_process::PrivateFilesystemFiles;
    use build_program_launch::{ReviewedProgram, RunStatus, run_worker_if_requested};
    use platform_custody::temporary_directory::TemporaryDirectory;
    use std::{process::Command, time::Duration};

    run_worker_if_requested();
    let scratch = TemporaryDirectory::create("program-launch-test", false).unwrap();
    let image = scratch.path().join("reviewed");
    let worker = std::env::current_exe().unwrap();
    build_program_launch::require_host(&worker).unwrap();
    let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/guest.c");
    assert!(
        Command::new("cc")
            .args(["-static", "-O2"])
            .arg(input)
            .arg("-o")
            .arg(&image)
            .status()
            .unwrap()
            .success()
    );
    let program = ReviewedProgram::capture(&image).unwrap();
    // The approved image remains bound even after its path is overwritten.
    std::fs::write(image, "changed").unwrap();
    assert!(
        ReviewedProgram::capture(&worker).is_err(),
        "test runner has a dynamic loader"
    );
    let run = |args: &[&[u8]], files, timeout| {
        let args = args.iter().map(|arg| arg.to_vec()).collect::<Vec<_>>();
        program.run(&worker, &args, files, timeout).unwrap()
    };
    let files = PrivateFilesystemFiles {
        initial: [(b"in".to_vec(), b"A".as_slice().into())].into(),
        outputs: vec![b"nested/out".to_vec(), b"absent".to_vec()],
    };
    let timeout = Duration::from_secs(3);
    let observed = run(&[b"probe", b"", b"two words"], files, timeout);
    assert_eq!(observed.status, RunStatus::Exited(17));
    assert_eq!(observed.outputs, [Some(b"B".to_vec()), None]);
    assert_eq!(
        run(&[b"143"], Default::default(), timeout).status,
        RunStatus::Exited(143)
    );
    assert_eq!(
        run(&[b"signal"], Default::default(), timeout).status,
        RunStatus::AbnormalExit
    );
    assert_eq!(
        run(&[b"never"], Default::default(), Duration::from_millis(150)).status,
        RunStatus::TimedOut
    );
    let marker = scratch.path().file_name().unwrap().as_encoded_bytes();
    assert_eq!(
        run(&[b"descendant", marker], Default::default(), timeout).status,
        RunStatus::Exited(19)
    );
    for process in std::fs::read_dir("/proc").unwrap().flatten() {
        if let Ok(argv) = std::fs::read(process.path().join("cmdline")) {
            assert!(
                !argv.split(|&b| b == 0).any(|arg| arg == marker),
                "a detached child survived program return"
            );
        }
    }
    for invalid in [b"a\0b".as_slice(), &[0xff]] {
        assert_eq!(
            run(&[invalid], Default::default(), timeout).status,
            RunStatus::StartFailed
        );
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {
    use build_program_launch::ReviewedProgram;
    assert!(ReviewedProgram::capture(&std::env::current_exe().unwrap()).is_err());
}
