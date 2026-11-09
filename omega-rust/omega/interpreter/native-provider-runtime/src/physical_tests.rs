// Physical tests verifying shared-backing page persistence across process
// unmapping and cross-process pointer linking inside the provider worker.

use crate::{channel, shared_backing::SharedBacking};
use bounded_process::{BoundedProcessChild, BoundedProcessLimits, BoundedProcessPrepared};
use std::{
    io::{Read, Write},
    process::Command,
    time::{Duration, Instant},
};

const TIMEOUT: Duration = Duration::from_secs(5);
mod native_calls;

#[test]
fn internal_native_provider_worker() {
    crate::run_worker_if_requested();
}

// A worker must consume the sponsor handshake, so that processes its foreign
// code starts never inherit it and wrongly start up as a worker. The parent
// test spawns this binary with the four variables set; this probe reads them
// once and checks that nothing is left for a child to inherit.
#[test]
fn sponsorship_probe() {
    use crate::worker::{CHANNEL_ENV, PARENT_ENV, ROOT_ENV, TIMEOUT_ENV};
    if std::env::var_os("OMEGA_SPONSORSHIP_PROBE").is_none() {
        return;
    }
    let sponsorship = crate::worker::take_sponsorship().expect("handshake is present");
    assert_eq!(sponsorship.endpoint, "probe-channel");
    #[cfg(windows)]
    assert_eq!(sponsorship.parent.as_deref(), Some("1".as_ref()));
    assert_eq!(sponsorship.root.as_deref(), Some("probe-root".as_ref()));
    assert_eq!(sponsorship.timeout.as_deref(), Some("1000".as_ref()));
    for name in [CHANNEL_ENV, PARENT_ENV, ROOT_ENV, TIMEOUT_ENV] {
        assert!(
            std::env::var_os(name).is_none(),
            "{name} would leak to children"
        );
    }
    assert!(crate::worker::take_sponsorship().is_none());
}

#[test]
fn worker_does_not_pass_its_handshake_to_children() {
    use crate::worker::{CHANNEL_ENV, PARENT_ENV, ROOT_ENV, TIMEOUT_ENV};
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "physical_tests::sponsorship_probe",
            "--nocapture",
        ])
        .env("OMEGA_SPONSORSHIP_PROBE", "1")
        .env(CHANNEL_ENV, "probe-channel")
        .env(PARENT_ENV, "1")
        .env(ROOT_ENV, "probe-root")
        .env(TIMEOUT_ENV, "1000")
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    let report = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "probe failed:\n{report}");
    assert!(report.contains("1 passed"), "probe did not run:\n{report}");
}

#[test]
fn inherited_worker_shared_home() {
    let Some(path) = std::env::var_os("OMEGA_NATIVE_HOME_TEST") else {
        return;
    };
    let endpoint = std::env::var("OMEGA_NATIVE_HOME_CHANNEL").unwrap();
    #[cfg(unix)]
    let mut channel = channel::receive(endpoint.parse().unwrap(), TIMEOUT).unwrap();
    #[cfg(windows)]
    let mut channel = channel::receive(
        &endpoint,
        std::env::var("OMEGA_NATIVE_HOME_PARENT")
            .unwrap()
            .parse()
            .unwrap(),
        TIMEOUT,
    )
    .unwrap();
    let mut backing = SharedBacking::attach(std::path::Path::new(&path), 8).unwrap();
    let mut bytes = [0; 8];
    backing.read(0, &mut bytes).unwrap();
    assert_eq!(u64::from_le_bytes(bytes), 37);
    assert!(backing.pointer(1, 8, 1).is_err());
    assert!(backing.pointer(1, 1, 8).is_err());
    channel.write_all(&bytes).unwrap();
    let mut command = [0];
    channel.read_exact(&mut command).unwrap();
    assert_eq!(&command, b"R");
    backing.read(0, &mut bytes).unwrap();
    assert_eq!(u64::from_le_bytes(bytes), 91);
    backing.write(0, &99_u64.to_le_bytes()).unwrap();
    channel.write_all(&99_u64.to_le_bytes()).unwrap();
    channel.read_exact(&mut command).unwrap();
    assert_eq!(&command, b"D");
    backing.read(0, &mut bytes).unwrap();
    assert_eq!(u64::from_le_bytes(bytes), 99);
    backing.write(0, &101_u64.to_le_bytes()).unwrap();
    channel.write_all(&101_u64.to_le_bytes()).unwrap();
}

