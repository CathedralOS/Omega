use super::*;

#[test]
fn network_phases_launch_selected_program_with_ordinary_host_descendants_and_writes() {
    let parent = tempfile_root("host-routed");
    let discovery_root = parent.join("discovery");
    let quarantine = parent.join("quarantine");
    std::fs::create_dir_all(&discovery_root).expect("create discovery root");
    std::fs::create_dir_all(&quarantine).expect("create quarantine root");
    let discovery_root = discovery_root
        .canonicalize()
        .expect("canonical discovery root");
    let quarantine = quarantine
        .canonicalize()
        .expect("canonical quarantine root");
    let host_state = parent.join("host-selected-state");

    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let (mut discovery, observation) = backend
        .command_with_discovery_observation(Path::new("/bin/sh"), &discovery_root)
        .expect("build host-routed discovery command");
    assert_eq!(discovery.get_program(), Path::new("/bin/sh").as_os_str());
    assert!(observation.generated_policy_sha256().is_none());
    discovery.current_dir(&discovery_root);
    let status = discovery
        .args(["-c", "/usr/bin/touch \"$1\"", "resolver-test"])
        .arg(&host_state)
        .status()
        .expect("run host-selected discovery descendant");
    assert!(status.success());
    assert!(host_state.exists());

    let (fetch, fetch_observation) = backend
        .command_with_observation(
            Path::new("/bin/sh"),
            ResolverExecutionPhase::Fetch,
            Some(&quarantine),
        )
        .expect("build host-routed fetch command");
    assert_eq!(fetch.get_program(), Path::new("/bin/sh").as_os_str());
    assert!(fetch_observation.generated_policy_sha256().is_none());
    for guarantee in [
        ResolverExecutionGuarantee::FilesystemWritesConfined,
        ResolverExecutionGuarantee::ExecutablePathsConfined,
        ResolverExecutionGuarantee::DescendantProcessesContained,
    ] {
        assert_eq!(
            disposition(&fetch_observation, guarantee),
            ResolverExecutionGuaranteeDisposition::Unavailable
        );
    }
    assert_eq!(
        disposition(
            &fetch_observation,
            ResolverExecutionGuarantee::NetworkDenied
        ),
        ResolverExecutionGuaranteeDisposition::NotRequired
    );

    std::fs::remove_file(host_state).expect("remove host-state canary");
    std::fs::remove_dir(discovery_root).expect("remove discovery root");
    std::fs::remove_dir(quarantine).expect("remove quarantine root");
    std::fs::remove_dir(parent).expect("remove test parent");
}

fn tempfile_root(label: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "omega-resolver-{label}-{}-{sequence}",
        std::process::id()
    ))
}
