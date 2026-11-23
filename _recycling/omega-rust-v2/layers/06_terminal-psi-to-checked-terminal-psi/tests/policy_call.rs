use omega_checker::{check_record_constructor, check_selected_policy_call};
use omega_psi::{
    DeclarationId, LoweredPackage, SelectedPolicyDefinition, TemplateAccess, TemplateCallTarget, TemplateOperand, TemplateResult, TemplateStep,
    TypeExpressionStep,
};
use omega_resolver::{ParsedSource, StageSource};

const OPEN: &str = include_str!("../../04_typed-trees-to-prepared-trees/tests/fixtures/access_policy/selected.omg");
const EMPTY: &str = include_str!("../../05_prepared-trees-to-terminal-psi/tests/fixtures/policy_templates/only_empty.omg");
const ZEROED: &str = include_str!("../../05_prepared-trees-to-terminal-psi/tests/fixtures/policy_templates/dotdot.omg");

fn selected(text: &str) -> SelectedPolicyDefinition {
    selected_inputs(text).0
}

fn selected_inputs(text: &str) -> (SelectedPolicyDefinition, omega_psi::PolicyConstructionInputs) {
    with_prepared(text, |prepared| {
        let [policy] = prepared.typed().access_policies.as_slice() else {
            panic!("one selected domain policy")
        };
        (
            omega_lowering::lower_selected_policy_definition(prepared, policy).unwrap(),
            omega_lowering::lower_policy_construction_inputs(prepared, policy).unwrap(),
        )
    })
}

