use super::*;

#[test]
fn host_selected_git_keeps_phase_limits_and_observations_without_seatbelt_claims() {
    let parent =
        std::env::temp_dir().join(format!("omega-resolver-host-git-{}", std::process::id()));
    let mutable_root = parent.join("mutable");
    std::fs::create_dir_all(&mutable_root).expect("create host-Git mutable root");
    let mutable_root = mutable_root
        .canonicalize()
        .expect("canonicalize host-Git mutable root");
    let host_selected_state = parent.join("host-selected-state");
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");

    let (mut command, observation) = backend
        .prepare_host_git(
            Path::new("/bin/bash"),
            ResolverExecutionPhase::RepositoryInitialization,
            Some(&mutable_root),
        )
        .expect("prepare helper-compatible host Git execution")
        .into_parts();
    assert_eq!(command.get_program(), Path::new("/bin/bash").as_os_str());
    assert_eq!(
        observation.phase(),
        ResolverExecutionPhase::RepositoryInitialization
    );
    assert_eq!(observation.mutable_root(), Some(mutable_root.as_path()));
    assert!(observation.generated_policy_sha256().is_none());
    assert_eq!(observation.resource_ceilings().core_dump_bytes(), Some(0));
    assert_eq!(observation.resource_ceilings().cpu_seconds(), Some(120));
    assert_eq!(observation.resource_ceilings().open_files(), Some(256));
    for guarantee in [
        ResolverExecutionGuarantee::FilesystemWritesConfined,
        ResolverExecutionGuarantee::ExecutablePathsConfined,
        ResolverExecutionGuarantee::DescendantProcessesContained,
        ResolverExecutionGuarantee::NetworkDenied,
    ] {
        assert_eq!(
            disposition(&observation, guarantee),
            ResolverExecutionGuaranteeDisposition::Unavailable
        );
    }
    for guarantee in [
        ResolverExecutionGuarantee::CoreDumpsDenied,
        ResolverExecutionGuarantee::CpuTimeConfined,
        ResolverExecutionGuarantee::SingleFileSizeConfined,
        ResolverExecutionGuarantee::OpenFilesConfined,
    ] {
        assert_eq!(
            disposition(&observation, guarantee),
            ResolverExecutionGuaranteeDisposition::Enforced
        );
    }

    command.current_dir(&mutable_root);
    let status = command
        .args(["-c", "/usr/bin/touch \"$1\"", "resolver-test"])
        .arg(&host_selected_state)
        .status()
        .expect("run host-selected descendant through bounded process preparation");
    assert!(status.success());
    assert!(host_selected_state.exists());

    let inspection = backend
        .prepare_host_git_inspection(Path::new("/bin/sh"), &mutable_root)
        .expect("prepare helper-compatible host Git inspection")
        .into_parts()
        .1;
    assert_eq!(
        inspection.phase(),
        ResolverExecutionPhase::RepositoryInspection
    );
    assert_eq!(
        inspection.inspection_read_root(),
        Some(mutable_root.as_path())
    );
    assert!(inspection.generated_policy_sha256().is_none());
    assert_eq!(
        disposition(
            &inspection,
            ResolverExecutionGuarantee::ExecutablePathsConfined
        ),
        ResolverExecutionGuaranteeDisposition::Unavailable
    );

    std::fs::remove_file(host_selected_state).expect("remove host-selected state canary");
    std::fs::remove_dir(mutable_root).expect("remove mutable root");
    std::fs::remove_dir(parent).expect("remove test parent");
}
