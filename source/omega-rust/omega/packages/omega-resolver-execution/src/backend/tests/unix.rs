use super::*;
use crate::process::limits::{CHILD_OPEN_FILE_LIMIT, intersect_limit};

#[cfg(unix)]
#[test]
fn compiler_resource_ceilings_never_loosen_inherited_limits() {
    use rustix::process::Rlimit;

    assert_eq!(
        intersect_limit(
            Rlimit {
                current: Some(64),
                maximum: Some(1_024),
            },
            256,
        ),
        Rlimit {
            current: Some(64),
            maximum: Some(256),
        }
    );
    assert_eq!(
        intersect_limit(
            Rlimit {
                current: Some(64),
                maximum: Some(64),
            },
            256,
        ),
        Rlimit {
            current: Some(64),
            maximum: Some(64),
        }
    );
    assert_eq!(
        intersect_limit(
            Rlimit {
                current: None,
                maximum: None,
            },
            256,
        ),
        Rlimit {
            current: Some(256),
            maximum: Some(256),
        }
    );
}

#[cfg(unix)]
#[test]
fn child_open_file_limit_is_enforced() {
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    #[cfg(target_os = "macos")]
    let helper_executables = [Path::new("/bin/bash").to_path_buf()];
    #[cfg(not(target_os = "macos"))]
    let helper_executables = [];
    let inspection_root = inspection_root();
    let mut command = backend
        .command_with_inspection_read_root(
            Path::new("/bin/sh"),
            &helper_executables,
            &inspection_root,
        )
        .expect("build limited shell");
    let output = command
        .args(["-c", "ulimit -n"])
        .output()
        .expect("run limited shell");
    assert!(output.status.success());
    let limit = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u64>()
        .expect("shell reports a numeric descriptor limit");
    assert!(limit <= CHILD_OPEN_FILE_LIMIT);
}
