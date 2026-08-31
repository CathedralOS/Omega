use crate::support::*;

#[test]
fn operator_realization_rejects_post_check_reference_access_drift() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub machine observes(value: &mut i32) -> bool { true }

pub data CheckedReference {}
pub operator CheckedReference::identity(value: i32) -> i32
requires observes(&mut value) == true;

pub machine provide_identity(input: i32) -> i32
satisfies CheckedReference::identity
requires observes(&mut input) == true
{ input }
"#,
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
    let mut checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("reference-bearing operator realization should check");
    let provider = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "provide_identity")
        .expect("provider machine");
    let provider_fact = checked
        .facts
        .proof
        .contract_facts
        .iter()
        .map(|(_, fact)| fact)
        .find(|fact| {
            matches!(
                fact.owner,
                psi_checked_trees::ContractProofFactOwner::Machine { machine_symbol }
                    if machine_symbol == provider.symbol
            )
        })
        .expect("provider contract fact");
    let psi_typed_trees::domain::ProofFact::Expression(call_expression) =
        checked.proof_facts.get(provider_fact.fact)
    else {
        panic!("provider expression contract")
    };
    let psi_typed_trees::expression::ExpressionNode::Binary(contract) =
        checked.expression_table.expression(*call_expression)
    else {
        panic!("provider binary contract")
    };
    let psi_typed_trees::expression::ExpressionNode::Call(call) =
        checked.expression_table.expression(contract.left)
    else {
        panic!("provider contract call")
    };
    let [borrow] = checked.expression_table.expression_handles(call.arguments) else {
        panic!("one provider contract argument")
    };
    let borrow = *borrow;
    let psi_typed_trees::expression::ExpressionNode::Borrow(borrow) =
        checked.typed.expression_table.expression_mut(borrow)
    else {
        panic!("explicit mutable reference argument")
    };
    borrow.access = psi_language_core::ReferenceAccess::Shared;

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("post-check reference access drift must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-realization contracts do not equal compiler rederivation")
    }));
}

#[test]
fn changing_checked_operator_realization_changes_only_the_callable_value() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let compile = |selected: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub data FirstMath {{}}
pub data OtherMath {{}}
pub operator FirstMath::identity(value: i32) -> i32;
pub operator OtherMath::identity(value: i32) -> i32;

pub machine provide_identity(input: i32) -> i32
satisfies {selected}::identity
{{
    input
}}
"#,
            ),
        );
        package.write("build.omg", build);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("operator selection fixture should check");
        project_checked_package_review(&checked).expect("operator selection should project")
    };

    let first = compile("FirstMath");
    let other = compile("OtherMath");
    assert_eq!(first.public_operators(), other.public_operators());
    let first_callable = first
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "provide_identity")
        .expect("first provider callable");
    let other_callable = other
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "provide_identity")
        .expect("other provider callable");
    assert_ne!(
        first_callable.operator_realizations(),
        other_callable.operator_realizations()
    );

    let first_rows = first
        .canonical_rows()
        .expect("first operator realization rows")
        .into_iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::Callable)
        .collect::<Vec<_>>();
    let other_rows = other
        .canonical_rows()
        .expect("other operator realization rows")
        .into_iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::Callable)
        .collect::<Vec<_>>();
    assert_eq!(first_rows.len(), other_rows.len());
    assert!(
        first_rows
            .iter()
            .zip(&other_rows)
            .all(|(left, right)| left.key_bytes() == right.key_bytes())
    );
    assert_eq!(
        first_rows
            .iter()
            .zip(&other_rows)
            .filter(|(left, right)| left.canonical_bytes() != right.canonical_bytes())
            .count(),
        1,
        "only the provider callable value should change"
    );
}

#[test]
fn operator_realization_rejects_post_check_reselection() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data FirstMath {}
pub data StrongerMath {}
pub operator FirstMath::identity(value: i32) -> i32
ensures result == value;
pub operator StrongerMath::identity(value: i32) -> i32
ensures result == 0;

pub machine provide_identity(input: i32) -> i32
satisfies FirstMath::identity
ensures result == input
{
    transition { _ -> input }
}
"#,
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
    let mut checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("weaker operator realization control fixture should check");
    let stronger = checked
        .typed
        .operators()
        .iter()
        .find(|operator| {
            checked
                .typed
                .operator_path_members(operator.name)
                .first()
                .is_some_and(|owner| owner.as_str() == "StrongerMath")
        })
        .expect("stronger operator declaration");
    let stronger_namespace = checked.typed.operator_path_members(stronger.name)[0].clone();
    let satisfies = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "provide_identity")
        .expect("provider machine")
        .satisfies;
    checked
        .typed
        .tables
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .name = stronger_namespace;

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("post-check redirection to a stronger operator must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-realization contracts do not equal compiler rederivation")
    }));
}

