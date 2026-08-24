use omega_compiler::{
    PACKAGE_REVIEW_ENCODING_VERSION, PackageCompilationInputs, PackageReviewArithmeticDomain,
    PackageReviewCallableRole, PackageReviewCastForm, PackageReviewContractBinaryOperator,
    PackageReviewContractExpression, PackageReviewContractFact, PackageReviewContractKind,
    PackageReviewCrashInterface, PackageReviewCrashRouteGuard,
    PackageReviewDangerousAuthorityClass, PackageReviewDataMember,
    PackageReviewDomainClassification, PackageReviewDomainEstablishmentKind,
    PackageReviewNominalOwner, PackageReviewPropositionBinderKind,
    PackageReviewPropositionBinderValue, PackageReviewPropositionEvidence,
    PackageReviewRepresentationAbiCommitment, PackageReviewRepresentationMechanism,
    PackageReviewSynchronousInvocation, PackageSourceBinding, compile_to_checked_with_packages,
    project_checked_package_review,
};
use psi_core::PackageKeyIdentity;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempPackage(PathBuf);

impl TempPackage {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-package-review-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create package review fixture");
        Self(path)
    }

    fn write(&self, path: impl AsRef<Path>, source: &str) {
        fs::write(self.0.join(path), source).expect("write package review fixture source");
    }
}

impl Drop for TempPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn package_identity() -> PackageKeyIdentity {
    PackageKeyIdentity::from_digest([41; 32]).expect("nonzero package identity")
}

fn package_inputs(root: &Path) -> PackageCompilationInputs {
    PackageCompilationInputs::new(
        package_identity(),
        vec![PackageSourceBinding::new(
            package_identity(),
            root.to_owned(),
        )],
        Vec::new(),
    )
    .expect("single-package review graph should validate")
}

#[test]
fn dangerous_authority_classification_requires_exact_toolchain_provenance() {
    let Some(target) = host_target_name() else {
        return;
    };

    let canonical = TempPackage::new();
    canonical.write(
        "main.omg",
        r#"use omega::language::std::filesystem_host;

pub data Journal { files: FilesystemHost; written: i64; }

pub machine Journal::append(&mut self, descriptor: i32, bytes: &[u8])
reaches FilesystemHost
invokes FilesystemHost;
{
    self.written = self.files.write(descriptor, bytes);
}
"#,
    );
    canonical.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
"#,
    );
    let canonical_checked = compile_to_checked_with_packages(
        &canonical.0.join("main.omg"),
        Some(target),
        package_inputs(&canonical.0),
    )
    .expect("canonical filesystem fixture should check");
    let canonical_review = project_checked_package_review(&canonical_checked)
        .expect("canonical filesystem review should close");
    let [authority] = canonical_review.dangerous_authorities() else {
        panic!("canonical filesystem authority row")
    };
    assert_eq!(
        authority.class(),
        PackageReviewDangerousAuthorityClass::Filesystem
    );
    assert_eq!(
        authority.service().owner(),
        PackageReviewNominalOwner::ToolchainUnbound
    );
    assert_eq!(authority.service().path(), "FilesystemHost");

    let lookalike = TempPackage::new();
    lookalike.write(
        "main.omg",
        r#"pub boundary trait FilesystemHost {
    machine write(descriptor: i32, bytes: &[u8]) -> i64;
}

pub data Journal { files: FilesystemHost; written: i64; }

pub machine Journal::append(&mut self, descriptor: i32, bytes: &[u8])
reaches FilesystemHost
invokes FilesystemHost;
{
    self.written = self.files.write(descriptor, bytes);
}
"#,
    );
    lookalike.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
"#,
    );
    let lookalike_checked = compile_to_checked_with_packages(
        &lookalike.0.join("main.omg"),
        Some(target),
        package_inputs(&lookalike.0),
    )
    .expect("package-owned filesystem lookalike should check as ordinary source");
    let lookalike_review = project_checked_package_review(&lookalike_checked)
        .expect("package-owned filesystem lookalike review should close");
    assert!(
        lookalike_review.dangerous_authorities().is_empty(),
        "package-controlled naming must not mint a compiler-owned risk class"
    );
}

#[test]
fn representation_tcb_retains_private_opaque_data_as_unbound() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write("main.omg", "boundary data InternalToken;\n");
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("opaque representation fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("opaque representation review should close");
    let [row] = review.representation_tcb() else {
        panic!("one private representation-TCB row")
    };
    assert_eq!(row.declaration().path(), "InternalToken");
    assert_eq!(
        row.declaration().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(row.abi(), PackageReviewRepresentationAbiCommitment::Unbound);
    assert_eq!(
        row.mechanism(),
        PackageReviewRepresentationMechanism::Unbound
    );
    assert!(
        review.public_data().is_empty(),
        "ordinary public API projection remains visibility-scoped"
    );

    let control = TempPackage::new();
    control.write("main.omg", "data InternalToken { }\n");
    control.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
"#,
    );
    let control_checked = compile_to_checked_with_packages(
        &control.0.join("main.omg"),
        Some(target),
        package_inputs(&control.0),
    )
    .expect("ordinary private representation fixture should check");
    let control_review = project_checked_package_review(&control_checked)
        .expect("ordinary private representation review should close");
    assert!(control_review.public_data().is_empty());
    assert!(control_review.representation_tcb().is_empty());
    assert_ne!(
        review
            .canonical_review_bytes()
            .expect("opaque review encoding"),
        control_review
            .canonical_review_bytes()
            .expect("ordinary review encoding"),
        "a private opaque representation-TCB row must enter comparison identity"
    );
}

