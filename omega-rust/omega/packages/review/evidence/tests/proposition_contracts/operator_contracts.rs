use crate::support::*;

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
        r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
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
    assert_eq!(less.spelling(), Some(language_core::OperatorSpelling::Less));
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
    assert_eq!(trap.cause(), PackageReviewCrashCause::Trap);
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
    assert_eq!(abort.cause(), PackageReviewCrashCause::Abort);
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
    let owner = checked_trees::ContractProofFactOwner::OperatorDeclaration {
        operator_symbol: ordered_symbol,
    };
    let (checked_contract_handle, checked_contract) = checked
        .facts
        .proof
        .contract_facts
        .iter()
        .find(|(_, fact)| fact.owner == owner)
        .map(|(handle, fact)| (handle, *fact))
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
        .append(checked_contract);
    assert_owner_row_rejects(&duplicate, 2);

    let mut wrong_owner = checked.clone();
    wrong_owner
        .facts
        .proof
        .contract_facts
        .get_mut(checked_contract_handle)
        .owner = checked_trees::ContractProofFactOwner::Unknown;
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
            r#"machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#,
        );
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some("windows_x86_64"),
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
    assert_eq!(trap.cause(), PackageReviewCrashCause::Trap);
    assert_eq!(
        trap.alternative_guards(),
        [PackageReviewCrashRouteGuard::Expression(
            PackageReviewContractExpression::Parameter(1)
        )]
    );
    assert_eq!(abort.cause(), PackageReviewCrashCause::Abort);
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