#[test]
fn operator_realization_rejects_coordinated_typed_contract_tampering() {
    let Some(target) = host_target_name() else {
        return;
    };
    let package = TempPackage::new();
    package.write(
        "main.omg",
        r#"pub data FirstMath {}
pub data StrongerMath {}
pub operator FirstMath::identity(value: i32) -> i32
ensures result == value;
pub operator StrongerMath::identity(value: i32) -> i32
ensures result == 0;

pub machine provide_identity(input: i32) -> i32
satisfies FirstMath::identity
ensures result == input
{
    transition { _ -> input }
}
"#,
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
    let mut checked = compile_to_checked_with_packages(
        &package.0.join("main.omg"),
        Some(target),
        package_inputs(&package.0),
    )
    .expect("operator contract-custody fixture should check");
    let stronger = checked
        .typed
        .operators()
        .iter()
        .find(|operator| {
            checked
                .typed
                .operator_path_members(operator.name)
                .first()
                .is_some_and(|owner| owner.as_str() == "StrongerMath")
        })
        .expect("stronger operator declaration");
    let stronger_namespace = checked.typed.operator_path_members(stronger.name)[0].clone();
    let stronger_fact = checked.typed.operator_contracts(stronger)[0].facts.start();
    let psi_typed_trees::domain::ProofFact::Expression(stronger_expression) =
        checked.typed.proof_facts.get(stronger_fact)
    else {
        panic!("stronger operator expression contract")
    };
    let stronger_expression_node = checked
        .typed
        .expression_table
        .expression(*stronger_expression)
        .clone();
    let provider = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "provide_identity")
        .expect("provider machine");
    let provider_fact = checked.typed.machine_contracts(provider)[0].facts.start();
    let psi_typed_trees::domain::ProofFact::Expression(provider_expression) =
        checked.typed.proof_facts.get(provider_fact)
    else {
        panic!("provider expression contract")
    };
    let provider_expression = *provider_expression;
    let satisfies = provider.satisfies;

    checked
        .typed
        .tables
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .name = stronger_namespace;
    *checked
        .typed
        .expression_table
        .expression_mut(provider_expression) = stronger_expression_node;

    let mutated_provider = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "provide_identity")
        .expect("mutated provider machine");
    let mutated_operator = checked
        .typed
        .operators()
        .iter()
        .find(|operator| {
            checked
                .typed
                .operator_path_members(operator.name)
                .first()
                .is_some_and(|owner| owner.as_str() == "StrongerMath")
        })
        .expect("mutated stronger operator selection");
    psi_validation::validate_checked_operator_realization_contract(
        &checked.typed,
        mutated_provider,
        mutated_operator,
    )
    .expect("coordinated mutable typed state would pass contract revalidation alone");

    let diagnostics = project_checked_package_review(&checked)
        .expect_err("checked custody must reject coordinated typed contract tampering");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-realization contracts do not equal compiler rederivation")
    }));
}

