use crate::support::*;

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
machine ping_leaf() satisfies Host::ping via Binding::DllImport("omega-test", "host_ping");
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
        r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build)
crashes Abort
{
    builder.package("review-fixture");
    builder.accept_boundary<Host>();
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
    let observations = checked
        .build_observation_summary()
        .expect("selected build machine publishes build observation evidence");
    assert_eq!(observations.ceiling(), BuildObservationClass::Hermetic);
    assert_eq!(observations.realized(), BuildObservationClass::Hermetic);
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
    let rows = review
        .canonical_rows()
        .expect("review projection should have canonical comparison rows");
    assert_eq!(
        rows,
        review
            .canonical_rows()
            .expect("repeated row encoding must be deterministic")
    );
    assert!(rows.windows(2).all(|pair| {
        (pair[0].kind(), pair[0].key_bytes()) < (pair[1].kind(), pair[1].key_bytes())
    }));
    assert!(
        rows.iter()
            .any(|row| row.kind() == PackageReviewCanonicalRowKind::ProjectionHeader)
    );
    assert!(
        rows.iter()
            .any(|row| row.kind() == PackageReviewCanonicalRowKind::Callable)
    );
    let row_magic = b"OMEGA-PACKAGE-REVIEW-ROW\0";
    for row in &rows {
        assert!(row.canonical_bytes().starts_with(row_magic));
        assert_eq!(
            &row.canonical_bytes()[row_magic.len()..row_magic.len() + 2],
            &PACKAGE_REVIEW_ROW_ENCODING_VERSION.to_le_bytes()
        );
    }

    assert_eq!(review.package(), package_identity());
    assert_eq!(
        review.target().target_name(),
        target,
        "review identity must retain the deployment profile, not only its native ABI",
    );
    assert_eq!(PACKAGE_REVIEW_ENCODING_VERSION, 96);
    assert_eq!(PACKAGE_REVIEW_ROW_ENCODING_VERSION, 54);
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
        boundary.checked_service_reach(),
        &PackageReviewCheckedServiceReach::NoCheckedBody
    );
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
    assert!(
        builder
            .type_identity()
            .canonical()
            .contains("toolchain-source-owner"),
        "source-backed Build must retain its exact toolchain source owner: {}",
        builder.type_identity().canonical(),
    );
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
    assert!(matches!(
        public.checked_service_reach(),
        PackageReviewCheckedServiceReach::CheckedBody {
            realized,
            concrete,
        } if realized.is_empty() && concrete.is_empty()
    ));
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
    assert_eq!(published_crash.cause(), PackageReviewCrashCause::Abort);
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
    assert_eq!(crash_site.cause(), PackageReviewCrashCause::Abort);
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
    let mut helper_calls = crash
        .checked_calls()
        .iter()
        .filter(|call| call.target_machine().path() == "helper");
    let crash_call = helper_calls
        .next()
        .expect("one normalized helper crash call");
    assert!(
        helper_calls.next().is_none(),
        "the helper crash route must remain unique"
    );
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
    let [grant] = provider.grants() else {
        panic!("one exact selected-provider grant")
    };
    assert_eq!(
        grant.selector_kind(),
        PackageReviewProviderGrantSelectorKind::ProviderSlot
    );
    assert_eq!(
        grant.selected_plan_digest(),
        checked.selected_provider_plans().plans()[0]
            .identity_digest()
            .as_bytes()
    );
    assert_eq!(provider.schema_declaration().path(), "Host");
    assert_eq!(
        provider.schema_declaration().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(provider.provider_type_package(), None);
    assert_eq!(provider.provider_type_declaration(), None);
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
    let [provider_declarations] = provider.row_declarations() else {
        panic!("one exact requirement/realization declaration pair")
    };
    assert_eq!(
        provider_declarations.requirement().path(),
        provider.schema().methods[0].requirement_identity
    );
    assert_eq!(
        provider_declarations.requirement().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_eq!(provider_declarations.realization().path(), "ping_leaf");
    assert_eq!(
        provider_declarations.realization().owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert!(matches!(
        &provider.rows()[0].binding,
        omega_effects::provider_plan::ProviderBinding::StringBackedImportBootstrap {
            library,
            symbol,
        } if library == "omega-test" && symbol == "host_ping"
    ));
    let provider_row = rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::SelectedProviderSet)
        .expect("selected provider canonical row");
    assert_eq!(
        provider_row.source().compiler_derivations(),
        [
            PackageReviewSyntheticSourceKind::UniqueCoveringProviderSelection,
            PackageReviewSyntheticSourceKind::FreeExternalProviderType,
        ]
    );
    let provider_locations = provider_row
        .source()
        .authored_locations()
        .expect("implicit provider still retains authored schema and realization provenance");
    assert!(provider_locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::ProviderGrant
            && location.relative_path() == "build.omg"
    }));
    assert!(provider_locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::ProviderSchemaDeclaration
            && location.relative_path() == "main.omg"
    }));
    assert!(provider_locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::ProviderRequirementDeclaration
            && location.relative_path() == "main.omg"
    }));
    assert!(provider_locations.iter().any(|location| {
        location.role() == PackageReviewSourceLocationRole::ProviderRealization
            && location.relative_path() == "main.omg"
    }));
    assert!(!provider_locations.iter().any(|location| matches!(
        location.role(),
        PackageReviewSourceLocationRole::ProviderSelection
            | PackageReviewSourceLocationRole::ProviderTypeDeclaration
    )));
}