#[test]
fn review_projects_root_boundary_and_build_authority() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"boundary machine host_ping() reaches <= Host;
boundary trait Host { machine ping(); }
machine ping_leaf() satisfies Host::ping via Binding::VtableSlot(1);
data Receipt [linear] { code: i32; }
pub data Packet [copy] { #1 value: u32; }
pub domain Packet::Ready;
domain Packet::Private;
data PrivatePacket { hidden: u32; }
machine helper()
crashes Abort
{
    crash Abort;
}
pub machine public_api() { }
machine private_api() { }
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build)
crashes Abort
{
    helper();
    let receipt: Receipt = Receipt { code: 1 };
    crash Abort;
}
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("package fixture should check");
    let review = project_checked_package_review(&checked).expect("review projection should close");
    let encoded = review
        .canonical_review_bytes()
        .expect("review projection should have a canonical comparison encoding");
    let magic = b"OMEGA-PACKAGE-REVIEW\0";
    assert!(encoded.starts_with(magic));
    assert_eq!(
        &encoded[magic.len()..magic.len() + 2],
        &PACKAGE_REVIEW_ENCODING_VERSION.to_le_bytes(),
    );
    assert_eq!(
        encoded,
        review
            .canonical_review_bytes()
            .expect("repeated encoding must be deterministic")
    );

    assert_eq!(review.package(), package_identity());
    assert_eq!(
        review.target().target_name(),
        target,
        "review identity must retain the deployment profile, not only its native ABI",
    );
    assert_eq!(PACKAGE_REVIEW_ENCODING_VERSION, 21);
    let [ready] = review.public_domains() else {
        panic!("one package-owned public domain row")
    };
    assert_eq!(ready.identity().path(), "Packet::Ready");
    assert!(ready.type_parameters().is_empty());
    assert!(!ready.target_type().canonical().is_empty());
    assert!(ready.index_arguments().is_empty());
    let [packet] = review.public_data() else {
        panic!("one package-owned public data row")
    };
    assert_eq!(packet.identity().path(), "Packet");
    assert_eq!(packet.lifetime_parameter_count(), 0);
    assert_eq!(packet.members().len(), 1);
    let PackageReviewDataMember::Field(value) = &packet.members()[0] else {
        panic!("Packet value field")
    };
    assert_eq!(value.identity(), Some(1));
    assert_eq!(value.name(), "value");
    assert!(!value.type_identity().canonical().is_empty());
    assert_eq!(review.callables().len(), 3);
    let boundary = review
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Boundary)
        .expect("boundary row");
    assert_eq!(boundary.identity().path(), "host_ping");
    assert_eq!(boundary.lifetime_parameter_count(), 0);
    assert!(boundary.type_parameters().is_empty());
    assert!(boundary.parameters().is_empty());
    assert!(!boundary.return_type().canonical().is_empty());
    assert_eq!(
        boundary.identity().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    let [declared] = boundary
        .declared_service_reach()
        .expect("installation-bound declaration retains its upper bound")
    else {
        panic!("one declared upper-bound service")
    };
    assert_eq!(declared.path(), "Host");
    assert_eq!(
        declared.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(
        boundary.realized_service_reach(),
        boundary
            .declared_service_reach()
            .expect("published upper bound")
    );
    assert!(boundary.concrete_service_reach().is_empty());
    assert!(boundary.capability_flows().is_empty());
    assert_eq!(boundary.declared_synchronous_invocations(), Some(&[][..]));
    assert!(boundary.realized_synchronous_invocations().is_empty());
    let [installation] = boundary.unresolved_installation_reaches() else {
        panic!("one normalized installation-bound reach row")
    };
    assert_eq!(
        installation.requirement().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert!(installation.requirement().path().contains("host_ping"));
    let [upper_bound] = installation.upper_bound() else {
        panic!("one normalized installation upper-bound service")
    };
    assert_eq!(
        upper_bound.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(upper_bound.path(), "Host");

    let build = review
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Build)
        .expect("build row");
    assert_eq!(build.identity().path(), "build");
    let [builder] = build.parameters() else {
        panic!("build entry retains its builder parameter")
    };
    assert_eq!(builder.name(), "builder");
    assert!(builder.is_mutable());
    assert!(!builder.is_const());
    assert!(!builder.is_self());
    assert!(!builder.type_identity().canonical().is_empty());
    assert_eq!(build.declared_service_reach(), None);
    assert_eq!(build.declared_synchronous_invocations(), None);

    let public = review
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("ordinary public callable row");
    assert_eq!(public.identity().path(), "public_api");
    assert_eq!(
        public.identity().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(public.declared_service_reach(), Some(&[][..]));
    assert_eq!(public.declared_synchronous_invocations(), Some(&[][..]));
    assert!(public.realized_service_reach().is_empty());
    assert!(public.realized_synchronous_invocations().is_empty());
    assert_eq!(
        public.checked_crash().interface(),
        PackageReviewCrashInterface::PublishedCeiling
    );
    assert!(
        review
            .callables()
            .iter()
            .all(|callable| callable.identity().path() != "private_api")
    );
    let crash = build.checked_crash();
    assert_eq!(
        crash.interface(),
        PackageReviewCrashInterface::PublishedCeiling
    );
    let [published_crash] = crash.published() else {
        panic!("one normalized published crash route")
    };
    assert_eq!(
        published_crash.cause(),
        psi_checked_trees::CrashCause::Abort
    );
    assert_eq!(
        published_crash.alternative_guards(),
        [PackageReviewCrashRouteGuard::Truth]
    );
    let [crash_site] = crash.checked_sites() else {
        panic!("one normalized checked crash site")
    };
    assert_eq!(
        crash_site.state().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(crash_site.cause(), psi_checked_trees::CrashCause::Abort);
    assert_eq!(crash_site.guard_covering_buckets(), [1]);
    assert!(!crash_site.frontier_lower_bound().is_empty());
    assert!(
        crash_site
            .frontier_lower_bound()
            .iter()
            .all(|claim| claim.machine().owner()
                == PackageReviewNominalOwner::Package(package_identity())
                && claim.state().owner() == PackageReviewNominalOwner::Package(package_identity()))
    );
    let [crash_call] = crash.checked_calls() else {
        panic!("one normalized checked crash call")
    };
    assert_eq!(
        crash_call.state().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(
        crash_call.target_machine().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(crash_call.target_machine().path(), "helper");
    assert_eq!(
        crash_call.target_state().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );

    let [provider] = review.selected_providers() else {
        panic!("one selected provider review row")
    };
    assert_eq!(provider.realizing_package(), Some(package_identity()));
    assert_eq!(provider.provider_type_package(), None);
    assert_eq!(provider.service_schema(), "Host");
    assert_eq!(
        provider.schema().trait_package_identity,
        Some(package_identity())
    );
    assert_eq!(
        provider.schema().methods[0].requirement_owner_package_identity,
        Some(package_identity())
    );
    assert_eq!(provider.rows().len(), 1);
    assert!(matches!(
        provider.rows()[0].binding,
        omega_effects::provider_plan::ProviderBinding::VtableSlot { index: 1 }
    ));
}

#[test]
fn review_projects_exact_accepted_boundary_contracts() {
    let Some(target) = host_target_name() else {
        return;
    };
    let compile_claim = |value: u8| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!("boundary machine trusted_zero() -> u64\nensures result == {value};\n"),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#,
        );
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("accepted boundary claim should check");
        project_checked_package_review(&checked).expect("accepted boundary contract review")
    };

    let zero = compile_claim(0);
    let boundary = zero
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Boundary)
        .expect("boundary callable row");
    let [contract] = boundary.contracts() else {
        panic!("one exact accepted contract row")
    };
    assert_eq!(contract.kind(), PackageReviewContractKind::Ensures);
    assert_eq!(contract.binding(), None);
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        operator,
        left,
        right,
    }) = contract.fact()
    else {
        panic!("exact equality expression")
    };
    assert_eq!(*operator, PackageReviewContractBinaryOperator::Equal);
    assert_eq!(**left, PackageReviewContractExpression::Result);
    assert_eq!(
        **right,
        PackageReviewContractExpression::Integer("0".to_owned())
    );

    let one = compile_claim(1);
    assert_ne!(
        zero.canonical_review_bytes().expect("zero claim encoding"),
        one.canonical_review_bytes().expect("one claim encoding"),
        "changing an accepted guarantee must change exact review evidence",
    );
}

#[test]
fn review_projects_exact_public_domain_membership_contracts() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub domain u64::Trusted;

pub machine consume(value: u64)
requires value in u64::Trusted
{ }
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("checked public-domain membership requirement should check");
    let review = project_checked_package_review(&checked).expect("membership contract review");
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("public callable row");
    let [contract] = callable.contracts() else {
        panic!("one exact membership contract")
    };
    let PackageReviewContractFact::Membership { value, domain } = contract.fact() else {
        panic!("exact membership row")
    };
    assert_eq!(*value, PackageReviewContractExpression::Parameter(0));
    assert_eq!(domain.path(), "u64::Trusted");
    assert_eq!(
        domain.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );

    let hidden = TempPackage::new();
    hidden.write(
        "main.omg",
        r#"domain u64::Hidden;
pub machine consume(value: u64)
requires value in u64::Hidden
{ }
"#,
    );
    hidden.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &hidden.0.join("main.omg"),
        Some(target),
        package_inputs(&hidden.0),
    )
    .expect("private-domain contract fixture should check before package review");
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("a public callable must not expose a private domain");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("exposes non-public domain `u64::Hidden`")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn review_projects_structural_propositions_and_alpha_normalizes_their_binders() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    original.write(
        "main.omg",
        r#"proposition equivalent<Element>(left: Element, right: Element);
pub machine compare<Value>(left: Value, right: Value)
requires equivalent<Value>(left, right)
{ }
"#,
    );
    renamed.write(
        "main.omg",
        r#"proposition equivalent<Item>(left: Item, right: Item);
pub machine compare<Compared>(left: Compared, right: Compared)
requires equivalent<Compared>(left, right)
{ }
"#,
    );
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#;
    original.write("build.omg", build);
    renamed.write("build.omg", build);

    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("generic proposition fixture should check");
        project_checked_package_review(&checked).expect("generic proposition review")
    };
    let original = project(&original);
    let renamed = project(&renamed);
    let compare = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("compare"))
        .expect("public comparison callable");
    let [contract] = compare.contracts() else {
        panic!("one proposition contract")
    };
    assert_eq!(contract.evidence_lane_position(), None);
    let PackageReviewContractFact::Proposition(application) = contract.fact() else {
        panic!("exact proposition application")
    };
    assert_eq!(application.declaration().path(), "equivalent");
    let [binder] = application.binders() else {
        panic!("one proposition binder")
    };
    assert_eq!(binder.kind(), &PackageReviewPropositionBinderKind::Type);
    let [argument] = application.binder_arguments() else {
        panic!("one proposition binder argument")
    };
    assert_eq!(
        argument.value(),
        &PackageReviewPropositionBinderValue::GenericBinder(0)
    );
    assert_eq!(application.parameter_types().len(), 2);
    assert_eq!(
        application.arguments(),
        [
            PackageReviewContractExpression::Parameter(0),
            PackageReviewContractExpression::Parameter(1),
        ]
    );
    assert_eq!(
        application.evidence(),
        &PackageReviewPropositionEvidence::FactOnly
    );
    assert_eq!(
        original
            .canonical_review_bytes()
            .expect("original encoding"),
        renamed.canonical_review_bytes().expect("renamed encoding"),
        "renaming callable and proposition binders must not alter package evidence",
    );
}

