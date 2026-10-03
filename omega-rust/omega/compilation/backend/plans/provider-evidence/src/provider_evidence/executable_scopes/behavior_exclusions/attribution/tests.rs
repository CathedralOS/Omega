use super::{
    BehaviorExclusion, BoundaryServiceOwners, EvidenceGapKind, ProhibitedSite,
    boundary_trait_identity,
};
use semantic_vocabulary::{BlockId, BoundaryMachineId, MachineId, OperationId, ServiceId};
use symbols::SymbolHandle;
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, CrashCause, ServiceDeclaration,
    TerminalModule,
};
use typed_trees::TypedTrees;
use typed_trees::signature::StateSignature;
use typed_trees::trait_definition::TraitDefinition;

fn fixture() -> (TypedTrees, TerminalModule, SymbolHandle) {
    let mut program = TypedTrees::default();
    let symbol = SymbolHandle::from_parts(1, 1);
    let mut declaration = TraitDefinition {
        symbol,
        name: "Sink".into(),
        is_boundary: true,
        ..Default::default()
    };
    let signature = StateSignature {
        name: "emit".into(),
        ..Default::default()
    };
    let identity = program
        .normalized_trait_requirement_overload_identity(&declaration, &signature)
        .identity();
    program.push_trait_machine_signature(&mut declaration, signature);
    program.push_trait_definition(declaration);
    let mut module = TerminalModule::for_entry(MachineId::new(1).unwrap());
    module.services.push(ServiceDeclaration {
        id: ServiceId::new(7).unwrap(),
        identity: "Sink".to_owned(),
        parents: Vec::new(),
    });
    module.boundary_machines.push(BoundaryMachineDeclaration {
        unconditional_termination: false,
        completion: terminal_psi::BoundaryCompletion::NormalReturn,
        kind: terminal_psi::BoundaryRequirementKind::Trait,
        scalar_ensures: Vec::new(),
        scalar_requires: Vec::new(),
        id: BoundaryMachineId::new(1).unwrap(),
        identity,
        attachment: None,
        parameter_order: Vec::new(),
        scalar_parameters: Vec::new(),
        crash_routes: Vec::new(),
        structural_parameters: Vec::new(),
        result: BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        fixed_service_reach: Vec::new(),
        published_service_ceiling: Vec::new(),
    });
    (program, module, symbol)
}

#[test]
fn declared_owner_rejoins_exact_requirement_and_module_service() {
    let (program, mut module, _) = fixture();
    let boundary = module.boundary_machines[0].id;
    let owners = BoundaryServiceOwners::from_declared_traits(&program, &module);
    assert_eq!(owners.owner(boundary), Some(module.services[0].id));
    assert_eq!(
        owners,
        BoundaryServiceOwners::from_module_identities(&module)
    );

    module.boundary_machines[0].identity.push_str("-foreign");
    assert_eq!(
        BoundaryServiceOwners::from_declared_traits(&program, &module).owner(boundary),
        None
    );
    let (program, mut module, _) = fixture();
    module.services[0].identity = "Other".to_owned();
    assert_eq!(
        BoundaryServiceOwners::from_declared_traits(&program, &module).owner(boundary),
        None
    );
}

#[test]
fn trait_identity_uses_exact_handle_and_boundary_kind() {
    let (mut program, module, symbol) = fixture();
    assert_eq!(
        boundary_trait_identity(&program, symbol).as_deref(),
        Some("Sink")
    );
    assert_eq!(
        boundary_trait_identity(&program, SymbolHandle::from_parts(1, 2)),
        None
    );
    let ordinary = SymbolHandle::from_parts(2, 1);
    program.push_trait_definition(TraitDefinition {
        symbol: ordinary,
        name: "Ordinary".into(),
        ..Default::default()
    });
    assert_eq!(boundary_trait_identity(&program, ordinary), None);
    let mut ordinary_only = TypedTrees::default();
    let mut declaration = program.traits()[0].clone();
    declaration.is_boundary = false;
    ordinary_only.push_trait_definition(declaration);
    assert_eq!(
        BoundaryServiceOwners::from_declared_traits(&ordinary_only, &module)
            .owner(module.boundary_machines[0].id),
        None
    );
}

#[test]
fn evidence_descriptions_keep_exact_sites_and_missing_evidence_distinct() {
    let (_, module, _) = fixture();
    let service = module.services[0].id;
    assert_eq!(
        BehaviorExclusion::Service(service).describe(&module),
        "service `Sink`"
    );
    let absent = ServiceId::new(99).unwrap();
    assert_eq!(
        BehaviorExclusion::Service(absent).describe(&module),
        format!("service {absent}")
    );
    assert_eq!(
        BehaviorExclusion::CrashCause(CrashCause::Trap).describe(&module),
        "crash cause Trap"
    );
    assert_eq!(
        BehaviorExclusion::PhysicalAuthorityClass(crate::TerminalAuthorityClass::ProcessOutput)
            .describe(&module),
        "physical authority class ProcessOutput"
    );
    let block = BlockId::new(3).unwrap();
    let operation = OperationId::new(4).unwrap();
    let boundary = module.boundary_machines[0].id;
    assert_eq!(
        ProhibitedSite::BoundaryCall {
            block,
            operation,
            boundary
        }
        .describe(&module),
        format!(
            "call to boundary `{}` (block {block}, operation {operation})",
            module.boundary_machines[0].identity
        )
    );
    let missing_boundary = BoundaryMachineId::new(99).unwrap();
    assert_eq!(
        ProhibitedSite::BoundaryCall {
            block,
            operation,
            boundary: missing_boundary
        }
        .describe(&module),
        format!("call to boundary `<undeclared boundary>` (block {block}, operation {operation})")
    );
    assert_eq!(
        ProhibitedSite::CrashTerminator { block }.describe(&module),
        format!("crash terminator in block {block}")
    );
    assert_eq!(
        ProhibitedSite::DynamicCall { block, operation }.describe(&module),
        format!("dynamic call (block {block}, operation {operation})")
    );
    assert_eq!(
        ProhibitedSite::PortWrite { block, operation }.describe(&module),
        format!("port write (block {block}, operation {operation})")
    );
    assert_eq!(
        EvidenceGapKind::DynamicCall.describe(),
        "a dynamic call has no bounded retained realization"
    );
    assert_eq!(
        EvidenceGapKind::UnknownEntry.describe(),
        "the selected entry is not retained in the module"
    );
    assert_eq!(
        EvidenceGapKind::UnknownCallee(module.entry).describe(),
        format!(
            "call target machine {} is not retained in the module",
            module.entry
        )
    );
    assert_eq!(
        EvidenceGapKind::UnknownBoundary(boundary).describe(),
        format!("boundary {boundary} has no retained declaration")
    );
}