#[test]
fn private_worker_and_host_share_pages_until_each_mapping_retires() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("omega-native-home-{}-{nonce}", std::process::id()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700);
        builder.create(&directory).unwrap();
    }
    #[cfg(not(unix))]
    std::fs::DirBuilder::new().create(&directory).unwrap();
    let path = directory.join("home");
    let mut backing = SharedBacking::create(&path, 8).unwrap();
    backing.write(0, &37_u64.to_le_bytes()).unwrap();
    assert!(SharedBacking::create(&path, 8).is_err());
    assert!(SharedBacking::attach(&path, 7).is_err());
    let private = channel::prepare(TIMEOUT).unwrap();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args([
        "--exact",
        "physical_tests::inherited_worker_shared_home",
        "--nocapture",
    ]);
    command.env("OMEGA_NATIVE_HOME_TEST", &path);
    command.env("OMEGA_NATIVE_HOME_PARENT", std::process::id().to_string());
    let mut prepared = BoundedProcessPrepared::new(
        command,
        BoundedProcessLimits::new(
            10,
            4 * 1024 * 1024 * 1024,
            1024 * 1024,
            128,
            8,
            1024 * 1024 * 1024,
            2 * 1024 * 1024 * 1024,
        ),
        "native shared home physical test",
    )
    .unwrap();
    prepared.stdin_null().stdout_null().stderr_null();
    #[cfg(unix)]
    let mut channel = {
        let descriptor = prepared.retain_descriptor(private.child).unwrap();
        prepared.env("OMEGA_NATIVE_HOME_CHANNEL", descriptor.to_string());
        private.parent
    };
    #[cfg(windows)]
    prepared.env("OMEGA_NATIVE_HOME_CHANNEL", &private.name);
    let mut child = BoundedProcessChild::spawn(prepared).unwrap();
    #[cfg(windows)]
    let mut channel = private.accept(child.id()).unwrap();
    let mut bytes = [0; 8];
    channel.read_exact(&mut bytes).unwrap();
    assert_eq!(u64::from_le_bytes(bytes), 37);
    backing.write(0, &91_u64.to_le_bytes()).unwrap();
    channel.write_all(b"R").unwrap();
    channel.read_exact(&mut bytes).unwrap();
    backing.read(0, &mut bytes).unwrap();
    assert_eq!(u64::from_le_bytes(bytes), 99);
    drop(backing);
    #[cfg(unix)]
    std::fs::remove_file(&path).unwrap();
    channel.write_all(b"D").unwrap();
    channel.read_exact(&mut bytes).unwrap();
    assert_eq!(u64::from_le_bytes(bytes), 101);
    drop(channel);
    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "physical worker did not exit");
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(status.success(), "physical worker: {status:?}");
    child.terminate().unwrap();
    child.finish().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn pointer_links_use_worker_homes_and_reject_foreign_incarnations() {
    use crate::{NativePointerSlot, NativePointerTarget, NativeProviderProcess};
    let launch = || {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            "physical_tests::internal_native_provider_worker",
            "--nocapture",
        ]);
        NativeProviderProcess::launch(command, TIMEOUT).unwrap()
    };
    let mut process = launch();
    let descriptor = process.create_home(24, 8).unwrap();
    let first = process.create_home(24, 8).unwrap();
    let second = process.create_home(24, 8).unwrap();
    let slot = NativePointerSlot {
        home: descriptor,
        offset: 8,
    };
    let target = |home| NativePointerTarget {
        home,
        offset: 0,
        byte_size: 24,
        alignment: 8,
    };
    process
        .write_home(descriptor, 0, &91_u64.to_le_bytes())
        .unwrap();
    process
        .write_home(descriptor, 16, &37_u64.to_le_bytes())
        .unwrap();
    process.link_pointer(slot, target(first)).unwrap();
    assert!(process.pointer_matches(slot, target(first)).unwrap());
    assert!(!process.pointer_matches(slot, target(second)).unwrap());
    process.link_pointer(slot, target(second)).unwrap();
    assert!(!process.pointer_matches(slot, target(first)).unwrap());
    assert!(process.pointer_matches(slot, target(second)).unwrap());
    let mut bytes = [0; 8];
    process.read_home(descriptor, 0, &mut bytes).unwrap();
    assert_eq!(u64::from_le_bytes(bytes), 91);
    process.read_home(descriptor, 16, &mut bytes).unwrap();
    assert_eq!(u64::from_le_bytes(bytes), 37);
    assert!(
        process
            .link_pointer(
                NativePointerSlot {
                    home: descriptor,
                    offset: 24
                },
                target(first)
            )
            .is_err()
    );
    assert!(
        process
            .link_pointer(
                slot,
                NativePointerTarget {
                    byte_size: 25,
                    ..target(first)
                }
            )
            .is_err()
    );
    assert!(
        process
            .link_pointer(NativePointerSlot { offset: 1, ..slot }, target(first))
            .is_err()
    );
    let mut fresh = launch();
    let new_home = fresh.create_home(24, 8).unwrap();
    assert!(
        fresh
            .link_pointer(
                NativePointerSlot {
                    home: new_home,
                    offset: 8
                },
                target(first)
            )
            .is_err()
    );
    assert!(fresh.pointer_matches(slot, target(new_home)).is_err());
    process.close().unwrap();
    fresh.close().unwrap();
}
