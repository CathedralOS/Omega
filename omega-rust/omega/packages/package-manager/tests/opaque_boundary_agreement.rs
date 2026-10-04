//! The consumer's Build selects representations for its combined source
//! compilation; a producer's historical review selection is not that authority.
//! Direct checked compilation must reject invalid demands without constructing
//! package reviews. `Calling<...>` comes from the real
//! standard library because it creates an actual demand row; a plain boundary
//! trait does not.

use build_declarations::DependencyPurpose;
use compiler::{CheckedCompileRequest, compile_to_checked};
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use package_evidence::record::PackageReviewNominalOwner;
use package_manager::resolution::graph::GitResolutionOptions;
use package_manager::resolution::graph::{
    PackageSourceClosureLimits, ResolvedPackageSourceClosure,
    resolve_external_local_project_closure,
};
use package_manager::review::{SemanticBindingReview, compile_resolved_package_reviews};
use package_source::PrimaryGitChoices;
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolver};
use semantic_vocabulary::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use target::TargetProfile;

const TARGET: TargetProfile = TargetProfile::WindowsX64;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Tree {
    root: PathBuf,
}

impl Tree {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "omega-opaque-boundary-agreement-{}-{nanos}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("create temporary fixture root");
        Self { root }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn storage(&self, name: &str) -> SourceResolver {
        SourceResolver::for_hardened_base(self.path(name), PrimaryGitChoices::default())
            .expect("create source resolver storage")
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

const PRODUCER_TYPES: &str = r#"use omega::language::core::representation;
pub boundary data Token;
pub data Carrier { value: u64; }
pub TokenRepresentation: Carrier satisfies OpaqueRepresentation<Token>;
"#;

const CALLING_POLICY_SOURCE: &str = r#"use omega_language_std::calling;
pub data TransferPolicy { }
pub TransferPolicyCallingPolicy: TransferPolicy satisfies CallingPolicy;

machine TransferPolicy::plan(signature: BoundarySignature) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    transition signature.parameter_count == 1 {
        true -> build(signature, signature.parameters[0])
        _ -> reject()
    }

    state build(signature: BoundarySignature, root: u64) -> BoundaryPlanResult {
        transition root < 256 {
            true -> materialize(signature, root)
            _ -> reject()
        }
    }

    state materialize(signature: BoundarySignature, root: u64) -> BoundaryPlanResult {
        let mut output: BoundaryEntryPlan;
        output.call.convention = CallingConvention::MicrosoftX64;
        output.call.parameter_count = 1;
        output.call.parameters[0].shape.class = AbiValueClass::Integer;
        output.call.parameters[0].shape.byte_size = signature.shapes[root].byte_size;
        output.call.parameters[0].shape.alignment = signature.shapes[root].alignment;
        output.call.parameters[0].location_count = 1;
        output.call.parameters[0].locations[0] = ValueLocation::Register {
            register: MachineRegister::X86Rcx, value_byte_offset: 0,
            byte_size: signature.shapes[root].byte_size,
        };
        output.call.stack_alignment = 16;
        output.call.shadow_bytes = 32;
        output.call.entry_control = EntryControl::CallReturn;
        BoundaryPlanResult::Accepted { plan: output }
    }

    state reject() -> BoundaryPlanResult {
        BoundaryPlanResult::Rejected {
            reason: CallingPolicyRejection { reason: "invalid transfer signature" },
        }
    }
}

pub boundary trait TokenBoundary: Calling<TransferPolicy> {
    machine accept(token: Token);
}
"#;

fn omega_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// Every other test in this crate finds the repository by a marker rather than
/// by counting ancestors, because a fixed count silently survives a directory
/// move and then resolves somewhere outside the checkout. This one counted, and
/// after the Rust tree moved to the repository root its seventh ancestor was
/// the home directory, so the producer's build named a standard library that
/// does not exist.
fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|ancestor| {
            ancestor.join("Cargo.toml").is_file() && ancestor.join("source/omega").is_dir()
        })
        .expect("package-manager should live beneath the Omega repository")
        .to_path_buf()
}

fn producer_build(standard_library: &Path) -> String {
    format!(
        r#"machine build(builder: &mut Build) {{
    Build::package(builder, "opaque_producer");
    Build::depend(builder, Source::Path {{
        location: "{}"
    }});
    builder.select_representation<Token, TokenRepresentation>();
}}
"#,
        omega_path(standard_library)
    )
}

const CONSUMER_SOURCE: &str = r#"use producer::main;