#[test]
fn review_projects_named_witness_interfaces_through_transparent_aliases() {
    let Some(target) = host_target_name() else {
        return;
    };
    let direct = TempPackage::new();
    let aliased = TempPackage::new();
    let direct_source = r#"pub trait EvidenceBase<Element> {
    machine inherited(value: Element);
}
pub trait Evidence<Element>: EvidenceBase<Element> {
    machine witness(value: Element);
}
proposition carries<Element>(value: Element) evidence Evidence<Element>;
pub machine consume()
requires proof: carries<i32>(1)
{ }
"#;
    let aliased_source = r#"pub trait EvidenceBase<Element> {
    machine inherited(value: Element);
}
pub trait Evidence<Element>: EvidenceBase<Element> {
    machine witness(value: Element);
}
proposition carries<Element>(value: Element) evidence Evidence<Element>;
proposition forwarded<Item>(value: Item) = carries<Item>(value);
pub machine consume()
requires evidence: forwarded<i32>(1)
{ }
"#;
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#;
    direct.write("main.omg", direct_source);
    direct.write("build.omg", build);
    aliased.write("main.omg", aliased_source);
    aliased.write("build.omg", build);

    let compile = |package: &TempPackage| {
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("named witness fixture should check")
    };
    let direct_checked = compile(&direct);
    let direct_review =
        project_checked_package_review(&direct_checked).expect("direct witness review");
    let aliased_review =
        project_checked_package_review(&compile(&aliased)).expect("aliased witness review");
    let consume = direct_review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("consume"))
        .expect("public consumer");
    let [contract] = consume.contracts() else {
        panic!("one named witness contract")
    };
    assert_eq!(
        contract.binding(),
        None,
        "a named requires spelling is a callee-local alias"
    );
    assert_eq!(contract.evidence_lane_position(), Some(0));
    let PackageReviewContractFact::Proposition(application) = contract.fact() else {
        panic!("witness proposition application")
    };
    assert_eq!(application.declaration().path(), "carries");
    let PackageReviewPropositionEvidence::Witness(interface) = application.evidence() else {
        panic!("witness interface")
    };
    assert_eq!(interface.trait_identity().path(), "Evidence");
    assert_eq!(interface.arguments().len(), 1);
    assert_eq!(interface.requirements().len(), 2);
    assert!(interface.requirements().iter().any(|requirement| {
        requirement.declaring_trait().path() == "Evidence"
            && requirement.requirement().path().contains("witness")
            && requirement.declaring_trait_arguments().len() == 1
    }));
    assert!(interface.requirements().iter().any(|requirement| {
        requirement.declaring_trait().path() == "EvidenceBase"
            && requirement.requirement().path().contains("inherited")
            && requirement.declaring_trait_arguments().len() == 1
    }));
    assert_eq!(
        direct_review
            .canonical_review_bytes()
            .expect("direct witness encoding"),
        aliased_review
            .canonical_review_bytes()
            .expect("aliased witness encoding"),
        "a transparent proposition alias and local requires-binding rename must not mint package identity",
    );

    let mut diagnostic_spoof = compile(&direct);
    let term_handles = diagnostic_spoof
        .facts
        .proof
        .evidence_terms
        .iter()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    for handle in term_handles {
        let term = diagnostic_spoof.facts.proof.evidence_terms.get_mut(handle);
        term.evidence_type = "spoofed diagnostic evidence".to_owned();
        term.proposition
            .arguments
            .fill("spoofed argument".to_owned());
        for argument in &mut term.proposition.binder_arguments {
            argument.identity = "spoofed binder".to_owned();
        }
        if let Some(interface) = term.evidence_interface.as_mut() {
            interface.arguments.fill("spoofed interface".to_owned());
            for requirement in &mut interface.requirements {
                requirement
                    .declaring_trait_arguments
                    .fill("spoofed requirement".to_owned());
            }
        }
    }
    let spoofed_review = project_checked_package_review(&diagnostic_spoof)
        .expect("diagnostic strings are not review identity");
    assert_eq!(
        direct_review
            .canonical_review_bytes()
            .expect("structural witness encoding"),
        spoofed_review
            .canonical_review_bytes()
            .expect("spoofed diagnostic witness encoding"),
        "checked diagnostic strings must not influence package evidence",
    );
}

#[test]
fn named_evidence_lane_order_changes_canonical_review_identity() {
    let Some(target) = host_target_name() else {
        return;
    };
    let first = TempPackage::new();
    let second = TempPackage::new();
    let prefix = r#"pub trait Evidence {}
proposition left_fact() evidence Evidence;
proposition right_fact() evidence Evidence;
"#;
    first.write(
        "main.omg",
        &format!(
            "{prefix}pub machine consume()\nrequires left: left_fact()\nrequires right: right_fact()\n{{ }}\n"
        ),
    );
    second.write(
        "main.omg",
        &format!(
            "{prefix}pub machine consume()\nrequires right: right_fact()\nrequires left: left_fact()\n{{ }}\n"
        ),
    );
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);
    let encode = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("named evidence lane fixture should check");
        project_checked_package_review(&checked)
            .expect("named evidence lane review")
            .canonical_review_bytes()
            .expect("named evidence lane encoding")
    };
    assert_ne!(
        encode(&first),
        encode(&second),
        "reordering positional erased proof inputs must change package evidence",
    );
}

