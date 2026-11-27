/*

End-to-end CLI check that a provider a Build selects runs on both engines
on each selected application engine.

`Build::select_provider<Service, Native>(builder, target)` chooses which
implementation answers a boundary requirement on one target
(`wiki/spec/build/provider_selection.md`). Native application assembly once
refused every such program before execution, even after the selected body was
checked. The settled plan now reaches the boundary settlement with one
provenance row per canonical plan row, so the call runs the selected machine.

The control's entry traps unless `Service::go(3)` returns 4. Its twins keep
the entry and change only the provider: an inline body runs the same way, and
a provider that returns its input unchanged makes both engines trap at the
entry's own check rather than being refused before execution.

*/

#[path = "support/project.rs"]
mod project;
mod support;

use std::process::ExitStatus;

const REQUIREMENT: &str = "\
boundary trait Service {
    machine go(value: u32) -> u32 requires value <= 10 ensures result <= 11 reaches none crashes none;
}
";

const ENTRY: &str = "\
machine entry() crashes Trap {
    transition Service::go(3) == 4 { true -> done(), _ -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
";

const BUILD: &str = "use main.omg as app;
\
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: \"selected_provider\" };
    Build::select_provider::<|app|::Service, |app|::Native>(builder, linux_x86_64);
    Build::select_provider::<|app|::Service, |app|::Native>(builder, linux_arm64);
    Build::select_provider::<|app|::Service, |app|::Native>(builder, macos_arm64);
    Build::select_provider::<|app|::Service, |app|::Native>(builder, windows_x86_64);
    Build::bind(builder, linux_x86_64::ProgramEntry, |app|::entry);
    Build::bind(builder, linux_arm64::ProgramEntry, |app|::entry);
    Build::bind(builder, macos_arm64::ProgramEntry, |app|::entry);
    Build::bind(builder, windows_x86_64::ProgramEntry, |app|::entry);
}
";

const CONTRACT: &str = "requires value <= 10 ensures result <= 11 reaches none crashes none";

fn referenced(body: &str) -> String {
    format!(
        "{REQUIREMENT}machine provider_go(value: u32) -> u32 {CONTRACT} {{ {body} }}
Native satisfies Service {{
    Service::go = provider_go;
}}
{ENTRY}"
    )
}

fn run_fixture(engine: &str, name: &str, product: &str, build: &str) -> (ExitStatus, String) {
    let scratch = project::stage(name, &[("main.omg", product), ("build.omg", build)]);
    let output = project::omega(
        scratch.path(),
        &[
            "run",
            "--engine",
            engine,
            "build.omg",
            "--build-input",
            "build.omg",
            "--build-input",
            "main.omg",
        ],
    );
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status, report)
}

fn assert_normal_return(engine: &str, (status, report): (ExitStatus, String)) {
    let expected = if engine == "native" {
        "native exit 0"
    } else {
        "Psi exit 0"
    };
    assert!(
        status.success() && report.lines().any(|line| line == expected),
        "{report}"
    );
}

#[test]
fn a_selected_generic_provider_keeps_its_uncalled_requirement() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let target = host.target_name();
    let program = r#"
data Tag {}
data Word {}
data Receipt<T> { value: T; }
data Outcome<T> { case Ready(value: T); case Empty; }
boundary trait Echo<P, T> {
    machine carry(x: u32) -> u32 reaches none crashes none;
    machine restore(x: Outcome<Receipt<T>>) -> Outcome<Receipt<T>> reaches none crashes none;
}
Copy<P, T, R> satisfies Echo<P, T> {
    machine carry(x: u32) -> u32 reaches none crashes none { x }
    machine restore(x: Outcome<Receipt<T>>) -> Outcome<Receipt<T>> reaches none crashes none { x }
}
machine entry() {}
"#;
    let build = format!(
        r#"use main.omg as app;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application {{ name: "uncalled_provider" }};
    Build::select_provider::<|app|::Echo<|app|::Tag, |app|::Word>, |app|::Copy<|app|::Tag, |app|::Word, u32>>(builder, {target});
    Build::bind(builder, {target}::ProgramEntry, |app|::entry);
}}"#
    );
    for engine in ["native", "psi"] {
        assert_normal_return(
            engine,
            run_fixture(engine, "uncalled_requirement", program, &build),
        );

        let wrong = build.replace("|app|::Copy<|app|::Tag, |app|::Word, u32>", "|app|::Copy<|app|::Word, |app|::Tag, u32>");
        let (status, report) = run_fixture(engine, "wrong_uncalled_requirement", program, &wrong);
        assert!(
            !status.success() && report.contains("does not satisfy"),
            "{report}"
        );
    }
}

