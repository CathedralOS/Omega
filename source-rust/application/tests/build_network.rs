//! An accepted Build sends a real datagram through its own registration's backing.
#[path = "support/project.rs"]
mod project;
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::{
    collections::HashSet,
    fs,
    io::ErrorKind,
    net::{SocketAddr, UdpSocket},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

struct Fixture {
    packages: TemporaryDirectory,
    peer: UdpSocket,
}

impl Fixture {
    fn new() -> Self {
        let peer = UdpSocket::bind("127.0.0.1:0").unwrap();
        peer.set_nonblocking(true).unwrap();
        Self {
            packages: TemporaryDirectory::create("omega-build-network", false).unwrap(),
            peer,
        }
    }

    fn package(&self, name: &str, build: &str) {
        project::write(
            &self.packages.path().join(name),
            &[
                ("build.omg", build),
                ("main.omg", "public machine answer() -> u32 { 47u32 }"),
            ],
        );
    }

    fn root(&self) -> PathBuf {
        self.packages.path().join("root")
    }

    fn cli(&self, args: &[&str], grant: bool, peer: Option<SocketAddr>) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omega"));
        command
            .args(args)
            .current_dir(self.root())
            .env(
                "OMEGA_LIBRARY_ROOT",
                support::repository_root().join("source/library"),
            )
            .stdin(Stdio::null());
        if grant {
            command.args(["--allow", "Network"]);
        }
        if let Some(address) = peer {
            command.args([
                "--network-peer",
                &address.to_string(),
                "--network-max-datagram",
                "64",
            ]);
        }
        command.output().expect("omega runs")
    }

    fn build_review(&self) -> PathBuf {
        self.root().join("build/package-manager/review-build.txt")
    }

    fn product_review(&self) -> PathBuf {
        let profile = target::TargetProfile::host_if_supported().expect("hosted target");
        self.root().join(format!(
            "build/package-manager/review-{}.txt",
            profile.target_name()
        ))
    }

    fn update(&self, grant: bool) -> Output {
        let peer = grant.then(|| self.peer.local_addr().unwrap());
        let mut result = self.cli(&["packages", "update"], grant, peer);
        let mut edited = HashSet::new();
        while result.status.code() == Some(3) {
            let mut accepted = false;
            for doc in [self.build_review(), self.product_review()] {
                let Ok(text) = fs::read_to_string(&doc) else {
                    continue;
                };
                if text.contains(" pending\n") {
                    assert!(
                        edited.insert((doc.clone(), text.clone())),
                        "review repeated after acceptance: {text}"
                    );
                    decide(&doc, &text, "accept");
                    accepted = true;
                }
            }
            assert!(accepted, "no pending decision: {}", report(&result));
            result = self.cli(&["packages", "update", "--resume"], grant, peer);
        }
        result
    }

    fn packets(&self) -> Vec<String> {
        let mut packets = Vec::new();
        let mut bytes = [0u8; 256];
        loop {
            match self.peer.recv(&mut bytes) {
                Ok(len) => packets.push(String::from_utf8(bytes[..len].to_vec()).unwrap()),
                Err(error) if error.kind() == ErrorKind::WouldBlock => return packets,
                Err(error) => panic!("reading loopback peer: {error}"),
            }
        }
    }
}