#[test]
fn review_projects_proof_static_evidence_members_by_lane_and_requirement() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    let source = |binding: &str| {
        format!(
            r#"pub trait EvidenceBase<Element> {{
    machine modulus() -> Element;
}}
pub trait Evidence<Element>: EvidenceBase<Element> {{
}}
proposition holds<Element>() evidence Evidence<Element>;
proposition selected<machine Witness>();
pub machine caller()
requires {binding}: holds<i32>()
requires selected<{binding}.modulus>()
{{ }}
"#
        )
    };
    original.write("main.omg", &source("proof"));
    renamed.write("main.omg", &source("evidence"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#;
    original.write("build.omg", build);
    renamed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("proof-static projection fixture should check");
        project_checked_package_review(&checked).expect("proof-static projection review")
    };
    let original = project(&original);
    let renamed = project(&renamed);
    let caller = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("caller"))
        .expect("public caller");
    let selected = caller
        .contracts()
        .iter()
        .find_map(|contract| {
            let PackageReviewContractFact::Proposition(application) = contract.fact() else {
                return None;
            };
            (application.declaration().path() == "selected").then_some(application)
        })
        .expect("selected proposition row");
    let holds = caller
        .contracts()
        .iter()
        .find_map(|contract| {
            let PackageReviewContractFact::Proposition(application) = contract.fact() else {
                return None;
            };
            (application.declaration().path() == "holds").then_some(application)
        })
        .expect("source witness proposition row");
    let [argument] = selected.binder_arguments() else {
        panic!("one projected static machine argument")
    };
    let PackageReviewPropositionBinderValue::EvidenceProjection {
        source_kind,
        source_lane_position,
        declaring_trait,
        declaring_trait_arguments,
        requirement,
    } = argument.value()
    else {
        panic!("exact proof-static evidence projection")
    };
    assert_eq!(*source_kind, PackageReviewContractKind::Requires);
    assert_eq!(*source_lane_position, 0);
    assert_eq!(declaring_trait.path(), "EvidenceBase");
    assert!(requirement.path().contains("modulus"));
    let PackageReviewPropositionEvidence::Witness(source_interface) = holds.evidence() else {
        panic!("source witness interface")
    };
    let source_requirement = source_interface
        .requirements()
        .iter()
        .find(|candidate| candidate.requirement() == requirement)
        .expect("inherited source requirement");
    assert_eq!(
        declaring_trait_arguments,
        source_requirement.declaring_trait_arguments(),
        "the projection must retain the exact inherited requirement template anchored by the source lane",
    );
    assert_eq!(
        original
            .canonical_review_bytes()
            .expect("original proof-static encoding"),
        renamed
            .canonical_review_bytes()
            .expect("renamed proof-static encoding"),
        "renaming the local evidence term must not alter its lane-based package identity",
    );
}

#[test]
fn review_rejects_unrepresented_callable_contract_expressions() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"machine computed_zero() -> u64 { 0 }
boundary machine trusted_zero() -> u64
ensures result == computed_zero();
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("effect-free build-time contract call should check");
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("unrepresented contract expression must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("contract expression form not yet represented")
    }));
}

#[test]
fn review_projects_contract_member_paths_with_exact_receivers_and_fields() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let changed = TempPackage::new();
    let source = |left_receiver: &str, right_receiver: &str| {
        format!(
            r#"pub data Pair [copy] {{
    left: i32;
    right: i32;
}}
pub machine compare(first: Pair, second: Pair)
requires {left_receiver}.left == {right_receiver}.right
{{ }}
"#
        )
    };
    original.write("main.omg", &source("first", "second"));
    changed.write("main.omg", &source("second", "first"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#;
    original.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("member-path contract fixture should check");
        project_checked_package_review(&checked).expect("member-path package review")
    };
    let original = project(&original);
    let changed = project(&changed);
    let compare = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "compare")
        .expect("public comparison callable");
    let [contract] = compare.contracts() else {
        panic!("one member-path contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        left,
        right,
        ..
    }) = contract.fact()
    else {
        panic!("binary member-path contract")
    };
    let PackageReviewContractExpression::Member {
        receiver: left_receiver,
        member: left_member,
        case_variant: left_variant,
    } = left.as_ref()
    else {
        panic!("left member path")
    };
    let PackageReviewContractExpression::Member {
        receiver: right_receiver,
        member: right_member,
        case_variant: right_variant,
    } = right.as_ref()
    else {
        panic!("right member path")
    };
    assert_eq!(
        left_receiver.as_ref(),
        &PackageReviewContractExpression::Parameter(0)
    );
    assert_eq!(
        right_receiver.as_ref(),
        &PackageReviewContractExpression::Parameter(1)
    );
    assert_eq!(left_member.path(), "Pair::left");
    assert_eq!(right_member.path(), "Pair::right");
    assert!(left_variant.is_none());
    assert!(right_variant.is_none());
    assert_eq!(
        left_member.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_ne!(
        original
            .canonical_review_bytes()
            .expect("original member-path encoding"),
        changed
            .canonical_review_bytes()
            .expect("changed member-path encoding"),
        "changing only the receiver coordinates must change package review identity",
    );
}

#[test]
fn review_projects_contract_casts_without_diagnostic_spelling() {
    let Some(target) = host_target_name() else {
        return;
    };
    let u16_cast = TempPackage::new();
    let u32_cast = TempPackage::new();
    let source = |target_type: &str| {
        format!(
            r#"pub machine compare(value: u8)
requires (value as {target_type}) == 1
{{ }}
"#
        )
    };
    u16_cast.write("main.omg", &source("u16"));
    u32_cast.write("main.omg", &source("u32"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#;
    u16_cast.write("build.omg", build);
    u32_cast.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("exact widening cast contract should check");
        project_checked_package_review(&checked).expect("cast contract package review")
    };
    let u16_cast = project(&u16_cast);
    let u32_cast = project(&u32_cast);
    let compare = u16_cast
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "compare")
        .expect("public comparison callable");
    let [contract] = compare.contracts() else {
        panic!("one cast contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        left, ..
    }) = contract.fact()
    else {
        panic!("binary cast contract")
    };
    let PackageReviewContractExpression::Cast {
        value,
        target,
        arithmetic_domain,
        semantic_domain,
        semantic_domain_arguments,
        form,
    } = left.as_ref()
    else {
        panic!("structural cast expression")
    };
    assert_eq!(
        value.as_ref(),
        &PackageReviewContractExpression::Parameter(0)
    );
    assert!(target.canonical().contains("u16"));
    assert_eq!(*arithmetic_domain, PackageReviewArithmeticDomain::Exact);
    assert!(semantic_domain.is_none());
    assert!(semantic_domain_arguments.is_empty());
    assert_eq!(*form, PackageReviewCastForm::Value);
    assert_ne!(
        u16_cast
            .canonical_review_bytes()
            .expect("u16 cast encoding"),
        u32_cast
            .canonical_review_bytes()
            .expect("u32 cast encoding"),
        "changing the exact cast target must change package review identity",
    );
}

#[test]
fn review_casts_retain_public_semantic_domains_and_reject_private_exposure() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#;
    let public = TempPackage::new();
    public.write(
        "main.omg",
        r#"pub domain u16::Tagged;
pub machine compare(value: u8)
requires (value as u16 in Tagged) == 1
{ }
"#,
    );
    public.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &public.0.join("main.omg"),
        Some(target),
        package_inputs(&public.0),
    )
    .expect("public semantic-domain cast contract should check");
    let review = project_checked_package_review(&checked).expect("public domain cast review");
    let compare = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "compare")
        .expect("public comparison callable");
    let [contract] = compare.contracts() else {
        panic!("one public-domain cast contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        left, ..
    }) = contract.fact()
    else {
        panic!("binary public-domain cast contract")
    };
    let PackageReviewContractExpression::Cast {
        semantic_domain: Some(domain),
        ..
    } = left.as_ref()
    else {
        panic!("semantic domain cast identity")
    };
    assert_eq!(domain.path(), "u16::Tagged");
    assert_eq!(
        domain.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );

    let private = TempPackage::new();
    private.write(
        "main.omg",
        r#"domain u16::Hidden;
pub machine compare(value: u8)
requires (value as u16 in Hidden) == 1
{ }
"#,
    );
    private.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &private.0.join("main.omg"),
        Some(target),
        package_inputs(&private.0),
    )
    .expect("private semantic-domain cast contract should check before package review");
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("a public contract must not expose a private semantic domain");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("exposes non-public semantic domain")
    }));
}