#[test]
fn a_selected_referenced_provider_runs_on_both_engines() {
    for engine in ["native", "psi"] {
        assert_normal_return(
            engine,
            run_fixture(engine, "referenced", &referenced("value + 1"), BUILD),
        );
    }
}

#[test]
fn a_selected_inline_provider_runs_on_both_engines() {
    let inline = format!(
        "{REQUIREMENT}Native satisfies Service {{
    machine go(value: u32) -> u32 {CONTRACT} {{ value + 1 }}
}}
{ENTRY}"
    );
    for engine in ["native", "psi"] {
        assert_normal_return(engine, run_fixture(engine, "inline", &inline, BUILD));
    }
}

#[test]
fn a_selected_provider_with_another_answer_traps_at_the_entry_check() {
    for engine in ["native", "psi"] {
        let (status, report) = run_fixture(engine, "wrong-answer", &referenced("value"), BUILD);
        assert!(!status.success(), "{report}");
        assert!(!report.contains("no product route yet"), "{report}");
        if engine == "psi" {
            assert!(report.contains("Psi crash: Trap"), "{report}");
        }
        assert!(!report.contains("native exit 0"), "{report}");
    }
}

#[test]
fn a_checked_provider_calls_each_callback_with_its_original_arguments() {
    let sample =
        support::repository_root().join("samples/cli/facts/dynamic_trait_dispatch/target_body");
    let program = std::fs::read_to_string(sample.join("main.omg")).unwrap();
    let build = std::fs::read_to_string(sample.join("build.omg")).unwrap();
    for engine in ["native", "psi"] {
        assert_normal_return(
            engine,
            run_fixture(engine, "callback-arguments", &program, &build),
        );
    }
}

#[test]
fn two_qualifications_of_the_same_carrier_select_different_providers() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let target = host.target_name();
    let program = r#"
data Tag {}
data Other {}
domain Tag::Left;
domain Tag::Right;
boundary trait Service<T> { machine value() -> u32 reaches none crashes none; }
First satisfies Service<Tag in Tag::Left> { machine value() -> u32 { 29u32 } }
Second satisfies Service<Tag in Tag::Right> { machine value() -> u32 { 37u32 } }
machine entry() crashes Trap {
    let left: u32 = Service::<Tag in Tag::Left>::value();
    let right: u32 = Service::<Tag in Tag::Right>::value();
    transition left == 29u32 && right == 37u32 { true -> done(), _ -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}
"#;
    let build = format!(
        r#"use main.omg as app;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application {{ name: "qualified_providers" }};
    Build::select_provider::<|app|::Service<|app|::Tag in |app|::Tag::Left>, |app|::First>(builder, {target});
    Build::select_provider::<|app|::Service<|app|::Tag in |app|::Tag::Right>, |app|::Second>(builder, {target});
    Build::bind(builder, {target}::ProgramEntry, |app|::entry);
}}"#
    );
    for engine in ["native", "psi"] {
        assert_normal_return(engine, run_fixture(engine, "qualified", program, &build));

        let wrong_provider = build.replace("Tag::Right>, Second", "Tag::Right>, First");
        let (status, report) =
            run_fixture(engine, "wrong_qualified_provider", program, &wrong_provider);
        assert!(
            !status.success() && report.contains("does not satisfy"),
            "{report}"
        );

        let wrong_carrier = build.replace("Tag in Tag::Right", "Other in Tag::Right");
        let (status, report) =
            run_fixture(engine, "wrong_qualified_carrier", program, &wrong_carrier);
        assert!(
            !status.success() && report.contains("does not qualify"),
            "{report}"
        );
    }
}