fn with_prepared<T>(
    text: &str,
    check: impl FnOnce(&mut omega_preparation::PreparedTrees<'_>) -> T,
) -> T {
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
    check(&mut omega_preparation::prepare_source_package(&package).unwrap())
}

fn toolchain() -> (LoweredPackage, DeclarationId) {
    let source = omega_toolchain::SourcePackage::toolchain_definition();
    let prepared = omega_preparation::prepare_source_package(&source).unwrap();
    let unit = omega_lowering::lower_package(&prepared).unwrap();
    let id = unit
        .machines
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
        (ZEROED, "Only", Some("Only::routes"), 1),
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

#[test]
fn zeroed_routes_require_the_selected_case_field_and_its_exact_applied_type() {
    let (unit, id) = toolchain();
    let constructor = check_record_constructor(&unit, &id).unwrap();
    let open = selected(OPEN);
    let TemplateStep::Construct(unrestricted) = &open.plan.body[0] else {
        unreachable!()
    };
    let check_mutation = |mutate: &dyn Fn(&mut omega_psi::TemplateConstruct)| {
        let mut selection = selected(ZEROED);
        let TemplateStep::Construct(issuance) = &mut selection.plan.body[0] else {
            unreachable!()
        };
        mutate(issuance);
        check_selected_policy_call(&selection, &constructor).unwrap_err()
    };
    let invalid_payload = "unsupported issuance payload or incorrect field ownership";
    assert_eq!(
        check_mutation(&|issuance| issuance.declaration = unrestricted.declaration.clone()),
        invalid_payload
    );
    assert_eq!(
        check_mutation(&|issuance| {
            let [(field, _)] = issuance.fields.as_mut_slice() else { unreachable!() };
            std::sync::Arc::make_mut(&mut field.name).package = "impostor".into();
        }),
        invalid_payload
    );
    assert_eq!(
        check_mutation(&|issuance| {
            let [(_, TemplateOperand::Zero(ty))] = issuance.fields.as_mut_slice() else {
                unreachable!()
            };
            let TypeExpressionStep::GenericParameter { index, .. } = &mut ty.0[0] else {
                unreachable!()
            };
            *index += 1;
        }),
        invalid_payload
    );
}

#[test]
fn matching_zeroed_field_types_still_require_a_slice_of_declared_elements() {
    let element = |unit: &LoweredPackage| {
        unit.data
            .iter()
            .position(|data| data.declaration.path == r"language\core\reflect\CallableInfo")
            .unwrap()
    };
    // The caller's zero and the unit's field agree in both cases. First both
    // name the element itself instead of a sequence of it.
    let (mut unit, id) = toolchain();
    let mut selection = selected(ZEROED);
    let TemplateStep::Construct(issuance) = &mut selection.plan.body[0] else {
        unreachable!()
    };
    let [(field, TemplateOperand::Zero(ty))] = issuance.fields.as_mut_slice() else {
        unreachable!()
    };
    assert_eq!(ty.0.pop(), Some(TypeExpressionStep::Slice));
    let row = unit.data[element(&unit)].carrier;
    let declared = unit
        .data
        .iter_mut()
        .flat_map(|data| &mut data.cases)
        .flat_map(|case| &mut case.fields)
        .find(|schema| &schema.declaration == field)
        .unwrap();
    declared.carrier = row;
    let constructor = check_record_constructor(&unit, &id).unwrap();
    assert_eq!(
        check_selected_policy_call(&selection, &constructor).unwrap_err(),
        "unsupported issuance payload or incorrect field ownership"
    );

    // Then the element's declaration also identifies an alias, so the
    // sequence has no single declared element.
    let (mut unit, id) = toolchain();
    let selection = selected(ZEROED);
    let callable = &unit.data[element(&unit)];
    let alias = omega_psi::LoweredAlias {
        declaration: callable.declaration.clone(),
        target: omega_psi::AliasTarget::Builtin("bool".into()),
        asserted_multiplicity: None,
        public: true,
        site: callable.site.clone(),
    };
    unit.aliases.push(alias);
    let constructor = check_record_constructor(&unit, &id).unwrap();
    assert_eq!(
        check_selected_policy_call(&selection, &constructor).unwrap_err(),
        "constructor data declaration also identifies an alias"
    );
}

#[test]
fn an_open_template_of_the_constructor_is_a_second_proof_subject() {
    // The unit publishes one closed body. A template under the same
    // declaration would be another candidate, and the proof reads neither in
    // place of the other.
    let (mut unit, id) = toolchain();
    check_record_constructor(&unit, &id).unwrap();
    let mut template = selected(OPEN).plan;
    template.declaration = id.clone();
    unit.machine_templates.push(template);
    assert_eq!(check_record_constructor(&unit, &id).unwrap_err(), "ambiguous constructor declaration");
}

#[test]
fn the_constructor_proof_holds_for_the_body_a_selecting_package_republishes() {
    // A package that selects the policy lowers its plan with the toolchain's
    // constructor body beside it, under the toolchain's identity. The same
    // judgment reads that body: same declared formals, result and field origins.
    let (defining, id) = toolchain();
    let expected = check_record_constructor(&defining, &id).unwrap();
    let unit = with_prepared(OPEN, |prepared| {
        let [policy] = prepared.typed().access_policies.as_slice() else {
            panic!("one selected domain policy")
        };
        omega_lowering::lower_selected_policy_unit(prepared, policy)
            .unwrap()
            .expect("the selected plan has a closed body")
    });
    let checked = check_record_constructor(&unit, &id).unwrap();
    assert!(std::ptr::eq(checked.unit(), unit.package()));
    assert_eq!(checked.machine().declaration, id);
    assert_eq!(checked.parameters(), expected.parameters());
    assert_eq!(checked.result().ty, expected.result().ty);
    assert_eq!(checked.result().declaration, expected.result().declaration);
    // The establishing operation is still the toolchain's authored construction.
    assert_eq!(checked.result().site, expected.result().site);
    assert_eq!(checked.fields(), expected.fields());
    // The selecting package's own machines are other declarations.
    assert!(
        unit.machines
            .iter()
            .any(|machine| machine.declaration.package == "app")
    );
    assert!(
        check_record_constructor(
            &unit,
            &unit
                .machines
                .iter()
                .find(|machine| machine.declaration.package == "app")
                .unwrap()
                .declaration
        )
        .is_err()
    );
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

#[test]
fn frozen_construction_binds_the_selected_policy_and_its_actual_field_transfers() {
    use omega_checker::check_policy_construction;
    use omega_psi::{PolicyCasePayload, PolicyConstructionField};
    let (unit, id) = toolchain();
    let constructor = check_record_constructor(&unit, &id).unwrap();
    for text in [OPEN, EMPTY, ZEROED] {
        let (definition, inputs) = selected_inputs(text);
        let call = check_selected_policy_call(&definition, &constructor).unwrap();
        let checked = check_policy_construction(&call, &inputs).unwrap();
        assert_eq!(checked.inputs(), &inputs);
        assert_eq!(checked.inputs().package_key, [0x42; 32]);
        assert!(!checked.inputs().schema.storage.is_empty());
        let [
            (schema_id, PolicyConstructionField::Schema(schema)),
            (issuance_id, PolicyConstructionField::Issuance { case, payload, .. }),
            rest @ ..,
        ] = checked.inputs().fields.as_slice()
        else {
            panic!("expected the actual returned plan fields")
        };
        assert_eq!(schema_id.path, r"language\core\AccessPlan::schema");
        assert_eq!(schema, &inputs.schema);
        assert_eq!(issuance_id.path, r"language\core\AccessPlan::issuance");
        assert_eq!(case, call.issuance_case());
        assert_eq!(
            *payload,
            if text == OPEN {
                PolicyCasePayload::NoFields
            } else {
                PolicyCasePayload::EmptySlice
            }
        );
        assert_eq!(rest.len(), 7);
        assert!(
            rest.iter()
                .all(|(_, v)| matches!(v, PolicyConstructionField::Zero(_)))
        );
    }

    let (definition, original) = selected_inputs(EMPTY);
    let call = check_selected_policy_call(&definition, &constructor).unwrap();
    let (_, open_inputs) = selected_inputs(OPEN);
    assert!(check_policy_construction(&call, &open_inputs).is_err());
    let (_, sibling) = selected_inputs(&EMPTY.replace("::Locked", "::Sibling"));
    assert!(check_policy_construction(&call, &sibling).is_err());
    let mutants: &[fn(&mut omega_psi::PolicyConstructionInputs)] = &[
        |p| p.fields.swap(0, 1),
        |p| p.fields.truncate(2),
        |p| p.schema.storage = [0x42].into(),
        |p| {
            let PolicyConstructionField::Schema(copied) = &mut p.fields[0].1 else {
                unreachable!()
            };
            copied.storage = copied.storage.to_vec().into();
            assert_eq!(copied.storage, p.schema.storage);
            assert!(!std::sync::Arc::ptr_eq(&copied.storage, &p.schema.storage));
        },
        |p| std::sync::Arc::make_mut(&mut p.schema.record.name).path = "ForeignSchema".into(),
        |p| {
            let PolicyConstructionField::Issuance { payload, .. } = &mut p.fields[1].1 else {
                unreachable!()
            };
            *payload = PolicyCasePayload::NoFields;
        },
        |p| {
            let PolicyConstructionField::Zero(ty) = &mut p.fields[2].1 else {
                unreachable!()
            };
            ty.0.clear();
        },
        |p| p.result.0.clear(),
        |p| p.construction_site = p.selection_site.clone(),
        |p| std::sync::Arc::make_mut(&mut p.constructor.name).package = "another".into(),
    ];
    for mutate in mutants {
        let mut different = original.clone();
        mutate(&mut different);
        assert!(check_policy_construction(&call, &different).is_err());
    }
}