#[test]
fn public_callable_signatures_are_exact_and_lifetime_alpha_normalized() {
    let Some(target) = host_target_name() else {
        return;
    };
    let original = TempPackage::new();
    let renamed = TempPackage::new();
    let changed = TempPackage::new();
    original.write(
        "main.omg",
        r#"pub machine borrow<'source, 'temporary>(
    source: &'source [u8],
    temporary: &'temporary [u8]
) -> &'source [u8] { source }
pub machine identity<Element [copy]>(value: Element) -> Element { value }
"#,
    );
    renamed.write(
        "main.omg",
        r#"pub machine borrow<'origin, 'scratch>(
    source: &'origin [u8],
    temporary: &'scratch [u8]
) -> &'origin [u8] { source }
pub machine identity<Value [copy]>(value: Value) -> Value { value }
"#,
    );
    changed.write(
        "main.omg",
        r#"pub machine borrow<'source, 'temporary>(
    source: &'source [u8],
    temporary: &'temporary [u8]
) -> &'temporary [u8] { temporary }
pub machine identity<Element [copy]>(value: Element) -> Element { value }
"#,
    );
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#;
    original.write("build.omg", build);
    renamed.write("build.omg", build);
    changed.write("build.omg", build);

    let review = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("public callable signature fixture should check");
        project_checked_package_review(&checked).expect("callable signature review should close")
    };
    let original = review(&original);
    let renamed = review(&renamed);
    let changed = review(&changed);
    let borrow = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("borrow"))
        .expect("borrow callable row");
    assert_eq!(borrow.lifetime_parameter_count(), 2);
    assert_eq!(borrow.parameters().len(), 2);
    assert!(borrow.type_parameters().is_empty());
    assert!(!borrow.return_type().canonical().is_empty());
    let identity = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("identity"))
        .expect("generic identity callable row");
    assert_eq!(identity.type_parameters().len(), 1);
    assert_eq!(identity.parameters().len(), 1);

    assert_eq!(
        original
            .canonical_review_bytes()
            .expect("original encoding"),
        renamed.canonical_review_bytes().expect("renamed encoding"),
        "renaming lifetime and type binders must not alter canonical review evidence",
    );
    assert_ne!(
        original
            .canonical_review_bytes()
            .expect("original encoding"),
        changed.canonical_review_bytes().expect("changed encoding"),
        "changing the result's borrow relationship must alter canonical review evidence",
    );
}

#[test]
fn review_projects_exact_public_callable_conformances_and_rejects_unrepresented_forms() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#;
    let satisfying = TempPackage::new();
    satisfying.write(
        "main.omg",
        r#"pub trait Handler<Element> { machine handle(value: Element) -> Element; }
pub machine handle(value: u32) -> u32 satisfies Handler<u32>::handle { value }
"#,
    );
    satisfying.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &satisfying.0.join("main.omg"),
        Some(target),
        package_inputs(&satisfying.0),
    )
    .expect("public satisfier fixture should check");
    let review = project_checked_package_review(&checked).expect("public conformance review");
    let handle = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("handle"))
        .expect("public handle row");
    let [conformance] = handle.conformances() else {
        panic!("one exact public callable conformance")
    };
    assert_eq!(conformance.trait_identity().path(), "Handler");
    assert!(conformance.requirement_identity().path().contains("handle"));
    assert_eq!(conformance.arguments().len(), 1);
    assert_eq!(conformance.alias(), None);

    let hidden = TempPackage::new();
    hidden.write(
        "main.omg",
        r#"trait Hidden { machine handle(); }
pub machine handle() satisfies Hidden::handle { }
"#,
    );
    hidden.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &hidden.0.join("main.omg"),
        Some(target),
        package_inputs(&hidden.0),
    )
    .expect("private-trait satisfier fixture should check");
    let diagnostics = project_checked_package_review(&checked).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("realizes non-public trait `Hidden`")
    }));

    let generic = TempPackage::new();
    generic.write(
        "main.omg",
        r#"pub machine register<machine Selected>()
where machine Selected();
{ }
"#,
    );
    generic.write("build.omg", build);
    let checked = compile_to_checked_with_packages(
        &generic.0.join("main.omg"),
        Some(target),
        package_inputs(&generic.0),
    )
    .expect("public static-machine fixture should check");
    let diagnostics = project_checked_package_review(&checked).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("conformance bounds not yet represented")
            || diagnostic
                .message
                .contains("static machine or proposition parameter not yet represented")
    }));
}

#[test]
fn public_machine_visibility_survives_checked_compilation_and_strict_empty_contracts() {
    let package = TempPackage::new();
    package.write("main.omg", "pub machine Package::entry() { }\n");
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public machine should check");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Package::entry")
        .expect("checked public machine");
    assert!(machine.is_public);
    assert_eq!(
        machine.supply_mode,
        psi_language_semantics::MachineSupplyMode::CheckedBody
    );

    let service = checked
        .facts
        .service_reaches
        .for_machine(machine.symbol)
        .expect("checked service contract");
    assert!(matches!(
        service.interface,
        psi_language_semantics::ServiceReachInterface::PublishedCeiling(_)
    ));
    let invocation = checked
        .facts
        .synchronous_invocations
        .for_machine(machine.symbol)
        .expect("checked invocation contract");
    assert_eq!(
        invocation.interface,
        psi_language_semantics::SynchronousInvocationInterface::PublishedCeiling
    );
    assert!(matches!(
        checked
            .facts
            .suspensions
            .for_machine(machine.symbol)
            .expect("checked suspension contract")
            .interface,
        psi_language_semantics::SuspensionInterface::PublishedMaySuspend(false)
    ));
    assert!(matches!(
        checked
            .facts
            .blocking
            .for_machine(machine.symbol)
            .expect("checked blocking contract")
            .interface,
        psi_language_semantics::BlockingInterface::PublishedMayBlock(false)
    ));
    assert_eq!(
        checked
            .facts
            .contract_plans
            .for_machine(machine.symbol)
            .expect("checked contract")
            .crash
            .interface(),
        psi_checked_trees::CrashInterface::PublishedCeiling
    );
}

#[test]
fn public_machine_cannot_hide_realized_reach_invocation_or_operational_effects() {
    let cases = [
        (
            "invocation",
            r#"boundary trait Handler { machine handle(); }
pub machine public_api(handler: &mut Handler) { handler.handle(); }
"#,
            &["omits `invokes handler;`"][..],
        ),
        (
            "operational",
            r#"boundary trait Waiting { machine wait() reaches Waiting suspends; blocks; }
pub machine public_api(waiting: &mut Waiting)
reaches Waiting
invokes waiting;
{
    suspend block waiting.wait();
}
"#,
            &["omits `suspends;`", "omits `blocks;`"][..],
        ),
        (
            "crash",
            r#"pub machine public_api() { crash Abort; }
"#,
            &["crash"][..],
        ),
    ];

    for (label, source, expected_messages) in cases {
        let package = TempPackage::new();
        package.write("main.omg", source);
        let diagnostics = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            None,
            package_inputs(&package.0),
        )
        .unwrap_err();
        for expected in expected_messages {
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(expected)),
                "{label} omission should mention `{expected}`: {diagnostics:#?}"
            );
        }
    }
}

