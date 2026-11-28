//! An embedding host explicitly supplies a checked leaf package to an edited application.
//! Both invocations resolve source, execute Build and admit the receiving native Product.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

#[path = "support/project.rs"]
mod project;

use build_execution::{ComputedBuildConfig, DependencyValidationSubject};
use compiler::{Artifact, CompileRequest, ComponentSourcePublication, ComponentSuppliers, Program};
use diagnostics::Diagnostic;
use package_compilation::{PackageBuildHandoff, PackageDependencyAcquisition};
use project_build::PlannedProduct;
use psi_authorization::{HostProducerTrust, authorize_terminal_artifact};
use semantic_vocabulary::PackageKeyIdentity;
use sha2::{Digest, Sha256};
use std::{
    cell::RefCell, collections::BTreeMap, fs, path::Path, process::Command, rc::Rc, sync::Arc,
    time::Instant,
};
use target::TargetProfile;
use terminal_codec::CanonicalTerminalArtifact;

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

const TARGET: TargetProfile = TargetProfile::LinuxX64;

// This deliberately handles only a leaf explicitly validated by its receiving Build.
// The resolver supplies the immutable revision in the validation subject's package inputs.
// Build still executes for each occurrence. The caller has no network, program or file grants.
struct SupplierInputs {
    validated: DependencyValidationSubject,
    request: PackageDependencyAcquisition,
    handoff: PackageBuildHandoff,
    library: String,
    receiving_build: Arc<ComputedBuildConfig>,
}

impl SupplierInputs {
    fn receive(product: &PlannedProduct) -> Self {
        let inputs = product.package_inputs().unwrap();
        assert!(
            inputs.accepted_semantic_bindings().next().is_none(),
            "fixture has no accepted bindings"
        );
        let [validated] = product.evaluated().dependency_validation.as_slice() else {
            panic!("expected one dependency explicitly validated by the receiver")
        };
        let [request] = inputs.dependency_acquisitions() else {
            panic!("expected one acquired dependency")
        };
        let mut handoffs = inputs.dependency_build_handoff_instances();
        let (purpose, handoff) = handoffs.next().expect("a checked dependency handoff");
        assert!(handoffs.next().is_none());
        assert!(purpose.is_product() && validated.purpose == purpose);
        assert_eq!(request.package, validated.inputs.root());
        assert_eq!(handoff.package(), request.package);
        assert_eq!(handoff.dependency_closure().packages(), &[request.package]);
        assert_eq!(validated.inputs.packages().count(), 1);
        Self {
            validated: validated.clone(),
            request: request.clone(),
            handoff: handoff.clone(),
            library: package_compilation::toolchain_library::identity().unwrap(),
            receiving_build: product.evaluated().configuration.clone(),
        }
    }

    fn agrees(&self, current: &Self) -> bool {
        self.validated.purpose == current.validated.purpose
            && self.validated.inputs.identity_digest() == current.validated.inputs.identity_digest()
            && self
                .validated
                .configuration
                .same_product_and_observations(&current.validated.configuration)
            && self.request == current.request
            && self.handoff == current.handoff
            && self.library == current.library
            && self
                .receiving_build
                .config
                .settings
                .proof_index_expansion_terms
                == current
                    .receiving_build
                    .config
                    .settings
                    .proof_index_expansion_terms
            && self.receiving_build.product_selection_requests
                == current.receiving_build.product_selection_requests
            && self.receiving_build.declared_product_selections
                == current.receiving_build.declared_product_selections
    }
}

// The caller owns the exact producer-issued artifact and its authorization. Native compilation
// still receives the edited root, performs provider verification and enforces receiver policy.
struct DeclaredSupplier<'a> {
    producer: &'a SupplierInputs,
    receiver: &'a SupplierInputs,
    path: &'a Path,
    expected_bytes: [u8; 32],
    publication: RefCell<Option<ComponentSourcePublication>>,
}

