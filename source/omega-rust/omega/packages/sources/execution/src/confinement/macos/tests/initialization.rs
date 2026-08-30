use super::*;

#[test]
fn initialization_retains_closed_writes_and_execution() {
    let parent = std::env::temp_dir().join(format!(
        "omega-resolver-initialization-{}",
        std::process::id()
    ));
    let root = parent.join("mutable");
    std::fs::create_dir_all(&root).expect("create initialization root");
    let root = root.canonicalize().expect("canonical initialization root");
    let inside = root.join("inside");
    let outside = parent.join("outside");
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");

    let mut allowed = backend
        .command(
            Path::new("/bin/bash"),
            ResolverExecutionPhase::RepositoryInitialization,
            Some(&root),
        )
        .expect("build initialization policy");
    let status = allowed
        .args(["-c", "printf allowed > \"$1\"", "resolver-test"])
        .arg(&inside)
        .status()
        .expect("run allowed initialization write");
    assert!(status.success());

    let mut denied_write = backend
        .command(
            Path::new("/bin/bash"),
            ResolverExecutionPhase::RepositoryInitialization,
            Some(&root),
        )
        .expect("build initialization policy");
    let status = denied_write
        .args(["-c", "printf denied > \"$1\"", "resolver-test"])
        .arg(&outside)
        .status()
        .expect("attempt external initialization write");
    assert!(!status.success());
    assert!(!outside.exists());

    let mut denied_descendant = backend
        .command(
            Path::new("/bin/bash"),
            ResolverExecutionPhase::RepositoryInitialization,
            Some(&root),
        )
        .expect("build initialization execution policy");
    let status = denied_descendant
        .args(["-c", "/usr/bin/true"])
        .status()
        .expect("attempt initialization descendant");
    assert!(!status.success());

    std::fs::remove_file(inside).expect("remove initialization output");
    std::fs::remove_dir(root).expect("remove initialization root");
    std::fs::remove_dir(parent).expect("remove initialization parent");
}