#[test]
fn exact_synchronous_invocations_change_v21_comparison_encoding() {
    let quiet = TempPackage::new();
    let invoking = TempPackage::new();
    quiet.write(
        "main.omg",
        r#"boundary trait Handler { machine handle(); }
boundary trait Host { machine ping() reaches Host; }
pub machine dispatch(handler: &mut Handler)
reaches Host
invokes handler;
invokes Host;
{ }
"#,
    );
    invoking.write(
        "main.omg",
        r#"boundary trait Handler { machine handle(); }
boundary trait Host { machine ping() reaches Host; }
pub machine dispatch(handler: &mut Handler)
invokes handler;
invokes Host;
{
    handler.handle();
    Host::ping();
}
"#,
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#;
    quiet.write("build.omg", build);
    invoking.write("build.omg", build);

    let compile = |package: &TempPackage| {
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("invocation comparison fixture should check")
    };
    let quiet = project_checked_package_review(&compile(&quiet)).expect("quiet review");
    let invoking = project_checked_package_review(&compile(&invoking)).expect("invoking review");
    let dispatch = invoking
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("public dispatch row");
    let quiet_dispatch = quiet
        .callables()
        .iter()
        .find(|callable| callable.role() == PackageReviewCallableRole::Public)
        .expect("quiet public dispatch row");
    let declared = dispatch
        .declared_synchronous_invocations()
        .expect("published invocation ceiling");
    assert_eq!(declared.len(), 2);
    assert_eq!(
        declared[0],
        PackageReviewSynchronousInvocation::Parameter(0)
    );
    let PackageReviewSynchronousInvocation::Service(service) = &declared[1] else {
        panic!("second exact invocation should be a service identity")
    };
    assert_eq!(service.path(), "Host");
    assert_eq!(
        service.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(
        quiet_dispatch.declared_synchronous_invocations(),
        Some(declared)
    );
    assert!(quiet_dispatch.realized_synchronous_invocations().is_empty());
    assert_eq!(quiet_dispatch.contracts(), dispatch.contracts());
    assert_eq!(dispatch.realized_synchronous_invocations(), declared);
    assert_ne!(
        quiet.canonical_review_bytes().expect("quiet encoding"),
        invoking
            .canonical_review_bytes()
            .expect("invoking encoding")
    );
}

#[test]
fn review_rejects_target_free_and_standalone_checked_programs() {
    let package = TempPackage::new();
    package.write("main.omg", "machine local() { }\n");

    let target_free = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        None,
        package_inputs(&package.0),
    )
    .expect("target-free package fixture should check");
    let diagnostics = project_checked_package_review(&target_free)
        .expect_err("review must require an explicit target");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires one explicit target selection")
    }));

    let standalone = omega_compiler::compile_to_checked(&package.0.join("main.omg"), None)
        .expect("standalone fixture should check");
    let diagnostics = project_checked_package_review(&standalone)
        .expect_err("review must require package-aware compilation");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requires package-aware checked compilation")
    }));
}

#[test]
fn review_distinguishes_profiles_that_share_a_native_target() {
    let package = TempPackage::new();
    package.write("main.omg", "machine local() { }\n");
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target uefi_x64 { }
machine build(builder: &mut Build) { }
"#,
    );

    let windows = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("Windows review fixture should check");
    let uefi = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("uefi_x64"),
        package_inputs(&package.0),
    )
    .expect("UEFI review fixture should check");

    assert_eq!(
        windows.selected_native_target(),
        uefi.selected_native_target()
    );
    let windows = project_checked_package_review(&windows).expect("Windows review projection");
    let uefi = project_checked_package_review(&uefi).expect("UEFI review projection");
    assert_eq!(windows.target(), omega_target::TargetProfile::WindowsX64);
    assert_eq!(uefi.target(), omega_target::TargetProfile::UefiX64);
    assert_ne!(windows.target(), uefi.target());
    assert_ne!(
        windows.canonical_review_bytes().expect("Windows encoding"),
        uefi.canonical_review_bytes().expect("UEFI encoding"),
    );
}

#[test]
fn review_encoding_ignores_unreviewed_arena_insertion_order() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    first.write("main.omg", "boundary machine host_ping();\n");
    second.write(
        "main.omg",
        "machine unrelated() { }\nboundary machine host_ping();\n",
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let compile = |package: &TempPackage| {
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("arena-order fixture should check")
    };
    let first = project_checked_package_review(&compile(&first))
        .expect("first arena-order review")
        .canonical_review_bytes()
        .expect("first arena-order encoding");
    let second = project_checked_package_review(&compile(&second))
        .expect("second arena-order review")
        .canonical_review_bytes()
        .expect("second arena-order encoding");

    assert_eq!(first, second);
}

#[test]
fn public_data_and_numbered_wire_shape_changes_change_v21_comparison_encoding() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    first.write(
        "main.omg",
        "pub data Packet [copy] { #1 value: u32; }\ndata Private { ignored: u32; }\n",
    );
    second.write(
        "main.omg",
        "pub data Packet [copy] { #1 value: u64; }\ndata Private { changed: i64; }\n",
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let encode = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public-shape fixture should check");
        project_checked_package_review(&checked)
            .expect("public-shape review should close")
            .canonical_review_bytes()
            .expect("public-shape encoding")
    };

    assert_ne!(encode(&first), encode(&second));
}

#[test]
fn public_domain_shape_changes_change_v21_comparison_encoding() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    first.write(
        "main.omg",
        "pub data Packet { value: u32; }\npub domain Packet::Ready;\n",
    );
    second.write(
        "main.omg",
        "pub data Packet { value: u32; }\npub domain Packet::Prepared;\n",
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let encode = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public-domain fixture should check");
        project_checked_package_review(&checked)
            .expect("public-domain review should close")
            .canonical_review_bytes()
            .expect("public-domain encoding")
    };

    assert_ne!(encode(&first), encode(&second));
}

#[test]
fn public_domain_generic_binders_are_alpha_normalized() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    first.write(
        "main.omg",
        r#"pub data Unit { code: u32; }
pub domain<Carrier, const Index: Unit> Carrier::Tagged<Index>;
"#,
    );
    second.write(
        "main.omg",
        r#"pub data Unit { code: u32; }
pub domain<Value, const Tag: Unit> Value::Tagged<Tag>;
"#,
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let encode = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("generic public-domain fixture should check");
        project_checked_package_review(&checked)
            .expect("generic public-domain review should close")
            .canonical_review_bytes()
            .expect("generic public-domain encoding")
    };

    assert_eq!(encode(&first), encode(&second));
}

#[test]
fn public_domain_classification_and_establishment_routes_are_exact_review_rows() {
    let classified = TempPackage::new();
    let routed = TempPackage::new();
    classified.write(
        "main.omg",
        r#"pub data SchedulerHandle { id: u64; }
pub domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
"#,
    );
    routed.write(
        "main.omg",
        r#"pub data SchedulerHandle { id: u64; }
pub domain SchedulerHandle::WeakFair
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
"#,
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#;
    classified.write("build.omg", build);
    routed.write("build.omg", build);

    let compile = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("routed public-domain fixture should check");
        project_checked_package_review(&checked).expect("routed public-domain review should close")
    };
    let classified_review = compile(&classified);
    let [domain] = classified_review.public_domains() else {
        panic!("one classified public domain row")
    };
    assert_eq!(
        domain.classification(),
        Some(PackageReviewDomainClassification::ProgressProfile)
    );
    let [route] = domain.establishment_routes() else {
        panic!("one exact establishment route")
    };
    assert_eq!(
        route.kind(),
        PackageReviewDomainEstablishmentKind::BoundaryRequirement
    );
    assert_eq!(route.trait_identity().path(), "SchedulerAdmission");
    assert_eq!(
        route.requirement_identity().path(),
        "SchedulerAdmission::grant"
    );

    assert_ne!(
        classified_review
            .canonical_review_bytes()
            .expect("classified public-domain encoding"),
        compile(&routed)
            .canonical_review_bytes()
            .expect("unclassified routed public-domain encoding")
    );
}