impl ComponentSuppliers for DeclaredSupplier<'_> {
    fn publish(
        &self,
        package: PackageKeyIdentity,
        target: TargetProfile,
    ) -> Result<ComponentSourcePublication, Vec<Diagnostic>> {
        if package != self.producer.request.package
            || target != self.producer.handoff.target()
            || !self.producer.agrees(self.receiver)
        {
            return Err(vec![Diagnostic::error(
                "the declared supplier does not match this package occurrence",
            )]);
        }
        let refuse = |reason: String| vec![Diagnostic::error(reason)];
        let bytes = fs::read(self.path).map_err(|e| refuse(e.to_string()))?;
        if <[u8; 32]>::from(Sha256::digest(&bytes)) != self.expected_bytes {
            return Err(refuse("different bytes from the declared producer".into()));
        }
        let received = Arc::new(
            CanonicalTerminalArtifact::from_bytes(&bytes).map_err(|e| refuse(e.to_string()))?,
        );
        let mut slot = self.publication.borrow_mut();
        let original = slot
            .as_ref()
            .ok_or_else(|| refuse("the declared supplier was already consumed".into()))?;
        original
            .authorization
            .require_artifact(&received)
            .map_err(|e| refuse(e.to_string()))?;
        // The host saw this compiler produce these exact bytes. Neither a package
        // declaration nor the decoded manifest supplied the host's pin.
        let trust = HostProducerTrust::accept("host's explicitly exported local compiler product");
        let authorization = authorize_terminal_artifact(received, Some(&trust))
            .map_err(|e| refuse(e.to_string()))?;
        let mut published = slot.take().unwrap();
        published.authorization = authorization;
        Ok(published)
    }
}

fn open(root: &Path, output: &Path) -> Rc<PlannedProduct> {
    package_manager::project::open_product(root, TARGET, true, output, None).unwrap()
}

fn build(root: &Path, output: &Path, product: &PlannedProduct, supplied: &dyn ComponentSuppliers) {
    let stage = product.product_stage(&[TARGET]).unwrap();
    let Program::Source {
        source,
        target,
        runtime_consent,
        ..
    } = stage.program(TARGET, vec![]).unwrap()
    else {
        panic!("expected the receiving project's source")
    };
    let program = Program::Source {
        source,
        target,
        runtime_consent,
        suppliers: supplied,
    };
    let compiled = compiler::compile(
        CompileRequest::Native {
            program,
            entry_memory: &[],
        },
        Default::default(),
    )
    .unwrap();
    let Artifact::Native(native) = compiled.artifact else {
        panic!("expected native application")
    };
    let published = native.publish(root, output).unwrap();
    let exe = published.checked_native_executable_path().unwrap();
    published.validate_component_process_inputs(exe).unwrap();
    let run = Command::new(exe)
        .current_dir(output)
        .env_remove("LD_LIBRARY_PATH")
        .output()
        .unwrap();
    assert!(
        run.status.success() && run.stdout.is_empty() && run.stderr.is_empty(),
        "{run:?}"
    );
}

fn images(output: &Path) -> BTreeMap<std::ffi::OsString, [u8; 32]> {
    let mut result = BTreeMap::new();
    for entry in fs::read_dir(output).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file()
            && (path.extension().is_some_and(|ext| ext == "so")
                || path.file_name().is_some_and(|name| name == "omega-program"))
        {
            result.insert(
                path.file_name().unwrap().to_owned(),
                Sha256::digest(fs::read(path).unwrap()).into(),
            );
        }
    }
    assert_eq!(
        result.len(),
        2,
        "expected the executable and its one supplier"
    );
    result
}

#[test]
fn explicit_leaf_publication_serves_an_edited_receiver() {
    worker_budget::ExecutionBudget::for_host()
        .unwrap()
        .run(exercise);
}

