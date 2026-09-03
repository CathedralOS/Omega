use crate::support::*;

const BUILD: &str = r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;

#[test]
fn checked_and_external_exact_edges_publish_the_same_lifetime_partition() {
    let Some(target) = host_target_name() else {
        return;
    };
    let project = |source: &str| {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write("build.omg", BUILD);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("exact lifetime realization should check");
        project_checked_package_review(&checked).expect("exact lifetime edge should project")
    };

    let checked = project(
        r#"pub boundary trait Pair<'left, 'right> {
    machine consume(first: &'left u64, second: &'right u64) reaches Pair;
}
pub data CheckedPair { }
pub machine CheckedPair::consume<'unused, 'x, 'y>(first: &'x u64, second: &'y u64)
    satisfies Pair<'x, 'y>::consume
{
}
"#,
    );
    let external = project(
        r#"pub boundary trait Pair<'left, 'right> {
    machine consume(first: &'left u64, second: &'right u64) reaches Pair;
}
pub data ExternalPair { }
pub machine ExternalPair::consume<'y, 'unused, 'x>(first: &'x u64, second: &'y u64)
    satisfies Pair<'x, 'y>::consume
    via Binding::Syscall(60);
"#,
    );

    let checked_conformance = checked
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "CheckedPair::consume")
        .and_then(|callable| callable.conformances().first())
        .expect("checked exact edge");
    let external_conformance = external
        .external_executable_supply()
        .iter()
        .find(|supply| supply.callable().path() == "ExternalPair::consume")
        .and_then(|supply| supply.conformance())
        .expect("external exact edge");
    assert_eq!(checked_conformance.requirement_lifetime_partition(), [0, 1],);
    assert_eq!(
        checked_conformance.requirement_lifetime_partition(),
        external_conformance.requirement_lifetime_partition(),
        "supply mode and private realizer binder order are outside edge identity",
    );

    let checked_provider_partition =
        &checked.selected_providers()[0].rows()[0].requirement_lifetime_partition;
    let external_provider_partition =
        &external.selected_providers()[0].rows()[0].requirement_lifetime_partition;
    assert_eq!(checked_provider_partition, external_provider_partition);
    assert_eq!(checked_provider_partition, &[0, 1]);
}