fn report(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn decide(path: &Path, text: &str, decision: &str) {
    assert!(text.contains(" pending\n"), "{text}");
    fs::write(path, text.replace(" pending\n", &format!(" {decision}\n"))).unwrap();
}

fn assert_send_path(report: &str, prefix: &str) {
    let (_, path) = report.split_once(prefix).expect(report);
    let entries: Vec<_> = path.lines().next().unwrap().split(" -> ").collect();
    assert!(
        entries.first().is_some_and(|entry| entry.contains("build"))
            && entries.iter().any(|entry| entry.contains("send_marker"))
            && entries.last().is_some_and(|entry| entry.contains("send")),
        "{report}"
    );
}

fn sender(marker: &str) -> String {
    format!(
        r#"use {{omega}}/build/network.omg as network;
machine build(builder: @mutable BuildPlan) blocks {{
    let sent: bool = block send_marker();
    transition sent {{
        true -> success(builder),
        false -> refused(builder)
    }}
    machine success(builder: @mutable BuildPlan) {{ builder.product = BuildProduct::Package {{ name: "{marker}" }}; }}
    machine refused(builder: @mutable BuildPlan) {{ builder.product = BuildProduct::Package {{ name: "send_failed" }}; }}
}}
machine send_marker() -> bool blocks {{ block |network|::send("{marker}") }}"#
    )
}

fn consumer(name: &str, children: &[(&str, bool)]) -> String {
    let mut body = String::new();
    for (index, (child, inherits)) in children.iter().enumerate() {
        let option = if *inherits {
            ", Networking::Inherit"
        } else {
            ""
        };
        body.push_str(&format!(r#"
    let child{index}: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path {{ location: "../{child}" }}){option});
    suspend Build::prepare_dependency(builder, child{index});"#));
    }
    format!(
        r#"use {{omega}}/build/dependency.omg as dependency;
machine build(builder: @mutable BuildPlan) suspends {{{body}
    builder.product = BuildProduct::Package {{ name: "{name}" }};
}}"#
    )
}

#[test]
fn root_network_needs_a_peer_and_exact_project_acceptance_before_sending() {
    let fixture = Fixture::new();
    fixture.package("root", &sender("network_root"));
    let lock = fixture.root().join("omega.lock");
    let no_backing = fixture.cli(&["packages", "update"], true, None);
    assert_eq!(no_backing.status.code(), Some(1), "{}", report(&no_backing));
    assert!(
        report(&no_backing).contains("no fixed UDP peer"),
        "{}",
        report(&no_backing)
    );
    let denial = report(&no_backing);
    assert_send_path(&denial, "call path: ");
    assert!(fixture.packets().is_empty());
    assert!(!lock.exists());

    let peer = fixture.peer.local_addr().unwrap();
    let request = fixture.cli(&["packages", "update"], true, Some(peer));
    assert_eq!(request.status.code(), Some(3), "{}", report(&request));
    let phase = fixture.build_review();
    let draft = fs::read_to_string(&phase).unwrap();
    assert!(
        draft.contains("build-request send-datagram")
            && draft
                .lines()
                .any(|line| line.trim() == "physical-authority HostNetworkStack")
            && !draft.contains("OffMachineNetworkTransmit")
            && draft.contains(&format!("network-peer {peer} max-datagram-bytes 64")),
        "{draft}"
    );
    assert!(fixture.packets().is_empty());
    assert!(!lock.exists());
    assert_send_path(&draft, "call-path ");
    decide(&phase, &draft, "reject");
    let rejected = fixture.cli(&["packages", "update", "--resume"], true, Some(peer));
    assert_eq!(rejected.status.code(), Some(3), "{}", report(&rejected));
    assert!(fixture.packets().is_empty());
    assert!(!lock.exists());

    for altered in [
        draft.replace("max-datagram-bytes 64", "max-datagram-bytes 65"),
        draft.replace("send_marker", "different_helper"),
        draft.replace(
            "physical-authority HostNetworkStack",
            "physical-authority OffMachineNetworkTransmit",
        ),
    ] {
        decide(&phase, &altered, "accept");
        let tampered = fixture.cli(&["packages", "update", "--resume"], true, Some(peer));
        assert!(!tampered.status.success());
        assert!(
            report(&tampered).contains("findings changed"),
            "{}",
            report(&tampered)
        );
        assert!(fixture.packets().is_empty());
        assert!(!lock.exists());
    }

    decide(&phase, &draft, "accept");
    let approved = fixture.cli(&["packages", "update", "--resume"], true, Some(peer));
    assert_eq!(approved.status.code(), Some(3), "{}", report(&approved));
    assert_eq!(fixture.packets(), ["network_root"]);
    assert!(!lock.exists());

    let product = fixture.product_review();
    decide(&product, &fs::read_to_string(&product).unwrap(), "accept");
    let published = fixture.cli(&["packages", "update", "--resume"], true, Some(peer));
    assert!(published.status.success(), "{}", report(&published));
    let original_lock = fs::read(&lock).unwrap();

    let changed = UdpSocket::bind("127.0.0.1:0").unwrap();
    changed.set_nonblocking(true).unwrap();
    let new_peer = changed.local_addr().unwrap();
    let fresh = fixture.cli(&["packages", "update"], true, Some(new_peer));
    assert_eq!(fresh.status.code(), Some(3), "{}", report(&fresh));
    let changed_doc = fs::read_to_string(&phase).unwrap();
    assert!(
        changed_doc.contains(&format!("network-peer {new_peer} max-datagram-bytes 64")),
        "{changed_doc}"
    );
    assert!(changed_doc.contains(" pending\n"), "{changed_doc}");
    assert_eq!(fs::read(&lock).unwrap(), original_lock);
    assert_eq!(
        changed.recv(&mut [0u8; 64]).unwrap_err().kind(),
        ErrorKind::WouldBlock
    );
}

#[test]
fn remote_udp_requires_review_of_off_machine_transmission_before_sending() {
    let fixture = Fixture::new();
    fixture.package("root", &sender("remote_review"));
    let endpoint = "192.0.2.1:9".parse().unwrap();
    let request = fixture.cli(&["packages", "update"], true, Some(endpoint));
    assert_eq!(request.status.code(), Some(3), "{}", report(&request));
    let review = fs::read_to_string(fixture.build_review()).unwrap();
    let classes: Vec<_> = review
        .lines()
        .filter_map(|line| line.trim().strip_prefix("physical-authority "))
        .collect();
    assert_eq!(
        classes,
        ["HostNetworkStack", "OffMachineNetworkTransmit"],
        "{review}"
    );
    assert!(!fixture.root().join("omega.lock").exists());
}

#[test]
fn an_inheriting_child_can_send_and_its_offline_sibling_cannot() {
    let fixture = Fixture::new();
    fixture.package("root", &consumer("root", &[("live", true)]));
    fixture.package("live", &sender("live"));
    let admitted = fixture.update(true);
    assert!(admitted.status.success(), "{}", report(&admitted));
    let packets = fixture.packets();
    assert!(
        !packets.is_empty() && packets.iter().all(|s| s == "live"),
        "{packets:?}"
    );

    let lock = fixture.root().join("omega.lock");
    let accepted_lock = fs::read(&lock).unwrap();
    fixture.package("offline", &sender("offline"));
    fixture.package(
        "root",
        &consumer("root", &[("live", true), ("offline", false)]),
    );
    let refused = fixture.update(true);
    let message = report(&refused);
    assert_eq!(refused.status.code(), Some(1), "{message}");
    assert!(
        message.contains("requires Network before entry") && message.contains("offline"),
        "{message}"
    );
    assert!(fixture.packets().iter().all(|s| s == "live"));
    assert_eq!(fs::read(&lock).unwrap(), accepted_lock);
}

#[test]
fn a_child_cannot_implicitly_pass_its_grant_to_a_grandchild() {
    let fixture = Fixture::new();
    fixture.package("root", &consumer("root", &[("middle", true)]));
    fixture.package("middle", &consumer("middle", &[("leaf", false)]));
    fixture.package("leaf", &sender("leaf"));
    let refused = fixture.update(true);
    let message = report(&refused);
    assert_eq!(refused.status.code(), Some(1), "{message}");
    assert!(
        message.contains("requires Network before entry") && message.contains("leaf"),
        "{message}"
    );
    assert!(fixture.packets().is_empty());
    assert!(!fixture.root().join("omega.lock").exists());
}

#[test]
fn inheriting_from_an_offline_parent_keeps_the_child_offline() {
    let fixture = Fixture::new();
    fixture.package("root", &consumer("root", &[("child", true)]));
    fixture.package("child", &sender("child"));
    let refused = fixture.update(false);
    let message = report(&refused);
    assert_eq!(refused.status.code(), Some(1), "{message}");
    assert!(
        message.contains("requires Network before entry") && message.contains("child"),
        "{message}"
    );
    assert!(fixture.packets().is_empty());
    assert!(!fixture.root().join("omega.lock").exists());
}

#[test]
fn staged_build_delegates_once_per_registration_without_a_review_or_lock() {
    let fixture = Fixture::new();
    fixture.package("shared", &sender("shared"));
    for parent in ["left", "right"] {
        fixture.package(parent, &consumer(parent, &[("shared", true)]));
    }
    let profile = target::TargetProfile::host_if_supported().unwrap();
    let build = format!(
        r#"use main.omg as app;
machine build(builder: @mutable BuildPlan) suspends {{
    let left: DependencyRegistration = Build::dependency(builder, Source::Path {{ location: "../left" }}, Networking::Inherit);
    suspend Build::prepare_dependency(builder, left);
    let right: DependencyRegistration = Build::dependency(builder, Source::Path {{ location: "../right" }}, Networking::Inherit);
    suspend Build::prepare_dependency(builder, right);
    builder.product = BuildProduct::Application {{ name: "app" }};
    Build::bind(builder, {}::ProgramEntry, |app|::main);
}}"#,
        profile.target_name()
    );
    project::write(
        &fixture.root(),
        &[("build.omg", &build), ("main.omg", "machine main() {}")],
    );
    let peer = fixture.peer.local_addr().unwrap();
    let lock = fixture.root().join("omega.lock");
    assert!(!lock.exists());
    for adjacent_lock in [None, Some("invalid version 4 lock")] {
        if let Some(contents) = adjacent_lock {
            fs::write(&lock, contents).unwrap();
        }
        let output = fixture.cli(&["build", "build.omg"], true, Some(peer));
        assert!(output.status.success(), "{}", report(&output));
        assert_eq!(fixture.packets(), ["shared", "shared"]);
        assert!(!fixture.build_review().exists());
        assert_eq!(fs::read_to_string(&lock).ok().as_deref(), adjacent_lock);
    }
}

#[test]
fn the_same_source_requires_a_network_grant_on_each_registration() {
    let fixture = Fixture::new();
    fixture.package("shared", &sender("shared"));
    fixture.package("left", &consumer("left", &[("shared", true)]));
    fixture.package("right", &consumer("right", &[("shared", false)]));
    fixture.package(
        "root",
        &consumer("root", &[("left", true), ("right", true)]),
    );
    let refused = fixture.update(true);
    let message = report(&refused);
    assert_eq!(refused.status.code(), Some(1), "{message}");
    assert!(
        message.contains("requires Network before entry") && message.contains("shared"),
        "{message}"
    );
    assert!(!fixture.root().join("omega.lock").exists());
}

fn reply_consumer() -> &'static str {
    r#"use {omega}/build/network.omg as network;
use {omega}/language/core/sandbox/datagram.omg as datagram;

machine build(builder: @mutable BuildPlan) blocks {
    let mutable bytes: [u8; 5] = [10u8, 11u8, 12u8, 13u8, 14u8];
    let sent: bool = block |network|::send("query");
    let outcome: |datagram|::ReceiveResult = block |network|::receive_reply(@mutable bytes[1..4]);
    transition outcome {
        |datagram|::ReceiveResult::Received { count } -> inspect(builder, sent, count, @bytes),
        _ -> failed(builder)
    }
    machine inspect(builder: @mutable BuildPlan, sent: bool, count: u64, bytes: @[u8; 5]) {
        transition sent && count == 2u64 && bytes[0] == 10u8 && bytes[1] == 79u8 && bytes[2] == 75u8 && bytes[3] == 13u8 && bytes[4] == 14u8 {
            true -> passed(builder),
            false -> failed(builder)
        }
    }
    machine passed(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "reply_ok" }; }
    machine failed(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "reply_failed" }; }
}"#
}

