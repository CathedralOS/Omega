use super::*;
#[cfg(windows)]
use crate::{ResolverExecutionBackendIdentity, ResolverExecutionGuarantee};

#[test]
fn policy_observation_is_complete_canonical_and_binds_closed_phase_roots() {
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let executable = if cfg!(windows) {
        Path::new(r"C:\Windows\System32\cmd.exe")
    } else {
        Path::new("/bin/sh")
    };
    let mutable_root = inspection_root();
    let inspection_root = inspection_root();
    let inspection = backend
        .command_with_inspection_read_root_observation(executable, &inspection_root)
        .expect("issue inspection policy observation")
        .1;
    let fetch = backend
        .command_with_observation(
            executable,
            ResolverExecutionPhase::Fetch,
            Some(&mutable_root),
        )
        .expect("issue fetch policy observation")
        .1;

    assert_eq!(inspection.guarantees().len(), 12);
    assert!(
        inspection
            .guarantees()
            .windows(2)
            .all(|rows| rows[0].guarantee() < rows[1].guarantee())
    );
    assert_eq!(
        inspection.canonical_bytes(),
        backend
            .command_with_inspection_read_root_observation(executable, &inspection_root)
            .expect("reissue inspection policy observation")
            .1
            .canonical_bytes()
    );
    assert_ne!(inspection.canonical_bytes(), fetch.canonical_bytes());

    let alternate_inspection_root = inspection_root.join("alternate");
    let alternate = backend
        .command_with_inspection_read_root_observation(executable, &alternate_inspection_root)
        .expect("issue alternate inspection policy observation")
        .1;
    assert_ne!(inspection.canonical_bytes(), alternate.canonical_bytes());
    assert_eq!(inspection.executable(), executable);
    assert_eq!(
        inspection.inspection_read_root(),
        Some(inspection_root.as_path())
    );
    assert_eq!(fetch.mutable_root(), Some(mutable_root.as_path()));

    #[cfg(unix)]
    {
        assert_eq!(inspection.resource_ceilings().core_dump_bytes(), Some(0));
        assert_eq!(inspection.resource_ceilings().cpu_seconds(), Some(120));
        assert_eq!(
            inspection.resource_ceilings().single_file_bytes(),
            Some(1024 * 1024 * 1024)
        );
        assert_eq!(inspection.resource_ceilings().open_files(), Some(256));
    }
    #[cfg(windows)]
    {
        assert!(matches!(
            backend.identity(),
            ResolverExecutionBackendIdentity::WindowsJobObject
        ));
        assert_eq!(inspection.resource_ceilings().process_count(), Some(16));
        let disposition = |guarantee| {
            inspection
                .guarantees()
                .iter()
                .find(|row| row.guarantee() == guarantee)
                .expect("complete Windows guarantee row")
                .disposition()
        };
        for guarantee in [
            ResolverExecutionGuarantee::DescendantProcessesContained,
            ResolverExecutionGuarantee::CpuTimeConfined,
            ResolverExecutionGuarantee::ProcessCountConfined,
            ResolverExecutionGuarantee::AggregateResourcesConfined,
        ] {
            assert_eq!(
                disposition(guarantee),
                ResolverExecutionGuaranteeDisposition::Enforced
            );
        }
    }
}

#[test]
fn discovery_observation_binds_working_root_without_transport_or_route_fields() {
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let executable = if cfg!(windows) {
        Path::new(r"C:\Windows\System32\cmd.exe")
    } else {
        Path::new("/bin/sh")
    };
    let root = inspection_root();
    let first = backend
        .command_with_discovery_observation(executable, &root)
        .expect("issue host-routed discovery observation")
        .1;
    let alternate = backend
        .command_with_discovery_observation(executable, &root.join("alternate"))
        .expect("issue alternate discovery observation")
        .1;

    assert_eq!(first.discovery_read_root(), Some(root.as_path()));
    assert_ne!(first.canonical_bytes(), alternate.canonical_bytes());
}
