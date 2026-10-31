//! Recovery preserves demands even when no field accessor is exposed. This is
//! wire preservation, not evidence that any resource or operation was admitted.

use abstract_operations::StructuralTypeCatalog;
use access_plans::{AccessFields, AccessPlan, BoundaryReach, validate_access_plan};
use layout_plans::{LayoutFieldEntryReport, LayoutPlacementReport, LayoutPlanReport};
use semantic_vocabulary::{
    BoundaryMachineId, StructuralDomainId, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    NominalDeclarationIdentity, TerminalAccessLifecycleExecution, TerminalAccessLifecycleOperation,
    TerminalAccessLifecyclePlan, TerminalAccessProfileRequirement, TerminalAccessStorage,
    TerminalAccessViewPlan, TerminalIdentityIndex,
};

use super::{FixedViewCopyPlan, FixedViewCopyPolicy, plan, structural_function};

fn catalog() -> StructuralTypeCatalog {
    let layout = LayoutPlanReport {
        schema_report_fingerprint: 19,
        offsets: None,
        size: Some(8),
        align: 8,
        entries: vec![LayoutFieldEntryReport {
            field: "payload".into(),
            member_identity: None,
            placement: LayoutPlacementReport::At { offset: 0 },
        }],
    };
    let placement = validate_access_plan(AccessPlan {
        access: AccessFields::inaccessible(&layout).unwrap(),
        layout,
        reach: BoundaryReach::default(),
    })
    .unwrap();
    let schema = StructuralTypeId::new(1).unwrap();
    let view = TerminalAccessViewPlan {
        carrier: StructuralTypeId::new(2).unwrap(),
        schema,
        view_identity: TerminalIdentityIndex(3),
        policy_identity: TerminalIdentityIndex(4),
        policy_plan_machine_identity: TerminalIdentityIndex(5),
        schema_identity: TerminalIdentityIndex(6),
        placement: placement.clone(),
        fields: vec![],
    };
    let initialize = TerminalAccessLifecyclePlan {
        boundary: BoundaryMachineId::new(7).unwrap(),
        operation: TerminalAccessLifecycleOperation::Initialize,
        execution: TerminalAccessLifecycleExecution::CompilerPrimitive {
            profile_type: Some(StructuralTypeId::new(8).unwrap()),
            profile_requirement: Some(TerminalAccessProfileRequirement {
                boundary: BoundaryMachineId::new(9).unwrap(),
                owner: NominalDeclarationIdentity::new(TerminalIdentityIndex(10)),
                callable: TerminalIdentityIndex(11),
            }),
        },
        schema,
        image_fields: vec![StructuralFieldId::new(12).unwrap()],
        storage: TerminalAccessStorage {
            carrier: StructuralTypeId::new(13).unwrap(),
            base: StructuralFieldId::new(14).unwrap(),
            length: StructuralFieldId::new(15).unwrap(),
            granted: StructuralDomainId::new(16).unwrap(),
            vacant: Some(StructuralDomainId::new(17).unwrap()),
            resident: Some(StructuralDomainId::new(18).unwrap()),
        },
        policy_identity: view.policy_identity,
        policy_plan_machine_identity: view.policy_plan_machine_identity,
        schema_identity: view.schema_identity,
        placement,
    };
    let mut extraction = initialize.clone();
    extraction.boundary = BoundaryMachineId::new(20).unwrap();
    extraction.operation = TerminalAccessLifecycleOperation::MoveOut;
    let mut selected = initialize.clone();
    selected.boundary = BoundaryMachineId::new(21).unwrap();
    selected.execution = TerminalAccessLifecycleExecution::SelectedProvider;
    StructuralTypeCatalog::default()
        .with_access_views(vec![view])
        .with_access_lifecycles(vec![initialize, extraction, selected])
        .unwrap()
}

#[test]
fn recovery_preserves_complete_placement_catalog_and_its_identity() {
    use selected_instructions::selected_instruction_plan_identity;
    let catalog = catalog();
    let mut source = plan(FixedViewCopyPolicy::SharedEntryAfterCompareBeforeBranchV1);
    let mut function = structural_function();
    function.structural.as_mut().unwrap().structural_types = catalog.clone();
    std::sync::Arc::make_mut(&mut source.transformed)
        .functions
        .push(function);
    let bytes = source.encode().unwrap();
    let decoded = FixedViewCopyPlan::decode(&bytes).unwrap();
    assert_eq!(decoded, source);
    assert_eq!(decoded.encode().unwrap(), bytes);
    let identity = selected_instruction_plan_identity(&source.transformed);
    assert_eq!(
        selected_instruction_plan_identity(&decoded.transformed),
        identity
    );

    // Every retained role contributes, including inaccessible whole-image fields
    // and the selected profile requirement rather than just geometric bytes.
    for changed_catalog in [
        catalog.clone().with_access_views(vec![]),
        catalog.clone().with_access_lifecycles(vec![]).unwrap(),
        {
            let mut lifecycle = catalog.access_lifecycles().to_vec();
            lifecycle[0].storage.base = StructuralFieldId::new(22).unwrap();
            catalog.clone().with_access_lifecycles(lifecycle).unwrap()
        },
        {
            let mut lifecycle = catalog.access_lifecycles().to_vec();
            lifecycle[0].image_fields.clear();
            catalog.clone().with_access_lifecycles(lifecycle).unwrap()
        },
    ] {
        let mut changed = source.clone();
        std::sync::Arc::make_mut(&mut changed.transformed)
            .functions
            .iter_mut()
            .next_back()
            .unwrap()
            .structural
            .as_mut()
            .unwrap()
            .structural_types = changed_catalog;
        assert_ne!(changed.encode().unwrap(), bytes);
        assert_ne!(
            selected_instruction_plan_identity(&changed.transformed),
            identity
        );
        assert_eq!(
            FixedViewCopyPlan::decode(&changed.encode().unwrap()).unwrap(),
            changed
        );
    }
}

#[test]
fn standalone_catalogs_reject_truncation_and_trailing_bytes() {
    let catalog = catalog();
    let lifecycle =
        terminal_codec::encode_access_lifecycle_catalog(catalog.access_lifecycles()).unwrap();
    let views = terminal_codec::encode_access_view_catalog(catalog.access_views()).unwrap();
    for end in 0..lifecycle.len() {
        assert!(
            terminal_codec::decode_access_lifecycle_catalog(&lifecycle[..end]).is_err(),
            "truncated lifecycle catalog accepted at {end}"
        );
    }
    for end in 0..views.len() {
        assert!(
            terminal_codec::decode_access_view_catalog(&views[..end]).is_err(),
            "truncated view catalog accepted at {end}"
        );
    }
    let mut trailing = lifecycle;
    trailing.push(0);
    assert_eq!(
        terminal_codec::decode_access_lifecycle_catalog(&trailing),
        Err(terminal_codec::CodecError::TrailingBytes(1))
    );
    let mut trailing = views;
    trailing.push(0);
    assert_eq!(
        terminal_codec::decode_access_view_catalog(&trailing),
        Err(terminal_codec::CodecError::TrailingBytes(1))
    );
}