#[test]
fn approving_a_send_alone_cannot_run_a_build_that_also_receives() {
    let fixture = Fixture::new();
    fixture.package("root", reply_consumer());
    let before = fixture.cli(
        &["packages", "update"],
        true,
        Some(fixture.peer.local_addr().unwrap()),
    );
    assert_eq!(before.status.code(), Some(3), "{}", report(&before));
    let phase = fixture.build_review();
    let draft = fs::read_to_string(&phase).unwrap();
    assert!(
        draft.contains("build-request send-datagram")
            && draft.contains("build-request receive-datagram-reply"),
        "{draft}"
    );
    let mut send = false;
    let mut edited = String::new();
    for line in draft.lines() {
        if let Some(operation) = line.trim().strip_prefix("build-request ") {
            send = operation == "send-datagram";
        }
        if send && line.starts_with("decision build ") {
            edited.push_str(&line.replace(" pending", " accept"));
        } else {
            edited.push_str(line);
        }
        edited.push('\n');
    }
    assert_ne!(edited, draft);
    assert!(edited.contains(" pending\n"));
    fs::write(&phase, edited).unwrap();
    let withheld = fixture.cli(
        &["packages", "update", "--resume"],
        true,
        Some(fixture.peer.local_addr().unwrap()),
    );
    assert_eq!(withheld.status.code(), Some(3), "{}", report(&withheld));
    assert!(fixture.packets().is_empty());
    assert!(!fixture.root().join("omega.lock").exists());
}