#[test]
fn review_projects_plan_name_provider_grant() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"boundary trait Host { machine ping(); }
machine ping_leaf() satisfies Host::ping via Binding::DllImport("omega-test", "host_ping");
"#,
    );
    package.write(
        "build.omg",
        &format!(
            r#"target windows_x86_64 {{ }}
target linux_x86_64 {{ }}
target linux_arm64 {{ }}
target macos_arm64 {{ }}
machine build(builder: &mut Build) {{
    builder.package("review-fixture");
    builder.accept_boundary<{target}::satisfies::Host>();
}}
"#,
        ),
    );

    let checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("plan-name provider grant fixture should check");
    let review = project_checked_package_review(&checked)
        .expect("plan-name provider grant should project exactly");
    let [provider] = review.selected_providers() else {
        panic!("one selected provider")
    };
    let [grant] = provider.grants() else {
        panic!("one selected-provider grant")
    };
    assert_eq!(
        grant.selector_kind(),
        PackageReviewProviderGrantSelectorKind::PlanName
    );
    assert_eq!(
        grant.selected_plan_digest(),
        checked.selected_provider_plans().plans()[0]
            .identity_digest()
            .as_bytes()
    );
    let granted_bytes = review
        .canonical_review_bytes()
        .expect("granted provider review encodes canonically");

    package.write(
        "build.omg",
        r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let unchecked_grant = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("otherwise identical provider fixture without a grant should check");
    let ungranted_review = project_checked_package_review(&unchecked_grant)
        .expect("ungranted selected provider should still project");
    assert!(ungranted_review.selected_providers()[0].grants().is_empty());
    assert_ne!(
        granted_bytes,
        ungranted_review
            .canonical_review_bytes()
            .expect("ungranted provider review encodes canonically"),
        "an authored provider grant must change canonical package evidence",
    );
}

#[test]
fn review_projects_exact_accepted_boundary_contracts() {
    let Some(target) = host_target_name() else {
        return;
    };
    let zero_source = "boundary machine trusted_zero() -> u64\nensures result == 0;\n";
    let compile_claim = |value: u8| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!("boundary machine trusted_zero() -> u64\nensures result == {value};\n"),
        );
        package.write(
            "build.omg",
            r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
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
    assert_eq!(
        boundary.supply(),
        PackageReviewCallableSupply::AdmissionClaim,
        "a bodyless boundary guarantee must remain an explicit trust-bearing accepted claim",
    );
    assert_eq!(
        boundary.checked_service_reach(),
        &PackageReviewCheckedServiceReach::NoCheckedBody
    );
    assert!(zero.dangerous_authority_slack().is_empty());
    let [contract] = boundary.contracts() else {
        panic!("one exact accepted contract row")
    };
    assert_eq!(contract.kind(), PackageReviewContractKind::Ensures);
    assert_eq!(contract.binding(), None);
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        meaning,
        operator,
        left,
        right,
    }) = contract.fact()
    else {
        panic!("exact equality expression")
    };
    assert_eq!(meaning, &PackageReviewContractOperatorMeaning::Builtin);
    assert_eq!(*operator, PackageReviewContractBinaryOperator::Equal);
    assert_eq!(**left, PackageReviewContractExpression::Result);
    assert_eq!(
        **right,
        PackageReviewContractExpression::Integer("0".to_owned())
    );
    let zero_rows = zero.canonical_rows().expect("zero claim rows");
    let accepted_claims = zero_rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::AcceptedClaim)
        .collect::<Vec<_>>();
    let [accepted_claim] = accepted_claims.as_slice() else {
        panic!("one explicit accepted-claim row")
    };
    assert_eq!(
        accepted_claim.risk(),
        PackageReviewCanonicalRowRisk::Blocking
    );
    assert!(
        accepted_claim
            .key_bytes()
            .windows("trusted_zero".len())
            .any(|window| window == b"trusted_zero")
    );
    let claim_locations = accepted_claim
        .source()
        .authored_locations()
        .expect("accepted claim declaration and contract source");
    assert!(claim_locations.iter().any(|location| {
        location.relative_path() == "main.omg"
            && location.role() == PackageReviewSourceLocationRole::Declaration
    }));
    assert!(claim_locations.iter().any(|location| {
        let start = usize::try_from(location.start_byte()).unwrap();
        let end = usize::try_from(location.end_byte()).unwrap();
        location.relative_path() == "main.omg"
            && location.role() == PackageReviewSourceLocationRole::ContractClause
            && &zero_source[start..end] == "ensures"
    }));
    let recovered_claim = decode_package_review_canonical_row(
        &encode_package_review_canonical_row(accepted_claim).expect("encode accepted claim row"),
    )
    .expect("recover accepted claim row");
    assert!(
        recovered_claim
            .source()
            .authored_locations()
            .is_some_and(|locations| locations.iter().any(|location| {
                location.role() == PackageReviewSourceLocationRole::ContractClause
            }))
    );

    let one = compile_claim(1);
    let one_rows = one.canonical_rows().expect("one claim rows");
    let one_claim = one_rows
        .iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::AcceptedClaim)
        .expect("changed accepted claim row");
    assert_ne!(
        accepted_claim.canonical_bytes(),
        one_claim.canonical_bytes(),
        "changing an accepted guarantee must change its trust-bearing row",
    );
    assert_ne!(
        zero.canonical_review_bytes().expect("zero claim encoding"),
        one.canonical_review_bytes().expect("one claim encoding"),
        "changing an accepted guarantee must change exact review evidence",
    );
}
