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
fn nested_signature_case_membership_retains_its_parameter_scope() {
    for (parameter_type, subject) in [
        ("Message", "value"),
        ("&Message", "value"),
        ("Wrapper", "value.message"),
        ("[Message; 2]", "value[0]"),
    ] {
        let fixture = Fixture::local(&format!(
            r#"
pub data Message {{ case Empty; case Data(value: u8); }}
pub data Wrapper {{ message: Message; }}
pub machine accepts<machine Work>()
where machine Work(value: {parameter_type}) crashes Abort {subject} in Message::Data;
{{}}
"#
        ));
        let policy = project(&fixture);
        let expression = nested_guard(&policy);
        let PackageReviewContractExpression::CaseMembership {
            subject: captured,
            case,
        } = expression
        else {
            panic!("nested membership")
        };
        let parameter = PackageReviewContractExpression::Parameter(0);
        match captured.as_ref() {
            PackageReviewContractExpression::Member {
                receiver,
                member,
                case_variant,
            } => {
                assert_eq!(subject, "value.message");
                assert_eq!(receiver.as_ref(), &parameter);
                assert_eq!(member.path(), "Wrapper::message");
                assert!(case_variant.is_none());
            }
            PackageReviewContractExpression::Indexed {
                meaning,
                collection,
                ..
            } => {
                assert_eq!(subject, "value[0]");
                assert_eq!(*meaning, PackageReviewContractOperatorMeaning::Builtin);
                assert_eq!(collection.as_ref(), &parameter);
            }
            other => {
                assert_eq!(subject, "value");
                assert_eq!(other, &parameter);
            }
        }
        assert_eq!(case.path(), "Message::Data");
        let baseline = package_evidence::project_checked_package_policy(
            &fixture.checked,
            fixture.target,
            package_identity(),
        )
        .expect("nested membership composes into whole package policy");
        let bytes = baseline.canonical_bytes().unwrap();
        assert_eq!(
            baseline,
            PackagePolicyBaseline::recover_canonical(
                &bytes,
                PackagePolicyRecoveryLimits::default()
            )
            .unwrap()
        );
    }
}

fn nested_guard(policy: &PackagePolicyCallables) -> &PackageReviewContractExpression {
    let PackagePolicyTypeParameterKind::Machine(contract) =
        callable(policy, "accepts").type_parameters()[0].kind()
    else {
        panic!("static machine contract")
    };
    let signature = contract.structural().unwrap();
    let [route] = signature.published_crash() else {
        panic!("nested crash route")
    };
    let [PackagePolicyCrashGuard::Expression(expression)] = route.alternative_guards() else {
        panic!("one nested expression")
    };
    expression
}

#[test]
fn nested_membership_rejects_another_signatures_same_spelled_parameter() {
    use typed_trees::expression::ExpressionNode;
    let fixture = Fixture::local(
        r#"
pub data Message { case Empty; case Data(value: u8); }
pub machine accepts<machine Work, machine Other>()
where machine Work(value: Message) crashes Abort value in Message::Data;
where machine Other(value: Message) crashes Abort value in Message::Empty;
{}
"#,
    );
    project(&fixture);
    let subjects = fixture
        .checked
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            use language_semantics::declaration_selection::{
                AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionKind,
            };
            if !fixture
                .checked
                .expression_table
                .authored_selection_occurrences(handle)
                .any(|occurrence| {
                    fixture
                        .checked
                        .authored_declaration_selections()
                        .get(occurrence)
                        .is_some_and(|selection| {
                            selection.kind() == AuthoredDeclarationSelectionKind::CaseMembership
                                && selection.exposure()
                                    == AuthoredDeclarationSelectionExposure::PublicInterface
                        })
                })
            {
                return None;
            }
            let ExpressionNode::Binary(binary) = expression else {
                return None;
            };
            let ExpressionNode::Name(name) =
                fixture.checked.expression_table.expression(binary.left)
            else {
                return None;
            };
            (fixture
                .checked
                .expression_table
                .name_path_members(name.members)
                .len()
                == 1)
                .then_some((binary.left, *name))
        })
        .collect::<Vec<_>>();
    let [(first, _), (_, other)] = subjects.as_slice() else {
        panic!("two membership subjects: {subjects:?}")
    };
    let mut altered = fixture.checked.clone();
    *altered.typed.expression_table.expression_mut(*first) = ExpressionNode::Name(*other);
    assert!(project_checked_callable_policy(&altered, fixture.target, package_identity()).is_err());
}

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