#[test]
fn provider_choices_expand_type_aliases_and_check_their_erased_arguments() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let target = host.target_name();
    let program = r#"
data Packet<T> { value: T; }
data Pack<T> = Packet<T>;
data WordPacket = Pack<u32>;
data CopyPacket<T [copy]> = Packet<u32>;
data Ticket [linear] {}
boundary trait Reading<T> { machine read() -> u32 reaches none crashes none; }
Words satisfies Reading<Packet<u32>> { machine read() -> u32 { 29u32 } }
Wide satisfies Reading<Packet<u64>> { machine read() -> u32 { 37u32 } }
machine entry() crashes Trap {
    let word: u32 = Reading::<Packet<u32>>::read();
    let wide: u32 = Reading::<Packet<u64>>::read();
    transition word == 29u32 && wide == 37u32 { true -> done(), _ -> bad() }
    machine done() {} machine bad() { crash Trap; }
}
"#;
    let build = format!(
        r#"use main.omg as app;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application {{ name: "aliased_providers" }};
    Build::select_provider::<|app|::Reading<|app|::WordPacket>, |app|::Words>(builder, {target});
    Build::select_provider::<|app|::Reading<|app|::Pack<u64>>, |app|::Wide>(builder, {target});
    Build::bind(builder, {target}::ProgramEntry, |app|::entry);
}}"#
    );
    for engine in ["native", "psi"] {
        assert_normal_return(
            engine,
            run_fixture(engine, "aliased_slots", program, &build),
        );
        let erased = build.replace("|app|::WordPacket", "|app|::CopyPacket<|app|::Ticket>");
        let (status, report) = run_fixture(engine, "invalid_alias_actual", program, &erased);
        assert!(!status.success() && report.contains("copy"), "{report}");
    }
}

#[test]
fn an_ordinary_build_helper_forwards_the_type_of_its_checked_alias() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let target = host.target_name();
    let product = r#"boundary trait Reading<T> { machine read() -> u32 reaches none crashes none; }
Words satisfies Reading<u32> { machine read() -> u32 { 29u32 } }
machine entry() crashes Trap {
    let word: u32 = Reading::<u32>::read();
    transition word == 29u32 { true -> done(), _ -> bad() }
    machine done() {} machine bad() { crash Trap; }
}
"#;
    let build = format!(
        r#"use main.omg as app;
data Word<T [copy]> = u32;
data Ticket [linear] {{}}
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application {{ name: "helper_alias" }};
    choose::<Word<u64>>(builder);
    Build::bind(builder, {target}::ProgramEntry, |app|::entry);
}}
machine choose<T>(builder: @mutable BuildPlan) {{
    Build::select_provider::<|app|::Reading<T>, |app|::Words>(builder, {target});
}}"#
    );
    for engine in ["native", "psi"] {
        assert_normal_return(engine, run_fixture(engine, "helper_alias", product, &build));

        let invalid = build.replace("Word<u64>", "Word<Ticket>");
        let (status, report) = run_fixture(engine, "helper_alias_bound", product, &invalid);
        assert!(
            !status.success() && report.contains("requires `[copy]`"),
            "{report}"
        );
    }
}

