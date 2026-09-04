use super::*;
use crate::ResolverExecutionChild;
use std::io::Read;
use std::time::{Duration, Instant};

#[cfg(unix)]
#[test]
fn child_open_file_limit_is_enforced() {
    let inspection_root = inspection_root();
    let shell = Path::new("/bin/bash")
        .canonicalize()
        .expect("canonical shell");
    let backend = ResolverExecutionBackend::open(&shell, &[] as &[std::path::PathBuf])
        .expect("open resolver backend");
    let mut prepared = backend
        .prepare_inspection(&inspection_root)
        .expect("build limited shell");
    prepared
        .args(["-c", "ulimit -n"])
        .stdin_null()
        .stdout_piped()
        .stderr_null();
    let mut child = ResolverExecutionChild::spawn(prepared).expect("run limited shell");
    let mut stdout = String::new();
    child
        .take_stdout()
        .expect("limited shell stdout was piped")
        .read_to_string(&mut stdout)
        .expect("read descriptor limit");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if child.try_wait().expect("wait for limited shell").is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "limited shell timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
    child
        .terminate()
        .expect("close limited shell process group");
    let completion = child.finish().expect("finish limited shell");
    assert!(completion.status().success());
    let limit = stdout
        .trim()
        .parse::<u64>()
        .expect("shell reports a numeric descriptor limit");
    assert!(limit <= 256);
}
