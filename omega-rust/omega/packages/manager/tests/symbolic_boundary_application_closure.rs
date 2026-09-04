use omega_boundary_applications::{BoundaryApplication, BoundaryApplicationArgument};
use omega_compiler::compile_to_checked_with_packages;
use omega_package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use omega_package_evidence::{
    project_checked_package_review,
    record::{
        CheckedPackageBoundaryApplicationDemandReview,
        CheckedPackageBoundaryApplicationRealizationReview, CheckedPackageCallableReview,
        CheckedPackageReviewProjection, PackageReviewBoundaryApplication,
        PackageReviewBoundaryApplicationArgument,
    },
};
use omega_package_manager::review::{
    ConcreteProducerBinderCategory, ConcreteProducerTypeSpecialization,
    ConcreteProducerTypeSubstitution, SymbolicBoundaryApplicationClosureError,
    SymbolicBoundaryApplicationClosureRequest,
    close_supplied_reviewed_symbolic_boundary_applications as close_reviewed_symbolic_boundary_applications,
};
use psi_core::PackageKeyIdentity;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMPORARY_PACKAGE: AtomicU64 = AtomicU64::new(0);

struct TemporaryPackage(PathBuf);

impl TemporaryPackage {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-symbolic-boundary-closure-{}-{}",
            std::process::id(),
            NEXT_TEMPORARY_PACKAGE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).expect("create symbolic-closure test package");
        Self(path)
    }
}

impl Drop for TemporaryPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn producer_package_identity() -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([0x29; 32]).expect("nonzero package identity")
}

fn consumer_package_identity() -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([0x41; 32]).expect("nonzero package identity")
}

fn host_target_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("windows_x86_64"),
        ("linux", "x86_64") => Some("linux_x86_64"),
        ("linux", "aarch64") => Some("linux_arm64"),
        ("macos", "aarch64") => Some("macos_arm64"),
        _ => None,
    }
}

struct ReviewedArtifacts {
    producer: CheckedPackageReviewProjection,
    consumer: CheckedPackageReviewProjection,
}

fn reviewed_fixture() -> Option<ReviewedArtifacts> {
    let Some(target) = host_target_name() else {
        eprintln!(
            "SKIP symbolic boundary application fixture: unsupported host {}/{}",
            std::env::consts::OS,
            std::env::consts::ARCH,
        );
        return None;
    };
    let tree = TemporaryPackage::new();
    let producer = tree.0.join("producer");
    let consumer = tree.0.join("consumer");
    fs::create_dir(&producer).expect("create producer package");
    fs::create_dir(&consumer).expect("create consumer package");
    fs::write(
        producer.join("main.omg"),
        r#"pub data GenericMath {}
pub boundary operator GenericMath::identity<Element>(value: Element) -> Element;
pub boundary operator GenericMath::other<Element>(value: Element) -> Element;
pub boundary operator GenericMath::bounded<Element [copy]>(value: Element) -> Element;

pub data GenericProvider {}
pub machine GenericProvider::identity<Value>(value: Value) -> Value
satisfies GenericMath::identity
{ value }
pub machine GenericProvider::other<Value>(value: Value) -> Value
satisfies GenericMath::other
{ value }
pub machine GenericProvider::bounded<Value [copy]>(value: Value) -> Value
satisfies GenericMath::bounded
{ value }

pub machine compare<Element>(value: Element) -> Element {
    GenericMath::identity(value)
}
pub machine compare_again<Element>(value: Element) -> Element {
    GenericMath::identity(value)
}
pub machine compare_unused<Element, Other>(value: Element) -> Element {
    GenericMath::identity(value)
}
pub machine compare_const<Element, const Count: u64>(value: Element) -> Element {
    GenericMath::identity(value)
}
pub machine compare_lifetime<'scope, Element>(value: Element) -> Element {
    GenericMath::identity(value)
}
pub machine compare_bounded<Element [copy]>(value: Element) -> Element {
    GenericMath::bounded(value)
}
"#,
    )
    .expect("write producer main");
    fs::write(
        producer.join("build.omg"),
        r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("producer"); }
"#,
    )
    .expect("write producer build");
    fs::write(
        consumer.join("main.omg"),
        r#"use producer::main;

pub machine exercise(value: i32) -> i32 {
    GenericMath::identity(value)
}
pub machine exercise_other(value: i32) -> i32 {
    GenericMath::other(value)
}
pub machine exercise_bounded(value: i32) -> i32 {
    GenericMath::bounded(value)
}
"#,
    )
    .expect("write consumer main");
    fs::write(
        consumer.join("build.omg"),
        r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("consumer"); }