#[test]
fn public_domain_establishment_route_order_is_canonical() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    let source = |routes: &str| {
        format!(
            r#"pub data SchedulerHandle {{ id: u64; }}
pub domain SchedulerHandle::Scheduled
established by {routes};
pub boundary trait PrimaryAdmission {{
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in Scheduled;
}}
pub boundary trait BackupAdmission {{
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in Scheduled;
}}
"#
        )
    };
    first.write(
        "main.omg",
        &source("PrimaryAdmission::grant, BackupAdmission::grant"),
    );
    second.write(
        "main.omg",
        &source("BackupAdmission::grant, PrimaryAdmission::grant"),
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let encode = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("multi-route public-domain fixture should check");
        project_checked_package_review(&checked)
            .expect("multi-route public-domain review should close")
            .canonical_review_bytes()
            .expect("multi-route public-domain encoding")
    };

    assert_eq!(encode(&first), encode(&second));
}

#[test]
fn public_domain_aliases_flatten_to_canonical_package_qualified_atoms() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    first.write(
        "main.omg",
        r#"pub data Socket { descriptor: u64; }
pub domain Socket::Connected;
pub domain Socket::Authenticated;
pub domain Socket::Trusted = Socket::Authenticated;
pub domain Socket::Usable = Socket::Connected & Socket::Trusted;
pub domain u64::Portable = Carry::Portable;
"#,
    );
    second.write(
        "main.omg",
        r#"pub data Socket { descriptor: u64; }
pub domain Socket::Connected;
pub domain Socket::Authenticated;
pub domain Socket::Trusted = Socket::Authenticated;
pub domain Socket::Usable = Socket::Trusted & Socket::Connected;
pub domain u64::Portable = Carry::Portable;
"#,
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let compile = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public-domain alias fixture should check");
        project_checked_package_review(&checked).expect("public-domain alias review should close")
    };
    let first_review = compile(&first);
    let usable = first_review
        .public_domains()
        .iter()
        .find(|domain| domain.identity().path() == "Socket::Usable")
        .expect("usable alias row");
    let usable_atoms = usable.alias_expansion().expect("usable alias expansion");
    assert_eq!(
        usable_atoms
            .iter()
            .map(|atom| atom.path())
            .collect::<Vec<_>>(),
        ["Socket::Authenticated", "Socket::Connected"]
    );
    assert!(usable_atoms.iter().all(|atom| {
        matches!(
            atom.owner(),
            PackageReviewNominalOwner::Package(identity) if identity == package_identity()
        )
    }));

    let portable = first_review
        .public_domains()
        .iter()
        .find(|domain| domain.identity().path() == "u64::Portable")
        .expect("portable alias row");
    let portable_atoms = portable
        .alias_expansion()
        .expect("portable alias expansion");
    assert_eq!(portable_atoms.len(), 4);
    assert!(portable_atoms.iter().all(|atom| {
        matches!(atom.owner(), PackageReviewNominalOwner::ToolchainUnbound)
            && atom.path().starts_with("Carry::")
    }));

    assert_eq!(
        first_review
            .canonical_review_bytes()
            .expect("first alias encoding"),
        compile(&second)
            .canonical_review_bytes()
            .expect("reordered alias encoding")
    );
}

#[test]
fn public_trait_shape_retains_boundary_parent_and_alpha_normalized_requirements() {
    let first = TempPackage::new();
    let second = TempPackage::new();
    first.write(
        "main.omg",
        r#"pub trait Parent<Element> {
    operator < compare(left: Element, right: Element) -> bool;
}
pub boundary trait Service<Element>: Parent<Element> {
    machine Self::exchange(&mut self, item: Element) -> Element;
}
"#,
    );
    second.write(
        "main.omg",
        r#"pub trait Parent<Value> {
    operator < compare(left: Value, right: Value) -> bool;
}
pub boundary trait Service<Value>: Parent<Value> {
    machine Self::exchange(&mut self, item: Value) -> Value;
}
"#,
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#;
    first.write("build.omg", build);
    second.write("build.omg", build);

    let compile = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public-trait fixture should check");
        project_checked_package_review(&checked).expect("public-trait review should close")
    };
    let first_review = compile(&first);
    let parent_shape = first_review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Parent")
        .expect("parent trait row");
    let [compare] = parent_shape.requirements() else {
        panic!("one fixed-operator requirement")
    };
    assert_eq!(
        compare.spelling(),
        Some(psi_language_core::OperatorSpelling::Less)
    );
    let service = first_review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Service")
        .expect("service trait row");
    assert!(service.is_boundary());
    assert_eq!(service.type_parameters().len(), 1);
    let [parent] = service.parents() else {
        panic!("one exact parent edge")
    };
    assert_eq!(
        parent.kind(),
        psi_typed_trees::trait_definition::TraitCompositionKind::Policy
    );
    assert_eq!(parent.identity().path(), "Parent");
    assert_eq!(parent.arguments().len(), 1);
    let [exchange] = service.requirements() else {
        panic!("one exact requirement row")
    };
    assert_eq!(exchange.identity().path(), "Service::exchange");
    assert!(exchange.spelling().is_none());
    assert!(exchange.type_parameters().is_empty());
    let [receiver, item] = exchange.parameters() else {
        panic!("receiver and item parameters")
    };
    assert!(receiver.is_self());
    assert!(receiver.is_mutable());
    assert!(!receiver.is_const());
    assert!(receiver.type_identity().canonical().contains("trait-self"));
    assert_eq!(item.name(), "item");
    assert!(!item.is_self());
    assert_eq!(
        item.type_identity().canonical(),
        exchange.return_type().canonical()
    );

    assert_eq!(
        first_review
            .canonical_review_bytes()
            .expect("first public-trait encoding"),
        compile(&second)
            .canonical_review_bytes()
            .expect("renamed-binder public-trait encoding")
    );
}

#[test]
fn public_lifetime_contracts_are_alpha_normalized_and_relationship_sensitive() {
    let first = TempPackage::new();
    let renamed = TempPackage::new();
    let changed = TempPackage::new();
    first.write(
        "main.omg",
        r#"pub data View<'left, 'right> {
    first: &'left [u8];
    second: &'right [u8];
}
pub trait Parent<'source> {
    machine borrow<'temporary>(source: &'source [u8], temporary: &'temporary [u8]) -> &'source [u8];
}
pub trait Child<'child>: Parent<'child> { }
"#,
    );
    renamed.write(
        "main.omg",
        r#"pub data View<'primary, 'secondary> {
    first: &'primary [u8];
    second: &'secondary [u8];
}
pub trait Parent<'origin> {
    machine borrow<'scratch>(source: &'origin [u8], temporary: &'scratch [u8]) -> &'origin [u8];
}
pub trait Child<'region>: Parent<'region> { }
"#,
    );
    changed.write(
        "main.omg",
        r#"pub data View<'left, 'right> {
    first: &'left [u8];
    second: &'left [u8];
}
pub trait Parent<'source> {
    machine borrow<'temporary>(source: &'source [u8], temporary: &'temporary [u8]) -> &'temporary [u8];
}
pub trait Child<'child>: Parent<'child> { }
"#,
    );
    let build = r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#;
    first.write("build.omg", build);
    renamed.write("build.omg", build);
    changed.write("build.omg", build);

    let compile = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public lifetime fixture should check");
        project_checked_package_review(&checked).expect("public lifetime review should close")
    };
    let first_review = compile(&first);
    let view = first_review
        .public_data()
        .iter()
        .find(|shape| shape.identity().path() == "View")
        .expect("view data row");
    assert_eq!(view.lifetime_parameter_count(), 2);
    let [
        PackageReviewDataMember::Field(first_field),
        PackageReviewDataMember::Field(second_field),
    ] = view.members()
    else {
        panic!("two view fields")
    };
    assert_ne!(
        first_field.type_identity().canonical(),
        second_field.type_identity().canonical()
    );

    let parent = first_review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Parent")
        .expect("parent trait row");
    assert_eq!(parent.lifetime_parameter_count(), 1);
    let [borrow] = parent.requirements() else {
        panic!("borrow requirement")
    };
    assert_eq!(borrow.lifetime_parameter_count(), 1);
    let [source, temporary] = borrow.parameters() else {
        panic!("borrow parameters")
    };
    assert_ne!(
        source.type_identity().canonical(),
        temporary.type_identity().canonical()
    );
    assert_eq!(
        source.type_identity().canonical(),
        borrow.return_type().canonical()
    );

    let child = first_review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Child")
        .expect("child trait row");
    let [parent_edge] = child.parents() else {
        panic!("parent edge")
    };
    assert_eq!(parent_edge.lifetime_arguments(), &[0]);

    let first_bytes = first_review
        .canonical_review_bytes()
        .expect("first lifetime encoding");
    assert_eq!(
        first_bytes,
        compile(&renamed)
            .canonical_review_bytes()
            .expect("renamed lifetime encoding")
    );
    assert_ne!(
        first_bytes,
        compile(&changed)
            .canonical_review_bytes()
            .expect("changed lifetime encoding")
    );
}