#[test]
fn receiving_requires_delegation_and_a_reply_requires_a_prior_send() {
    let fixture = Fixture::new();
    fixture.package("root", &consumer("parent", &[("reader", false)]));
    let source = r#"use {omega}/build/network.omg as network;
use {omega}/language/core/sandbox/datagram.omg as datagram;
machine build(b: @mutable BuildPlan) blocks {
    let mutable byte: [u8; 1] = [99u8];
    let result: |datagram|::ReceiveResult = block |network|::receive_reply(@mutable byte[..]);
    transition result {
        |datagram|::ReceiveResult::Unavailable -> done(b, byte[0]),
        _ -> wrong(b)
    }
    machine done(b: @mutable BuildPlan, sentinel: u8) {
        transition sentinel {
            99u8 -> untouched(b),
            _ -> wrong(b)
        }
    }
    machine untouched(b: @mutable BuildPlan) { b.product = BuildProduct::Package { name: "no_open_flow" }; }
    machine wrong(b: @mutable BuildPlan) { b.product = BuildProduct::Package { name: "bad_reply" }; }
}"#;
    fixture.package("reader", source);
    let offline = fixture.update(true);
    let message = report(&offline);
    assert_eq!(offline.status.code(), Some(1), "{message}");
    assert!(
        message.contains("reader/build.omg")
            && message.contains("requires Network before entry")
            && message.contains("receive"),
        "{message}"
    );
    assert!(!fixture.root().join("omega.lock").exists());
    fixture.package("root", &consumer("parent", &[("reader", true)]));
    let admitted = fixture.update(true);
    assert!(admitted.status.success(), "{}", report(&admitted));
    let lock = fs::read_to_string(fixture.root().join("omega.lock")).unwrap();
    assert!(lock.contains("no_open_flow") && !lock.contains("bad_reply"));
    assert!(fixture.packets().is_empty());
}

