use super::*;

const SOURCE: &str = r#"
pub trait Equatable { machine equals(&self, rhs: &Self) -> bool; }
pub data Message { case Empty; case Data(value: u8); }
MessageEquatable: Message satisfies Equatable;
pub machine inspect(left: Message, right: Message)
crashes Abort left == right;
{}
"#;

const MEMBERSHIP_SOURCE: &str = r#"
pub data Message { case Empty; case Data(value: u8); }
pub machine inspect(left: Message, right: Message)
crashes Abort left in Message::Data;
{}
"#;

#[test]
fn structural_sum_crash_contract_preserves_case_meaning_in_package_policy() {
    let fixture = Fixture::local(SOURCE);
    let policy = project(&fixture);
    assert_eq!(
        guard(&policy),
        &PackageReviewContractExpression::Binary {
            meaning: PackageReviewContractOperatorMeaning::Builtin,
            operator: PackageReviewContractBinaryOperator::Equal,
            left: Box::new(PackageReviewContractExpression::Parameter(0)),
            right: Box::new(PackageReviewContractExpression::Parameter(1)),
        }
    );
}

#[test]
fn authored_payload_case_crash_contract_preserves_case_meaning_in_package_policy() {
    let fixture = Fixture::local(MEMBERSHIP_SOURCE);
    let policy = project(&fixture);
    let PackageReviewContractExpression::CaseMembership { subject, case } = guard(&policy) else {
        panic!("membership retains a classifier, not an equality operand")
    };
    assert_eq!(
        subject.as_ref(),
        &PackageReviewContractExpression::Parameter(0)
    );
    assert_eq!(case.path(), "Message::Data");
    assert_eq!(
        case.owner(),
        PackageReviewNominalOwner::Package(package_identity())
    );
    package_evidence::project_checked_package_review(&fixture.checked)
        .expect("complete review retains membership")
        .canonical_review_bytes()
        .expect("complete review encodes membership");
    let baseline = package_evidence::project_checked_package_policy(
        &fixture.checked,
        fixture.target,
        package_identity(),
    )
    .expect("complete package policy retains membership");
    let bytes = baseline.canonical_bytes().unwrap();
    assert_eq!(
        baseline,
        PackagePolicyBaseline::recover_canonical(&bytes, PackagePolicyRecoveryLimits::default(),)
            .unwrap()
    );
}

#[test]
fn authored_case_requirement_uses_the_same_contract_projection() {
    let fixture = Fixture::local(&MEMBERSHIP_SOURCE.replace("crashes Abort", "requires"));
    let policy = project(&fixture);
    assert_eq!(callable(&policy, "inspect").contracts().len(), 1);
    package_evidence::project_checked_package_review(&fixture.checked)
        .expect("case precondition has exact review meaning")
        .canonical_review_bytes()
        .unwrap();
}

fn guard(policy: &PackagePolicyCallables) -> &PackageReviewContractExpression {
    let [route] = callable(policy, "inspect").checked_crash().published() else {
        panic!("one published route")
    };
    let [PackagePolicyCrashGuard::Expression(expression)] = route.alternative_guards() else {
        panic!("one expression guard")
    };
    expression
}

#[test]
fn membership_distinguishes_case_and_foreign_package_owner() {
    let root = "use dependency::helpers;\npub machine inspect(left: Message) crashes Abort left in Message::Data; {}";
    let dependency = "pub data Message { case Empty; case Data(value: u8); }";
    let mut policies = Vec::new();
    for owner in [42, 43] {
        let owner = PackageKeyIdentity::from_digest([owner; 32]).unwrap();
        let policy = project(&Fixture::foreign(root, dependency, owner));
        let PackageReviewContractExpression::CaseMembership { case, .. } = guard(&policy) else {
            panic!("nominal membership")
        };
        assert_eq!(case.owner(), PackageReviewNominalOwner::Package(owner));
        policies.push(policy);
    }
    let empty = project(&Fixture::foreign(
        &root.replace("Message::Data", "Message::Empty"),
        dependency,
        PackageKeyIdentity::from_digest([42; 32]).unwrap(),
    ));
    for other in [&policies[1], &empty] {
        assert_ne!(guard(&policies[0]), guard(other));
        assert_ne!(
            policies[0].canonical_bytes().unwrap(),
            other.canonical_bytes().unwrap()
        );
    }
}

#[test]
fn membership_rejects_missing_selection_and_non_classifier_rhs() {
    use typed_trees::expression::ExpressionNode;
    let fixture = Fixture::local(MEMBERSHIP_SOURCE);
    let mut missing = fixture.checked.clone();
    missing
        .typed
        .retain_authored_declaration_selections(Default::default());
    assert!(project_checked_callable_policy(&missing, fixture.target, package_identity()).is_err());
    let classifier = fixture
        .checked
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            let ExpressionNode::Binary(binary) = node else {
                return None;
            };
            fixture
                .checked
                .expression_table
                .authored_selection_occurrences(handle)
                .any(|occurrence| {
                    fixture
                        .checked
                        .authored_declaration_selections()
                        .get(occurrence)
                        .is_some_and(|selection| {
                            selection.kind()
                                == typed_trees::AuthoredDeclarationSelectionKind::CaseMembership
                        })
                })
                .then_some(binary.right)
        })
        .expect("case classifier");
    let mut altered = fixture.checked.clone();
    *altered.typed.expression_table.expression_mut(classifier) = ExpressionNode::Boolean(true);
    assert!(project_checked_callable_policy(&altered, fixture.target, package_identity()).is_err());
}
