//! The package driver executes each selected policy against the domain's frozen schema.
//! The lowering consumer publishes its answer and preserves the distinction between
//! introducing, forwarding and erasing the qualification.
#[path = "support/source.rs"]
mod sources;

use omega::lower_package_with_policy_answers;
use omega_lowering::{
    BlockTerminator, DomainIssuers, LoweredMachine, LoweredPackage, LoweringError, QualificationDirection, ScalarQualificationChange, TypeId, TypeShape,
};
use omega_preparation::{PreparedTrees, prepare_source_package};
use omega_resolved::DeclarationId;

const UNRESTRICTED: &str = include_str!("fixtures/domain_access/unrestricted/access_plan.omg");
const NOBODY: &str = include_str!("fixtures/domain_access/empty-issuers/policy.omg");
const ISSUE: &str = "\nmachine issue(x:i32)->i32 in Locked { x as +Locked }";
const FORWARD: &str = "\nmachine forward(x:i32 in Locked)->i32 in Locked { x }";
const PENDING: &str = "domain access policy needs admitted execution of the selected plan for its semantic schema";

/// Prepare the package under a Build owner. Nothing about its policies is
/// checked or validated yet.
fn prepare_unchecked(source: &str) -> PreparedTrees<'_> {
    let mut package = sources::resolve(&[("policy.omg", source)]);
    package.bind_package_key("app", [0x42; 32]).unwrap();
    prepare_source_package(&package).unwrap()
}

fn selected(
    prepared: &PreparedTrees<'_>,
    domain: &str,
) -> DeclarationId {
    prepared
        .typed()
        .access_policies
        .iter()
        .find(|selection| selection.domain.path == domain)
        .expect("selected domain")
        .domain
        .clone()
}

fn lower_validated(
    source: &str,
    domains: &[&str],
) -> Result<LoweredPackage, LoweringError> {
    let mut prepared = prepare_unchecked(source);
    let answers = omega::prepare_policy_answers(&mut prepared, Default::default())?;
    assert_eq!(answers.len(), domains.len());
    assert!(
        domains
            .iter()
            .all(|domain| answers.iter().any(|answer| answer.domain().path == *domain))
    );
    lower_package_with_policy_answers(&prepared, &answers)
}

fn machine<'a>(
    package: &'a LoweredPackage,
    path: &str,
) -> &'a LoweredMachine {
    package
        .machines
        .iter()
        .find(|machine| machine.declaration.path == path)
        .unwrap_or_else(|| panic!("machine {path} has a body"))
}

fn issuers(
    package: &LoweredPackage,
    path: &str,
) -> DomainIssuers {
    package
        .domains
        .iter()
        .find(|domain| domain.declaration.path == path)
        .unwrap_or_else(|| panic!("domain {path} has a row"))
        .issuers
}

/// The complete domain membership a type row carries.
fn memberships(
    package: &LoweredPackage,
    ty: TypeId,
) -> Vec<&str> {
    match package.types.get(ty) {
        Some(TypeShape::QualifiedScalar { domains, .. }) => package
            .types
            .domain_set(*domains)
            .unwrap()
            .iter()
            .map(|id| id.path.as_str())
            .collect(),
        _ => Vec::new(),
    }
}

/// The cast is the machine's one qualification change: an ordinary addition
/// whose fresh destination carries exactly `domain`.
fn assert_introduces(
    package: &LoweredPackage,
    machine_path: &str,
    domain: &str,
) {
    let body = &machine(package, machine_path).body;
    let BlockTerminator::Jump(edge) = &body.blocks[0].terminator else {
        panic!("{machine_path}: expected the cast edge")
    };
    assert_eq!(
        edge.scalar_qualification_changes,
        [ScalarQualificationChange {
            argument_index: 0,
            direction: QualificationDirection::Add
        }]
    );
    assert!(edge.domain_completions.is_empty());
    let [destination] = body.blocks[edge.target.get() - 1].scalar_parameters[..] else {
        panic!("{machine_path}: expected one qualified parameter")
    };
    assert_eq!(memberships(package, body.values[destination.index()].ty), [domain]);
}

/// The formal arrives with the qualification and the result keeps it. No edge
/// adds it, so forwarding asks nothing of the domain's issuers.
fn assert_forwards(
    package: &LoweredPackage,
    machine_path: &str,
    domain: &str,
) {
    let forward = machine(package, machine_path);
    assert_eq!(memberships(package, forward.parameter_types[0]), [domain]);
    assert_eq!(memberships(package, forward.result_type.expect("a scalar result")), [domain]);
    let [block] = forward.body.blocks.as_slice() else {
        panic!("{machine_path}: expected one block")
    };
    assert!(matches!(block.terminator, BlockTerminator::Return { .. }));
}

fn spelled<'a>(
    error: &LoweringError,
    source: &'a str,
) -> &'a str {
    std::str::from_utf8(error.site.span.slice(source.as_bytes()).unwrap()).unwrap()
}

#[test]
fn an_unrestricted_plan_lets_a_machine_introduce_the_domain() {
    let source = format!("{UNRESTRICTED}{ISSUE}{FORWARD}");
    let package = lower_validated(&source, &["i32::Locked"]).unwrap();
    assert_eq!(issuers(&package, "i32::Locked"), DomainIssuers::Unrestricted);
    assert_introduces(&package, "issue", "i32::Locked");
    assert_forwards(&package, "forward", "i32::Locked");

    // The plan adds no issuer restriction, so the domain row and both bodies
    // say what they say for the same domain declared without a policy.
    let plain = format!("domain i32::Locked;{ISSUE}{FORWARD}");
    let plain = sources::lower(&[("policy.omg", &plain)]).unwrap();
    assert_eq!(issuers(&plain, "i32::Locked"), DomainIssuers::Unrestricted);
    assert_introduces(&plain, "issue", "i32::Locked");
    assert_forwards(&plain, "forward", "i32::Locked");
}

