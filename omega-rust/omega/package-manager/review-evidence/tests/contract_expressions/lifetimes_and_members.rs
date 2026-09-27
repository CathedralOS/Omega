use crate::support::*;
use compiler::CheckedCompileRequest;

#[test]
fn attached_self_members_use_the_exact_entry_formal_not_the_machine_root() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"
pub data Owner { value: u64; }
pub data Other { value: u64; }
pub machine Other::check(&self) {}
pub domain u64::Small requires self <= 10;
pub machine Owner::check(&self)
requires self.value <= 10
{}
"#,
    );
    package.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.package(\"review_fixture\"); }\n",
    );
    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
    })
    .expect("attached self contract checks");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Owner::check")
        .unwrap();
    let machine_symbol = machine.symbol;
    let parameter = checked.state_parameters(&checked.machine_states(machine)[0])[0].symbol;
    let review =
        project_checked_package_review(&checked).expect("formal-root self member projects");
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "Owner::check")
        .unwrap();
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        left, ..
    }) = callable.contracts()[0].fact()
    else {
        panic!("member comparison")
    };
    let PackageReviewContractExpression::Member {
        receiver, member, ..
    } = left.as_ref()
    else {
        panic!("self member")
    };
    assert_eq!(
        receiver.as_ref(),
        &PackageReviewContractExpression::Parameter(0)
    );
    assert_eq!(member.path(), "Owner::value");
    let mut wrong_owner = checked.clone();
    let other = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Other::check")
        .unwrap()
        .symbol;
    let expressions = checked
        .expression_table
        .iter_expressions()
        .filter_map(|(handle, expression)| {
            let typed_trees::expression::ExpressionNode::Name(path) = expression else {
                return None;
            };
            (path.head_symbol == machine_symbol
                && checked
                    .expression_table
                    .name_path_members(path.members)
                    .first()
                    .is_some_and(|name| name.as_str() == "self"))
            .then_some(handle)
        })
        .collect::<Vec<_>>();
    assert!(
        !expressions.is_empty(),
        "checked source stamps the exact machine root for self"
    );
    for expression in expressions {
        let typed_trees::expression::ExpressionNode::Name(path) = wrong_owner
            .typed
            .expression_table
            .expression_mut(expression)
        else {
            unreachable!()
        };
        path.head_symbol = other;
        path.symbol = other;
    }
    assert!(
        project_checked_package_review(&wrong_owner).is_err(),
        "a same-named method on another attachment cannot lend its self root"
    );
    let mut changed = checked.clone();
    let places = changed
        .facts
        .semantic
        .places
        .iter()
        .filter_map(|(handle, place)| {
            (place.root == facts::PlaceRoot::Symbol(parameter)).then_some(handle)
        })
        .collect::<Vec<_>>();
    assert!(!places.is_empty());
    for handle in places {
        changed.facts.semantic.places.get_mut(handle).root =
            facts::PlaceRoot::Symbol(machine_symbol);
    }
    let diagnostics = project_checked_package_review(&changed)
        .expect_err("old machine-root places cannot substitute for the retained self formal");
    assert!(format!("{diagnostics:?}").contains("exact checked place rows"));
}

#[test]
fn machine_precondition_members_require_exact_entry_point_and_declaration_origin() {
    use facts::{FactOrigin, FactPayload, ProgramPoint};

    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"
pub data Pair [copy] { left: i32; right: i32; }
pub machine compare(value: Pair)
requires value.left == value.right
{}
"#,
    );
    package.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.package(\"review_fixture\"); }\n",
    );
    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
    })
    .expect("member precondition checks at its canonical entry");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "compare")
        .unwrap();
    let symbol = machine.symbol;
    let entry = checked.machine_states(machine).first().unwrap().symbol;
    project_checked_package_review(&checked).expect("entry-scoped member precondition projects");
    for wrong_origin in [false, true] {
        let mut changed = checked.clone();
        let mut changed_rows = 0;
        let handles = changed
            .facts
            .semantic
            .facts
            .iter()
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        for handle in handles {
            let fact = changed.facts.semantic.facts.get_mut(handle);
            if fact.point
                == (ProgramPoint::State {
                    machine_symbol: symbol,
                    state_symbol: entry,
                })
                && fact.origin
                    == (FactOrigin::MachineContract {
                        machine_symbol: symbol,
                    })
                && matches!(fact.payload, FactPayload::ContractBooleanExpression { .. })
            {
                if wrong_origin {
                    fact.origin = FactOrigin::Unknown;
                } else {
                    fact.point = ProgramPoint::Machine {
                        machine_symbol: symbol,
                    };
                }
                changed_rows += 1;
            }
        }
        assert!(
            changed_rows > 0,
            "mutate actual retained member dependency rows"
        );
        let diagnostics = project_checked_package_review(&changed).expect_err(
            "stale declaration-wide or wrong-origin facts cannot replace entry evidence",
        );
        assert!(format!("{diagnostics:?}").contains("exact checked place rows"));
    }
}

