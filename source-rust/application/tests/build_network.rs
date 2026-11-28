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

    let altered_limit = draft.replace("max-datagram-bytes 64", "max-datagram-bytes 65");
    decide(&phase, &altered_limit, "accept");
    let tampered = fixture.cli(&["packages", "update", "--resume"], true, Some(peer));
    assert!(!tampered.status.success());
    assert!(
        report(&tampered).contains("findings changed"),
        "{}",
        report(&tampered)
    );
    assert!(fixture.packets().is_empty());
    assert!(!lock.exists());

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

#[test]
fn a_checked_recording_answers_the_same_send_without_live_network_backing() {
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
    machine send(bytes: @[u8]) -> bool { bytes.length != 8u64 }
}"#,
        )],
    );
    fixture.package("root", r#"
use {recordings}/main.omg as recording;
use {omega}/build/network.omg as network;
use {omega}/language/core/sandbox/datagram.omg as datagram;
machine build(builder: @mutable BuildPlan) suspends blocks {
    suspend Build::await_provider::<|datagram|::Datagram, |recording|::RecordedFailure>(builder);
    let sent: bool = block |network|::send("recorded");
    transition sent {
        false -> recorded(builder),
        true -> unexpected(builder)
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
