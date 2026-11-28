//! Native address formation and checked upgrades both produce ordinary, typed borrows.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[path = "support/project.rs"]
mod project;
mod support;

use platform_custody::temporary_directory::TemporaryDirectory;
use std::fs;
use std::process::{Command, Output, Stdio};

const UNSAFE: &str = "use {omega}/language/core/unsafe.omg as native;";
const READ: &str = "RawMemoryRead";
const WRITE: &str = "RawMemoryWrite";
const UPGRADE: &str = "BorrowMutabilityUpgrade";

struct Program(TemporaryDirectory);

impl Program {
    fn new(body: &str, grants: &[&str]) -> Self {
        let enabled = grants
            .iter()
            .map(|atom| format!("    Build::enable_unsafe::<|native|::{atom}>(plan);\n"))
            .collect::<String>();
        let build = format!(
            r#"{UNSAFE}
use main.omg as app;
machine build(plan: @mutable BuildPlan) {{
    plan.product = BuildProduct::Application {{ name: "native_borrows" }};
{enabled}    Build::bind(plan, linux_x86_64::ProgramEntry, |app|::main);
}}"#
        );
        let source = format!("{UNSAFE}\n{body}");
        Self(project::stage(
            "typed-native-borrow",
            &[("build.omg", &build), ("main.omg", &source)],
        ))
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omega"));
        command
            .args(args)
            .current_dir(self.0.path())
            .env(
                "OMEGA_LIBRARY_ROOT",
                support::repository_root().join("source/library"),
            )
            .stdin(Stdio::null());
        command
    }

    fn check(&self) -> Output {
        self.command(&["--check", "--target", "linux_x86_64", "build.omg"])
            .output()
            .unwrap()
    }

    fn interpreted(&self) -> Output {
        self.command(&["run", "--outputs", "observed", "build.omg"])
            .output()
            .unwrap()
    }

    fn assert_native_returns(&self) {
        let result = project::build_and_run_command(&mut self.command(&["build", "build.omg"]));
        assert!(
            result.compiler.status.success(),
            "{}",
            report(&result.compiler)
        );
        let native = result.execution.expect("built the executable");
        assert_eq!(native.status.code(), Some(0), "{native:?}");
        assert!(
            native.stdout.is_empty() && native.stderr.is_empty(),
            "{native:?}"
        );
    }
}

