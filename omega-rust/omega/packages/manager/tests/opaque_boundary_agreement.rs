//! This canary pins independently compiled producer/consumer review compilation.
//! It requires the consumer's by-value `Token` demand to rejoin the producer's
//! declaration, availability row, and selected application through
//! `PackagePolicyRepresentation::rejoin_foreign_demands`, called from
//! `review/candidate/compilation/package_pass.rs`, and rejects a differing
//! consumer application. `Calling<...>` comes from the real
//! standard library because it creates an actual demand row; a plain boundary
//! trait does not.

use package_evidence::record::{
    PackagePolicyRepresentationAgreementError, PackageReviewNominalOwner,
};
use package_manager::resolution::graph::GitResolutionOptions;
use package_manager::resolution::graph::{
    PackageSourceClosureLimits, ResolvedPackageSourceClosure,
    resolve_external_local_project_closure,
};
use package_manager::review::{
    CompileResolvedPackageReviewsError, SemanticBindingReview, compile_resolved_package_reviews,
};
use package_source::PrimaryGitChoices;
use package_source::{ExternalSourceContext, LocalSourceLimits, SourceResolverStorage};
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

    fn storage(&self, name: &str) -> SourceResolverStorage {
        SourceResolverStorage::for_hardened_base(self.path(name), PrimaryGitChoices::default())
            .expect("create source resolver storage")
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

const PRODUCER_SOURCE: &str = r#"use omega::language::core::representation;
use omega_language_std::calling;

pub boundary data Token;
pub data Carrier { value: u64; }
pub TokenRepresentation: Carrier satisfies OpaqueRepresentation<Token>;

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

fn producer_build(standard_library: &Path) -> String {
    format!(
        r#"machine build(builder: &mut Build) {{
    builder.package("opaque-producer");
    builder.depend(Source::Path {{
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

data Main { }
machine Main::main(&mut self) { }
"#;

const MATCHING_CONSUMER_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.application("opaque-consumer");
    builder.depend_as("producer", Source::Path {
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

data Main { }
machine Main::main(&mut self) { }
"#;

const MISMATCHING_CONSUMER_BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.application("opaque-consumer");
    builder.depend_as("producer", Source::Path {
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
    let producer = tree.path("sources/producer");
    let consumer = tree.path("sources/consumer");
    fs::create_dir_all(&producer).expect("create producer package");
    fs::create_dir_all(&consumer).expect("create consumer package");
    fs::write(producer.join("main.omg"), PRODUCER_SOURCE).expect("write producer source");
    let standard_library = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(7)
        .expect("repository root")
        .join("source/library/std");
    fs::write(
        producer.join("build.omg"),
        producer_build(&standard_library),
    )
    .expect("write producer build");

    fs::write(consumer.join("main.omg"), consumer_source).expect("write consumer source");
    fs::write(consumer.join("build.omg"), consumer_build).expect("write consumer build");

    let storage = tree.storage("closure-cache");
    resolve_external_local_project_closure(
        consumer,
        ExternalSourceContext::derive(b"opaque-boundary-agreement"),
        &storage,
        LocalSourceLimits::default(),
        PackageSourceClosureLimits::default(),
        GitResolutionOptions::default(),
    )
    .expect("resolve producer and consumer package closure")
}

#[test]
fn independently_compiled_opaque_boundary_agreement_rejoins_foreign_demand() {
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
        .find(|review| review.key().name().as_str() == "opaque-producer")
        .expect("producer review");
    let consumer = reviews
        .reviews()
        .iter()
        .find(|review| review.key().name().as_str() == "opaque-consumer")
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
fn independently_compiled_opaque_boundary_rejects_a_different_consumer_conformance() {
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
    assert!(matches!(
        result,
        Err(
            CompileResolvedPackageReviewsError::RepresentationAgreement {
                error: PackagePolicyRepresentationAgreementError::SelectedApplicationMismatch { .. },
                ..
            }
        )
    ));
}
