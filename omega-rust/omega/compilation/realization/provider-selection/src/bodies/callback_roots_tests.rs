// Exercise the shared installed-helper join directly. These owner tests do not
// claim artifact verification or physical interrupt installation.
use super::CallbackProviderDeclarations;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId,
    ScalarType, ValueId,
};
use terminal_psi::{
    Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
};
fn machine_id(value: u64) -> MachineId {
    MachineId::new(value).unwrap()
}

fn scalar_type() -> ScalarType {
    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
}

fn machine(identity: u64, kind: OperationKind) -> TerminalMachine {
    let result = ValueDeclaration {
        id: ValueId::new(1).unwrap(),
        scalar_type: scalar_type(),
        qualifications: Default::default(),
    };
    TerminalMachine {
        id: machine_id(identity),
        attachment: None,
        declaration: None,
        nominal_cleanup_subject: None,
        parameters: Vec::new(),
        parameter_order: Vec::new(),
        structural_parameters: Vec::new(),
        structural_places: Vec::new(),
        entry_claims: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        content_entry_claims: Vec::new(),
        result: TerminalMachineResult::Scalar(result),
        entry: BlockId::new(1).unwrap(),
        blocks: vec![Block {
            id: BlockId::new(1).unwrap(),
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations: vec![Operation::unsuspended(
                OperationId::new(1).unwrap(),
                OperationResult::Scalar(result),
                kind,
            )],
            terminator: Terminator::Return {
                edge: EdgeId::new(1).unwrap(),
                value: result.id,
                cleanup_actions: Vec::new(),
            },
        }],
        ranked_scc: None,
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        contract: MachineContract::empty(ContractId::new(identity).unwrap()),
        assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
    }
}

fn installed_module() -> TerminalModule {
    let mut module = TerminalModule::for_entry(machine_id(1));
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let nominal = terminal_psi::NominalDeclarationIdentity::new(identities.intern("installation"));
    let operation = OperationId::new(10).unwrap();
    for ordinal in 1..=3 {
        let text = format!("helper_{ordinal}");
        let name = identities.intern(&text);
        let declaration = terminal_psi::CheckedMachineDeclaration {
            machine: terminal_psi::NominalDeclarationIdentity::new(name),
            state: terminal_psi::NominalDeclarationIdentity::new(name),
            semantic_declaration: name,
            parameter_types: Vec::new(),
            callable: name,
            result_type: name,
            contract: [ordinal as u8; 32],
            published_may_suspend: false,
            published_may_block: false,
            specialization: None,
        };
        let mut body = machine(
            ordinal as u64,
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(0),
            },
        );
        body.declaration = Some(declaration.clone());
        module.machines.push(body);
        module
            .callback_applications
            .push(terminal_psi::TerminalCallbackApplication {
                application_index: ordinal,
                registrar: operation,
                static_machine_ordinal: ordinal,
                requirement: "helper".into(),
                calling_owner_application: "installation".into(),
                boundary: None,
                published_contract: [0; 32],
                entry_admitted: false,
                body: declaration,
            });
    }
    // An unrelated ordinary function is not an installed helper.
    module.machines.push(machine(
        4,
        OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(0),
        },
    ));
    module.installed_interrupt_registrations.push(
        terminal_psi::TerminalInstalledInterruptRegistration {
            operation,
            installer: semantic_vocabulary::BoundaryMachineId::new(1).unwrap(),
            service: semantic_vocabulary::ServiceId::new(1).unwrap(),
            registrar_boundary: semantic_vocabulary::BoundaryMachineId::new(2).unwrap(),
            registrar: nominal.clone(),
            registrar_owner: nominal.clone(),
            registrar_application: "installation".into(),
            registrar_declaring_application: "installation".into(),
            registrar_arguments: Vec::new(),
            registrar_lifetimes: Vec::new(),
            registrar_requirement: "install".into(),
            slot_declaration: nominal,
            slot_commitment: [1; 32],
            slot_encoding: Vec::new(),
            callbacks: Vec::new(),
        },
    );
    module.identities = identities.build();
    module
}

#[test]
fn installed_helpers_rejoin_exact_bodies_and_refuse_missing_or_substituted_evidence() {
    let module = installed_module();
    let roots = CallbackProviderDeclarations::installed_helper_roots(&module);
    assert_eq!(roots.len(), 1);
    assert_eq!(
        roots[0].1.as_ref().unwrap(),
        &[machine_id(1), machine_id(2), machine_id(3)]
    );
    let mut missing = module.clone();
    missing.callback_applications.remove(1);
    assert!(
        CallbackProviderDeclarations::installed_helper_roots(&missing)[0]
            .1
            .is_err()
    );
    let mut substituted = module.clone();
    let callable = substituted.machines[1]
        .declaration
        .as_ref()
        .unwrap()
        .callable;
    substituted.machines[0]
        .declaration
        .as_mut()
        .unwrap()
        .callable = callable;
    assert!(
        CallbackProviderDeclarations::installed_helper_roots(&substituted)[0]
            .1
            .is_err()
    );
    let mut duplicate = module.clone();
    duplicate.machines.push(module.machines[0].clone());
    assert!(
        CallbackProviderDeclarations::installed_helper_roots(&duplicate)[0]
            .1
            .is_err()
    );
    let mut duplicate_registration = module.clone();
    duplicate_registration
        .installed_interrupt_registrations
        .push(module.installed_interrupt_registrations[0].clone());
    assert!(
        CallbackProviderDeclarations::installed_helper_roots(&duplicate_registration)[0]
            .1
            .is_err()
    );
}

#[test]
fn installed_helper_demand_retains_dormant_errors_until_the_registering_caller_is_reached() {
    use terminal_psi::ProofDeclarationId as Declaration;
    let helpers = vec![
        (
            Declaration(1),
            OperationId::new(10).unwrap(),
            Ok(vec![Declaration(2), Declaration(3), Declaration(4)]),
        ),
        (
            Declaration(5),
            OperationId::new(20).unwrap(),
            Err("missing exact helper".into()),
        ),
    ];
    let mut roots = vec![Declaration(1)];
    assert!(
        CallbackProviderDeclarations::extend_reached_installed_helpers(
            &helpers,
            &[Declaration(1)],
            &mut roots
        )
        .unwrap()
    );
    assert_eq!(
        roots,
        vec![
            Declaration(1),
            Declaration(2),
            Declaration(3),
            Declaration(4)
        ]
    );
    assert!(
        !CallbackProviderDeclarations::extend_reached_installed_helpers(
            &helpers,
            &[Declaration(1)],
            &mut roots
        )
        .unwrap()
    );
    let error = CallbackProviderDeclarations::extend_reached_installed_helpers(
        &helpers,
        &[Declaration(1), Declaration(5)],
        &mut roots,
    )
    .unwrap_err();
    assert!(error[0].message.contains("missing exact helper"));
}
