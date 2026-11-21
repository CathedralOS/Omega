use omega_checker::{FieldOrigin, check_record_constructor};
use omega_psi::{DeclarationId, LoweredPackage, MachineTemplate, PredicateOperator, PredicateStep, TemplateOperand, TemplateStep};

fn toolchain_unit() -> (LoweredPackage, DeclarationId) {
    let source = omega_toolchain::SourcePackage::toolchain_definition();
    let prepared = omega_preparation::prepare_source_package(&source).expect("prepare the defining toolchain package");
    let unit = omega_lowering::lower_package(&prepared).expect("lower the defining toolchain package");
    let id = unit
        .machine_templates
        .iter()
        .find(|machine| machine.declaration.package == omega_toolchain::TOOLCHAIN_PACKAGE && machine.declaration.path == r"language\core\access\new_plan")
        .expect("provided constructor")
        .declaration
        .clone();
    (unit, id)
}

#[test]
fn constructor_proof_binds_its_input_transfers_zeroes_and_nested_guarantees() {
    let (unit, id) = toolchain_unit();
    let checked = check_record_constructor(&unit, &id).unwrap();
    assert!(std::ptr::eq(checked.unit(), &unit));
    assert_eq!(checked.machine().declaration, id);
    let fields = checked.fields();
    assert_eq!(fields.len(), 9);
    assert!(matches!(fields[0].1, FieldOrigin::CopySharedFormal { formal: 0, .. }));
    assert!(matches!(fields[1].1, FieldOrigin::MoveFormal { formal: 1, .. }));
    assert!(
        fields[2..]
            .iter()
            .all(|(_, field)| matches!(field, FieldOrigin::Zero(_)))
    );
    assert_eq!(checked.machine().signature.ensures.len(), 4);
    assert!(checked.result().establishes);
}

#[test]
fn a_source_body_and_catalog_operation_cannot_claim_the_same_machine() {
    let (mut unit, id) = toolchain_unit();
    check_record_constructor(&unit, &id).unwrap();
    unit.catalog_declarations.push(id.clone());
    assert!(check_record_constructor(&unit, &id).is_err());
}

#[test]
fn requirement_identity_cannot_also_identify_the_constructor() {
    let (mut unit, id) = toolchain_unit();
    check_record_constructor(&unit, &id).unwrap();
    let requirement = unit
        .traits
        .iter_mut()
        .flat_map(|owner| &mut owner.dependent_requirements)
        .next()
        .unwrap();
    requirement.declaration = id.clone();
    assert!(check_record_constructor(&unit, &id).is_err());
}

#[test]
fn changing_one_constructor_fact_invalidates_its_proof() {
    type Mutation = fn(&mut MachineTemplate);
    let changes: &[(&str, Mutation)] = &[
        ("read the issuance as a shared schema", |m| {
            let TemplateStep::CopyReferent { formal, .. } = &mut m.body[0] else {
                unreachable!()
            };
            *formal = 1;
        }),
        ("use a spent copy for both fields", |m| {
            let Some(TemplateStep::Construct(c)) = m.body.last_mut() else {
                unreachable!()
            };
            c.fields[1].1 = TemplateOperand::Step(0);
        }),
        ("omit a required field", |m| {
            let Some(TemplateStep::Construct(c)) = m.body.last_mut() else {
                unreachable!()
            };
            c.fields.pop();
        }),
        ("erase the outer establishment obligation", |m| {
            let Some(TemplateStep::Construct(c)) = m.body.last_mut() else {
                unreachable!()
            };
            c.establishes = false;
        }),
        ("assert that the new plan contains a reader", |m| {
            let n = m.signature.ensures[1]
                .predicate
                .iter_mut()
                .find_map(|s| if let PredicateStep::Integer(n) = s { Some(n) } else { None })
                .unwrap();
            *n = 1.into();
        }),
        ("introduce blocking", |m| m.fixed_pause.block = true),
        ("use a constructor as its own input", |m| {
            let future = m.body.len() - 1;
            let Some(TemplateStep::Construct(c)) = m.body.last_mut() else {
                unreachable!()
            };
            c.fields[1].1 = TemplateOperand::Step(future);
        }),
        ("leave a reference formal unaccounted for", |m| {
            m.signature.parameters.push(m.signature.parameters[0].clone())
        }),
        ("declare an unhandled crash route", |m| {
            m.signature.crash_ceiling = Some(vec![omega_psi::CrashCause::Abort])
        }),
    ];
    for (reason, change) in changes {
        let (mut unit, id) = toolchain_unit();
        check_record_constructor(&unit, &id).unwrap();
        change(
            unit.machine_templates
                .iter_mut()
                .find(|m| m.declaration == id)
                .unwrap(),
        );
        assert!(check_record_constructor(&unit, &id).is_err(), "{reason}");
    }
}