fn report(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

const ADDRESSED_BYTES: &str = r#"
machine read_byte(location: address) -> u8 {
    let byte: @u8 = |native|::borrow_at::<u8>(location);
    $byte
}
machine store_byte(location: address) {
    let byte: @mutable u8 = |native|::borrow_mutable_at::<u8>(location);
    $byte = 71u8;
}
machine main() crashes Trap {
    let mutable owned: u8 = 19u8;
    let location: address = address_of(@owned);
    let before: u8 = read_byte(location);
    store_byte(location);
    let after: u8 = read_byte(location);
    transition before == 19u8 && after == 71u8 && owned == 71u8 {
        true -> done(), false -> wrong()
    }
    machine done() {}
    machine wrong() crashes Trap { crash Trap; }
}
"#;

const UPGRADED_BYTE: &str = r#"
machine unused_reader(location: address) -> u8 {
    let native_byte: @u8 = |native|::borrow_at::<u8>(location);
    $native_byte
}
machine change(byte: @u8) {
    let writable: @mutable u8 = |native|::upgrade_mutability::<u8>(byte);
    $writable = 31u8;
}
machine main() crashes Trap {
    let mutable owned: u8 = 17u8;
    change(@owned);
    transition owned == 31u8 { true -> done(), false -> wrong() }
    machine done() {}
    machine wrong() crashes Trap { crash Trap; }
}
"#;

#[test]
fn native_references_read_and_write_an_owned_byte_at_its_exposed_address() {
    Program::new(ADDRESSED_BYTES, &[READ, WRITE]).assert_native_returns();
}

#[test]
fn an_upgrade_reborrows_the_same_owned_byte_natively_and_in_psi() {
    let program = Program::new(UPGRADED_BYTE, &[UPGRADE]);
    program.assert_native_returns();
    let interpreted = program.interpreted();
    assert!(interpreted.status.success(), "{}", report(&interpreted));
    let observed = fs::read_to_string(program.0.path().join("observed/termination.tsv")).unwrap();
    assert_eq!(observed.trim_end(), "psi\treturn");
}

fn reading_address(binding: &str, op: &str) -> String {
    format!(
        r#"
machine inspect(location: address) -> u8 {{
    let byte: {binding} = |native|::{op}::<u8>(location);
    $byte
}}
machine main() crashes Trap {{
    let mutable owned: u8 = 45u8;
    transition inspect(address_of(@owned)) == 45u8 {{ true -> done(), false -> wrong() }}
    machine done() {{}}
    machine wrong() crashes Trap {{ crash Trap; }}
}}"#
    )
}

#[test]
fn each_address_operation_is_refused_before_interpreted_entry() {
    for (binding, op, grants) in [
        ("@u8", "borrow_at", &[READ][..]),
        ("@mutable u8", "borrow_mutable_at", &[READ, WRITE][..]),
    ] {
        let source = reading_address(binding, op);
        let program = Program::new(&source, grants);
        let checked = program.check();
        assert!(checked.status.success(), "{}", report(&checked));
        let refused = program.interpreted();
        let message = report(&refused);
        assert!(!refused.status.success(), "{message}");
        assert!(
            message.to_ascii_lowercase().contains("native") && message.contains(op),
            "{message}"
        );
        assert!(!program.0.path().join("observed/termination.tsv").exists());
    }
}

#[test]
fn the_product_enables_every_atom_reached_by_its_native_operations() {
    let mutable_address = reading_address("@mutable u8", "borrow_mutable_at");
    for (source, grants, missing) in [
        (mutable_address.as_str(), &[READ][..], WRITE),
        (mutable_address.as_str(), &[WRITE][..], READ),
        (UPGRADED_BYTE, &[][..], UPGRADE),
    ] {
        let refused = Program::new(source, grants).check();
        let message = report(&refused);
        assert!(!refused.status.success(), "{message}");
        assert!(
            message.contains(&format!("did not enable Unsafe + {missing}")),
            "{message}"
        );
    }
}

#[test]
fn an_address_traced_to_a_local_cannot_promise_the_callers_lifetime() {
    let wrapper = r#"
machine choose<'out>(location: address) -> @'out u8 {
    |native|::borrow_at::<u8>(location)
}
"#;
    let address = [
        wrapper,
        r#"
machine main() {
    let local: u8 = 29u8;
    let byte: @u8 = choose(address_of(@local));
    let read: u8 = $byte;
}
"#,
    ]
    .concat();
    let accepted = Program::new(&address, &[READ]).check();
    assert!(accepted.status.success(), "{}", report(&accepted));
    let source = r#"
machine stale<'out>() -> @'out u8 {
    let local: u8 = 29u8;
    let location: address = address_of(@local);
    |native|::borrow_at::<u8>(location)
}
machine main() {
    let dangling: @u8 = stale();
    let read: u8 = $dangling;
}
"#;
    // Check only. Executing a program that has escaped local storage would
    // violate the caller's lifetime premise.
    let forwarded = [
        wrapper,
        &source.replace("|native|::borrow_at::<u8>(location)", "choose(location)"),
    ]
    .concat();
    for candidate in [source, &forwarded] {
        let refused = Program::new(candidate, &[READ]).check();
        assert_loan_refusal(&refused);
    }
}

#[test]
fn upgrading_a_borrow_cannot_extend_its_input_lifetime() {
    let source = r#"
machine extend<'out>(input: @u8) -> @'out mutable u8 {
    |native|::upgrade_mutability::<u8>(input)
}
machine main() {
    let mutable local: u8 = 0u8;
    let view: @mutable u8 = extend(@local);
    $view = 1u8;
}
"#;
    let refused = Program::new(source, &[UPGRADE]).check();
    assert_loan_refusal(&refused);
}

#[test]
fn a_known_shared_borrow_cannot_be_used_during_an_exclusive_loan() {
    for (formation, grants) in [
        (
            "|native|::borrow_mutable_at::<u8>(location)",
            &[READ, WRITE][..],
        ),
        ("|native|::upgrade_mutability::<u8>(shared)", &[UPGRADE][..]),
    ] {
        let source = format!(
            r#"
machine main() {{
    let mutable owned: u8 = 17u8;
    let location: address = address_of(@owned);
    let shared: @u8 = @owned;
    let exclusive: @mutable u8 = {formation};
    let read: u8 = $shared;
    $exclusive = read;
}}"#
        );
        let refused = Program::new(&source, grants).check();
        assert_loan_refusal(&refused);
    }
}

fn assert_loan_refusal(output: &Output) {
    let message = report(output);
    assert!(!output.status.success(), "{message}");
    let diagnostic = message.to_ascii_lowercase();
    assert!(
        ["lifetime", "outlive", "loan", "escapes", "conflict"]
            .iter()
            .any(|reason| diagnostic.contains(reason)),
        "{message}"
    );
}

#[test]
fn a_write_through_an_upgrade_updates_only_its_referents_facts() {
    let source = r#"
data Pair { changed: u8; untouched: u8; }
machine restore(pair: @mutable Pair)
requires pair.changed == 0u8 && pair.untouched == 7u8
ensures pair.changed == 0u8 && pair.untouched == 7u8
{
    let view: @mutable u8 = |native|::upgrade_mutability::<u8>(@pair.changed);
    $view = 1u8;
    $view = 0u8;
}
machine main() {
    let mutable pair: Pair = Pair { changed: 0u8, untouched: 7u8 };
    restore(@mutable pair);
}
"#;
    let restored = Program::new(source, &[UPGRADE]).check();
    assert!(restored.status.success(), "{}", report(&restored));
    let changed = source.replace("    $view = 0u8;", "");
    let refused = Program::new(&changed, &[UPGRADE]).check();
    let message = report(&refused);
    assert!(!refused.status.success(), "{message}");
    assert!(
        message.contains("cannot prove ensures") && message.contains("restore"),
        "{message}"
    );
    let disjoint = changed.replace(
        "ensures pair.changed == 0u8 && pair.untouched == 7u8",
        "ensures pair.untouched == 7u8",
    );
    let retained = Program::new(&disjoint, &[UPGRADE]).check();
    assert!(retained.status.success(), "{}", report(&retained));
}

#[test]
fn native_borrows_preserve_a_records_layout_and_field_width() {
    let source = r#"
data Counter { first: u8; count: u32; last: u8; }
machine update(location: address) {
    let counter: @mutable Counter = |native|::borrow_mutable_at::<Counter>(location);
    counter.count = 65793u32;
}
machine read(location: address) -> u32 {
    let counter: @Counter = |native|::borrow_at::<Counter>(location);
    counter.count
}
machine main() crashes Trap {
    let mutable owned: Counter = Counter { first: 11u8, count: 35u32, last: 91u8 };
    let location: address = address_of(@owned);
    update(location);
    transition read(location) == 65793u32 && owned.count == 65793u32
        && owned.first == 11u8 && owned.last == 91u8 {
        true -> done(), false -> wrong()
    }
    machine done() {}
    machine wrong() crashes Trap { crash Trap; }
}
"#;
    Program::new(source, &[READ, WRITE]).assert_native_returns();
}

#[test]
fn every_build_and_dependency_edge_must_admit_a_reachable_upgrade() {
    let editor = r#"use {renderer}/main.omg as renderer;
machine main() {
    let mutable value: u8 = 17u8;
    |renderer|::change(@value);
}"#;
    let renderer = r#"use {memory}/main.omg as memory;
public machine change(byte: @u8) { |memory|::change(byte); }"#;
    let memory = format!(
        r#"{UNSAFE}
public machine change(byte: @u8) {{
    let writable: @mutable u8 = |native|::upgrade_mutability::<u8>(byte);
    $writable = 31u8;
}}"#
    );
    let program = Program(project::stage(
        "transitive-native-borrow",
        &[
            ("main.omg", editor),
            ("renderer/main.omg", renderer),
            ("renderer/memory/main.omg", &memory),
        ],
    ));

    for (withholding_build, closed_edge_to, refusal) in [
        (None, None, None),
        (Some("editor"), None, Some("root Product did not enable")),
        (
            Some("renderer"),
            None,
            Some("dependency `renderer` did not enable"),
        ),
        (
            Some("memory"),
            None,
            Some("dependency `memory` did not enable"),
        ),
        (
            None,
            Some("renderer"),
            Some("edge `renderer` did not admit"),
        ),
        (None, Some("memory"), Some("edge `memory` did not admit")),
    ] {
        for (file, name, child) in [
            ("build.omg", "editor", Some("renderer")),
            ("renderer/build.omg", "renderer", Some("memory")),
            ("renderer/memory/build.omg", "memory", None),
        ] {
            let source = chain_build(name, child, withholding_build != Some(name), closed_edge_to);
            project::write(program.0.path(), &[(file, &source)]);
        }
        let checked = program.check();
        let message = report(&checked);
        match refusal {
            None => assert!(checked.status.success(), "{message}"),
            Some(reason) => {
                assert!(!checked.status.success(), "{message}");
                assert!(
                    message.contains(reason)
                        && message.contains("Unsafe + BorrowMutabilityUpgrade")
                        && message.contains("upgrade_mutability"),
                    "{message}"
                );
            }
        }
    }
}

fn chain_build(
    name: &str,
    child: Option<&str>,
    enabled: bool,
    closed_edge_to: Option<&str>,
) -> String {
    let mut imports = format!("{UNSAFE}\n");
    let mut body = String::new();
    if name == "editor" {
        imports.push_str("use main.omg as app;\n");
        body.push_str("plan.product = BuildProduct::Application { name: \"editor\" };\n");
    } else {
        body.push_str(&format!(
            "plan.product = BuildProduct::Package {{ name: \"{name}\" }};\n"
        ));
    }
    if enabled {
        body.push_str("Build::enable_unsafe::<|native|::BorrowMutabilityUpgrade>(plan);\n");
    }
    if let Some(child) = child {
        imports.push_str("use {omega}/build/dependency.omg as dependency;\n");
        body.push_str(&format!(r#"let request: DependencyDescription = |dependency|::from_source(Source::Path {{ location: "{child}" }});
"#));
        if closed_edge_to == Some(child) {
            body.push_str("_ = Build::dependency(plan, request);\n");
        } else {
            body.push_str(r#"let allowed: [BoundaryServiceReference; 1] = [Build::service::<|native|::BorrowMutabilityUpgrade>(plan)];
_ = |dependency|::add(plan, request, @allowed[..]);
"#);
        }
    }
    if name == "editor" {
        body.push_str("Build::bind(plan, linux_x86_64::ProgramEntry, |app|::main);\n");
    }
    format!("{imports}machine build(plan: @mutable BuildPlan) {{\n{body}}}\n")
}

#[test]
fn a_derived_address_writes_only_the_middle_byte_of_an_owned_array() {
    let source = r#"
machine change(base: address) crashes Trap {
    transition base <= 9223372036854775807 { true -> store(base), false -> wrong() }
    machine store(base: address)
    requires base <= 9223372036854775807
    {
        let middle: address = base + 1u64;
        let byte: @mutable u8 = |native|::borrow_mutable_at::<u8>(middle);
        $byte = 83u8;
    }
    machine wrong() crashes Trap { crash Trap; }
}
machine main() crashes Trap {
    let mutable bytes: [u8; 3] = [11u8, 23u8, 47u8];
    change(address_of(@bytes[0u64]));
    transition bytes[0u64] == 11u8 && bytes[1u64] == 83u8 && bytes[2u64] == 47u8 {
        true -> done(), false -> wrong()
    }
    machine done() {}
    machine wrong() crashes Trap { crash Trap; }
}
"#;
    Program::new(source, &[READ, WRITE]).assert_native_returns();
}

#[test]
fn equal_opaque_address_operands_cannot_form_overlapping_incompatible_loans() {
    let source = r#"
machine overlap(location: address) {
    let shared: @u8 = |native|::borrow_at::<u8>(location);
    let exclusive: @mutable u8 = |native|::borrow_mutable_at::<u8>(location);
    let read: u8 = $shared;
    $exclusive = read;
}
machine main() {
    let mutable byte: u8 = 47u8;
    overlap(address_of(@byte));
}
"#;
    let refused = Program::new(source, &[READ, WRITE]).check();
    assert_loan_refusal(&refused);
}

#[test]
fn a_caller_tracks_changes_made_by_a_helper_through_its_shared_input() {
    for (call, postcondition, provable) in [
        ("set_one(@pair.changed);", "pair.changed == 0u8", false),
        ("set_one(@pair.changed);", "pair.untouched == 7u8", true),
        (
            "_ = read(@pair.changed);",
            "pair.changed == 0u8 && pair.untouched == 7u8",
            true,
        ),
    ] {
        let source = format!(
            r#"
data Pair {{ changed: u8; untouched: u8; }}
machine set_one(byte: @u8) {{
    let writable: @mutable u8 = |native|::upgrade_mutability::<u8>(byte);
    $writable = 1u8;
}}
machine read(byte: @u8) -> u8 {{ $byte }}
machine observe(pair: @Pair)
requires pair.changed == 0u8 && pair.untouched == 7u8
ensures {postcondition}
{{
    {call}
}}
machine main() {{
    let mutable pair: Pair = Pair {{ changed: 0u8, untouched: 7u8 }};
    observe(@pair);
}}
"#
        );
        let checked = Program::new(&source, &[UPGRADE]).check();
        let message = report(&checked);
        assert_eq!(
            checked.status.success(),
            provable,
            "{call} {postcondition}: {message}"
        );
        if !provable {
            assert!(
                message.contains("cannot prove ensures") && message.contains("observe"),
                "{message}"
            );
        }
    }
}

#[test]
fn trusted_saved_psi_still_needs_source_policy_for_reachable_native_borrows() {
    let dormant = r#"
public machine dormant(location: address) -> u8 {
    let byte: @u8 = |native|::borrow_at::<u8>(location);
    $byte
}
machine main() {}
"#;
    for (source, grants, reachable) in [
        (ADDRESSED_BYTES, &[READ, WRITE][..], true),
        (UPGRADED_BYTE, &[UPGRADE][..], true),
        (dormant, &[][..], false),
    ] {
        let producer = Program::new(source, grants);
        let published = producer
            .command(&[
                "--emit-psi",
                "program.psi",
                "--target",
                "linux_x86_64",
                "build.omg",
            ])
            .output()
            .unwrap();
        assert!(published.status.success(), "{}", report(&published));

        let bytes = fs::read(producer.0.path().join("program.psi")).unwrap();
        let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).unwrap();
        let manifest = artifact.manifest().identity().to_string();
        let consumer = TemporaryDirectory::create("saved-native-borrow-consumer", false).unwrap();
        fs::write(consumer.path().join("program.psi"), &bytes).unwrap();
        let built = producer
            .command(&[
                "build",
                "--trusted-artifact",
                &manifest,
                "--target",
                "linux_x86_64",
                "--build-dir",
                "output",
                "program.psi",
            ])
            .env("OMEGA_LIBRARY_ROOT", consumer.path().join("absent-library"))
            .current_dir(consumer.path())
            .output()
            .unwrap();
        let message = report(&built);
        let image = consumer.path().join("output/omega-program");
        if reachable {
            assert!(!built.status.success(), "{message}");
            assert!(
                message.contains("saved Psi has no source Build policy for")
                    && message.contains("build the original source Product"),
                "{message}"
            );
            assert!(!image.exists());
        } else {
            assert!(built.status.success(), "{message}");
            let ran = Command::new(image).stdin(Stdio::null()).output().unwrap();
            assert_eq!(ran.status.code(), Some(0), "{ran:?}");
        }
    }
}