#[test]
fn an_inheriting_child_uses_only_its_admitted_peers_reply() {
    use std::{sync::mpsc, thread};
    let fixture = Fixture::new();
    fixture.package("root", &consumer("parent", &[("child", true)]));
    fixture.package("child", reply_consumer());
    let socket = fixture.peer.try_clone().unwrap();
    let stranger = UdpSocket::bind("127.0.0.1:0").unwrap();
    let (stop, shutdown) = mpsc::channel::<()>();
    let server = thread::spawn(move || {
        let mut count = 0;
        while matches!(shutdown.try_recv(), Err(mpsc::TryRecvError::Empty)) {
            let mut packet = [0; 16];
            match socket.recv_from(&mut packet) {
                Ok((n, client)) => {
                    assert_eq!(&packet[..n], b"query");
                    stranger.send_to(b"NO", client).unwrap();
                    socket.send_to(b"OK", client).unwrap();
                    count += 1;
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    thread::sleep(std::time::Duration::from_millis(2))
                }
                Err(error) => panic!("local reply peer: {error}"),
            }
        }
        count
    });
    let published = fixture.update(true);
    drop(stop);
    let requests = server.join().unwrap();
    assert!(published.status.success(), "{}", report(&published));
    assert!(requests > 0);
    let lock = fs::read_to_string(fixture.root().join("omega.lock")).unwrap();
    assert!(
        lock.contains("reply_ok") && !lock.contains("reply_failed"),
        "{lock}"
    );
}