"#,
    )
    .expect("write consumer build");

    let producer_inputs = PackageCompilationInputs::new_package(
        producer_package_identity(),
        vec![PackageSourceBinding::new(
            producer_package_identity(),
            "producer",
            producer.clone(),
        )],
        Vec::new(),
    )
    .expect("producer package input is canonical");
    let checked_producer =
        compile_to_checked_with_packages(&producer.join("main.omg"), Some(target), producer_inputs)
            .expect("producer symbolic generic applications check");
    let producer_review = project_checked_package_review(&checked_producer)
        .expect("producer projects to package review");

    let consumer_inputs = PackageCompilationInputs::new_package(
        consumer_package_identity(),
        vec![
            PackageSourceBinding::new(consumer_package_identity(), "consumer", consumer.clone()),
            PackageSourceBinding::new(producer_package_identity(), "producer", producer),
        ],
        vec![PackageDependencyBinding::new(
            consumer_package_identity(),
            "producer",
            producer_package_identity(),
        )],
    )
    .expect("cross-package input is canonical");
    let checked_consumer =
        compile_to_checked_with_packages(&consumer.join("main.omg"), Some(target), consumer_inputs)
            .expect("consumer concrete generic applications check");
    let consumer_review = project_checked_package_review(&checked_consumer)
        .expect("consumer projects to package review");
    Some(ReviewedArtifacts {
        producer: producer_review,
        consumer: consumer_review,
    })
}

fn demand<'a>(
    review: &'a CheckedPackageReviewProjection,
    producer: &str,
) -> &'a CheckedPackageBoundaryApplicationDemandReview {
    review
        .boundary_application_demands()
        .iter()
        .find(|demand| demand.producer_callable().path() == producer)
        .expect("named symbolic demand")
}

fn callable<'a>(
    review: &'a CheckedPackageReviewProjection,
    path: &str,
) -> &'a CheckedPackageCallableReview {
    review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == path)
        .expect("named reviewed callable")
}

fn selected<'a>(
    review: &'a CheckedPackageReviewProjection,
    operator: &str,
) -> &'a CheckedPackageBoundaryApplicationRealizationReview {
    review
        .boundary_application_realizations()
        .iter()
        .find(|application| application.operator_declaration().path() == operator)
        .expect("named closed selected application")
}

fn selected_type(
    selected: &CheckedPackageBoundaryApplicationRealizationReview,
) -> omega_package_evidence::record::PackageReviewTypeIdentity {
    let PackageReviewBoundaryApplication::Exact(arguments) = selected.application() else {
        panic!("generic selected application")
    };
    let [PackageReviewBoundaryApplicationArgument::Type { type_identity, .. }] =
        arguments.as_slice()
    else {
        panic!("one selected type argument")
    };
    type_identity.clone()
}

fn specialization(
    review: &CheckedPackageReviewProjection,
    producer: &str,
    ordinals: &[u32],
    concrete: &omega_package_evidence::record::PackageReviewTypeIdentity,
) -> ConcreteProducerTypeSpecialization {
    ConcreteProducerTypeSpecialization::new(
        review.package(),
        callable(review, producer).identity(),
        ordinals
            .iter()
            .map(|ordinal| ConcreteProducerTypeSubstitution::type_argument(*ordinal, concrete))
            .collect(),
    )
}

#[test]
fn an_empty_supplied_request_set_is_only_an_empty_non_authorizing_result() {
    let closed = close_reviewed_symbolic_boundary_applications(Vec::new())
        .expect("an empty supplied request list is canonical");
    assert!(closed.is_empty());
}

