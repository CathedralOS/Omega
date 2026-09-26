//! Execute an emitted image, not a C replacement or a source-lowering claim.

pub(super) fn assert_exit(bytes: &[u8], value: i32) {
    #[cfg(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    ))]
    {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "omega-physical-exit-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        // Exclusive creation gives cleanup ownership of exactly one new file.
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .expect("create physical exit executable");
        let cleanup = ExecutableFile(path);
        file.write_all(bytes).expect("write complete emitted image");
        file.set_permissions(std::fs::Permissions::from_mode(0o700))
            .expect("make image executable");
        drop(file);
        // Mach-O emission includes its ad-hoc signature; do not rewrite/sign
        // the independently validated image using a host tool.
        let status = std::process::Command::new(&cleanup.0)
            .status()
            .expect("launch emitted physical exit image");
        assert_eq!(
            status.code(),
            Some(value & 255),
            "physical i32 exit {value} must return its low byte, not a signal: {status}"
        );
    }
    #[cfg(not(any(
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    let _ = (bytes, value);
}

#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(target_os = "macos", target_arch = "aarch64")
))]
struct ExecutableFile(std::path::PathBuf);

#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    all(target_os = "macos", target_arch = "aarch64")
))]
impl Drop for ExecutableFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