#[test]
fn review_rejects_nominal_member_selection_custody_tamper() {
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data Pair [copy] {
    left: i32;
    right: i32;
}
pub proposition balanced(pair: Pair) = pair.left == pair.right;
"#,
    );
    package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
"#,
    );

    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&package.0)),
        ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
    })
    .expect("public nominal-member proposition should check");
    project_checked_package_review(&checked)
        .expect("untampered nominal-member selection custody should project");

    let members = checked
        .expression_table
        .iter_expressions()
        .filter_map(|(expression, node)| {
            let typed_trees::expression::ExpressionNode::Member(member) = node else {
                return None;
            };
            matches!(member.member.as_str(), "left" | "right").then_some((
                member.member.as_str().to_owned(),
                expression,
                checked
                    .expression_table
                    .authored_selection_occurrences(expression)
                    .collect::<Vec<_>>(),
            ))
        })
        .collect::<Vec<_>>();
    let left = members
        .iter()
        .find(|(name, _, _)| name == "left")
        .expect("left member expression");
    let right = members
        .iter()
        .find(|(name, _, _)| name == "right")
        .expect("right member expression");
    let [left_occurrence] = left.2.as_slice() else {
        panic!("left member must retain one exact selection")
    };
    let [right_occurrence] = right.2.as_slice() else {
        panic!("right member must retain one exact selection")
    };
    assert_ne!(left_occurrence, right_occurrence);

    let mut tampered = checked.clone();
    tampered
        .typed
        .expression_table
        .attach_authored_selection_occurrences(left.1, [*right_occurrence]);
    let diagnostics = project_checked_package_review(&tampered)
        .expect_err("duplicate nominal-member selection custody must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("nominal member has 2 exact checked member-selection rows")
    }));
}

#[test]
fn review_projects_computed_nominal_member_receivers() {
    let project = |selected: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub data Pair [copy] {{
    left: i32;
    right: i32;
}}
pub proposition selects_computed_member(value: i32) =
    (Pair {{ left: value, right: value }}).{selected} == value;
"#,
            ),
        );
        package.write(
            "build.omg",
            r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
"#,
        );
        let checked = compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some("windows_x86_64"))
        })
        .expect("computed nominal-member proposition should check");
        project_checked_package_review(&checked)
            .expect("computed nominal-member receiver should project")
    };

    let left_review = project("left");
    let right_review = project("right");
    let proposition = left_review
        .public_propositions()
        .iter()
        .find(|proposition| proposition.identity().path() == "selects_computed_member")
        .expect("computed-member proposition");
    let PackageReviewPublicPropositionBody::Transparent(PackageReviewContractFact::Expression(
        PackageReviewContractExpression::Binary { left, .. },
    )) = proposition.body()
    else {
        panic!("computed-member proposition definition")
    };
    let PackageReviewContractExpression::Member {
        receiver, member, ..
    } = left.as_ref()
    else {
        panic!("computed member expression")
    };
    assert!(matches!(
        receiver.as_ref(),
        PackageReviewContractExpression::Constructor { .. }
    ));
    assert_eq!(member.path(), "Pair::left");
    assert_ne!(
        left_review.canonical_review_bytes().unwrap(),
        right_review.canonical_review_bytes().unwrap(),
        "changing the exact member selected from an otherwise identical computed receiver must change review identity",
    );

    let case_package = TempPackage::new();
    case_package.write(
        "main.omg",
        r#"pub data Outcome [copy] {
    case Left(value: i32);
    case Right(value: i32);
}
pub proposition selects_case_member(value: i32) =
    (Outcome::Right { value: value }).value == value;
"#,
    );
    case_package.write(
        "build.omg",
        r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
"#,
    );
    let checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&case_package.0)),
        ..CheckedCompileRequest::new(&case_package.0.join("main.omg"), Some("windows_x86_64"))
    })
    .expect("computed case-member proposition should check");
    let case_review = project_checked_package_review(&checked)
        .expect("computed case-member receiver should project");
    let proposition = case_review
        .public_propositions()
        .iter()
        .find(|proposition| proposition.identity().path() == "selects_case_member")
        .expect("computed case-member proposition");
    let PackageReviewPublicPropositionBody::Transparent(PackageReviewContractFact::Expression(
        PackageReviewContractExpression::Binary { left, .. },
    )) = proposition.body()
    else {
        panic!("computed case-member proposition definition")
    };
    let PackageReviewContractExpression::Member {
        member,
        case_variant: Some(case_variant),
        ..
    } = left.as_ref()
    else {
        panic!("computed case member expression")
    };
    assert_eq!(member.path(), "Outcome::Right::value");
    assert_eq!(case_variant.path(), "Outcome::Right");
}