#[test]
fn public_trait_lifetime_declarations_validate_before_review() {
    for (source, expected) in [
        (
            "pub trait Parent<'left, 'right> { }\npub trait Child<'child>: Parent<'child> { }\n",
            "expects 2 lifetime argument(s), got 1",
        ),
        (
            "pub trait Parent<'source> { }\npub trait Child<'child>: Parent<'ghost> { }\n",
            "uses undeclared lifetime argument `'ghost'",
        ),
        (
            "pub trait Parent<'source> { machine borrow<'source>(value: &'source [u8]) -> &'source [u8]; }\n",
            "redeclares inherited lifetime `'source'",
        ),
    ] {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write(
            "build.omg",
            "target windows_x64 { }\nmachine build(builder: &mut Build) { }\n",
        );
        let diagnostics = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect_err("invalid parent lifetime application must reject");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "missing `{expected}` in {diagnostics:#?}"
        );
    }
}

#[test]
fn review_rejects_public_data_semantics_that_lack_canonical_rows() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Ledger
where
    count <= len,
{
    len: u32;
    count: u32;
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public data fact fixture should check");
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("review must not silently omit public data facts");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("uses proof facts not yet represented by package review")
    }));
}

#[test]
fn review_rejects_public_domain_semantics_that_lack_canonical_rows() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Packet { value: u32; }
pub domain Packet::Ready
    requires self.value == 0;
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public domain fact fixture should check");
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("review must not silently omit public domain facts");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("uses predicate facts not yet represented by package review")
    }));
}

#[test]
fn public_trait_operational_envelope_is_exact_review_shape() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary trait Console { }
pub boundary trait Worker {
    machine wait(handler: &mut Console)
    reaches <= Console
    invokes handler;
    invokes Console;
    suspends;
    blocks;
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public trait suspension fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public trait operational review should close");
    let worker = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "Worker")
        .expect("worker trait row");
    let [wait] = worker.requirements() else {
        panic!("one worker requirement")
    };
    let [console] = wait.service_reach() else {
        panic!("one exact service-reach row")
    };
    assert_eq!(console.path(), "Console");
    assert_eq!(
        console.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert!(wait.service_reach_is_installation_bound());
    assert_eq!(wait.synchronous_invocations().len(), 2);
    assert_eq!(wait.synchronous_invocations()[0].parameter(), Some(0));
    assert_eq!(
        wait.synchronous_invocations()[1]
            .service()
            .expect("service invocation")
            .path(),
        "Console"
    );
    assert!(wait.suspends());
    assert!(wait.blocks());
}

#[test]
fn public_trait_termination_is_parameter_rooted_review_shape() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub boundary trait SchedulerRuntime {
    machine wait(&self, scheduler: SchedulerRuntime)
    requires self in WeakFair
    requires scheduler in WeakFair
    terminates;
}
pub domain SchedulerRuntime::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerRuntime) -> SchedulerRuntime in WeakFair;
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("public trait termination fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("public trait termination review should close");
    let runtime = review
        .public_traits()
        .iter()
        .find(|shape| shape.identity().path() == "SchedulerRuntime")
        .expect("scheduler runtime trait row");
    let wait = runtime
        .requirements()
        .iter()
        .find(|requirement| requirement.identity().path() == "SchedulerRuntime::wait")
        .expect("wait requirement row");
    let premises = wait
        .termination()
        .premises()
        .expect("wait must promise termination");
    assert_eq!(premises.len(), 2);
    for premise in premises {
        assert_eq!(premise.profile().path(), "SchedulerRuntime::WeakFair");
        assert_eq!(
            premise.profile().owner(),
            PackageReviewNominalOwner::Package(package_identity())
        );
        assert!(premise.projections().is_empty());
    }
    assert!(premises[0].subject().is_receiver());
    assert_eq!(premises[1].subject().parameter(), Some(0));
}

#[test]
fn public_trait_termination_rejects_a_non_public_progress_profile() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data SchedulerHandle { }
domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
pub boundary trait SchedulerRuntime {
    machine wait(scheduler: SchedulerHandle)
    requires scheduler in WeakFair
    terminates;
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&package.0),
    )
    .expect("private progress profile fixture should check");
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("review must reject a private profile in a public trait contract");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("exposes non-public progress profile")
    }));
}

#[test]
fn review_rejects_unprojected_public_trait_contract_lanes() {
    let default_package = TempPackage::new();
    default_package.write(
        "main.omg",
        r#"pub trait Worker {
    machine wait() { }
}
"#,
    );
    default_package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &default_package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&default_package.0),
    )
    .expect("public trait default fixture should check");
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("review must not omit a public trait default realization");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("default realization not yet joined to package review")
    }));

    let precondition_package = TempPackage::new();
    precondition_package.write(
        "main.omg",
        r#"pub data SchedulerHandle { }
pub domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
pub boundary trait SchedulerRuntime {
    machine wait(scheduler: SchedulerHandle)
    requires scheduler in WeakFair;
}
"#,
    );
    precondition_package.write(
        "build.omg",
        r#"target windows_x64 { }
machine build(builder: &mut Build) { }
"#,
    );
    let checked = compile_to_checked_with_packages(
        &precondition_package.0.join("main.omg"),
        Some("windows_x64"),
        package_inputs(&precondition_package.0),
    )
    .expect("public progress precondition fixture should check");
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("a progress precondition without termination remains a proof contract");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("uses proof, boundary, or crash contracts")
    }));
}

#[test]
fn review_rejects_contract_entailment_stand_downs() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"machine unchecked_claim(a: u64, b: u64)
requires
    min(a, b) >= 1
ensures
    a >= 1
{
}
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { }
"#,
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("ordinary checking should retain the out-of-language stand-down");
    let [stand_down] = checked.contract_entailment_stand_downs() else {
        panic!("one exact contract-entailment stand-down")
    };
    assert_eq!(stand_down.contract_index, 1);
    assert_eq!(stand_down.fact_index, 0);
    assert_eq!(
        stand_down.reason,
        psi_validation::ContractEntailmentStandDownReason::OutsideEntailmentLanguage
    );

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("package review must fail closed on an unresolved stand-down");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rejects unresolved contract-entailment stand-down")
    }));
}

fn host_target_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("windows_x64"),
        ("linux", "x86_64") => Some("linux_x64"),
        ("linux", "aarch64") => Some("linux_arm64"),
        ("macos", "aarch64") => Some("macos_arm64"),
        _ => None,
    }
}