#[test]
fn unsupported_checked_operator_realization_neighbors_remain_fail_closed() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let private = TempPackage::new();
    private.write(
        "main.omg",
        r#"data CheckedMath {}
operator CheckedMath::identity(value: i32) -> i32;
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
{ input }
"#,
    );
    private.write("build.omg", build);
    let diagnostics = compile_to_checked_with_packages(
        &private.0.join("main.omg"),
        Some(target),
        package_inputs(&private.0),
    )
    .expect_err("compiler admission must reject a public satisfier of a private operator");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("private operator `CheckedMath::identity`")
    }));

    let cases = [
        (
            "external",
            r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
via Binding::Syscall(60);
"#,
            "one exact boundary operator",
        ),
        (
            "bodyless",
            r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;
pub boundary machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity;
"#,
            "without one checked implementation body",
        ),
    ];

    for (label, source, expected) in cases {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write("build.omg", build);
        let checked = compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .unwrap_or_else(|diagnostics| panic!("{label} fixture should check: {diagnostics:?}"));
        let diagnostics = project_checked_package_review(&checked)
            .expect_err("unsupported operator realization must fail closed");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{label}: {diagnostics:?}"
        );
    }

    let compile_admission_control = |source: &str| {
        let package = TempPackage::new();
        package.write("main.omg", source);
        package.write("build.omg", build);
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
        .expect("operator admission-drift control fixture should check")
    };

    let mut visibility_drift = compile_admission_control(
        r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
{ input }
"#,
    );
    let operator_roots = visibility_drift.typed.roots.operators;
    visibility_drift
        .typed
        .tables
        .operators
        .span_mut_or_empty(operator_roots)[0]
        .is_public = false;
    let diagnostics = project_checked_package_review(&visibility_drift)
        .expect_err("post-check private-to-public operator drift must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-realization contracts do not equal compiler rederivation")
    }));

    let mut alias_drift = compile_admission_control(
        r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity as Selected
{ input }
"#,
    );
    let provider = alias_drift
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "provide_identity")
        .expect("aliased provider machine");
    let satisfies = provider.satisfies;
    alias_drift
        .typed
        .tables
        .machine_trait_conformances
        .span_mut_or_empty(satisfies)[0]
        .alias = None;
    let diagnostics = project_checked_package_review(&alias_drift)
        .expect_err("post-check removal of an operator alias must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-realization contracts do not equal compiler rederivation")
    }));

    let mut signature_drift = compile_admission_control(
        r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;
machine u64_helper(value: u64) { }
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
{ input }
"#,
    );
    let helper = signature_drift
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "u64_helper")
        .expect("u64 helper machine");
    let helper_state = &signature_drift.typed.machine_states(helper)[0];
    let u64_type = signature_drift.typed.state_parameters(helper_state)[0].type_reference;
    let u64_node = signature_drift
        .typed
        .type_reference_table
        .type_reference(u64_type)
        .clone();
    let provider = signature_drift
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "provide_identity")
        .expect("signature-drift provider");
    let provider_state = &signature_drift.typed.machine_states(provider)[0];
    let provider_type = signature_drift.typed.state_parameters(provider_state)[0].type_reference;
    let operator_type = signature_drift.typed.operator_parameters(
        signature_drift
            .typed
            .operators()
            .first()
            .expect("signature-drift operator"),
    )[0]
    .type_reference;
    signature_drift
        .typed
        .type_reference_table
        .substitute_node(provider_type, u64_node.clone());
    signature_drift
        .typed
        .type_reference_table
        .substitute_node(operator_type, u64_node);
    let diagnostics = project_checked_package_review(&signature_drift)
        .expect_err("coordinated post-check overload-shape drift must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-realization contracts do not equal compiler rederivation")
    }));

    let mut lifetime_drift = compile_admission_control(
        r#"pub data CheckedBorrow {}
pub operator CheckedBorrow::observe(first: &[u8], second: &[u8]);
pub machine provide_observe<'first, 'second>(
    first: &'first [u8],
    second: &'second [u8]
)
satisfies CheckedBorrow::observe
{ }
"#,
    );
    let provider = lifetime_drift
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "provide_observe")
        .expect("lifetime-drift provider");
    let state = &lifetime_drift.typed.machine_states(provider)[0];
    let parameters = lifetime_drift.typed.state_parameters(state);
    let first_type = parameters[0].type_reference;
    let second_type = parameters[1].type_reference;
    assert_ne!(
        first_type, second_type,
        "distinct lifetime-bearing type nodes"
    );
    let second_node = lifetime_drift
        .typed
        .type_reference_table
        .type_reference(second_type)
        .clone();
    lifetime_drift
        .typed
        .type_reference_table
        .substitute_node(first_type, second_node);
    let diagnostics = project_checked_package_review(&lifetime_drift)
        .expect_err("post-check lifetime-topology drift must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("operator-realization contracts do not equal compiler rederivation")
    }));

    let generic = TempPackage::new();
    generic.write(
        "main.omg",
        r#"pub data CheckedMath {}
pub operator CheckedMath::identity(value: i32) -> i32;
machine generic<Element>() { }
pub machine provide_identity(input: i32) -> i32
satisfies CheckedMath::identity
{ input }
"#,
    );
    generic.write("build.omg", build);
    let mut checked = compile_to_checked_with_packages(
        &generic.0.join("main.omg"),
        Some(target),
        package_inputs(&generic.0),
    )
    .expect("generic-tamper control fixture should check");
    let type_parameters = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "generic")
        .expect("generic helper machine")
        .type_parameters;
    let mut forged_type_parameter = checked.clone();
    let operators = forged_type_parameter.typed.roots.operators;
    forged_type_parameter
        .typed
        .tables
        .operators
        .span_mut_or_empty(operators)[0]
        .type_parameters = type_parameters;
    forged_type_parameter
        .facts
        .operators
        .operator_realization_contracts =
        psi_typed_trees_to_checked_trees::derive_checked_operator_realization_contracts(
            &forged_type_parameter.typed,
        );
    let diagnostics = project_checked_package_review(&forged_type_parameter)
        .expect_err("post-check unmatched generic telescope must fail closed");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(
                "resolves to neither one exact trait requirement nor one exact checked operator"
            )),
        "{diagnostics:?}"
    );

    let operator = &checked.typed.operators()[0];
    let forged_lifetime = checked.typed.operator_path_members(operator.name)[0].clone();
    let operators = checked.typed.roots.operators;
    checked.typed.tables.operators.span_mut_or_empty(operators)[0]
        .lifetime_parameters
        .push(forged_lifetime);
    checked.facts.operators.operator_realization_contracts =
        psi_typed_trees_to_checked_trees::derive_checked_operator_realization_contracts(
            &checked.typed,
        );
    let diagnostics = project_checked_package_review(&checked)
        .expect_err("post-check lifetime-bearing operator realization must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("realizes lifetime-parameterized operator")
    }));

    let mut duplicate = compile_to_checked_with_packages(
        &generic.0.join("main.omg"),
        Some(target),
        package_inputs(&generic.0),
    )
    .expect("duplicate-realization control fixture should check");
    let machine_index = duplicate
        .typed
        .machines()
        .iter()
        .position(|machine| machine.name.as_str() == "provide_identity")
        .expect("provider machine index");
    let machine_symbol = duplicate.typed.machines()[machine_index].symbol;
    let repeated = duplicate
        .typed
        .machine_trait_conformances(&duplicate.typed.machines()[machine_index])[0]
        .clone();
    let repeated_checked = duplicate
        .facts
        .operators
        .operator_realization_contracts
        .iter()
        .find(|row| row.machine_symbol() == machine_symbol)
        .expect("provider checked operator-realization contract")
        .clone();
    duplicate
        .facts
        .operators
        .operator_realization_contracts
        .push(repeated_checked);
    let machine_roots = duplicate.typed.roots.machines;
    let tables = &mut duplicate.typed.tables;
    let machine = &mut tables.machines.span_mut_or_empty(machine_roots)[machine_index];
    tables
        .machine_trait_conformances
        .append_to_span(&mut machine.satisfies, repeated);
    let diagnostics = project_checked_package_review(&duplicate)
        .expect_err("duplicate exact operator realizations must fail closed");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("duplicate exact operator realization")
    }));
}

