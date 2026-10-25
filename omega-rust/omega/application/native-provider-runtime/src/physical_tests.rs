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
    if let Some(result) = crate::run_worker_if_requested() {
        result.unwrap();
    }
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
    // Parent's view and original file name are already gone on Unix. This
    // process still owns the same mapping, so its native pointers remain valid.
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
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&directory).unwrap();
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
