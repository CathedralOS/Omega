use super::*;

const MESSAGE: &str = "pub data Message { case Empty; case Data(value: u8); }\n";

#[test]
fn declaration_case_membership_survives_canonical_package_recovery() {
    for declaration in [
        "pub domain Message::Ready requires self in Message::Data;",
        "pub data Wrapper { message: Message; }\npub domain Wrapper::Ready requires self.message in Message::Data;",
        "pub data Wrapper where message in Message::Data, { message: Message; }",
    ] {
        let policy = project(&Fixture::local(&format!("{MESSAGE}{declaration}")));
        let predicates = if let [domain] = policy.public_domains() {
            domain.predicate_facts()
        } else {
            policy
                .public_data()
                .iter()
                .find(|data| data.identity().path() == "Wrapper")
                .expect("invariant owner")
                .invariants()
        };
        let [
            PackageReviewContractFact::Expression(
                PackageReviewContractExpression::CaseMembership { subject, case },
            ),
        ] = predicates
        else {
            panic!("one nominal membership predicate")
        };
        assert_eq!(case.path(), "Message::Data");
        assert_eq!(
            case.owner(),
            PackageReviewNominalOwner::Package(package_identity())
        );
        if declaration.starts_with("pub domain Message") {
            assert_eq!(
                subject.as_ref(),
                &PackageReviewContractExpression::DomainSubject
            );
        } else {
            let PackageReviewContractExpression::Member {
                receiver,
                member,
                case_variant,
            } = subject.as_ref()
            else {
                panic!("declaration-owned field")
            };
            assert_eq!(
                receiver.as_ref(),
                &PackageReviewContractExpression::DomainSubject
            );
            assert_eq!(member.path(), "Wrapper::message");
            assert!(case_variant.is_none());
        }
    }
}

#[test]
fn declaration_case_membership_keeps_foreign_carrier_and_case_identity() {
    for declaration in [
        "pub domain Message::Ready requires self in Message::Data;",
        "pub data Wrapper where message in Message::Data, { message: Message; }",
    ] {
        let source = format!("use dependency::api;\n{declaration}");
        let first = project(&Fixture::foreign(
            &source,
            MESSAGE,
            PackageKeyIdentity::from_digest([42; 32]).unwrap(),
        ));
        let foreign = project(&Fixture::foreign(
            &source,
            MESSAGE,
            PackageKeyIdentity::from_digest([43; 32]).unwrap(),
        ));
        let other_case = project(&Fixture::foreign(
            &source.replace("Message::Data", "Message::Empty"),
            MESSAGE,
            PackageKeyIdentity::from_digest([42; 32]).unwrap(),
        ));
        for changed in [&foreign, &other_case] {
            assert_ne!(
                first.canonical_bytes().unwrap(),
                changed.canonical_bytes().unwrap()
            );
        }
    }
}

#[test]
fn declaration_case_membership_rejects_missing_selection_and_definition_evidence() {
    for declaration in [
        "pub domain Message::Ready requires self in Message::Data;",
        "pub data Wrapper where message in Message::Data, { message: Message; }",
    ] {
        let fixture = Fixture::local(&format!("{MESSAGE}{declaration}"));
        project(&fixture);
        let mut missing = fixture.checked.clone();
        missing
            .typed
            .retain_authored_declaration_selections(Default::default());
        assert!(
            project_checked_package_policy(&missing, fixture.target, package_identity()).is_err()
        );
        let mut changed = fixture.checked.clone();
        let definition_rows = changed
            .facts
            .semantic
            .facts
            .iter()
            .filter_map(|(handle, fact)| {
                matches!(
                    fact.origin,
                    facts::FactOrigin::DomainDefinition { .. }
                        | facts::FactOrigin::DataDefinition { .. }
                )
                .then_some(handle)
            })
            .collect::<Vec<_>>();
        assert!(!definition_rows.is_empty());
        for handle in definition_rows {
            changed.facts.semantic.facts.get_mut(handle).origin = facts::FactOrigin::Unknown;
        }
        assert!(
            project_checked_package_policy(&changed, fixture.target, package_identity()).is_err()
        );
    }
}
