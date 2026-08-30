use super::*;

#[test]
fn inspection_retains_closed_writes_and_ambient_reads() {
    let parent =
        std::env::temp_dir().join(format!("omega-resolver-inspection-{}", std::process::id()));
    let repository = parent.join("repository");
    std::fs::create_dir_all(&repository).expect("create inspection repository");
    let repository = repository.canonicalize().expect("canonical repository");
    let ambient = parent.join("ambient-config");
    let marker = repository.join("marker");
    std::fs::write(&ambient, b"ambient").expect("write ambient canary");
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");

    let mut read = backend
        .command_with_inspection_read_root(Path::new("/bin/cat"), &repository)
        .expect("build inspection read policy");
    let output = read.arg(&ambient).output().expect("read ambient content");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"ambient");

    let mut denied = backend
        .command_with_inspection_read_root(Path::new("/bin/bash"), &repository)
        .expect("build inspection write policy");
    let status = denied
        .args(["-c", "printf denied > \"$1\"", "resolver-test"])
        .arg(&marker)
        .status()
        .expect("attempt inspection write");
    assert!(!status.success());
    assert!(!marker.exists());

    std::fs::remove_file(ambient).expect("remove ambient canary");
    std::fs::remove_dir(repository).expect("remove repository");
    std::fs::remove_dir(parent).expect("remove inspection parent");
}