#[test]
fn checked_operator_crash_routes_must_refine_the_declared_ceiling() {
    let Some(target) = host_target_name() else {
        return;
    };
    let build = r#"target windows_x86_64 { }
target linux_x86_64 { }
target linux_arm64 { }
target macos_arm64 { }
machine build(builder: &mut Build) { builder.package("review-fixture"); }
"#;
    let compile = |provider_route: &str| {
        let package = TempPackage::new();
        package.write(
            "main.omg",
            &format!(
                r#"pub data CheckedMath {{}}
pub operator CheckedMath::checked(value: bool) -> i32
crashes Trap
    value;
pub machine provide_checked(input: bool) -> i32
satisfies CheckedMath::checked
crashes Trap
    {provider_route}
{{
    transition {{
        {provider_route} -> fail()
        _ -> 0i32
    }}

    state fail() -> i32 {{
        crash Trap;
    }}
}}
"#,
            ),
        );
        package.write("build.omg", build);
        compile_to_checked_with_packages(
            &package.0.join("main.omg"),
            Some(target),
            package_inputs(&package.0),
        )
    };

    let checked = compile("input").expect("renamed exact crash route should refine the operator");
    let review = project_checked_package_review(&checked)
        .expect("checked operator crash refinement should project");
    let callable = review
        .callables()
        .iter()
        .find(|callable| callable.identity().path() == "provide_checked")
        .expect("checked operator provider callable");
    assert_eq!(callable.operator_realizations().len(), 1);
    assert_eq!(callable.checked_crash().published().len(), 1);
    assert_eq!(callable.checked_crash().checked_sites().len(), 1);

    let diagnostics = compile("!input")
        .expect_err("a disjoint provider crash route must not refine the operator ceiling");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("its `crashes Trap` routes are not contained by the operator crash ceiling")
    }));
}