fn exercise() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    package_compilation::toolchain_library::configure(&repo.join("source/library")).unwrap();
    let fixture = repo.join("tests/fixtures/synchronization/run/independent_scalar_results");
    let source = |name| fs::read_to_string(fixture.join(name)).unwrap();
    let supplier = source("supplier/main.omg");
    let receiver = source("application/build.omg")
        .replace("_ = |dependency|::add(build, dependency);", "let registered: DependencyRegistration = Build::dependency(build, dependency);\n    Build::validate(build, @registered);");
    let app = |value| {
        format!(
            r#"use {{signals}}/main.omg as signals;
machine main() reaches |signals|::Samples crashes Trap {{
    let value: u32 = |signals|::Samples::read({value}u32);
    transition value == 0u32 {{ true -> done(), false -> bad() }}
    machine done() {{}}
    machine bad() {{ crash Trap; }}
}}"#
        )
    };
    let dir = project::stage(
        "explicit-supplier",
        &[
            ("supplier/build.omg", &source("supplier/build.omg")),
            ("supplier/main.omg", &supplier),
            ("application/build.omg", &receiver),
            ("application/main.omg", &app(3)),
        ],
    );
    let root = dir.path().join("application/build.omg");
    let first = dir.path().join("first");
    let start = Instant::now();
    let planned = open(&root, &first);
    let produce = Instant::now();
    let original = SupplierInputs::receive(&planned);
    let publication = planned.publish(original.request.package, TARGET).unwrap();
    let artifact_path = dir.path().join("declared-supplier.psi");
    let bytes = publication.authorization.artifact().to_bytes();
    let expected_bytes = Sha256::digest(&bytes).into();
    fs::write(&artifact_path, &bytes).unwrap();
    eprintln!(
        "supplier_manifest={} supplier_bytes={}",
        publication.authorization.identity(),
        bytes.len()
    );
    let prepare_us = produce.elapsed().as_micros();
    // This deliberately pays for a separate explicit publication on the producing invocation.
    build(&root, &first, &planned, &*planned);
    eprintln!(
        "first_build_and_run_us={} explicit_publication_us={prepare_us}",
        start.elapsed().as_micros()
    );
    drop(planned);
    fs::write(root.with_file_name("main.omg"), app(8)).unwrap();
    let second = dir.path().join("second");
    let start = Instant::now();
    let edited = open(&root, &second);
    let current = SupplierInputs::receive(&edited);
    let supplied = DeclaredSupplier {
        producer: &original,
        receiver: &current,
        path: &artifact_path,
        expected_bytes,
        publication: RefCell::new(Some(publication)),
    };
    let planned_receiver_time = start.elapsed();
    assert!(
        supplied
            .publish(original.request.package, TargetProfile::LinuxArm64)
            .is_err()
    );
    fs::write(&artifact_path, b"different from the declared publication").unwrap();
    let refusal = supplied
        .publish(original.request.package, TARGET)
        .err()
        .expect("changed bytes must be refused");
    assert_eq!(
        refusal[0].message,
        "different bytes from the declared producer"
    );
    assert!(supplied.publication.borrow().is_some());
    fs::write(&artifact_path, &bytes).unwrap();
    let native_start = Instant::now();
    build(&root, &second, &edited, &supplied);
    assert!(supplied.publication.borrow().is_none());
    eprintln!(
        "edited_build_and_run_us={} supplier_publications_reused=1",
        (planned_receiver_time + native_start.elapsed()).as_micros()
    );
    let control_output = dir.path().join("edited-embedded-source");
    let start = Instant::now();
    let control_product = open(&root, &control_output);
    build(&root, &control_output, &control_product, &*control_product);
    eprintln!(
        "edited_embedded_source_build_and_run_us={}",
        start.elapsed().as_micros()
    );
    assert_eq!(images(&second), images(&control_output));
    drop(control_product);
    let control = dir.path().join("edited-cli-control");
    let start = Instant::now();
    let compiled = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["--target", "linux_x86_64", "--offline", "--build-dir"])
        .arg(&control)
        .arg(&root)
        .output()
        .unwrap();
    assert!(compiled.status.success(), "{compiled:?}");
    let cli_run = Command::new(control.join("omega-program"))
        .current_dir(&control)
        .env_remove("LD_LIBRARY_PATH")
        .output()
        .unwrap();
    assert!(
        cli_run.status.success() && cli_run.stdout.is_empty() && cli_run.stderr.is_empty(),
        "{cli_run:?}"
    );
    eprintln!(
        "edited_cli_control_build_and_run_us={}",
        start.elapsed().as_micros()
    );
    assert_eq!(
        images(&second),
        images(&control),
        "explicit input changed native output"
    );
    fs::write(
        dir.path().join("supplier/main.omg"),
        supplier.replace("0u32 }", "2u32 }"),
    )
    .unwrap();
    let different = open(&root, &dir.path().join("changed"));
    assert!(
        !original.agrees(&SupplierInputs::receive(&different)),
        "different immutable source resolution"
    );

    fs::write(dir.path().join("supplier/main.omg"), &supplier).unwrap();
    // A different exact local revision can contain identical imported Omega declarations.
    let revision_note = dir.path().join("supplier/REVISION.txt");
    fs::write(&revision_note, "new package revision").unwrap();
    let revised = open(&root, &dir.path().join("other-revision"));
    let revised = SupplierInputs::receive(&revised);
    assert_ne!(
        original.validated.inputs.identity_digest(),
        revised.validated.inputs.identity_digest()
    );
    assert!(!original.agrees(&revised), "changed exact local resolution");
    fs::remove_file(revision_note).unwrap();
    let other_provider = receiver.replace("::SourceSamples>", "::ForwardSample>");
    assert_ne!(other_provider, receiver);
    fs::write(&root, other_provider).unwrap();
    let selected = open(&root, &dir.path().join("other-provider"));
    assert!(
        !original.agrees(&SupplierInputs::receive(&selected)),
        "changed selected provider"
    );

    let granted = receiver.replace("use main.omg as app;",
        "use main.omg as app;\nuse {omega}/language/core/filesystem_authority.omg as auth;")
        .replace("let registered: DependencyRegistration = Build::dependency(build, dependency);",
        "let allowed: [BoundaryServiceReference; 1] = [Build::service::<|auth|::FilesystemContentRead>(build)];\n    let registered: DependencyRegistration = |dependency|::add(build, dependency, @allowed[..]);");
    fs::write(&root, granted).unwrap();
    let changed_grant = open(&root, &dir.path().join("other-grant"));
    let changed_grant = SupplierInputs::receive(&changed_grant);
    assert_ne!(original.request.request, changed_grant.request.request);
    assert!(!original.agrees(&changed_grant), "changed dependency grant");

    fs::write(&root, receiver).unwrap();
    fs::write(
        root.with_file_name("main.omg"),
        app(8).replace("{signals}/main.omg", "{signals}/absent.omg"),
    )
    .unwrap();
    let refused = Command::new(env!("CARGO_BIN_EXE_omega"))
        .args(["--target", "linux_x86_64", "--build-dir"])
        .arg(dir.path().join("missing-import"))
        .arg(&root)
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("absent.omg"),
        "{refused:?}"
    );
    // Exercise the public locked route separately. The explicit artifact supplies
    // no review decision when the project has an out-of-date package acceptance.
    fs::write(root.with_file_name("main.omg"), app(8)).unwrap();
    let project_dir = root.parent().unwrap();
    let update = project::omega(
        project_dir,
        &[
            "packages",
            "update",
            "--offline",
            "--target",
            "linux_x86_64",
        ],
    );
    assert!(
        update.status.success() && project_dir.join("omega.lock").is_file(),
        "{update:?}"
    );
    let check = || {
        project::omega(
            project_dir,
            &[
                "--check",
                "--offline",
                "--target",
                "linux_x86_64",
                "build.omg",
            ],
        )
    };
    let accepted = check();
    assert!(accepted.status.success(), "{accepted:?}");
    let lock = fs::read(project_dir.join("omega.lock")).unwrap();
    fs::write(
        dir.path().join("supplier/REVISION.txt"),
        "changed after acceptance",
    )
    .unwrap();
    let stale = check();
    let why = String::from_utf8_lossy(&stale.stderr);
    assert!(
        !stale.status.success() && why.contains("signals") && why.contains("omega packages update"),
        "{stale:?}"
    );
    assert_eq!(fs::read(project_dir.join("omega.lock")).unwrap(), lock);
}