#[test]
fn review_projects_computed_nominal_member_receivers_in_checked_contracts() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"machine build(builder: &mut Build) { builder.package("review_fixture"); }
"#;
    let compile = |selected: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub data Pair [copy] {{
    left: i32;
    right: i32;
}}
pub machine accepts_computed_member(value: i32)
requires (Pair {{ left: value, right: value }}).{selected} == value
{{}}
"#,
            ),
        );
        package.write("build.omg", build);
        compile_review_fixture(CheckedCompileRequest {
            package_inputs: Some(package_inputs(&package.0)),
            ..CheckedCompileRequest::new(&package.0.join("main.omg"), Some(target))
        })
        .expect("computed nominal-member machine contract should check")
    };

    let left_checked = compile("left");
    let left_review = project_checked_package_review(&left_checked)
        .expect("computed nominal-member machine contract should project");
    let right_review = project_checked_package_review(&compile("right"))
        .expect("changed computed nominal-member machine contract should project");
    let callable = left_review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "accepts_computed_member")
        .expect("computed-member callable");
    let [contract] = callable.contracts() else {
        panic!("one computed-member contract")
    };
    assert_eq!(contract.kind(), PackageReviewContractKind::Requires);
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        left, ..
    }) = contract.fact()
    else {
        panic!("computed-member equality contract")
    };
    let PackageReviewContractExpression::Member {
        receiver, member, ..
    } = left.as_ref()
    else {
        panic!("computed member contract expression")
    };
    assert!(matches!(
        receiver.as_ref(),
        PackageReviewContractExpression::Constructor { .. }
    ));
    assert_eq!(member.path(), "Pair::left");
    assert_ne!(
        left_review.canonical_review_bytes().unwrap(),
        right_review.canonical_review_bytes().unwrap(),
        "changing the exact member selected by a checked contract must change review identity",
    );

    let case_package = TempPackage::new();
    case_package.write(
        "main.omg",
        r#"pub data Outcome [copy] {
    case Left(value: i32);
    case Right(value: i32);
}
pub machine accepts_case_member(value: i32)
requires (Outcome::Right { value: value }).value == value
{}
"#,
    );
    case_package.write("build.omg", build);
    let case_checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&case_package.0)),
        ..CheckedCompileRequest::new(&case_package.0.join("main.omg"), Some(target))
    })
    .expect("computed case-member machine contract should check");
    let case_review = project_checked_package_review(&case_checked)
        .expect("computed case-member machine contract should project");
    let callable = case_review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "accepts_case_member")
        .expect("computed case-member callable");
    let [contract] = callable.contracts() else {
        panic!("one computed case-member contract")
    };
    let PackageReviewContractFact::Expression(PackageReviewContractExpression::Binary {
        left, ..
    }) = contract.fact()
    else {
        panic!("computed case-member equality contract")
    };
    let PackageReviewContractExpression::Member {
        member,
        case_variant: Some(case_variant),
        ..
    } = left.as_ref()
    else {
        panic!("computed case-member contract expression")
    };
    assert_eq!(member.path(), "Outcome::Right::value");
    assert_eq!(case_variant.path(), "Outcome::Right");

    let custody_package = TempPackage::new();
    custody_package.write(
        "main.omg",
        r#"pub data Pair [copy] {
    left: i32;
    right: i32;
}
pub machine compares_computed_members(value: i32)
requires (Pair { left: value, right: value }).left ==
         (Pair { left: value, right: value }).right
{}
"#,
    );
    custody_package.write("build.omg", build);
    let mut custody_checked = compile_review_fixture(CheckedCompileRequest {
        package_inputs: Some(package_inputs(&custody_package.0)),
        ..CheckedCompileRequest::new(&custody_package.0.join("main.omg"), Some(target))
    })
    .expect("computed-member custody fixture should check");
    let members = custody_checked
        .expression_table
        .iter_expressions()
        .filter_map(|(expression, node)| {
            let typed_trees::expression::ExpressionNode::Member(member) = node else {
                return None;
            };
            matches!(member.member.as_str(), "left" | "right").then_some((
                member.member.as_str().to_owned(),
                expression,
                custody_checked
                    .expression_table
                    .authored_selection_occurrences(expression)
                    .collect::<Vec<_>>(),
            ))
        })
        .collect::<Vec<_>>();
    let left = members
        .iter()
        .find(|(name, _, _)| name == "left")
        .expect("computed left member");
    let right = members
        .iter()
        .find(|(name, _, _)| name == "right")
        .expect("computed right member");
    let [right_occurrence] = right.2.as_slice() else {
        panic!("computed right member must retain one exact selection")
    };
    custody_checked
        .typed
        .expression_table
        .attach_authored_selection_occurrences(left.1, [*right_occurrence]);
    let diagnostics = project_checked_package_review(&custody_checked)
        .expect_err("computed-member selection custody drift must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("nominal member has 2 exact checked member-selection rows")
    }));
}