pub data ConsumerProvider { }
ConsumerProviderTokenBoundary: ConsumerProvider satisfies TokenBoundary;

machine ConsumerProvider::accept(token: Token)
    satisfies TokenBoundary::accept
{
}

machine main() { }
"#;

const MATCHING_CONSUMER_BUILD: &str = r#"machine build(builder: &mut Build) {
    Build::application(builder, "opaque_consumer");
    Build::depend_as(builder, "producer", Source::Path {
        location: "../producer"
    });
    builder.select_representation<Token, TokenRepresentation>();
}
"#;

const MISMATCHING_CONSUMER_SOURCE: &str = r#"use omega::language::core::representation;
use producer::main;

pub data OtherCarrier { value: u64; other: u64; }
pub OtherRepresentation: OtherCarrier satisfies OpaqueRepresentation<Token>;

pub data ConsumerProvider { }
ConsumerProviderTokenBoundary: ConsumerProvider satisfies TokenBoundary;

machine ConsumerProvider::accept(token: Token)
    satisfies TokenBoundary::accept
{
}

machine main() { }
"#;

const MISMATCHING_CONSUMER_BUILD: &str = r#"machine build(builder: &mut Build) {
    Build::application(builder, "opaque_consumer");
    Build::depend_as(builder, "producer", Source::Path {
        location: "../producer"
    });
    builder.select_representation<Token, OtherRepresentation>();
}
"#;

fn fixture(
    tree: &Tree,
    consumer_source: &str,
    consumer_build: &str,
) -> ResolvedPackageSourceClosure {
    write_sources(
        tree,
        &format!("{PRODUCER_TYPES}\n{CALLING_POLICY_SOURCE}"),
        consumer_source,
        consumer_build,
    );
    let storage = tree.storage("closure-cache");
    resolve_external_local_project_closure(
        tree.path("sources/consumer"),
        ExternalSourceContext::derive(b"opaque-boundary-agreement"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
        GitResolutionOptions::default(),
    )
    .expect("resolve producer and consumer package closure")
}

fn write_sources(tree: &Tree, producer_source: &str, consumer_source: &str, consumer_build: &str) {
    let producer = tree.path("sources/producer");
    let consumer = tree.path("sources/consumer");
    fs::create_dir_all(&producer).expect("create producer package");
    fs::create_dir_all(&consumer).expect("create consumer package");
    fs::write(producer.join("main.omg"), producer_source).expect("write producer source");
    let standard_library = repository_root().join("source/library/std");
    fs::write(
        producer.join("build.omg"),
        producer_build(&standard_library),
    )
    .expect("write producer build");

    fs::write(consumer.join("main.omg"), consumer_source).expect("write consumer source");
    fs::write(consumer.join("build.omg"), consumer_build).expect("write consumer build");
}

#[test]
fn package_reviews_retain_foreign_opaque_demand_ownership() {
    let tree = Tree::new();
    let closure = fixture(&tree, CONSUMER_SOURCE, MATCHING_CONSUMER_BUILD);
    let result = compile_resolved_package_reviews(
        &closure.for_exact_target(TARGET),
        &tree.path("matching-build"),
        SemanticBindingReview::Discover,
    );
    let reviews = result.expect("matching producer and consumer representation should agree");
    let producer = reviews
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "opaque_producer")
        .expect("producer review");
    let consumer = reviews
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "opaque_consumer")
        .expect("consumer review");
    assert!(!consumer.policy().representation().demands().is_empty());
    let producer_identity = producer.key().identity();
    let producer_owned_uses = consumer
        .policy()
        .representation()
        .demands()
        .iter()
        .flat_map(|demand| demand.calling().opaque_uses())
        .filter(|use_| {
            matches!(
                use_.opaque().owner(),
                PackageReviewNominalOwner::Package(owner) if owner == producer_identity
            )
        })
        .collect::<Vec<_>>();
    let [opaque_use] = producer_owned_uses.as_slice() else {
        panic!("expected one producer-owned consumer opaque use");
    };
    assert_eq!(opaque_use.opaque().path(), "Token");
    assert_eq!(opaque_use.carrier().path(), "Carrier");
    assert_eq!(
        opaque_use.application().declaration().path(),
        "TokenRepresentation"
    );
    assert_eq!(
        opaque_use.carrier().owner(),
        PackageReviewNominalOwner::Package(producer_identity)
    );
    assert_eq!(
        opaque_use.application().declaration().owner(),
        PackageReviewNominalOwner::Package(producer_identity)
    );
    assert_eq!(
        producer
            .policy()
            .representation()
            .selected_availability()
            .len(),
        1
    );
}

