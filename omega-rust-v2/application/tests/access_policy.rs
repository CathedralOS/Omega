use std::{fs, path::Path, process::Command};

const POLICY: &str = include_str!("../../layers/04_typed-trees-to-prepared-trees/tests/fixtures/access_policy/selected.omg");

fn diagnostic(
    root: &Path,
    stage: &str,
    policy: &str,
    consumer: &str,
    capture: bool,
) -> (bool, String) {
    fs::write(root.join("policy.omg"), policy).unwrap();
    fs::write(root.join("consumer.omg"), consumer).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_omega"));
    command.current_dir(root).args(["source", stage]);
    if capture {
        command.args(["--package-root", "."]);
    }
    let output = command
        .args(["--", "policy.omg", "consumer.omg"])
        .output()
        .unwrap();
    assert!(output.stdout.is_empty());
    (output.status.success(), String::from_utf8(output.stderr).unwrap().trim_end().to_owned())
}

#[test]
fn executing_the_selected_policy_distinguishes_issuance_from_forwarding() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target")
        .join(format!("policy-handoff-{}", std::process::id()));
    fs::create_dir(&root).unwrap();

    let issue = "machine issue(x:i32)->i32 in Locked {x as +Locked}";
    let forward = "machine forward(x:i32 in Locked)->i32 in Locked {x}";
    let expect_at = |source: &str, occurrence: &str, stage: &str, message: &str| {
        let offset = source.find(occurrence).unwrap();
        let prefix = &source[..offset];
        let line = prefix.bytes().filter(|&byte| byte == b'\n').count() + 1;
        let column = prefix.rsplit('\n').next().unwrap().len() + 1;
        format!("policy.omg:{line}:{column}: {stage}: {message} (bytes {offset}..{})", offset + occurrence.len())
    };
    let expect = |source: &str, message: &str| expect_at(source, "access Rule", "lowering", message);
    let empty = POLICY.replace("Issuance::Unrestricted{}", "Issuance::Only{routes:[]}");
    for policy in [POLICY, &empty] {
        for consumer in [issue, forward] {
            assert_eq!(diagnostic(&root, "prepare", policy, consumer, true), (true, String::new()));
        }
        assert_eq!(diagnostic(&root, "lower", policy, forward, true), (true, String::new()));
    }
    assert_eq!(diagnostic(&root, "lower", POLICY, issue, true), (true, String::new()));
    let cast = "x as +Locked";
    let at = issue.find(cast).unwrap();
    let denied = format!(
        "consumer.omg:1:{}: lowering: machine `issue` cannot introduce domain `i32::Locked`: its access plan admits no issuer (bytes {at}..{})",
        at + 1, at + cast.len()
    );
    assert_eq!(diagnostic(&root, "lower", &empty, issue, true), (false, denied));

    let replacement = "domain i32::Old; machine retag(x:i32 in Old)->i32 in Locked {x as -Old +Locked}";
    assert_eq!(diagnostic(&root, "lower", POLICY, replacement, true), (true, String::new()));
    let cast = "x as -Old +Locked";
    let at = replacement.find(cast).unwrap();
    let denied = format!(
        "consumer.omg:1:{}: lowering: machine `retag` cannot introduce domain `i32::Locked`: its access plan admits no issuer (bytes {at}..{})",
        at + 1,
        at + cast.len()
    );
    assert_eq!(diagnostic(&root, "lower", &empty, replacement, true), (false, denied));

    // The written child implicitly accounts for Locked. A fresh parent goes
    // through its own executed plan; a parent already on x can be retained.
    let child_policy = format!("{POLICY}\ndomain i32::Locked::Child requires true;");
    let locked_child = child_policy.replace("Issuance::Unrestricted{}", "Issuance::Only{routes:[]}");
    let mint_child = "machine mint(x:i32)->i32 in Locked::Child {x as +Locked::Child}";
    let retain_parent = "machine retain(x:i32 in Locked)->i32 in Locked::Child {x as +Locked::Child}";
    for policy in [&child_policy, &locked_child] {
        assert_eq!(diagnostic(&root, "lower", policy, retain_parent, true), (true, String::new()));
    }
    assert_eq!(diagnostic(&root, "lower", &child_policy, mint_child, true), (true, String::new()));
    let cast = "x as +Locked::Child";
    let at = mint_child.find(cast).unwrap();
    let denied = format!(
        "consumer.omg:1:{}: lowering: machine `mint` cannot introduce domain `i32::Locked`: its access plan admits no issuer (bytes {at}..{})",
        at + 1, at + cast.len()
    );
    assert_eq!(diagnostic(&root, "lower", &locked_child, mint_child, true), (false, denied));

    // Removing the Build owner still stops before construction can be checked.
    assert_eq!(
        diagnostic(&root, "lower", POLICY, issue, false),
        (false, expect(POLICY, "domain access policy needs the package owner for its semantic schema"))
    );

    // The constructor inside the relay is known, but the policy calls the
    // relay rather than the checked constructor directly.
    let relay = format!(
        "{}\nmachine relay<'t>(s:@TypeSchema<'t>, i:Issuance<'t>)->AccessPlan<'t> reaches none crashes none terminates {{ new_plan::<'t>(s,i) }}",
        POLICY.replace("new_plan::<'t>(@schema", "relay::<'t>(@schema")
    );
    assert_eq!(diagnostic(&root, "prepare", &relay, forward, true), (true, String::new()));
    assert_eq!(
        diagnostic(&root, "lower", &relay, forward, true),
        (
            false,
            expect_at(
                &relay,
                "relay::<'t>(@schema, Issuance::Unrestricted{})",
                "policy preparation",
                "template call needs its packaged declaration"
            )
        )
    );

    // A helper inside the issuance expression does not supply the frozen
    // constructor inputs that the direct caller judgment needs.
    let helper = include_str!("../../layers/05_prepared-trees-to-terminal-psi/tests/fixtures/policy_templates/helper.omg");
    assert_eq!(
        diagnostic(&root, "lower", helper, forward, true),
        (
            false,
            expect(helper, "domain access policy needs its evaluated plan for the selected semantic schema")
        )
    );

    fs::remove_file(root.join("policy.omg")).unwrap();
    fs::remove_file(root.join("consumer.omg")).unwrap();
    fs::remove_dir(root).unwrap();
}