#[test]
fn closes_direct_type_binder_and_deduplicates_only_after_exact_plan_rejoin() {
    let Some(artifacts) = reviewed_fixture() else {
        return;
    };
    let demand_row = demand(&artifacts.producer, "compare");
    let selected_identity = selected(&artifacts.consumer, "GenericMath::identity");
    let concrete = selected_type(selected_identity);
    let request = SymbolicBoundaryApplicationClosureRequest::new(
        &artifacts.producer,
        &artifacts.producer,
        &artifacts.consumer,
        demand_row,
        specialization(&artifacts.producer, "compare", &[0], &concrete),
        selected_identity,
    );
    let second_source = SymbolicBoundaryApplicationClosureRequest::new(
        &artifacts.producer,
        &artifacts.producer,
        &artifacts.consumer,
        demand(&artifacts.producer, "compare_again"),
        specialization(&artifacts.producer, "compare_again", &[0], &concrete),
        selected_identity,
    );
    let closed = close_reviewed_symbolic_boundary_applications(vec![
        request.clone(),
        request,
        second_source,
    ])
    .expect("exact reviewed substitutions close across artifacts");
    let [row] = closed.rows() else {
        panic!("equal closed demands deduplicate")
    };
    assert_eq!(
        row.requirement().overload(),
        demand_row.requirement_identity()
    );
    assert_eq!(
        row.selected_plan_digest(),
        selected_identity.selected_plan_digest()
    );
    assert_eq!(
        row.selected_application_package(),
        artifacts.consumer.package()
    );
    let BoundaryApplication::Exact(arguments) = row.application() else {
        panic!("one exact closed application")
    };
    assert!(matches!(
        arguments.as_slice(),
        [BoundaryApplicationArgument::Type {
            binder_ordinal: 0,
            type_identity,
        }] if type_identity.canonical() == concrete.canonical()
    ));
    assert_eq!(row.sources().len(), 2);
    assert_eq!(row.sources()[0].package(), artifacts.producer.package());
    assert!(
        row.sources()
            .iter()
            .all(|source| source.symbolic_arguments() == demand_row.arguments())
    );
    assert!(row.sources().iter().all(|source| {
        source.substitutions().len() == 1 && source.substitutions()[0].type_identity() == &concrete
    }));
}

#[test]
fn rejects_package_callable_operator_and_binder_drift() {
    let Some(artifacts) = reviewed_fixture() else {
        return;
    };
    let demand = demand(&artifacts.producer, "compare");
    let selected_identity = selected(&artifacts.consumer, "GenericMath::identity");
    let concrete = selected_type(selected_identity);

    let wrong_package = ConcreteProducerTypeSpecialization::new(
        PackageKeyIdentity::from_digest([0x72; 32]).unwrap(),
        callable(&artifacts.producer, "compare").identity(),
        vec![ConcreteProducerTypeSubstitution::type_argument(
            0, &concrete,
        )],
    );
    assert_eq!(
        close_reviewed_symbolic_boundary_applications(vec![
            SymbolicBoundaryApplicationClosureRequest::new(
                &artifacts.producer,
                &artifacts.producer,
                &artifacts.consumer,
                demand,
                wrong_package,
                selected_identity,
            ),
        ]),
        Err(SymbolicBoundaryApplicationClosureError::ProducerPackageMismatch),
    );

    assert_eq!(
        close_reviewed_symbolic_boundary_applications(vec![
            SymbolicBoundaryApplicationClosureRequest::new(
                &artifacts.producer,
                &artifacts.producer,
                &artifacts.consumer,
                demand,
                specialization(&artifacts.producer, "compare_unused", &[0, 1], &concrete),
                selected_identity,
            ),
        ]),
        Err(SymbolicBoundaryApplicationClosureError::ProducerCallableMismatch),
    );

    assert_eq!(
        close_reviewed_symbolic_boundary_applications(vec![
            SymbolicBoundaryApplicationClosureRequest::new(
                &artifacts.producer,
                &artifacts.consumer,
                &artifacts.consumer,
                demand,
                specialization(&artifacts.producer, "compare", &[0], &concrete),
                selected_identity,
            ),
        ]),
        Err(SymbolicBoundaryApplicationClosureError::OperatorPackageMismatch),
    );

    assert!(matches!(
        close_reviewed_symbolic_boundary_applications(vec![
            SymbolicBoundaryApplicationClosureRequest::new(
                &artifacts.producer,
                &artifacts.producer,
                &artifacts.consumer,
                demand,
                specialization(&artifacts.producer, "compare", &[0], &concrete),
                selected(&artifacts.consumer, "GenericMath::other"),
            ),
        ]),
        Err(SymbolicBoundaryApplicationClosureError::SelectedRequirementMismatch)
            | Err(SymbolicBoundaryApplicationClosureError::SelectedOperatorMismatch)
    ));

    assert_eq!(
        close_reviewed_symbolic_boundary_applications(vec![
            SymbolicBoundaryApplicationClosureRequest::new(
                &artifacts.producer,
                &artifacts.producer,
                &artifacts.consumer,
                demand,
                specialization(&artifacts.producer, "compare", &[1], &concrete),
                selected_identity,
            ),
        ]),
        Err(SymbolicBoundaryApplicationClosureError::NonCanonicalProducerSubstitution(1)),
    );
}

