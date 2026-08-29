mod support;

use support::*;

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
machine build(builder: &mut Build) { builder.package("review-fixture"); }
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
    let membership_row = review
        .canonical_rows()
        .expect("membership canonical rows")
        .into_iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::Callable
                && row
                    .key_bytes()
                    .windows("consume".len())
                    .any(|window| window == b"consume")
        })
        .expect("public consume callable row");
    assert!(
        membership_row
            .source()
            .authored_locations()
            .is_some_and(|locations| locations.iter().any(|location| {
                location.role() == PackageReviewSourceLocationRole::ContractClause
            }))
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
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let diagnostics = compile_to_checked_with_packages(
        &hidden.0.join("main.omg"),
        Some(target),
        package_inputs(&hidden.0),
    )
    .expect_err("ordinary visibility must reject a private domain in a public contract");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private domain `u64::Hidden`")
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
        r#"pub proposition equivalent<Element>(left: Element, right: Element);
pub machine compare<Value>(left: Value, right: Value)
requires equivalent<Value>(left, right)
{ }
"#,
    );
    renamed.write(
        "main.omg",
        r#"pub proposition equivalent<Item>(left: Item, right: Item);
pub machine compare<Compared>(left: Compared, right: Compared)
requires equivalent<Compared>(left, right)
{ }
"#,
    );
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
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
fn review_projects_unused_public_proposition_declarations_without_granting_facts() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    let source = r#"pub proposition ready();
pub proposition reflexive(value: i32) = value == value;
proposition hidden();
"#;
    package.write("main.omg", source);
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
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
    .expect("public proposition declarations should check");
    assert!(
        checked
            .facts
            .proof
            .proposition_vocabulary
            .applications
            .is_empty(),
        "publishing a bodyless proposition declaration must not manufacture an application fact"
    );

    let review = project_checked_package_review(&checked).expect("public proposition review");
    assert_eq!(review.public_propositions().len(), 2);
    let ready = review
        .public_propositions()
        .iter()
        .find(|shape| shape.identity().path() == "ready")
        .expect("unused public primitive proposition row");
    assert_eq!(ready.body(), &PackageReviewPublicPropositionBody::Primitive);
    let reflexive = review
        .public_propositions()
        .iter()
        .find(|shape| shape.identity().path() == "reflexive")
        .expect("public transparent proposition row");
    assert!(matches!(
        reflexive.body(),
        PackageReviewPublicPropositionBody::Transparent(PackageReviewContractFact::Expression(
            PackageReviewContractExpression::Binary { .. }
        ))
    ));
    let rows = review
        .canonical_rows()
        .expect("canonical public proposition rows");
    let proposition_rows = rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::PublicProposition)
        .count();
    assert_eq!(
        proposition_rows, 2,
        "private propositions stay out of public API rows"
    );
    let reflexive_row = rows
        .iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::PublicProposition
                && row
                    .key_bytes()
                    .windows("reflexive".len())
                    .any(|window| window == b"reflexive")
        })
        .expect("transparent proposition row");
    let locations = reflexive_row
        .source()
        .authored_locations()
        .expect("transparent proposition source custody");
    let formula = locations
        .iter()
        .find(|location| location.role() == PackageReviewSourceLocationRole::PropositionFormula)
        .expect("transparent proposition formula location");
    let start = usize::try_from(formula.start_byte()).unwrap();
    let end = usize::try_from(formula.end_byte()).unwrap();
    assert_eq!(&source[start..end], "value == value");
    let ready_row = rows
        .iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::PublicProposition
                && row
                    .key_bytes()
                    .windows("ready".len())
                    .any(|window| window == b"ready")
        })
        .expect("primitive proposition row");
    assert!(
        ready_row
            .source()
            .authored_locations()
            .unwrap()
            .iter()
            .all(|location| location.role() != PackageReviewSourceLocationRole::PropositionFormula)
    );
}

