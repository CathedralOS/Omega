use super::*;
use package_compilation::{
    PackageDependencyBinding, PackageGeneratedSourceBundle, PackageSourceBinding,
    PackageSourceConsumptionCommitment,
};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

fn identity(marker: u8) -> semantic_vocabulary::PackageKeyIdentity {
    semantic_vocabulary::PackageKeyIdentity::from_digest([marker; 32])
        .expect("nonzero package identity")
}

struct Fixture {
    root: PathBuf,
    main: PathBuf,
    dependency: PathBuf,
    inputs: PackageCompilationInputs,
}

impl Fixture {
    fn new(with_physical_generated_source: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "omega-source-checkpoint-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        let application = root.join("application");
        let dependency = root.join("dependency");
        fs::create_dir_all(&application).expect("create checkpoint application root");
        fs::create_dir_all(&dependency).expect("create checkpoint dependency root");
        let main = application.join("main.omg");
        fs::write(
            &main,
            "use dependency::generated;\nmachine root_value() -> u64 { generated_value() }\n",
        )
        .expect("write checkpoint root source");
        fs::write(
            application.join("build.omg"),
            r#"machine build(builder: &mut Build) {
    builder.package("checkpoint-root");
}
"#,
        )
        .expect("write checkpoint build source");
        if with_physical_generated_source {
            fs::write(
                dependency.join("generated.omg"),
                "pub machine generated_value() -> u64 { 1 }\n",
            )
            .expect("write colliding physical dependency source");
        }
        let inputs = PackageCompilationInputs::new_package(
            identity(1),
            vec![
                PackageSourceBinding::new(identity(1), "checkpoint-root", application),
                PackageSourceBinding::new(identity(2), "checkpoint-dependency", dependency.clone()),
            ],
            vec![PackageDependencyBinding::new(
                identity(1),
                "dependency",
                identity(2),
            )],
        )
        .expect("checkpoint package graph should close");
        Self {
            root,
            main,
            dependency,
            inputs,
        }
    }

    fn child_inputs(
        &self,
        target: target::TargetProfile,
        generated_source: &[u8],
    ) -> PackageCompilationInputs {
        let tree = build_output::replayed_single_ordinary_file(b"generated.omg", generated_source)
            .expect("generated checkpoint source should form a retained output tree");
        let generated = build_output::select_included_sources(&tree, &[b"generated.omg".to_vec()])
            .expect("generated checkpoint source should be selected");
        let bundle = PackageGeneratedSourceBundle::from_checked(
            identity(2),
            target,
            self.inputs.dependency_closure_for(identity(2)),
            PackageSourceConsumptionCommitment::for_test([3; 32]),
            generated,
        );
        self.inputs
            .clone()
            .with_complete_dependency_generated_sources(vec![bundle])
            .expect("exact checkpoint child should retain one dependency bundle")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn separately_prepared_exact_children_have_identical_source_assembly() {
    let fixture = Fixture::new(false);
    let inputs = fixture.child_inputs(
        target::TargetProfile::WindowsX64,
        b"pub machine generated_value() -> u64 { 7 }\n",
    );
    let mut checkpoint_timings = CompileTimings::default();
    let checkpoint = ImmutableSourceParseCheckpoint::prepare(
        &fixture.main,
        Some(&fixture.inputs),
        &mut checkpoint_timings,
    )
    .expect("prepare immutable source checkpoint");
    let (child_count, child) = checkpoint
        .for_exact_target("windows_x86_64", Some(&inputs))
        .expect("exact child source inputs should match")
        .assemble(&mut checkpoint_timings)
        .expect("assemble exact source child");

    let mut direct_timings = CompileTimings::default();
    let direct_checkpoint =
        ImmutableSourceParseCheckpoint::prepare(&fixture.main, Some(&inputs), &mut direct_timings)
            .expect("prepare independent exact source checkpoint");
    let (direct_count, direct) = direct_checkpoint
        .for_exact_target("windows_x86_64", Some(&inputs))
        .expect("independent exact child source inputs should match")
        .assemble(&mut direct_timings)
        .expect("independent exact source child should assemble");
    assert_eq!(child_count, direct_count);
    assert_eq!(child.syntax_trees, direct.syntax_trees);
    assert_eq!(child.sources, direct.sources);
    assert_eq!(child.build_source_id, direct.build_source_id);
    assert_eq!(
        child.source_scoped_top_level_bindings,
        direct.source_scoped_top_level_bindings,
    );
    assert_eq!(
        child.generated_source_custody,
        direct.generated_source_custody,
    );
}

#[test]
fn failing_target_child_does_not_change_a_successful_sibling() {
    let fixture = Fixture::new(false);
    let windows = fixture.child_inputs(
        target::TargetProfile::WindowsX64,
        b"pub machine generated_value() -> u64 { 7 }\n",
    );
    let linux = fixture.child_inputs(
        target::TargetProfile::LinuxX64,
        b"pub machine generated_value( {\n",
    );
    let mut timings = CompileTimings::default();
    let checkpoint =
        ImmutableSourceParseCheckpoint::prepare(&fixture.main, Some(&fixture.inputs), &mut timings)
            .expect("prepare immutable source checkpoint");
    let (_, before) = checkpoint
        .for_exact_target("windows_x86_64", Some(&windows))
        .expect("windows child should match checkpoint")
        .assemble(&mut timings)
        .expect("windows child should parse");
    let linux_result = checkpoint
        .for_exact_target("linux_x86_64", Some(&linux))
        .expect("linux child source inputs should match checkpoint")
        .assemble(&mut timings);
    assert!(
        linux_result.is_err(),
        "malformed linux generated source must reject only that child",
    );
    let (_, after) = checkpoint
        .for_exact_target("windows_x86_64", Some(&windows))
        .expect("windows child should still match checkpoint")
        .assemble(&mut timings)
        .expect("windows sibling must remain stable");
    assert_eq!(before.syntax_trees, after.syntax_trees);
    assert_eq!(before.sources, after.sources);
    assert_eq!(
        before.generated_source_custody,
        after.generated_source_custody,
    );
}

#[test]
fn exact_child_rejects_source_input_and_generated_physical_substitution() {
    let fixture = Fixture::new(true);
    let mut timings = CompileTimings::default();
    let checkpoint =
        ImmutableSourceParseCheckpoint::prepare(&fixture.main, Some(&fixture.inputs), &mut timings)
            .expect("prepare immutable source checkpoint");

    let changed_inputs = PackageCompilationInputs::new_package(
        fixture.inputs.root(),
        vec![
            PackageSourceBinding::new(
                fixture.inputs.root(),
                "renamed-checkpoint-root",
                fixture
                    .main
                    .parent()
                    .expect("checkpoint main has a package root")
                    .to_path_buf(),
            ),
            PackageSourceBinding::new(
                identity(2),
                "checkpoint-dependency",
                fixture.dependency.clone(),
            ),
        ],
        vec![PackageDependencyBinding::new(
            fixture.inputs.root(),
            "dependency",
            identity(2),
        )],
    )
    .expect("changed graph should remain internally valid");
    let mismatch = checkpoint
        .for_exact_target("windows_x86_64", Some(&changed_inputs))
        .err()
        .expect("source-input substitution must reject before child assembly");
    assert!(mismatch[0].message.contains("do not match"));

    let colliding = fixture.child_inputs(
        target::TargetProfile::WindowsX64,
        b"pub machine generated_value() -> u64 { 7 }\n",
    );
    let collision_result = checkpoint
        .for_exact_target("windows_x86_64", Some(&colliding))
        .expect("colliding child still matches source inputs")
        .assemble(&mut timings);
    let Err(collision) = collision_result else {
        panic!("generated source must not replace a physical source")
    };
    assert!(collision[0].message.contains("collides with physical"));
}