#[test]
fn checked_recordings_serve_send_and_receive_without_live_network_backing() {
    use build_declarations::{BuildDeclaration, BuildDependencySource, ProjectName};
    use package_manager::project::{BuildLibraryRequest, ProjectInput, open_products};

    package_compilation::toolchain_library::configure(
        &support::repository_root().join("source/library"),
    )
    .unwrap();
    let fixture = Fixture::new();
    fixture.package(
        "recorder",
        r#"
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Package { name: "recorder" };
}"#,
    );
    project::write(
        &fixture.packages.path().join("recorder"),
        &[(
            "main.omg",
            r#"
use {omega}/language/core/sandbox/datagram.omg as datagram;
public RecordedFailure satisfies |datagram|::Datagram {
    machine send(bytes: @[u8]) -> bool {
        transition 0u64 < bytes.length {
            true -> reply(bytes, 0u64),
            false -> true
        }
        machine reply(bytes: @[u8], index: u64) -> bool
        requires index < bytes.length
        {
            bytes.length != 1u64 || bytes[index] != 82u8
        }
    }
}
public RecordedReply satisfies |datagram|::DatagramReplies {
    machine receive(bytes: @mutable [u8]) -> |datagram|::ReceiveResult {
        transition 0u64 < bytes.length {
            true -> write(bytes, 0u64),
            false -> |datagram|::ReceiveResult::TooLarge
        }
        machine write(bytes: @mutable [u8], index: u64) -> |datagram|::ReceiveResult
        requires index < bytes.length
        {
            bytes[index] = 83u8;
            |datagram|::ReceiveResult::Received { count: 1u64 }
        }
    }
}"#,
        )],
    );
    fixture.package("root", r#"
use {recordings}/main.omg as recording;
use {omega}/build/network.omg as network;
use {omega}/language/core/sandbox/datagram.omg as datagram;
machine build(builder: @mutable BuildPlan) suspends blocks {
    suspend Build::await_provider::<|datagram|::Datagram, |recording|::RecordedFailure>(builder);
    suspend Build::await_provider::<|datagram|::DatagramReplies, |recording|::RecordedReply>(builder);
    let mutable payload: [u8; 3] = [88u8, 82u8, 89u8];
    let sent: bool = block |network|::send(@payload[1..2]);
    let received: |datagram|::ReceiveResult = block |network|::receive_reply(@mutable payload[1..2]);
    transition received {
        |datagram|::ReceiveResult::Received { count } -> inspect(builder, sent, count, @payload),
        _ -> unexpected(builder)
    }
    machine inspect(builder: @mutable BuildPlan, sent: bool, count: u64, bytes: @[u8; 3]) {
        transition !sent && count == 1u64 && bytes[0] == 88u8 && bytes[1] == 83u8 && bytes[2] == 89u8 {
            true -> recorded(builder),
            false -> unexpected(builder)
        }
    }
    machine recorded(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "recorded_failure" }; }
    machine unexpected(builder: @mutable BuildPlan) { builder.product = BuildProduct::Package { name: "unexpected_live_send" }; }
}"#);
    let libraries = [BuildLibraryRequest {
        alias: ProjectName::parse("recordings").unwrap(),
        source: BuildDependencySource::Path("../recorder".into()),
        max_reach: Default::default(),
    }];
    let target = target::TargetProfile::host_if_supported().unwrap();
    let result = open_products(
        ProjectInput::WithBuildLibraries {
            file: &fixture.root().join("build.omg"),
            libraries: &libraries,
        },
        &[target],
        true,
        false,
        &fixture.packages.path().join("out"),
        Default::default(),
    )
    .unwrap();
    let config = &result.select(target).unwrap().evaluated().configuration;
    let Some(BuildDeclaration::Package(package)) = &config.project_declaration else {
        panic!("Build did not return its package");
    };
    assert_eq!(package.name.as_str(), "recorded_failure");
    assert!(fixture.packets().is_empty());
}
