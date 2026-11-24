//! Compare direct and inherited public evidence in an ordinary package review.
use super::run_project_build;
use review_evidence::record::PackagePolicyBaseline;
use target::TargetProfile;

#[test]
fn inherited_conformance_retains_its_applied_parent() {
    let temp = platform_custody::temporary_directory::TemporaryDirectory::create(
        "review-inherited-conformance",
        false,
    )
    .unwrap();
    let root = temp.path();
    std::fs::write(
        root.join("build.omg"),
        r#"machine build(builder: @mutable Build) { Build::package(builder, "inherited"); }"#,
    )
    .unwrap();
    std::fs::write(
        root.join("main.omg"),
        r#"
public data Wrapped<T> { value: T; }
public trait Parent<T, U> { machine get(value: T) -> u32 reaches none crashes none; }
public trait Middle<X, Y> : Parent<Y, X> {}
public trait Child<A, B> : Middle<A, B> {}
public Reader satisfies Child<Wrapped<u32>, u16> { machine get(value: u16) -> u32 { 17u32 } }
public Direct satisfies Parent<u16, Wrapped<u32>> { machine get(value: u16) -> u32 { 17u32 } }
"#,
    )
    .unwrap();
    let target = TargetProfile::LinuxX64;
    let (returned, inputs) = run_project_build(&root.join("build.omg"), &[target]);
    let settled = returned.settle_dependency(inputs.clone()).unwrap();
    let product = settled.open_product(inputs).unwrap();
    let review = product
        .product_stage(&[target])
        .unwrap()
        .review_policy(target)
        .unwrap();
    let bytes = review.policy.canonical_bytes().unwrap();
    let saved = PackagePolicyBaseline::recover_canonical(&bytes, Default::default()).unwrap();
    let conformance = |name: &str| {
        &saved
            .public_api
            .conformances
            .iter()
            .find(|c| c.identity.path == format!("main.omg::{name}"))
            .unwrap()
            .interface
    };
    let inherited = conformance("Reader");
    let direct = conformance("Direct");
    assert_eq!(inherited.arguments.len(), 2);
    assert_ne!(inherited.arguments[0], inherited.arguments[1]);
    assert_eq!(
        direct.arguments,
        [
            inherited.arguments[1].clone(),
            inherited.arguments[0].clone()
        ]
    );
    let [parent] = direct.requirements.as_slice() else {
        panic!("expected the one checked Parent::get requirement");
    };
    assert_eq!(parent.declaring_trait, direct.trait_identity);
    assert_eq!(parent.declaring_trait_arguments, direct.arguments);
    assert_eq!(inherited.requirements, direct.requirements);
}
