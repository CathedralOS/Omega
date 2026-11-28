//! Package review retains each reason that an exposed service needs attention.
use super::support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::{fs, process::Command};

#[test]
fn review_classifies_exposed_core_services_by_identity_and_preserves_root_registrations() {
    let project = TemporaryDirectory::create("overlapping-service-audits", false).unwrap();
    fs::write(
        project.path().join("build.omg"),
        r#"use {omega}/language/core/process_authority.omg as process;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Package { name: "service_audit" };
    Build::audit_service::<|process|::StandardOutput>(builder);
    Build::audit_service::<|process|::StandardOutput>(builder);
    Build::audit_service::<|process|::StandardInput>(builder);
}"#,
    )
    .unwrap();
    fs::write(
        project.path().join("main.omg"),
        r#"use {omega}/language/core/process_authority.omg as process;
use {omega}/language/core/network_authority.omg as net;
boundary trait PacketWriter: |net|::NetworkSend {}
boundary trait Network {}
public machine send_ceiling() reaches PacketWriter {}
public machine local_ceiling() reaches Network {}
public machine output_ceiling() reaches |process|::StandardOutput {}"#,
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["packages", "audit", "--target", "linux_x86_64", "--details"])
        .env(
            "OMEGA_LIBRARY_ROOT",
            support::repository_root().join("source/library"),
        )
        .current_dir(project.path())
        .output()
        .unwrap();
    let report = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        output.status.code(),
        Some(3),
        "{report}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary = report.split_once("  dangerous-authority ").unwrap().1;
    let summary = summary.split_once("  external-supplies ").unwrap().0;
    let mut findings: Vec<_> = summary
        .lines()
        .filter_map(|line| {
            let (class, owned_service) = line.trim().split_once(" toolchain[")?;
            let (_, finding) = owned_service.split_once("]::")?;
            Some((class, finding))
        })
        .collect();
    findings.sort_unstable();
    let service = r#""StandardOutput""#;
    let slack = r#""StandardOutput" declared but absent from checked reach of "service_audit"::"main.omg::output_ceiling""#;
    assert_eq!(
        findings,
        [
            ("ConsumerAudit", service),
            ("ConsumerAudit", slack),
            ("Network", r#""Network""#),
            (
                "Network",
                r#""Network" declared but absent from checked reach of "service_audit"::"main.omg::send_ceiling""#
            ),
            ("Process", service),
            ("Process", slack)
        ],
        "{summary}"
    );
}

