use component_description::retained_plan::ProviderPlanInput;
use provider_plans::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceReceiverApplication,
};

fn selected_plan() -> ProviderPlan {
    let mut plan = ProviderPlan {
        name: "selected-provider".into(),
        provider_type: "Provider".into(),
        target: "linux_x86_64".into(),
        ..ProviderPlan::default()
    };
    plan.provider_application = plan.identities.intern("Provider<opaque>");
    let declaration = std::sync::Arc::make_mut(&mut plan.schema.declaration);
    declaration.trait_name = declaration.identities.intern("Boundary");
    let requirement = declaration.identities.intern("Boundary::consume(opaque)");
    let application = declaration.identities.intern("Boundary<opaque>");
    let argument = declaration.identities.intern("opaque");
    declaration
        .receiver_applications
        .push(ServiceReceiverApplication {
            application,
            trait_name: declaration.trait_name,
            trait_package_identity: None,
            argument_identities: vec![argument],
            lifetime_argument_identities: Vec::new(),
            requirement_identities: vec![requirement],
        });
    declaration.methods.push(ServiceMethod {
        name: "consume".into(),
        requirement_owner: "Boundary".into(),
        requirement_identity: requirement,
        ..ServiceMethod::default()
    });
    let requirement_identity = plan.identities.intern("Boundary::consume(opaque)");
    let machine_identity = plan.identities.intern("Provider::consume(opaque)");
    plan.rows.push(ProviderPlanRow {
        method: "consume".into(),
        requirement_identity,
        declared_requirement_identity: requirement_identity,
        calling_signature: None,
        requirement_lifetime_partition: Vec::new(),
        admitted_same_stack_bytes: 0,
        binding: ProviderBinding::CheckedAdapter {
            machine_identity,
            machine_package_identity: None,
        },
    });
    assert!(plan.validate_against_schema().is_empty());
    plan
}

#[test]
fn selection_identity_matches_the_published_v6_digest() {
    let empty = crate::SelectedProviderPlanFacts::default();
    let selected =
        crate::SelectedProviderPlanFacts::from_selected_plans(vec![selected_plan()]).unwrap();
    assert_eq!(
        wire::digest_encoder::hex(empty.identity_digest().as_bytes()),
        "ec730138e882da5a89e820e855d4a91b275d9e7bd579dfecf770f41c4769a7f2"
    );
    assert_eq!(
        wire::digest_encoder::hex(selected.identity_digest().as_bytes()),
        "52ee599c657b697f0630f6e4df6b527f4b753d6a1221962ba8dd53a5536ff876"
    );
}

#[test]
fn selected_custody_binds_numeric_declarations_in_canonical_row_order() {
    let mut plan = selected_plan();
    plan.provider_declaration = Some("SelectedMap".into());
    plan.rows[0].binding = ProviderBinding::CompilerIntrinsic {
        machine: "Catalog::consume".into(),
    };
    let declaration = std::sync::Arc::make_mut(&mut plan.schema.declaration);
    let requirement = declaration.identities.intern("Boundary::produce(opaque)");
    let mut method = declaration.methods[0].clone();
    method.name = "produce".into();
    method.requirement_identity = requirement;
    method.declared_type_signature = [0x72; 32];
    declaration.methods[0].declared_type_signature = [0x31; 32];
    declaration.methods.push(method);
    declaration.receiver_applications[0]
        .requirement_identities
        .push(requirement);
    let mut row = plan.rows[0].clone();
    row.method = "produce".into();
    row.requirement_identity = plan.identities.intern("Boundary::produce(opaque)");
    row.declared_requirement_identity = row.requirement_identity;
    row.binding = ProviderBinding::CompilerIntrinsic {
        machine: "Catalog::produce".into(),
    };
    plan.rows.push(row);
    plan.row_numeric_realizations = vec![
        Some("ConsumeDeclaration".into()),
        Some("ProduceDeclaration".into()),
    ];
    let preimage = plan.identity_preimage();
    let input = ProviderPlanInput::read(&preimage, plan.identity_digest().as_bytes()).unwrap();
    assert_eq!(
        input
            .rows
            .iter()
            .map(|row| row.declared_requirement)
            .collect::<Vec<_>>(),
        ["Boundary::consume(opaque)", "Boundary::produce(opaque)"],
    );
    let facts = |plan| crate::SelectedProviderPlanFacts::from_selected_plans(vec![plan]).unwrap();
    let selected = facts(plan.clone());
    let mut reordered = plan.clone();
    reordered.rows.reverse();
    reordered.row_numeric_realizations.reverse();
    assert_eq!(
        selected.identity_digest(),
        facts(reordered).identity_digest()
    );
    let mut substituted = plan.clone();
    substituted.row_numeric_realizations.swap(0, 1);
    assert_eq!(plan.identity_digest(), substituted.identity_digest());
    assert_ne!(
        selected.identity_digest(),
        facts(substituted).identity_digest()
    );
    plan.row_numeric_realizations.pop();
    assert!(
        crate::SelectedProviderPlanFacts::from_selected_plans(vec![plan])
            .unwrap_err()
            .contains("incomplete external row declarations")
    );
}

