use omega_checker::{check_record_constructor, check_selected_policy_call};
use omega_psi::{
    DeclarationId, LoweredPackage, SelectedPolicyDefinition, TemplateAccess, TemplateCallTarget, TemplateOperand, TemplateResult, TemplateStep,
    TypeExpressionStep,
};
use omega_resolver::{ParsedSource, StageSource};

const OPEN: &str = include_str!("../../04_typed-trees-to-prepared-trees/tests/fixtures/access_policy/selected.omg");
const EMPTY: &str = include_str!("../../05_prepared-trees-to-terminal-psi/tests/fixtures/policy_templates/only_empty.omg");

fn selected(text: &str) -> SelectedPolicyDefinition {
    let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
    let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
    let source = ParsedSource {
        logical_path: "policy.omg",
        text,
        tokens: &tokens,
        tree: &tree,
    };
    let mut package = omega_toolchain::resolve_source_package(&[StageSource { package: "app", source }]).unwrap();
    package.bind_package_key("app", [0x42; 32]).unwrap();
    let prepared = omega_preparation::prepare_source_package(&package).unwrap();
    let [policy] = prepared.typed().access_policies.as_slice() else {
        panic!("one selected domain policy")
    };
    omega_lowering::lower_selected_policy_definition(&prepared, policy).unwrap()
}

fn toolchain() -> (LoweredPackage, DeclarationId) {
    let source = omega_toolchain::SourcePackage::toolchain_definition();
    let prepared = omega_preparation::prepare_source_package(&source).unwrap();
    let unit = omega_lowering::lower_package(&prepared).unwrap();
    let id = unit
        .machine_templates
        .iter()
        .find(|machine| machine.declaration.package == omega_toolchain::TOOLCHAIN_PACKAGE && machine.declaration.path == r"language\core\access\new_plan")
        .unwrap()
        .declaration
        .clone();
    (unit, id)
}

#[test]
fn selected_source_cases_supply_different_constructor_arguments() {
    let (unit, id) = toolchain();
    let constructor = check_record_constructor(&unit, &id).unwrap();
    for (source, case, routes, call_index) in [
        (OPEN, "Unrestricted", None, 1),
        (EMPTY, "Only", Some("Only::routes"), 2),
    ] {
        let selection = selected(source);
        let checked = check_selected_policy_call(&selection, &constructor).unwrap();
        assert!(std::ptr::eq(checked.selection(), &selection));
        assert!(std::ptr::eq(checked.constructor(), &constructor));
        assert_eq!(checked.issuance_case().package, omega_toolchain::TOOLCHAIN_PACKAGE);
        assert_eq!(checked.issuance_case().path, format!(r"language\core\Issuance::{case}"));
        assert_eq!(
            checked.empty_list_field().map(|field| field.path.to_owned()),
            routes.map(|name| format!(r"language\core\Issuance::{name}"))
        );
        assert_eq!(checked.schema_loan(), call_index..=call_index);
    }
}

fn call(selection: &mut SelectedPolicyDefinition) -> &mut omega_psi::TemplateCall {
    let TemplateStep::Call(call) = selection.plan.body.last_mut().unwrap() else {
        panic!("expected call")
    };
    call
}

#[test]
fn caller_proof_rejects_foreign_owners_bad_loans_false_promises_and_wrong_transfers() {
    let (unit, id) = toolchain();
    let constructor = check_record_constructor(&unit, &id).unwrap();
    type Mutation = fn(&mut SelectedPolicyDefinition);
    let mutants: &[(&str, Mutation)] = &[
        ("callee owner", |s| {
            let TemplateCallTarget::Declaration(id) = &mut call(s).target else {
                unreachable!()
            };
            std::sync::Arc::make_mut(&mut id.name).package = "impostor".into();
        }),
        ("requirement owner", |s| {
            std::sync::Arc::make_mut(&mut s.conformance.members[0].requirement.declaration.name).package = "impostor".into()
        }),
        ("policy owner", |s| {
            std::sync::Arc::make_mut(&mut s.conformance.declaration.name).path = "OtherRule".into()
        }),
        ("enclosing region", |s| {
            let other = s.domain.clone();
            let TypeExpressionStep::GenericParameter { owner, .. } = &mut call(s).arguments[0].0[0] else {
                unreachable!()
            };
            *owner = other;
        }),
        ("schema formal", |s| call(s).operands[0] = TemplateOperand::Formal(0)),
        ("schema loan outlives call", |s| {
            let call = call(s);
            call.operands[0] = TemplateOperand::BorrowBefore {
                formal: 0,
                access: TemplateAccess::Shared,
                step: 2,
                site: call.site.clone(),
            };
        }),
        ("schema access", |s| {
            call(s).operands[0] = TemplateOperand::Borrow {
                formal: 0,
                access: TemplateAccess::Mut,
            }
        }),
        ("wrong issuance step", |s| call(s).operands[1] = TemplateOperand::Step(0)),
        ("wrong result", |s| s.plan.result = Some(TemplateResult::Operand(TemplateOperand::Step(1)))),
        ("unreported effect", |s| s.plan.fixed_pause.block = true),
        ("unexpected static argument", |s| {
            s.arguments
                .push(omega_psi::DeclarationTerm::StaticBoolean(false))
        }),
        ("swapped discriminant", |s| {
            let TemplateStep::Construct(data) = &mut s.plan.body[1] else { unreachable!() };
            std::sync::Arc::make_mut(&mut data.declaration.name).path = r"language\core\Issuance::Unrestricted".into();
        }),
        ("wrong array element", |s| {
            let TemplateStep::Array(array) = &mut s.plan.body[0] else { unreachable!() };
            array.element.0.clear();
        }),
        ("unproved guarantee", |s| {
            s.plan.signature.ensures.push(omega_psi::ContractClause {
                site: s.site.clone(),
                predicate: vec![omega_psi::PredicateStep::Boolean(false)],
            });
        }),
    ];
    for &(label, mutate) in mutants {
        let mut selection = selected(EMPTY);
        mutate(&mut selection);
        assert!(check_selected_policy_call(&selection, &constructor).is_err(), "{label}");
    }
    let mut selection = selected(EMPTY);
    let call = call(&mut selection);
    call.operands[0] = TemplateOperand::BorrowBefore {
        formal: 0,
        access: TemplateAccess::Shared,
        step: 0,
        site: call.site.clone(),
    };
    assert_eq!(
        check_selected_policy_call(&selection, &constructor)
            .unwrap()
            .schema_loan(),
        0..=2
    );
}