#[test]
fn a_plan_that_admits_nobody_refuses_introduction_and_keeps_forwarding() {
    const REFUSAL: &str = "machine `issue` cannot introduce domain `i32::Locked`: its access plan admits no issuer";
    let omitted = NOBODY.replace("Only{routes: []}", "Only{..}");
    assert_ne!(omitted, NOBODY);
    for policy in [NOBODY, &omitted] {
        // The cast is the one place that adds the qualification.
        let source = format!("{policy}{ISSUE}{FORWARD}");
        let error = lower_validated(&source, &["i32::Locked"]).unwrap_err();
        assert_eq!((error.message.as_str(), spelled(&error, &source)), (REFUSAL, "x as +Locked"));

        // An alias expands to the same domain and is refused for it.
        let aliased = format!("{policy}\ndomain i32::Kept = i32::Locked;\nmachine issue(x:i32)->i32 in Kept {{ x as +Kept }}");
        let error = lower_validated(&aliased, &["i32::Locked"]).unwrap_err();
        assert_eq!((error.message.as_str(), spelled(&error, &aliased)), (REFUSAL, "x as +Kept"));

        // A value that already carries the qualification still moves through
        // a formal, a call and a result, and a holder may still give it up.
        let source = format!(
            "{policy}{FORWARD}
            machine relay(x:i32 in Locked)->i32 in Locked {{ forward(x) }}
            machine release(x:i32 in Locked)->i32 {{ x as -Locked }}"
        );
        let package = lower_validated(&source, &["i32::Locked"]).unwrap();
        assert_eq!(issuers(&package, "i32::Locked"), DomainIssuers::Nobody);
        assert_forwards(&package, "forward", "i32::Locked");
        let relay = machine(&package, "relay");
        assert_eq!(memberships(&package, relay.parameter_types[0]), ["i32::Locked"]);
        assert_eq!(memberships(&package, relay.result_type.unwrap()), ["i32::Locked"]);
        let BlockTerminator::Jump(edge) = &machine(&package, "release").body.blocks[0].terminator else {
            panic!("expected the erasing cast edge")
        };
        assert_eq!(
            edge.scalar_qualification_changes,
            [ScalarQualificationChange {
                argument_index: 0,
                direction: QualificationDirection::Erase
            }]
        );
    }
}

#[test]
fn each_domain_follows_its_own_validated_plan() {
    let policies = format!(
        "{}\n{}",
        UNRESTRICTED.replace("Locked", "Open").replace("Rule", "Free"),
        NOBODY
            .replace("Locked", "Shut")
            .replace("Rule", "Closed")
            .replace("use omega", "// use omega")
    );
    let machines = "
        machine open(x:i32)->i32 in Open { x as +Open }
        machine keep(x:i32 in Shut)->i32 in Shut { x }";
    let source = format!("{policies}{machines}");
    let package = lower_validated(&source, &["i32::Open", "i32::Shut"]).unwrap();
    assert_eq!(issuers(&package, "i32::Open"), DomainIssuers::Unrestricted);
    assert_eq!(issuers(&package, "i32::Shut"), DomainIssuers::Nobody);
    assert_introduces(&package, "open", "i32::Open");
    assert_forwards(&package, "keep", "i32::Shut");

    let source = format!("{source}\nmachine shut(x:i32)->i32 in Shut {{ x as +Shut }}");
    let error = lower_validated(&source, &["i32::Open", "i32::Shut"]).unwrap_err();
    assert_eq!(
        (error.message.as_str(), spelled(&error, &source)),
        (
            "machine `shut` cannot introduce domain `i32::Shut`: its access plan admits no issuer",
            "x as +Shut"
        )
    );
}

#[test]
fn a_plan_validated_for_another_schema_does_not_publish_this_domain() {
    // Both domains select `Rule`, whose plan is unrestricted for any schema.
    let source = format!("{UNRESTRICTED}\ndomain i32::Sealed access Rule;\nmachine seal(x:i32)->i32 in Sealed {{ x as +Sealed }}");
    let mut prepared = prepare_unchecked(&source);
    let sealed = selected(&prepared, "i32::Sealed");
    let mut answers = omega::prepare_policy_answers(&mut prepared, Default::default()).unwrap();
    let position = answers
        .iter()
        .position(|answer| answer.domain() == &sealed)
        .unwrap();
    let own = answers.remove(position);
    let error = lower_package_with_policy_answers(&prepared, &answers).unwrap_err();
    assert_eq!((error.message.as_str(), spelled(&error, &source)), (PENDING, "access Rule"));
    assert_eq!(error.site.span.get_start_offset(), source.rfind("access Rule").unwrap());

    let locked = &answers[0];
    assert_eq!(
        omega_preparation::read_returned_access_plan(&prepared, &sealed, locked.returned()),
        Err("returned plan describes a different schema")
    );
    answers.push(own);
    let package = lower_package_with_policy_answers(&prepared, &answers).unwrap();
    assert_introduces(&package, "seal", "i32::Sealed");
}