#[test]
fn rejects_missing_extra_unsupported_and_unused_producer_substitutions() {
    let Some(artifacts) = reviewed_fixture() else {
        return;
    };
    let selected_identity = selected(&artifacts.consumer, "GenericMath::identity");
    let concrete = selected_type(selected_identity);
    let direct = demand(&artifacts.producer, "compare");

    assert_eq!(
        close_reviewed_symbolic_boundary_applications(vec![
            SymbolicBoundaryApplicationClosureRequest::new(
                &artifacts.producer,
                &artifacts.producer,
                &artifacts.consumer,
                direct,
                specialization(&artifacts.producer, "compare", &[], &concrete),
                selected_identity,
            ),
        ]),
        Err(SymbolicBoundaryApplicationClosureError::MissingProducerSubstitution(0)),
    );
    assert_eq!(
        close_reviewed_symbolic_boundary_applications(vec![
            SymbolicBoundaryApplicationClosureRequest::new(
                &artifacts.producer,
                &artifacts.producer,
                &artifacts.consumer,
                direct,
                specialization(&artifacts.producer, "compare", &[0, 1], &concrete),
                selected_identity,
            ),
        ]),
        Err(SymbolicBoundaryApplicationClosureError::ExtraProducerSubstitution(1)),
    );

    let wrong_category = ConcreteProducerTypeSpecialization::new(
        artifacts.producer.package(),
        callable(&artifacts.producer, "compare").identity(),
        vec![ConcreteProducerTypeSubstitution::new(
            0,
            ConcreteProducerBinderCategory::Const,
            &concrete,
        )],
    );
    assert_eq!(
        close_reviewed_symbolic_boundary_applications(vec![
            SymbolicBoundaryApplicationClosureRequest::new(
                &artifacts.producer,
                &artifacts.producer,
                &artifacts.consumer,
                direct,
                wrong_category,
                selected_identity,
            ),
        ]),
        Err(SymbolicBoundaryApplicationClosureError::ProducerSubstitutionCategoryMismatch(0)),
    );

    let const_producer = demand(&artifacts.producer, "compare_const");
    assert_eq!(
        close_reviewed_symbolic_boundary_applications(vec![
            SymbolicBoundaryApplicationClosureRequest::new(
                &artifacts.producer,
                &artifacts.producer,
                &artifacts.consumer,
                const_producer,
                specialization(&artifacts.producer, "compare_const", &[0, 1], &concrete),
                selected_identity,
            ),
        ]),
        Err(SymbolicBoundaryApplicationClosureError::UnsupportedProducerBinderCategory(1)),
    );

    let unused_producer = demand(&artifacts.producer, "compare_unused");
    assert_eq!(
        close_reviewed_symbolic_boundary_applications(vec![
            SymbolicBoundaryApplicationClosureRequest::new(
                &artifacts.producer,
                &artifacts.producer,
                &artifacts.consumer,
                unused_producer,
                specialization(&artifacts.producer, "compare_unused", &[0, 1], &concrete),
                selected_identity,
            ),
        ]),
        Err(SymbolicBoundaryApplicationClosureError::UnusedProducerSubstitution(1)),
    );

    let lifetime_producer = demand(&artifacts.producer, "compare_lifetime");
    assert_eq!(
        close_reviewed_symbolic_boundary_applications(vec![
            SymbolicBoundaryApplicationClosureRequest::new(
                &artifacts.producer,
                &artifacts.producer,
                &artifacts.consumer,
                lifetime_producer,
                specialization(&artifacts.producer, "compare_lifetime", &[0], &concrete),
                selected_identity,
            ),
        ]),
        Err(SymbolicBoundaryApplicationClosureError::UnsupportedProducerLifetimeTelescope),
    );

    let bounded_producer = demand(&artifacts.producer, "compare_bounded");
    let selected_bounded = selected(&artifacts.consumer, "GenericMath::bounded");
    let bounded_concrete = selected_type(selected_bounded);
    assert_eq!(
        close_reviewed_symbolic_boundary_applications(vec![
            SymbolicBoundaryApplicationClosureRequest::new(
                &artifacts.producer,
                &artifacts.producer,
                &artifacts.consumer,
                bounded_producer,
                specialization(
                    &artifacts.producer,
                    "compare_bounded",
                    &[0],
                    &bounded_concrete,
                ),
                selected_bounded,
            ),
        ]),
        Err(SymbolicBoundaryApplicationClosureError::UnsupportedOperatorBinderBounds(0)),
    );
}
