//! A Build script cannot make, keep or take apart the compiler-owned `Build`
//! value. The toolchain hands the script one `&mut Build` and afterwards
//! decodes what the script left in it, so a `Build` that source produced
//! itself would be read as if the toolchain had issued it.
//!
//! The rule is in `Build`'s declaration, not in the checker: its `seal`
//! field is `BuildSeal in Issued`, `BuildSeal` is not `[copy]`, and `Issued`
//! is governed by an access policy that names no source issuer
//! (`wiki/spec/resources/access.md`, "Plans and admitted supply"). These
//! tests compile a real project, with the toolchain's own Build prelude, and
//! change one machine of its Build script at a time.

// Compilation shares process-wide counters; run one at a time.
static COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The Build script every program here shares: it only uses the `Build` it
/// was lent.
const BUILD: &str = "\
machine build(builder: &mut Build) {
    Build::application(builder, \"sealed\");
    Build::bind(builder, linux_x86_64::ProgramEntry, main);
}
";

/// The messages `omega --check` would print for a project whose Build script
/// is `BUILD` followed by `machines`; empty when the project compiles.
fn refusals(name: &str, machines: &str) -> Vec<String> {
    let _compilation = COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root =
        std::env::temp_dir().join(format!("omega-build-value-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("staging directory");
    let main = root.join("main.omg");
    std::fs::write(&main, "machine main() { }\n").expect("staged program");
    std::fs::write(root.join("build.omg"), format!("{BUILD}{machines}")).expect("staged Build");
    let mut session =
        orchestration::BuildSession::create(&root.join("out")).expect("build session");
    let preparation = package_manager::operations::begin_local_project_preparation(
        &main,
        &target::TargetProfile::ALL,
        true,
    )
    .expect("project preparation")
    .expect("a project with a Build script");
    // What `omega --check` does first: type the project's Build program,
    // have the checker judge it, and run it.
    let resolved = orchestration::resolve_project_sources(
        &mut session,
        orchestration::ProjectSourceRequest {
            preparation,
            execution: Default::default(),
            collect_timings: false,
        },
    );
    let refusals = match resolved {
        Ok(_) => Vec::new(),
        Err(orchestration::ProjectSourceResolutionError::Compilation(error)) => error
            .into_diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect(),
        Err(other) => vec![other.to_string()],
    };
    drop(session);
    let _ = std::fs::remove_dir_all(&root);
    refusals
}
/// The twin is stopped, for `expected`, before its Build script runs.
fn refused_with(name: &str, machines: &str, expected: &str) {
    let refusals = refusals(name, machines);
    assert!(
        refusals.iter().any(|refusal| refusal.contains(expected)),
        "{name}: {refusals:#?}"
    );
}

/// The control: the shared Build script is accepted and runs, so every
/// refusal below belongs to the machine its twin adds.
#[test]
fn a_build_script_that_only_uses_the_build_it_was_lent_runs() {
    assert_eq!(refusals("control", ""), Vec::<String>::new());
}
/// Every field written out, the seal as a literal. A literal constructs the
/// bare `BuildSeal`, which is not a `BuildSeal in Issued`.
#[test]
fn constructing_a_build_is_not_accepted() {
    refused_with(
        "constructed",
        "machine wrong() {
    let optimizations: Optimizations;
    let forged: Build = Build {
        seal: BuildSeal { },
        x86_deployment_features: X86DeploymentFeatures::Baseline,
        subsystem: Subsystem::Console,
        freestanding: false,
        identifier: \"\",
        elf_interpreter: ElfInterpreterSelection { linux_arm64: \"\", linux_x86_64: \"\" },
        optimizations: optimizations,
        privileged_services: PrivilegedServices { port_io: false, interrupt_table: false },
        source: BuildSource { },
        inputs: BuildInputs { },
        output: BuildOutput { },
        log: BuildLog { },
        product: BuildProduct { },
        tests: BuildTests { disabled: false }
    };
}
",
        "typing record literal field type",
    );
}

/// Zero is not a seal the toolchain issued.
#[test]
fn zero_establishing_a_build_is_refused_by_name() {
    refused_with(
        "zeroed",
        "machine wrong() -> bool {\n    let forged: Build;\n    forged.freestanding\n}\n",
        "zero-established value in a domain that must be established",
    );
}

/// The seal cannot be made from its carrier: nobody in source is an issuer.
#[test]
fn issuing_a_seal_is_refused_by_name() {
    refused_with(
        "issued",
        "machine wrong(seal: BuildSeal) -> BuildSeal in Issued {\n    seal as +Issued\n}\n",
        "cast that adds a domain whose issuers are in an access plan",
    );
}

/// `BuildSeal` is not `[copy]`, and the script only ever holds a loan of
/// the `Build` that owns it.
#[test]
fn taking_the_seal_out_of_the_lent_build_is_not_accepted() {
    refused_with(
        "moved_out",
        "machine wrong(builder: &mut Build) -> BuildSeal in Issued {\n    builder.seal\n}\n",
        "owned extraction from borrowed storage",
    );
    refused_with(
        "copied_out",
        "machine wrong(builder: &Build) {\n    let seal: BuildSeal in Issued = builder.seal;\n}\n",
        "owned extraction from borrowed storage",
    );
}

#[test]
fn replacing_the_seal_is_not_accepted() {
    refused_with(
        "replaced",
        "machine wrong(builder: &mut Build) {\n    builder.seal = BuildSeal { };\n}\n",
        "replacement of a value governed by an access policy",
    );
}

/// A machine that takes the plain carrier would be free to look inside it.
#[test]
fn lending_the_seal_as_its_plain_carrier_is_not_accepted() {
    refused_with(
        "lent",
        "machine peek(seal: &BuildSeal) { }
machine wrong(builder: &Build) {
    peek(&builder.seal)
}
",
        "typing argument types",
    );
}

/// The script holds a loan, so a `Build` to keep or to hand back would have
/// to be taken out of it.
#[test]
fn storing_or_returning_the_lent_build_is_not_accepted() {
    refused_with(
        "stored",
        "data Kept<'storage> {
    build: Build<'storage>;
}
machine wrong(builder: &mut Build) {
    let kept: Kept = Kept { build: builder };
}
",
        "owned extraction from borrowed storage",
    );
    refused_with(
        "returned",
        "machine wrong(builder: &mut Build) -> Build {\n    builder\n}\n",
        "owned extraction from borrowed storage",
    );
}