#[test]
fn audit_identifies_the_network_operations_exposed_by_each_public_machine() {
    let project = TemporaryDirectory::create("review-network-operations", false).unwrap();
    fs::write(
        project.path().join("build.omg"),
        r#"machine build(b: @mutable BuildPlan) {
    b.product = BuildProduct::Package { name: "packet_io" };
}"#,
    )
    .unwrap();
    fs::write(project.path().join("main.omg"), r#"use {omega}/language/core/network_authority.omg as network;

public data Sender { id: u64; }
public data Destination { ipv4: u32; port: u16; }

public boundary trait DatagramTransport {
    machine send(sender: @Sender, bytes: @[u8]) reaches |network|::NetworkSend crashes none;
    machine send_to(peer: Destination, bytes: @[u8]) reaches |network|::NetworkInitiateContact + |network|::NetworkSend crashes none;
}

public machine transmit(sender: @Sender, bytes: @[u8]) {
    DatagramTransport::send(sender, bytes);
}
public machine transmit_to(peer: Destination, bytes: @[u8]) {
    DatagramTransport::send_to(peer, bytes);
}
public machine transfer(sender: Sender) -> Sender { sender }"#).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["packages", "audit", "--target", "linux_x86_64", "--offline"])
        .env(
            "OMEGA_LIBRARY_ROOT",
            support::repository_root().join("source/library"),
        )
        .current_dir(project.path())
        .output()
        .unwrap();
    let report = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        output.status.code(),
        Some(3),
        "{report}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for (machine, expected) in [
        ("transmit", &["Network", "NetworkSend"][..]),
        (
            "transmit_to",
            &["Network", "NetworkInitiateContact", "NetworkSend"][..],
        ),
        ("transfer", &[][..]),
    ] {
        let heading = format!(r#"  callable "packet_io"::"main.omg::{machine}" role Public"#);
        let row = report
            .split_once(&heading)
            .unwrap()
            .1
            .split("\n  callable ")
            .next()
            .unwrap();
        let checked = row
            .lines()
            .find_map(|line| line.trim().strip_prefix("checked-reach "))
            .unwrap();
        let actual: Vec<_> = checked
            .split(", ")
            .filter(|service| *service != "[]")
            .map(|service| {
                assert!(service.starts_with("toolchain["), "{service}");
                service.split_once("]::").unwrap().1.trim_matches('"')
            })
            .collect();
        assert_eq!(actual, expected, "{heading}");
    }
    let critical = report
        .split_once("  dangerous-authority 1 ")
        .unwrap()
        .1
        .split_once("  dangerous-authority-slack ")
        .unwrap()
        .0;
    assert!(
        critical
            .lines()
            .any(|line| line.trim().starts_with("Network toolchain[")
                && line.ends_with(r#"]::"Network""#)),
        "{critical}"
    );
    let owners: Vec<_> = critical
        .lines()
        .filter_map(|line| line.trim().strip_prefix("exposed by "))
        .collect();
    assert!(
        owners.contains(&r#""packet_io"::"main.omg::transmit""#),
        "{critical}"
    );
    assert!(
        owners.contains(&r#""packet_io"::"main.omg::transmit_to""#),
        "{critical}"
    );
    assert!(
        !owners.iter().any(|owner| owner.contains("transfer")),
        "{critical}"
    );
}

#[test]
fn fresh_audit_reports_each_build_and_a_version_four_lock_refuses_the_chain() {
    let project = TemporaryDirectory::create("audit-staged-package", false).unwrap();
    let root = project.path();
    for (file, source) in [
        (
            "build.omg",
            r#"machine build(b: @mutable BuildPlan) { b.product = BuildProduct::InterpretedBuild { entry: "prepare.omg" }; }"#,
        ),
        (
            "prepare.omg",
            r#"machine build(b: @mutable BuildPlan) { b.product = BuildProduct::InterpretedBuild { entry: "configure.omg" }; }"#,
        ),
        (
            "configure.omg",
            r#"machine build(b: @mutable BuildPlan) { b.product = BuildProduct::Package { name: "meter" }; }"#,
        ),
        ("main.omg", "public machine value() -> u64 { 17u64 }"),
    ] {
        fs::write(root.join(file), source).unwrap();
    }
    let command = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .current_dir(root)
            .env(
                "OMEGA_LIBRARY_ROOT",
                support::repository_root().join("source/library"),
            )
            .arg("packages")
            .args(args)
            .args(["--target", "linux_x86_64", "--offline"])
            .output()
            .unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        (output.status.code(), text)
    };
    let (code, report) = command(&["audit"]);
    assert!(matches!(code, Some(0 | 3)), "{report}");
    assert!(report.contains("fresh-analysis complete"), "{report}");
    assert!(
        report.contains("source-changed unknown (no accepted baseline)"),
        "{report}"
    );
    let stages: Vec<_> = report
        .lines()
        .filter(|line| line.starts_with("preparing Build") || line.starts_with("final Build"))
        .collect();
    assert_eq!(stages.len(), 3, "{report}");
    assert_eq!(
        report.matches("preparation concrete-reach ").count(),
        2,
        "{report}"
    );
    assert!(
        stages[0].starts_with("preparing Build \"build.omg\"")
            && stages[0].ends_with("selected \"prepare.omg\""),
        "{report}"
    );
    assert!(
        stages[1].starts_with("preparing Build \"prepare.omg\"")
            && stages[1].ends_with("selected \"configure.omg\""),
        "{report}"
    );
    assert!(
        stages[2].starts_with("final Build \"configure.omg\""),
        "{report}"
    );
    assert!(!root.join("omega.lock").exists());
    let (code, report) = command(&["update", "--dangerously-accept-all"]);
    assert_ne!(code, Some(0), "{report}");
    assert!(
        report.contains("a version 4 package lock cannot"),
        "{report}"
    );
    assert!(!root.join("omega.lock").exists());
}
