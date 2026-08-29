#![cfg(target_os = "macos")]

use super::{
    GitExecutionTransport, GitExecutor, LocalSourceLimits, RESOLVER_CONNECT_HELPER_BASENAME,
    ResolverExecutionPhase, ResolverExecutionRequestedEndpoint,
    git_resolution_captured_output_ceiling, git_resolution_network_transfer_ceiling,
    resolver_connect_helper_path, run_git_output, system_git_candidates,
};
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
fn loopback_acceptance_canary() -> (u16, std::thread::JoinHandle<()>) {
    use std::io::ErrorKind;
    use std::net::TcpListener;

    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind helper-chain canary");
    listener
        .set_nonblocking(true)
        .expect("make helper-chain canary nonblocking");
    let port = listener.local_addr().expect("read canary address").port();
    let acceptance = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            match listener.accept() {
                Ok((_connection, _address)) => return,
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < deadline,
                        "resolver helper chain did not reach the loopback listener"
                    );
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("helper-chain listener failed: {error}"),
            }
        }
    });
    (port, acceptance)
}

#[cfg(target_os = "macos")]
fn bounded_system_executor(transport: GitExecutionTransport, port: u16) -> GitExecutor {
    let executable = system_git_candidates()
        .iter()
        .map(Path::new)
        .find(|path| path.is_file())
        .expect("find concrete system Git");
    GitExecutor::open_with_budget_for_transport(
        executable,
        1,
        Duration::from_secs(3),
        git_resolution_captured_output_ceiling(LocalSourceLimits::default()),
        git_resolution_network_transfer_ceiling(LocalSourceLimits::default()),
        transport,
        ResolverExecutionRequestedEndpoint::new("127.0.0.1", port)
            .expect("construct loopback execution endpoint"),
    )
    .expect("open bounded system Git executor")
}

#[cfg(target_os = "macos")]
#[test]
fn macos_https_execution_chain_reaches_the_selected_endpoint() {
    let (port, acceptance) = loopback_acceptance_canary();
    let executor = bounded_system_executor(GitExecutionTransport::Https, port);
    let working_directory = std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary directory");
    let locator = format!("https://127.0.0.1:{port}/repository");
    let output = run_git_output(
        &executor,
        &working_directory,
        ResolverExecutionPhase::TransportDiscovery,
        [OsStr::new("ls-remote"), OsStr::new(&locator)],
    )
    .expect("launch the retained HTTPS executable chain");
    assert!(!output.status.success());
    acceptance.join().expect("observe HTTPS helper connection");
    let transfer = executor
        .network_transfer_observation()
        .expect("reconcile HTTPS transfer accounting");
    assert!(transfer.uploaded() > 0 || transfer.downloaded() > 0);
}

#[cfg(target_os = "macos")]
#[test]
fn macos_ssh_execution_chain_reaches_the_selected_endpoint() {
    if resolver_connect_helper_path()
        .ok()
        .and_then(|path| path.file_name().map(OsStr::to_owned))
        != Some(OsString::from(RESOLVER_CONNECT_HELPER_BASENAME))
    {
        return;
    }
    let (port, acceptance) = loopback_acceptance_canary();
    let executor = bounded_system_executor(GitExecutionTransport::Ssh, port);
    let working_directory = std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary directory");
    let locator = format!("ssh://127.0.0.1:{port}/repository");
    let output = run_git_output(
        &executor,
        &working_directory,
        ResolverExecutionPhase::TransportDiscovery,
        [OsStr::new("ls-remote"), OsStr::new(&locator)],
    )
    .expect("launch the retained SSH executable chain");
    assert!(!output.status.success());
    acceptance.join().expect("observe SSH helper connection");
}