#[test]
fn review_projects_unused_public_consts_with_exact_type_and_value_identity() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let project = |source: &str| {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write("build.omg", build);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("public const declaration should check");
        project_checked_package_review(&checked).expect("public const review")
    };

    let original_source = "pub const LIMIT: u64 = 4;\nconst HIDDEN_LIMIT: u64 = 2;\n";
    let original = project(original_source);
    let changed_value = project("pub const LIMIT: u64 = 5;\n");
    let changed_type = project("pub const LIMIT: u32 = 4;\n");
    let relocated = project("\n\npub const LIMIT: u64 = 4;\n");

    let [limit] = original.public_consts() else {
        panic!("private consts must stay out of public compatibility rows");
    };
    assert_eq!(limit.identity().path(), "LIMIT");
    assert!(limit.declared_type().canonical().contains("u64"));
    assert!(!limit.canonical_value_encoding().is_empty());
    let rows = original
        .canonical_rows()
        .expect("canonical public const rows");
    let const_rows = rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConst)
        .collect::<Vec<_>>();
    assert_eq!(const_rows.len(), 1);
    assert_eq!(
        const_rows[0].risk(),
        PackageReviewCanonicalRowRisk::Blocking
    );
    let locations = const_rows[0]
        .source()
        .authored_locations()
        .expect("public const source locations");
    assert_eq!(locations.len(), 2);
    let source_for_role = |role| {
        let location = locations
            .iter()
            .find(|location| location.role() == role)
            .expect("exact public const source role");
        let start = usize::try_from(location.start_byte()).unwrap();
        let end = usize::try_from(location.end_byte()).unwrap();
        &original_source[start..end]
    };
    assert_eq!(
        source_for_role(PackageReviewSourceLocationRole::Declaration),
        "LIMIT"
    );
    assert_eq!(
        source_for_role(PackageReviewSourceLocationRole::ConstInitializer),
        "4"
    );
    let original_initializer_start = locations
        .iter()
        .find(|location| location.role() == PackageReviewSourceLocationRole::ConstInitializer)
        .unwrap()
        .start_byte();
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_value.canonical_review_bytes().unwrap(),
        "changing a public const value must change package compatibility",
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_type.canonical_review_bytes().unwrap(),
        "changing a public const declared type must change package compatibility",
    );
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        relocated.canonical_review_bytes().unwrap(),
        "relocating identical const semantics must not change canonical review identity",
    );
    let relocated_row = relocated
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicConst)
        .expect("relocated public const row");
    let relocated_initializer = relocated_row
        .source()
        .authored_locations()
        .unwrap()
        .iter()
        .find(|location| location.role() == PackageReviewSourceLocationRole::ConstInitializer)
        .expect("relocated initializer location");
    assert_eq!(
        relocated_initializer.start_byte(),
        original_initializer_start + 2
    );
}

#[test]
fn review_projects_unused_public_operator_overloads_and_exact_contract_meaning() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Token [copy] { value: u64; }
pub operator < Token::less(left: Token, right: Token) -> bool;
pub operator Token::ordered(left: Token, right: Token) -> bool
ensures result == (left < right)
crashes Trap
    left < right
    left < right