#[test]
fn consumer_build_overrides_the_producers_historical_representation_choice() {
    let tree = Tree::new();
    let closure = fixture(
        &tree,
        MISMATCHING_CONSUMER_SOURCE,
        MISMATCHING_CONSUMER_BUILD,
    );
    let result = compile_resolved_package_reviews(
        &closure.for_exact_target(TARGET),
        &tree.path("mismatching-build"),
        SemanticBindingReview::Discover,
    );
    let reviews = result.expect("consumer Build owns the combined compilation's representation");
    let consumer = reviews
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "opaque_consumer")
        .expect("consumer review");
    let demands = consumer.policy().representation().demands();
    assert!(
        !demands.is_empty(),
        "the changed application has an actual demand"
    );
    assert!(
        demands
            .iter()
            .flat_map(|demand| demand.calling().opaque_uses())
            .any(
                |use_| use_.application().declaration().path() == "OtherRepresentation"
                    && use_.carrier().path() == "OtherCarrier"
            )
    );
}

/// Construct compiler inputs directly, without package resolution, review
/// reconstruction, or producer-policy agreement. Both packages retain exact
/// ownership so ordinary declaration admission checks foreign visibility.
fn check_without_package_review(
    producer_source: &str,
    selected: bool,
) -> Result<compiler::CheckedCompilation, Vec<diagnostics::Diagnostic>> {
    let tree = Tree::new();
    let consumer_identity = PackageKeyIdentity::from_digest([41; 32]).unwrap();
    let producer_identity = PackageKeyIdentity::from_digest([42; 32]).unwrap();
    let standard_identity = PackageKeyIdentity::from_digest([43; 32]).unwrap();
    let selection = if selected {
        "builder.select_representation<Token, TokenRepresentation>();"
    } else {
        ""
    };
    let consumer_build = format!(
        "use producer::main;\nmachine build(builder: &mut Build) {{ Build::package(builder, \"opaque_consumer\"); {selection} }}"
    );
    // The Build imports only the foreign type surface. The consumer owns its
    // calling policy in Product scope, so no second CallingPolicy conformance
    // is introduced by loading the producer's declarations in Build scope.
    write_sources(
        &tree,
        producer_source,
        &format!("{CALLING_POLICY_SOURCE}\n{CONSUMER_SOURCE}"),
        &consumer_build,
    );
    let inputs = PackageCompilationInputs::new_package(
        consumer_identity,
        vec![
            PackageSourceBinding::new(
                consumer_identity,
                "opaque_consumer",
                tree.path("sources/consumer"),
            ),
            PackageSourceBinding::new(
                producer_identity,
                "opaque_producer",
                tree.path("sources/producer"),
            ),
            PackageSourceBinding::new(
                standard_identity,
                "omega_language_std",
                repository_root().join("source/library/std"),
            ),
        ],
        vec![
            PackageDependencyBinding::new(consumer_identity, "producer", producer_identity),
            PackageDependencyBinding::for_purpose(
                consumer_identity,
                "producer",
                producer_identity,
                DependencyPurpose::Build,
            ),
            PackageDependencyBinding::new(
                consumer_identity,
                "omega_language_std",
                standard_identity,
            ),
        ],
    )
    .expect("direct checked package inputs");
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        build_dir: Some(tree.path("direct-checked-build")),
        ..CheckedCompileRequest::new(
            &tree.path("sources/consumer/main.omg"),
            Some("windows_x86_64"),
        )
    })
}

#[test]
fn checked_foreign_opaque_demand_accepts_a_public_selected_conformance_without_review() {
    check_without_package_review(PRODUCER_TYPES, true)
        .expect("public foreign representation checks without package review");
}

#[test]
fn checked_foreign_opaque_demand_rejects_missing_selection_without_review() {
    let diagnostics = check_without_package_review(PRODUCER_TYPES, false)
        .err()
        .expect("actual by-value demand requires a consumer selection");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("authoritative build selects no exact")),
        "{diagnostics:#?}"
    );
}

#[test]
fn checked_foreign_opaque_demand_rejects_a_private_conformance_without_review() {
    let producer = PRODUCER_TYPES.replace("pub TokenRepresentation:", "TokenRepresentation:");
    let diagnostics = check_without_package_review(&producer, true)
        .err()
        .expect("foreign private conformance cannot authorize a representation");
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message.contains("private conformance")
                && diagnostic.message.contains("TokenRepresentation")
        ),
        "{diagnostics:#?}"
    );
}
