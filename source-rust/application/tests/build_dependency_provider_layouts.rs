/*

End-to-end CLI check that a dependency's provider default is laid out only for
a target that chooses it (`wiki/spec/layouts/plans.md`, "Layout demand and
validation").

The kit package `source-rust/application/tests/fixtures/packages/layout-default-kit` declares target
defaults for `Measure`. Its Windows default stores an `Overlapping` record
whose layout policy places both fields at offset 0, which layout validation
refuses. Before this repair, every program depending on the kit was refused on
every target with that Windows overlap, even one that never calls `Measure`.

The kept program reaches `Measure` and replaces the Windows default in its own
Build, so the other targets choose the kit's `PairMeasure` default, whose Pair
layout is prepared for them; its entry traps unless byte 8 holds 42, the value
only the authored layout places there. Two variants drop the replacement: one
never reaches `Measure` and runs, because no target chooses a default; the
other reaches it, so Windows chooses the overlapping default and its layout
refuses the program.

*/

mod support;

use std::path::{Path, PathBuf};
use std::process::Command;

fn repository() -> PathBuf {
    support::repository_root().to_path_buf()
}

fn omega(arguments: &[&str], directory: &Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(arguments)
        .current_dir(directory)
        .stdin(std::process::Stdio::null())
        .output()
        .expect("omega runs");
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn fixture() -> PathBuf {
    repository().join("source-rust/application/tests/fixtures/providers/pass/default_provider_layout_demand_exit")
}

/// A copy of the kept program whose Build keeps every kit default, with `main`
/// as its entry source.
fn default_only(name: &str, main: &str) -> PathBuf {
    let kit = repository()
        .join("source-rust/application/tests/fixtures/packages/layout-default-kit")
        .canonicalize()
        .expect("kit package");
    let build = std::fs::read_to_string(fixture().join("build.omg")).expect("kept Build");
    // The path becomes an Omega string literal, where a backslash starts an
    // escape. A Windows path needs each separator doubled.
    let location = kit
        .to_str()
        .expect("UTF-8 repository path")
        .replace('\\', "\\\\");
    let defaults = build
        .replace("../../../packages/layout-default-kit", &location)
        .lines()
        .filter(|line| !line.contains("select_provider") && !line.contains("Windows takes"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_ne!(defaults, build);
    let root = std::env::temp_dir().join(format!(
        "omega-default-provider-layout-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("staged project");
    std::fs::write(root.join("build.omg"), defaults).expect("staged Build");
    std::fs::write(root.join("main.omg"), main).expect("staged program");
    root
}

fn assert_runs_on_each_engine(root: &std::path::Path) {
    for (engine, expected) in [("native", "native exit 0"), ("psi", "Psi exit 0")] {
        let report = omega(&["run", "--engine", engine, "build.omg"], root);
        assert!(report.lines().any(|line| line == expected), "{report}");
    }
}

#[test]
fn a_chosen_dependency_default_is_laid_out_for_its_target_and_runs() {
    assert_runs_on_each_engine(&fixture());
}

#[test]
fn an_unchosen_dependency_default_is_never_laid_out() {
    let root = default_only(
        "unreached",
        "public machine main() {\n}\n",
    );
    assert_runs_on_each_engine(&root);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_chosen_dependency_default_with_an_invalid_layout_is_refused() {
    let main = std::fs::read_to_string(fixture().join("main.omg")).expect("kept program");
    let root = default_only("reached", &main);
    // The authored Windows row must be valid even when the requested output is Linux.
    let report = omega(&["--check", "--target", "linux_x86_64", "build.omg"], &root);
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        report.contains(
            "layout policy for `Overlapping` on `windows_x86_64`: layout placements of field keys 1 and 2 overlap"
        ),
        "{report}"
    );
}

/// Saves `module` and reopens the saved bytes. Encoding trusts its producer;
/// the reader is the boundary that validates a program it did not produce,
/// so an altered table has to be refused there.
fn reopen(
    module: terminal_psi::TerminalModule,
) -> Result<terminal_codec::ClosedTerminalModule, terminal_codec::CodecError> {
    let saved = terminal_codec::ClosedTerminalModule::new(module)?;
    terminal_codec::ClosedTerminalModule::from_semantic_bytes(saved.semantic_bytes())
}

#[test]
fn an_entryless_library_demands_only_the_selected_targets_layouts() {
    use platform_custody::temporary_directory::TemporaryDirectory;
    let root = TemporaryDirectory::create("entryless-layout-demand", false).unwrap();
    std::fs::write(
        root.path().join("build.omg"),
        r#"use main.omg as app;
machine build(builder: @mutable BuildPlan) {
        builder.product = BuildProduct::Package { name: "layout_reach" };
        Build::select_provider::<|app|::Reading, |app|::ConstantReading>(builder, alpha_bootstrap);
    }"#,
    )
    .unwrap();

    // Alpha has no C record geometry. Its selected provider can still return
    // a constant while an unrelated declaration in the same library mentions CLayout.
    for (formal, representation, refusal) in [
        ("", "|c_layout|::CLayout<Packet>", None),
        (
            "pair: @PacketPair",
            "|c_layout|::CLayout<Packet>",
            Some(
                "layout policy for `Packet` on `alpha_bootstrap`: target has no complete layout facts",
            ),
        ),
        (
            "",
            "|layout|::LayoutStorage<Packet, |c_layout|::CAbiPolicy<OtherPacket>>",
            Some("does not supply the declared `FixedLayout` application of `LayoutStorage`"),
        ),
    ] {
        let source = format!(
            r#"use {{omega}}/language/core/c_layout.omg;
use {{omega}}/language/core/layout.omg;
            public data Packet [copy] {{ a: i64; b: i64; }}
            public data OtherPacket [copy] {{ a: i64; b: i64; }}
            public data PacketPair = [{representation}; 2];
            public machine accepts_pair(pair: @PacketPair) reaches none crashes none {{}}
            public boundary trait Reading {{
                machine value({formal}) -> i32 reaches none crashes none;
            }}
            public ConstantReading satisfies Reading {{
                machine value({formal}) -> i32 reaches none crashes none {{ 7 }}
            }}"#
        );
        std::fs::write(root.path().join("main.omg"), source).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_omega"))
            .args([
                "--emit-psi",
                "library.psi",
                "--target",
                "linux_x86_64",
                "build.omg",
            ])
            .current_dir(root.path())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.success(), refusal.is_none(), "{stderr}");
        if let Some(reason) = refusal {
            assert!(stderr.contains(reason), "{stderr}");
            assert!(!stderr.contains("defect of the checker"), "{stderr}");
        } else {
            let bytes = std::fs::read(root.path().join("library.psi")).unwrap();
            let saved = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).unwrap();
            let module = saved.semantic_module();
            let [pending] = module.layouts.target_layout_applications.as_slice() else {
                panic!("one unapplied CLayout for Packet");
            };
            assert!(pending.target_inputs.is_empty());
            assert!(pending.realizations.is_empty());
            assert_ne!(pending.record, pending.schema);
            let record = module.structural_type(pending.record).unwrap();
            assert!(
                matches!(record.shape, terminal_psi::StructuralTypeShape::Record {
                storage: terminal_psi::StructuralRecordStorage::TargetLayout(id), ..
            } if id == pending.id)
            );

            // The original publication reopens; each altered copy below differs
            // from it in one table only.
            reopen(module.clone()).expect("the published library reopens");

            let mut wrong_policy = module.clone();
            assert_ne!(pending.policy_identity, pending.plan_identity);
            wrong_policy.layouts.target_layout_applications[0].policy_identity =
                pending.plan_identity;
            let error = reopen(wrong_policy).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("native layout changed its selected semantic policy"),
                "{error}"
            );

            // Even without target placements, the logical fields must match
            // their source schema. Reopening a mismatched carrier must fail.
            let mut altered = module.clone();
            let schema = altered
                .types
                .structural_types
                .iter_mut()
                .find(|declaration| declaration.id == pending.schema)
                .unwrap();
            let terminal_psi::StructuralTypeShape::Record { fields, .. } = &mut schema.shape else {
                panic!("Packet has a record schema");
            };
            fields[0].field_type =
                terminal_psi::StructuralFieldType::Scalar(semantic_vocabulary::ScalarType::Boolean);
            let error = reopen(altered).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("layout record differs from its semantic field schema"),
                "{error}"
            );
        }
    }
}

#[test]
fn an_unreached_layout_constant_keeps_its_initializer_without_target_geometry() {
    use platform_custody::temporary_directory::TemporaryDirectory;
    use terminal_psi::{LayoutMeasurement, ScalarApplication};
    let root = TemporaryDirectory::create("entryless-layout-constant", false).unwrap();
    std::fs::write(
        root.path().join("build.omg"),
        r#"machine build(builder: @mutable BuildPlan) {
        builder.product = BuildProduct::Package { name: "observed_packet" };
    }"#,
    )
    .unwrap();
    std::fs::write(
        root.path().join("main.omg"),
        r#"use {omega}/language/core/c_layout.omg;
        public data Packet [copy] { first: u8; second: u64; last: u16; }
        public const BYTES: u64 = size_of::<|c_layout|::CLayout<Packet>>();
        public machine packet_bytes() -> u64 reaches none crashes none { BYTES }
        machine extent() -> u64 reaches none crashes none { size_of::<|c_layout|::CLayout<Packet>>() }
        public data Envelope [copy] { bytes: [u8; extent()]; }
        public machine read(packet: @Envelope) -> u64 reaches none crashes none { 0u64 }"#,
    )
    .unwrap();
    let report = omega(
        &[
            "--emit-psi",
            "library.psi",
            "--target",
            "linux_x86_64",
            "build.omg",
        ],
        root.path(),
    );
    let bytes =
        std::fs::read(root.path().join("library.psi")).unwrap_or_else(|_| panic!("{report}"));
    let saved = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes).unwrap();
    let module = saved.semantic_module();
    let [pending] = module.layouts.target_layout_applications.as_slice() else {
        panic!("one authored layout for Packet");
    };
    assert!(pending.target_inputs.is_empty() && pending.realizations.is_empty());
    let applications = &module.scalars.scalar_applications;
    assert!(module.operations().any(|operation| matches!(operation.kind,
        terminal_psi::OperationKind::ReadScalarApplication { application }
            if matches!(applications.declaration(application), Some(ScalarApplication::Evaluate { .. })))));
    assert!(module.types.structural_types.iter().any(|record| matches!(
        &record.shape,
        terminal_psi::StructuralTypeShape::FixedArray { length: terminal_psi::ArrayExtent::Application(extent), .. }
            if matches!(applications.declaration(*extent), Some(ScalarApplication::Evaluate { .. }))
    )), "the array's own extent retains its checked evaluation");
    let recipes = applications.declarations();
    assert!(
        recipes
            .iter()
            .any(|row| matches!(row, ScalarApplication::Evaluate { .. }))
    );
    assert!(recipes.iter().any(|row| matches!(row,
        ScalarApplication::LayoutObservation { subject, measurement: LayoutMeasurement::Size, .. }
            if *subject == pending.record)));
}