crashes Abort;
operator Token::hidden(value: Token) -> bool;
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
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
    .expect("unused public operators should check");
    let review = project_checked_package_review(&checked)
        .expect("unused public operators should project directly from declarations");
    assert_eq!(review.public_operators().len(), 2);
    let less = review
        .public_operators()
        .iter()
        .find(|operator| operator.coordinate().identity().path() == "Token::less")
        .expect("fixed-token overload row");
    assert_eq!(
        less.spelling(),
        Some(psi_language_core::OperatorSpelling::Less)
    );
    assert_eq!(less.parameters().len(), 2);
    assert!(less.coordinate().result_dispatch().is_empty());
    assert!(less.published_crash().is_empty());

    let ordered = review
        .public_operators()
        .iter()
        .find(|operator| operator.coordinate().identity().path() == "Token::ordered")
        .expect("named operator row");
    let [contract] = ordered.contracts() else {
        panic!("one exact public operator contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        meaning: PackageReviewContractOperatorMeaning::Builtin,
        right,
        ..
    }) = contract.fact()
    else {
        panic!("outer equality uses one compiler-owned builtin meaning")
    };
    let PackageReviewContractExpression::Binary {
        meaning: PackageReviewContractOperatorMeaning::Declared(selected),
        operator: PackageReviewContractBinaryOperator::Less,
        ..
    } = right.as_ref()
    else {
        panic!("inner comparison retains one exact declared overload")
    };
    assert_eq!(selected, less.coordinate());
    let [trap, abort] = ordered.published_crash() else {
        panic!("one guarded Trap and one unconditional Abort bucket")
    };
    assert_eq!(trap.cause(), psi_checked_trees::CrashCause::Trap);
    let [
        PackageReviewCrashRouteGuard::Expression(PackageReviewContractExpression::Binary {
            meaning: PackageReviewContractOperatorMeaning::Declared(selected_crash_operator),
            operator: PackageReviewContractBinaryOperator::Less,
            ..
        }),
    ] = trap.alternative_guards()
    else {
        panic!("duplicate guarded routes canonicalize to one exact declared-operator expression")
    };
    assert_eq!(selected_crash_operator, less.coordinate());
    assert_eq!(abort.cause(), psi_checked_trees::CrashCause::Abort);
    assert_eq!(
        abort.alternative_guards(),
        [PackageReviewCrashRouteGuard::Truth]
    );

    let rows = review.canonical_rows().expect("public operator rows");
    let operator_rows = rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::PublicOperator)
        .collect::<Vec<_>>();
    assert_eq!(operator_rows.len(), 2);
    assert!(operator_rows.iter().all(|row| {
        row.risk() == PackageReviewCanonicalRowRisk::Blocking
            && row.source().authored_locations().is_some()
    }));

    let ordered_symbol = checked
        .operators()
        .iter()
        .find(|operator| operator.is_public && !operator.contracts.is_empty())
        .map(|operator| operator.symbol)
        .expect("checked ordered operator declaration");
    let owner = psi_checked_trees::ContractProofFactOwner::OperatorDeclaration {
        operator_symbol: ordered_symbol,
    };
    let (checked_contract_handle, checked_contract) = checked
        .facts
        .proof
        .contract_facts
        .iter()
        .find(|(_, fact)| fact.owner == owner)
        .map(|(handle, fact)| (handle, fact.clone()))
        .expect("one checked operator-declaration contract row");

    let assert_owner_row_rejects = |checked: &CheckedCompilation, count: usize| {
        let diagnostics = project_checked_package_review(checked)
            .expect_err("malformed operator-declaration custody must reject review");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(&format!(
                "contract fact has {count} checked owner rows; expected one"
            ))
        }));
    };

    let mut missing = checked.clone();
    assert!(
        missing
            .facts
            .proof
            .contract_facts
            .free(checked_contract_handle)
    );
    assert_owner_row_rejects(&missing, 0);

    let mut duplicate = checked.clone();
    duplicate
        .facts
        .proof
        .contract_facts
        .append(checked_contract.clone());
    assert_owner_row_rejects(&duplicate, 2);

    let mut wrong_owner = checked.clone();
    wrong_owner
        .facts
        .proof
        .contract_facts
        .get_mut(checked_contract_handle)
        .owner = psi_checked_trees::ContractProofFactOwner::Unknown;
    assert_owner_row_rejects(&wrong_owner, 0);
}

#[test]
fn public_operator_crash_routes_are_canonical_sensitive_and_checked() {
    let compile = |routes: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub data Token [copy] {{ value: u64; }}
pub operator / divide(left: Token, right: Token) -> Token
crashes Trap;
pub operator Token::checked(value: Token, flag: bool) -> bool
{routes};
"#,
            ),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x64"),
            package_inputs(&package.0),
        )
        .expect("public operator crash fixture should check")
    };
    let project = |routes: &str| {
        project_checked_package_review(&compile(routes))
            .expect("public operator crash routes should project")
    };

    let original_routes = r#"crashes Trap
    flag
    flag
crashes Abort"#;
    let reordered_routes = r#"crashes Abort
