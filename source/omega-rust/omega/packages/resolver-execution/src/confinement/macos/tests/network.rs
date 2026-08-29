use super::*;

#[cfg(target_os = "macos")]
#[test]
fn seatbelt_denies_nonnetwork_tcp_and_confines_network_phases_to_the_broker() {
    use std::io::ErrorKind;
    use std::net::TcpListener;

    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind loopback canary");
    listener
        .set_nonblocking(true)
        .expect("make canary listener nonblocking");
    let port = listener.local_addr().expect("read canary address").port();
    let backend = ResolverExecutionBackend::open().expect("open resolver backend");
    let inspection_root = inspection_root();
    let mut denied = backend
        .command_with_inspection_read_root(Path::new("/usr/bin/nc"), &[], &inspection_root)
        .expect("build network-denied sandbox");
    let status = denied
        .args(["127.0.0.1", &port.to_string()])
        .stdin(Stdio::null())
        .status()
        .expect("attempt denied loopback connection");
    assert!(!status.success());
    assert!(matches!(listener.accept(), Err(error) if error.kind() == ErrorKind::WouldBlock));

    let mutable_root = std::env::temp_dir().join(format!(
        "omega-resolver-network-denial-{}-{port}",
        std::process::id(),
    ));
    std::fs::create_dir(&mutable_root).expect("create network-denial mutable root");
    let mutable_root = mutable_root
        .canonicalize()
        .expect("canonicalize network-denial mutable root");
    let mut denied = backend
        .command(
            Path::new("/usr/bin/nc"),
            &[],
            ResolverExecutionPhase::RepositoryInitialization,
            Some(&mutable_root),
        )
        .expect("build initialization network-denied sandbox");
    denied.current_dir(&mutable_root);
    let status = denied
        .args(["127.0.0.1", &port.to_string()])
        .stdin(Stdio::null())
        .status()
        .expect("attempt denied initialization loopback connection");
    assert!(!status.success());
    assert!(matches!(listener.accept(), Err(error) if error.kind() == ErrorKind::WouldBlock));
    std::fs::remove_dir(mutable_root).expect("remove network-denial mutable root");

    let route = backend
        .open_endpoint_route(
            ResolverExecutionRequestedEndpoint::new("127.0.0.1", port)
                .expect("construct broker destination"),
            ResolverExecutionTransferBudget::new(1024 * 1024).expect("construct transfer budget"),
        )
        .expect("open endpoint route");
    let broker_port = route.policy().broker_endpoint().port();
    let discovery_read_root = inspection_root.clone();
    let (mut allowed, _) = backend
        .command_with_discovery_route_observation(
            Path::new("/usr/bin/nc"),
            &[],
            ResolverExecutionNetworkTransport::Https,
            &route,
            &discovery_read_root,
        )
        .expect("build network-enabled sandbox");
    let status = allowed
        .args(["127.0.0.1", &broker_port.to_string()])
        .stdin(Stdio::null())
        .status()
        .expect("connect to exact loopback broker");
    assert!(status.success());

    let (mut direct, _) = backend
        .command_with_discovery_route_observation(
            Path::new("/usr/bin/nc"),
            &[],
            ResolverExecutionNetworkTransport::Https,
            &route,
            &discovery_read_root,
        )
        .expect("build endpoint-confined sandbox");
    let status = direct
        .args(["127.0.0.1", &port.to_string()])
        .stdin(Stdio::null())
        .status()
        .expect("attempt direct second-loopback connection");
    assert!(!status.success());
    assert!(matches!(listener.accept(), Err(error) if error.kind() == ErrorKind::WouldBlock));

    let observation = route.finish().expect("finish endpoint route");
    assert_eq!(observation.events().len(), 1);
    assert_eq!(
        observation.events()[0].outcome(),
        ResolverExecutionEndpointOutcome::MalformedConnect
    );
}