#[test]
fn a_resident_provider_requires_layout_storage_for_its_value_type() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    let program = r#"
use {omega}/language/core/layout.omg as layout;
use {omega}/language/core/reflect/reflect.omg as reflect;
use {omega}/language/core/access/access.omg as access;
public data Word {}
public data Other {}
public data TestMemory {}
public domain Word::Placed access NoFields;
public domain Other::Placed access NoFields;
public data WordPlacement = Word in Word::Placed;
public data OtherPlacement = Other in Other::Placed;
public data WordStorage = |layout|::LayoutStorage<Word, WordLayout>;
public NoFields satisfies |layout|::AccessPolicy {
    machine plan(schema: |reflect|::TypeSchema) -> |layout|::AccessPlan reaches none crashes none terminates {
        |access|::new_plan(@schema, |layout|::Issuance::Unrestricted)
    }
}
public WordLayout satisfies |layout|::FixedLayout<Word> {
    machine plan(schema: |layout|::Schema, target: |layout|::LayoutTarget) -> |layout|::LayoutPlan reaches none crashes none terminates {
        let entries: [|layout|::FieldEntry; 64];
        |layout|::LayoutPlan { entries: entries, entry_count: 0, size_fixed: 0, size_is_dynamic: false, align: 1 }
    }
}
machine storage_probe() {}
"#;
    let target = host.target_name();
    for engine in ["native", "psi"] {
        for (name, placement, value, storage, reason) in [
            (
                "plain_resident_storage",
                "WordPlacement",
                "Word",
                "Word",
                "`|app|::Word` is an ordinary value type",
            ),
            (
                "wrong_resident_schema",
                "OtherPlacement",
                "Other",
                "WordStorage",
                "storage represents `Word`, but the operation requires `|app|::Other`",
            ),
            (
                "unqualified_resident_index",
                "Word",
                "Word",
                "WordStorage",
                "must name a domain-qualified type",
            ),
            (
                "wrong_resident_index_carrier",
                "OtherPlacement",
                "Word",
                "WordStorage",
                "does not qualify the supplied value type",
            ),
        ] {
            let build = format!(
                r#"use main.omg as app;
use {{omega}}/language/core/layout.omg as layout;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application {{ name: "storage_probe" }};
    Build::accept_boundary::<|layout|::NativeAccess<|app|::{placement}, |app|::{value}, |app|::{storage}, |app|::TestMemory>>(builder);
    Build::select_provider::<|layout|::AccessLifecycle<|app|::{placement}, |app|::{value}>, |layout|::NativeAccess<|app|::{placement}, |app|::{value}, |app|::{storage}, |app|::TestMemory>>(builder, {target});
    Build::bind(builder, {target}::ProgramEntry, |app|::storage_probe);
}}"#
            );
            let (status, report) = run_fixture(engine, name, program, &build);
            assert!(!status.success() && report.contains(reason), "{report}");
        }
    }
}

#[test]
fn a_resident_index_alias_preserves_its_imported_value_type() {
    let Some(host) = target::TargetProfile::host_if_supported() else {
        return;
    };
    // Exercise the original sample's layout, domain and resource profile in a
    // selected provider, ahead of the independent vacancy and loan judgments
    // in the sample's allocating entry.
    let sample =
        support::repository_root().join("samples/cli/facts/resident_backing/local/main.omg");
    let sample = std::fs::read_to_string(sample).unwrap();
    let (definitions, _) = sample
        .split_once("\nboundary trait NativeAllocation {")
        .unwrap();
    let program = format!("{definitions}\nmachine storage_probe() {{}}\n");
    let target = host.target_name();
    let build = format!(
        r#"use main.omg as app;
use {{omega}}/language/core/layout.omg as layout;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application {{ name: "storage_probe" }};
    Build::accept_boundary::<|app|::MemorySupply>(builder);
    Build::accept_boundary::<|layout|::NativeAccess<|app|::WordPlacement, |app|::Word, |app|::WordStorage, |app|::TestMemory>>(builder);
    Build::select_provider::<|layout|::ResourceProfileSupply<|app|::TestMemory>, |app|::MemorySupply>(builder, {target});
    Build::select_provider::<|layout|::AccessLifecycle<|app|::WordPlacement, |app|::Word>, |layout|::NativeAccess<|app|::WordPlacement, |app|::Word, |app|::WordStorage, |app|::TestMemory>>(builder, {target});
    Build::bind(builder, {target}::ProgramEntry, |app|::storage_probe);
}}"#
    );
    for engine in ["native", "psi"] {
        let (status, report) = run_fixture(engine, "matching_resident_index", &program, &build);
        if status.success() {
            assert_normal_return(engine, (status, report));
        } else {
            assert!(report.contains("resident placement needs evaluated layout, access policy, live content and the selected resource profile for this grant"), "{report}");
        }
    }
}