crashes Trap
    flag"#;
    let original = project(original_routes);
    let reordered = project(reordered_routes);
    assert_eq!(
        original.canonical_review_bytes().unwrap(),
        reordered.canonical_review_bytes().unwrap(),
        "clause order and duplicate guards must not change operator crash identity",
    );
    let checked_operator = original
        .public_operators()
        .iter()
        .find(|operator| operator.coordinate().identity().path() == "Token::checked")
        .expect("domain-homed checked operator");
    let [trap, abort] = checked_operator.published_crash() else {
        panic!("one guarded Trap and one unconditional Abort bucket")
    };
    assert_eq!(trap.cause(), psi_checked_trees::CrashCause::Trap);
    assert_eq!(
        trap.alternative_guards(),
        [PackageReviewCrashRouteGuard::Expression(
            PackageReviewContractExpression::Parameter(1)
        )]
    );
    assert_eq!(abort.cause(), psi_checked_trees::CrashCause::Abort);
    assert_eq!(
        abort.alternative_guards(),
        [PackageReviewCrashRouteGuard::Truth]
    );
    let divide = original
        .public_operators()
        .iter()
        .find(|operator| operator.coordinate().identity().path() == "divide")
        .expect("root fixed-token operator");
    assert_eq!(divide.published_crash().len(), 1);

    let changed_guard = project(
        r#"crashes Trap
    !flag
crashes Abort"#,
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_guard.canonical_review_bytes().unwrap(),
        "changing a guarded route must change operator review identity",
    );
    let changed_cause = project(
        r#"crashes Abort
    flag"#,
    );
    assert_ne!(
        original.canonical_review_bytes().unwrap(),
        changed_cause.canonical_review_bytes().unwrap(),
        "changing the crash cause must change operator review identity",
    );

    let operator_row = original
        .canonical_rows()
        .unwrap()
        .into_iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::PublicOperator
                && row
                    .key_bytes()
                    .windows("Token::checked".len())
                    .any(|window| window == "Token::checked".as_bytes())
        })
        .expect("checked operator canonical row");
    let encoded = encode_package_review_canonical_row(&operator_row).unwrap();
    let decoded = decode_package_review_canonical_row(&encoded).unwrap();
    assert_eq!(decoded.kind(), operator_row.kind());
    assert_eq!(decoded.risk(), operator_row.risk());
    assert_eq!(decoded.key_bytes(), operator_row.key_bytes());
    assert_eq!(decoded.canonical_bytes(), operator_row.canonical_bytes());
    assert_eq!(decoded.source(), operator_row.source());

    let checked = compile(original_routes);
    let mut missing = checked.clone();
    missing.facts.operators.operator_crash_contracts.clear();
    let diagnostics = project_checked_package_review(&missing)
        .expect_err("missing checked operator crash rows must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-crash evidence does not equal compiler rederivation")
    }));

    let mut duplicate = checked;
    let duplicate_row = duplicate.facts.operators.operator_crash_contracts[0].clone();
    duplicate
        .facts
        .operators
        .operator_crash_contracts
        .push(duplicate_row);
    let diagnostics = project_checked_package_review(&duplicate)
        .expect_err("duplicate checked operator crash rows must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-crash evidence does not equal compiler rederivation")
    }));
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
pub proposition carries<Element>(value: Element) evidence Evidence<Element>;
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
pub proposition carries<Element>(value: Element) evidence Evidence<Element>;
pub proposition forwarded<Item>(value: Item) = carries<Item>(value);
pub machine consume()
requires evidence: forwarded<i32>(1)
{ }
"#;
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
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
    let forwarded_row = aliased_review
        .canonical_rows()
        .expect("aliased proposition rows")
        .into_iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::PublicProposition
                && row
                    .key_bytes()
                    .windows("forwarded".len())
                    .any(|window| window == b"forwarded")
        })
        .expect("transparent proposition application row");
    let forwarded_formula = forwarded_row
        .source()
        .authored_locations()
        .unwrap()
        .iter()
        .find(|location| location.role() == PackageReviewSourceLocationRole::PropositionFormula)
        .expect("transparent proposition application source");
    let start = usize::try_from(forwarded_formula.start_byte()).unwrap();
    let end = usize::try_from(forwarded_formula.end_byte()).unwrap();
    assert_eq!(&aliased_source[start..end], "carries<Item>(value)");
    let consume = direct_review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("consume"))
        .expect("public consumer");
    let [contract] = consume.contracts() else {
        panic!("one named witness contract")
    };
    let consume_row = direct_review
        .canonical_rows()
        .expect("named witness canonical rows")
        .into_iter()
        .find(|row| {
            row.kind() == PackageReviewCanonicalRowKind::Callable
                && row
                    .key_bytes()
                    .windows("consume".len())
                    .any(|window| window == b"consume")
        })
        .expect("named witness callable row");
    assert!(
        consume_row
            .source()
            .authored_locations()
            .is_some_and(|locations| locations.iter().any(|location| {
                let start = usize::try_from(location.start_byte()).unwrap();
                let end = usize::try_from(location.end_byte()).unwrap();
                location.role() == PackageReviewSourceLocationRole::ContractClause
                    && &direct_source[start..end] == "requires"
            }))
    );
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
    let [binder_argument] = application.binder_arguments() else {
        panic!("one witness proposition type argument")
    };
    let PackageReviewPropositionBinderValue::Type(type_identity) = binder_argument.value() else {
        panic!("concrete proposition type argument must use structural type identity")
    };
    assert!(type_identity.canonical().contains("compiler-type"));
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
    assert_ne!(
        direct_review
            .canonical_review_bytes()
            .expect("direct witness encoding"),
        aliased_review
            .canonical_review_bytes()
            .expect("aliased witness encoding"),
        "a published transparent alias is a distinct source API row even though contract semantic identity expands through it",
    );
    let direct_contract = direct_review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("consume"))
        .expect("direct public consumer")
        .contracts();
    let aliased_contract = aliased_review
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("consume"))
        .expect("aliased public consumer")
        .contracts();
    assert_eq!(
        direct_contract, aliased_contract,
        "transparent alias expansion must preserve the consuming contract's semantic row"
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
pub proposition left_fact() evidence Evidence;
pub proposition right_fact() evidence Evidence;
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
machine build(builder: &mut Build) { builder.package("review-fixture"); }
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
pub proposition holds<Element>() evidence Evidence<Element>;
pub proposition selected<machine Witness>();
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
machine build(builder: &mut Build) { builder.package("review-fixture"); }
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
fn review_projects_exact_concrete_machine_arguments_in_contract_calls() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    let changed = TempPackage::new();
    let source = |selected: &str| {
        format!(
            r#"pub machine chosen(value: u64) -> u64 {{ value }}
pub machine alternate(value: u64) -> u64 {{ value }}
pub machine apply<machine Selected>(value: u64) -> u64
where machine Selected(value: u64) -> u64
{{
    Selected(value)
}}
boundary machine trusted_zero() -> u64
ensures result == apply<{selected}>(0);
"#,
        )
    };
    package.write("main.omg", &source("chosen"));
    changed.write("main.omg", &source("alternate"));
    let build = r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    package.write("build.omg", build);
    changed.write("build.omg", build);
    let project = |package: &TempPackage| {
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("effect-free static contract call should check");
        project_checked_package_review(&checked)
            .expect("an exact concrete machine argument has a canonical contract row")
    };
    let review = project(&package);
    let changed = project(&changed);
    let trusted_zero = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "trusted_zero")
        .expect("trusted boundary callable");
    let [contract] = trusted_zero.contracts() else {
        panic!("one trusted-zero contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("trusted-zero equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("static apply call")
    };
    let [PackageReviewContractStaticArgument::ConcreteMachine(selected)] =
        static_arguments.as_slice()
    else {
        panic!("one exact concrete machine argument")
    };
    assert_eq!(selected.path(), "chosen::entry");
    assert_eq!(
        selected.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    assert_ne!(
        review
            .canonical_review_bytes()
            .expect("chosen-machine contract encoding"),
        changed
            .canonical_review_bytes()
            .expect("alternate-machine contract encoding"),
        "changing an exact concrete static-machine selection must change package-review identity",
    );
}

#[test]
fn review_projects_contract_machine_binders_by_canonical_static_ordinal() {
    let Some(target) = host_target_name() else {
        return;
    };
    let compile = |binder: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub machine apply<machine Selected>(value: u64) -> u64
where machine Selected(value: u64) -> u64
{{
    Selected(value)
}}
pub machine trusted_apply<machine {binder}>(value: u64) -> u64
where machine {binder}(value: u64) -> u64;
requires apply<{binder}>(value) == apply<{binder}>(value)
{{
    0
}}
"#,
            ),
        );
        package.write(
            "build.omg",
            r#"target windows_x64 { }
target linux_x64 { }
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
        .expect("generic public contract fixture should check");
        project_checked_package_review(&checked)
            .expect("a forwarded machine binder has a canonical contract row")
    };
    let original = compile("Operation");
    let renamed = compile("RenamedOperation");
    let trusted_apply = original
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "trusted_apply")
        .expect("trusted generic public callable");
    let [contract] = trusted_apply.contracts() else {
        panic!("one trusted-apply contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        right,
        ..
    }) = contract.fact()
    else {
        panic!("trusted-apply equality contract")
    };
    let PackageReviewContractExpression::Call {
        static_arguments, ..
    } = right.as_ref()
    else {
        panic!("generic apply call")
    };
    assert_eq!(
        static_arguments,
        &[PackageReviewContractStaticArgument::GenericMachineBinder(0)]
    );
    assert_eq!(
        original
            .canonical_review_bytes()
            .expect("original generic contract encoding"),
        renamed
            .canonical_review_bytes()
            .expect("renamed generic contract encoding"),
        "renaming a local machine binder must not alter package-review identity",
    );
}

#[test]
fn compiler_rejects_nested_machine_arguments_before_package_review() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"boundary machine sample(value: u64) -> u64;
machine inspect<machine Operation>() -> u64
where machine Operation<machine Inner>(value: u64) -> u64
where machine Inner(value: u64) -> u64;
{
    0
}
machine identity<machine Selected>(value: u64) -> u64
where machine Selected(value: u64) -> u64;
{
    value
}
boundary machine trusted_identity() -> u64
ensures result == inspect<identity<sample>>();
"#,
    );
    package.write(
        "build.omg",
        r#"target windows_x64 { }
target linux_x64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
    );
    let diagnostics = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect_err("nested machine applications must fail before checked lowering");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("nested machine application; recursive specialization identity")
    }));
}