#[test]
fn exact_local_rows_share_a_schema_without_sharing_bindings() {
    let mut plan = selected_plan();
    let declared = plan.rows[0].requirement_identity;
    plan.rows[0].requirement_identity = plan.identities.intern("Boundary::consume<First>(opaque)");
    plan.rows[0].calling_signature = semantic_vocabulary::CallingSignatureId::new(11);
    plan.rows[0].binding = ProviderBinding::Syscall { number: 1 };
    let mut second = plan.rows[0].clone();
    second.requirement_identity = plan.identities.intern("Boundary::consume<Second>(opaque)");
    second.calling_signature = semantic_vocabulary::CallingSignatureId::new(12);
    second.binding = ProviderBinding::Syscall { number: 2 };
    plan.rows.push(second);
    assert!(plan.validate_against_schema().is_empty());
    let facts = crate::SelectedProviderPlanFacts::from_selected_plans(vec![plan.clone()]).unwrap();
    for (identity, number) in [
        ("Boundary::consume<First>(opaque)", 1),
        ("Boundary::consume<Second>(opaque)", 2),
    ] {
        let (_, row) = facts.selected_requirement_row(identity).unwrap();
        assert_eq!(row.binding, ProviderBinding::Syscall { number });
        assert_eq!(row.declared_requirement_identity, declared);
    }
    assert!(
        facts
            .selected_requirement_row("Boundary::consume<Dormant>(opaque)")
            .is_none()
    );
    assert!(
        facts
            .selected_requirement_row("Boundary::consume(opaque)")
            .is_none()
    );
    let bytes = plan.identity_preimage();
    let input = ProviderPlanInput::read(&bytes, plan.identity_digest().as_bytes()).unwrap();
    assert_eq!(input.rows.len(), 2);
    assert!(input.rows.iter().all(
        |row| row.declared_requirement == "Boundary::consume(opaque)"
            && row.calling_signature.is_some()
    ));
    for binding in [
        ProviderBinding::CompilerIntrinsic {
            machine: "Unrelated::operation".into(),
        },
        ProviderBinding::VtableSlot { index: 0 },
    ] {
        let mut changed = plan.clone();
        changed.rows[0].binding = binding;
        assert!(!changed.validate_candidate_against_schema().is_empty());
        assert!(
            ProviderPlanInput::read(
                &changed.identity_preimage(),
                changed.identity_digest().as_bytes()
            )
            .is_err()
        );
    }
    let mut wrong_declaration = plan.clone();
    wrong_declaration.rows[0].declared_requirement_identity = wrong_declaration
        .identities
        .intern("Other::consume(opaque)");
    assert!(
        !wrong_declaration
            .validate_candidate_against_schema()
            .is_empty()
    );
    assert!(
        ProviderPlanInput::read(
            &wrong_declaration.identity_preimage(),
            wrong_declaration.identity_digest().as_bytes()
        )
        .is_err()
    );
    let mut duplicate = plan.clone();
    duplicate.rows.push(duplicate.rows[0].clone());
    assert!(!duplicate.validate_candidate_against_schema().is_empty());
    let mut missing = plan;
    missing.rows[0].calling_signature = None;
    assert!(!missing.validate_candidate_against_schema().is_empty());
}