#[test]
fn a_zeroed_nested_record_must_meet_its_own_data_predicate() {
    let (mut unit, id) = toolchain_unit();
    check_record_constructor(&unit, &id).unwrap();
    let reach = unit
        .data
        .iter_mut()
        .find(|d| d.declaration.path == r"language\core\BoundaryReach")
        .unwrap();
    let clause = &mut reach.requires[0].predicate;
    let [
        ..,
        PredicateStep::Integer(limit),
        PredicateStep::Apply(comparison),
    ] = &mut clause[..]
    else {
        panic!("reach count bound")
    };
    *limit = 0.into();
    *comparison = PredicateOperator::LessThan;
    assert!(check_record_constructor(&unit, &id).is_err(), "zero services cannot satisfy service_count < 0");
}

#[test]
fn whole_input_transfer_requires_the_declared_data_and_its_binders() {
    for missing in [false, true] {
        let (mut unit, id) = toolchain_unit();
        let index = unit
            .data_templates
            .iter()
            .position(|data| data.declaration.path == r"language\core\Issuance")
            .unwrap();
        if missing {
            unit.data_templates.remove(index);
        } else {
            unit.data_templates[index].parameters.clear();
        }
        assert!(check_record_constructor(&unit, &id).is_err());
    }
}

#[test]
fn zero_uses_the_first_cases_payload_and_cannot_create_a_reference() {
    let (mut unit, id) = toolchain_unit();
    check_record_constructor(&unit, &id).unwrap();
    let slots = unit
        .data_templates
        .iter_mut()
        .find(|data| data.declaration.path == r"language\core\AccessFieldSlot")
        .unwrap();
    // The same fields, types and case identities are present in both packages.
    // Selecting Entry makes FieldInfo's borrowed storage part of the active zero.
    slots.cases.swap(0, 1);
    assert!(check_record_constructor(&unit, &id).is_err());
}

#[test]
fn a_nominal_origin_and_an_alias_cannot_share_one_declaration() {
    let (mut unit, id) = toolchain_unit();
    let issuance = unit
        .data_templates
        .iter()
        .find(|data| data.declaration.path == r"language\core\Issuance")
        .unwrap();
    unit.aliases.push(omega_psi::LoweredAlias {
        declaration: issuance.declaration.clone(),
        target: omega_psi::AliasTarget::Builtin("bool".into()),
        asserted_multiplicity: None,
        public: true,
        site: issuance.site.clone(),
    });
    assert!(check_record_constructor(&unit, &id).is_err());
}

#[test]
fn a_sum_zero_requires_a_unique_first_case_of_its_own_schema() {
    let (mut unit, id) = toolchain_unit();
    let slots = unit
        .data_templates
        .iter_mut()
        .find(|data| data.declaration.path == r"language\core\AccessFieldSlot")
        .unwrap();
    slots.cases[1].declaration = slots.cases[0].declaration.clone();
    assert!(check_record_constructor(&unit, &id).is_err());
}
