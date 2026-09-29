use super::{
    ProgramEntryFusedServiceEstablishment, validate_terminal_fused_service_establishments,
};
use crate::{ProgramEntrySourceReceiverSignature, SelectedProgramEntrySourceSignature};
use effects::provider_plan::{ProviderPlan, ServiceSchema};
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, MachineId, StructuralFieldId, StructuralTypeId,
};
use symbols::SymbolHandle;
use terminal_psi::{
    BindingRelevance, Block, MachineContract, StructuralFieldDeclaration, StructuralFieldType,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator,
};

fn fixture(
    nested: bool,
) -> (
    TerminalModule,
    SelectedProgramEntrySourceSignature,
    ProgramEntryFusedServiceEstablishment,
    ProviderPlan,
) {
    let source = SelectedProgramEntrySourceSignature::from_checked_typed_entry(
        target::TargetProfile::MacosArm64.program_entry_slot(),
        SymbolHandle::from_arena_index(1),
        SymbolHandle::from_arena_index(2),
        "Main::entry".into(),
        "entry".into(),
        "Main::entry()".into(),
        ProgramEntrySourceReceiverSignature::ProvisionedMutable {
            source_position: 0,
            source_symbol: SymbolHandle::from_arena_index(3),
            normalized_type_identity: "Main".into(),
        },
        Vec::new(),
    )
    .unwrap();
    let plan = ProviderPlan {
        name: "console".into(),
        provider_type: "ConsoleProvider".into(),
        schema: ServiceSchema {
            trait_name: "Console".into(),
            ..Default::default()
        },
        ..Default::default()
    };
    let field = StructuralFieldDeclaration {
        id: StructuralFieldId::new(1).unwrap(),
        identity: "console".into(),
        relevance: BindingRelevance::Erased,
        field_type: StructuralFieldType::Erased {
            type_identity: "ConsoleCarrier".into(),
        },
    };
    let attachment = StructuralTypeId::new(1).unwrap();
    let child = StructuralTypeId::new(2).unwrap();
    let mut module = TerminalModule::for_entry(MachineId::new(1).unwrap());
    let fields = if nested {
        module.structural_types.push(StructuralTypeDeclaration {
            id: child,
            identity: "Banner".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![field],
            },
        });
        vec![StructuralFieldDeclaration {
            id: StructuralFieldId::new(1).unwrap(),
            identity: "banner".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(child),
        }]
    } else {
        vec![field]
    };
    module.structural_types.insert(
        0,
        StructuralTypeDeclaration {
            id: attachment,
            identity: "Main".into(),
            shape: StructuralTypeShape::Record { fields },
        },
    );
    let entry_block = BlockId::new(1).unwrap();
    module.machines.push(TerminalMachine {
        id: module.entry,
        attachment: Some(attachment),
        parameters: Vec::new(),
        structural_parameters: Vec::new(),
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: entry_block,
        blocks: vec![Block {
            id: entry_block,
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: ContractId::new(1).unwrap(),
            crash_routes: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    });
    let field_path = if nested {
        vec!["banner".into(), "console".into()]
    } else {
        vec!["console".into()]
    };
    let row = ProgramEntryFusedServiceEstablishment::new(
        source.identity(),
        source.target_slot(),
        "Main".into(),
        "Main".into(),
        "console".into(),
        field_path,
        "ConsoleCarrier".into(),
        "ConsoleCarrier".into(),
        ProgramEntryFusedServiceEstablishment::requirement_identity_for_schema(&plan.schema),
        plan.schema.identity_digest(),
        plan.identity_digest(),
    )
    .unwrap();
    (module, source, row, plan)
}

#[test]
fn terminal_replay_accepts_direct_and_nested_receiver_routes() {
    for nested in [false, true] {
        let (module, source, row, plan) = fixture(nested);
        validate_terminal_fused_service_establishments(
            &module,
            &source,
            Some(&fixture_eligibility()),
            &[row],
            &[plan],
        )
        .expect("receiver route and exact selected provider replay");
        validate_terminal_fused_service_establishments(&module, &source, None, &[], &[])
            .expect("no establishments require no receiver-field replay");
    }
}

#[test]
fn terminal_replay_rejects_substitution_and_ambiguous_routes() {
    let (module, source, row, plan) = fixture(true);
    for mutation in 0..6 {
        let mut changed = row.clone();
        match mutation {
            0 => changed.receiver_type_identity = "Other".into(),
            1 => changed.attachment_type_identity = "Other".into(),
            2 => changed.field_path[0] = "missing".into(),
            3 => changed.carrier_type_identity = "Other".into(),
            4 => changed.target_slot = target::TargetProfile::WindowsX64.program_entry_slot(),
            _ => changed.requirement_identity = "Other".into(),
        }
        assert!(
            validate_terminal_fused_service_establishments(
                &module,
                &source,
                Some(&fixture_eligibility()),
                &[changed],
                std::slice::from_ref(&plan),
            )
            .is_err(),
            "receipt mutation {mutation}"
        );
    }
    assert!(
        validate_terminal_fused_service_establishments(
            &module,
            &source,
            Some(&fixture_eligibility()),
            &[row.clone(), row.clone()],
            std::slice::from_ref(&plan),
        )
        .is_err(),
        "duplicate receiver route"
    );
    assert!(
        validate_terminal_fused_service_establishments(
            &module,
            &source,
            Some(&fixture_eligibility()),
            std::slice::from_ref(&row),
            &[plan.clone(), plan.clone()],
        )
        .is_err(),
        "ambiguous selected plan"
    );
    let mut changed_plan = plan.clone();
    changed_plan.provider_type = "OtherProvider".into();
    assert!(
        validate_terminal_fused_service_establishments(
            &module,
            &source,
            Some(&fixture_eligibility()),
            std::slice::from_ref(&row),
            &[changed_plan],
        )
        .is_err(),
        "same schema cannot substitute a provider"
    );
    let mut changed_module = module;
    if let StructuralTypeShape::Record { fields } = &mut changed_module.structural_types[1].shape {
        fields.push(fields[0].clone());
    }
    assert!(
        validate_terminal_fused_service_establishments(
            &changed_module,
            &source,
            Some(&fixture_eligibility()),
            &[row],
            &[plan],
        )
        .is_err(),
        "ambiguous nested field"
    );
}

fn fixture_eligibility() -> terminal_psi::CheckedProgramEntryReceiverEligibility {
    terminal_psi::CheckedProgramEntryReceiverEligibility::new(
        "Main".into(),
        "Main".into(),
        terminal_psi::CheckedProgramEntryReceiverProjection::Erased { source_position: 0 },
        StructuralTypeId::new(1).unwrap(),
        Vec::new(),
        terminal_psi::CheckedProgramEntryReceiverCleanup::None,
    )
}

#[test]
fn terminal_replay_uses_retained_context_place_without_attachment() {
    let (mut module, source, row, plan) = fixture(false);
    let place = semantic_vocabulary::PlaceId::new(7).unwrap();
    let context_type = StructuralTypeId::new(1).unwrap();
    module.machines[0].attachment = None;
    module.machines[0]
        .structural_parameters
        .push(terminal_psi::StructuralParameterDeclaration {
            place,
            position: 0,
            is_self: false,
            structural_type: context_type,
            multiplicity: terminal_psi::StructuralMultiplicity::Affine,
            access: terminal_psi::StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    let eligibility = terminal_psi::CheckedProgramEntryReceiverEligibility::new(
        "Main".into(),
        "Main".into(),
        terminal_psi::CheckedProgramEntryReceiverProjection::Retained {
            terminal_self: place,
            source_position: 0,
        },
        context_type,
        Vec::new(),
        terminal_psi::CheckedProgramEntryReceiverCleanup::None,
    );
    validate_terminal_fused_service_establishments(
        &module,
        &source,
        Some(&eligibility),
        std::slice::from_ref(&row),
        std::slice::from_ref(&plan),
    )
    .unwrap();
    for mutation in 0..4 {
        let mut changed = module.clone();
        match mutation {
            0 => {
                changed.machines[0].structural_parameters[0].place =
                    semantic_vocabulary::PlaceId::new(8).unwrap()
            }
            1 => {
                changed.machines[0].structural_parameters[0].access =
                    terminal_psi::StructuralAccess::SharedBorrow
            }
            2 => changed.machines[0].attachment = Some(context_type),
            _ => changed.machines[0].structural_parameters.clear(),
        }
        assert!(
            validate_terminal_fused_service_establishments(
                &changed,
                &source,
                Some(&eligibility),
                std::slice::from_ref(&row),
                std::slice::from_ref(&plan)
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
    assert!(
        validate_terminal_fused_service_establishments(&module, &source, None, &[row], &[plan])
            .is_err()
    );
}
