use super::*;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, InvocationSite, OperationId, PackageKeyIdentity,
};
use terminal_psi::{
    Block, CheckedMachineDeclaration, MachineContract, NominalDeclarationIdentity, Operation,
    OperationResult, TerminalIdentityTableBuilder, TerminalMachine, TerminalMachineResult,
    Terminator,
};

fn package(byte: u8) -> DeclarationCustody {
    DeclarationCustody::Package(PackageKeyIdentity::from_digest([byte; 32]).unwrap())
}

fn declaration_row(
    identity: &str,
    custody: DeclarationCustody,
    purpose: SelectionPurpose,
) -> SelectionDeclaration {
    SelectionDeclaration {
        identity: identity.into(),
        callable_identity: None,
        name: identity.into(),
        custody,
        purpose,
    }
}

fn source_question(
    owner: SelectionDeclarationId,
    selected: SelectionReference,
    purpose: SelectionPurpose,
) -> SelectionQuestion {
    SelectionQuestion {
        form: SelectionForm::Semantic { ordinal: 1 },
        occurrence: SelectionOccurrence {
            lexical_owner: owner,
            node: SelectionNode::Expression {
                index: 1,
                generation: 1,
            },
        },
        selected,
        purpose,
        role: SelectionRole::Type,
    }
}

struct PackageEdge {
    owner: PackageKeyIdentity,
}
impl DeclarationSelectionAuthority for PackageEdge {
    fn allows_declaration_selection(
        &self,
        requester: PackageKeyIdentity,
        owner: PackageKeyIdentity,
    ) -> bool {
        DeclarationCustody::Package(requester) == package(1) && owner == self.owner
    }
    fn package_label(&self, key: PackageKeyIdentity) -> String {
        format!("{key:?}")
    }
}

#[test]
fn each_source_question_uses_its_actual_dependency_purpose() {
    let module = TerminalModule::declarations();
    let build_edge = PackageEdge {
        owner: PackageKeyIdentity::from_digest([2; 32]).unwrap(),
    };
    let product_edge = PackageEdge {
        owner: PackageKeyIdentity::from_digest([3; 32]).unwrap(),
    };
    let authorities = SelectionAuthorities {
        build: Some(&build_edge),
        product: Some(&product_edge),
    };
    for purpose in [SelectionPurpose::Build, SelectionPurpose::Product] {
        let mut packet = SelectionQuestions::default();
        let caller = packet
            .declarations
            .push(declaration_row("caller", package(1), purpose));
        let helper = packet.declarations.push(declaration_row(
            "helper",
            package(2),
            SelectionPurpose::Product,
        ));
        packet.questions.push(source_question(
            caller,
            SelectionReference::Declaration(helper),
            purpose,
        ));
        // Type-only and other generic-form reads still require an edge.
        assert_eq!(
            admit_selection_questions(&module, &packet, &authorities).is_ok(),
            purpose == SelectionPurpose::Build
        );
        packet.declarations.get_mut(helper).unwrap().custody = package(3);
        assert_eq!(
            admit_selection_questions(&module, &packet, &authorities).is_ok(),
            purpose == SelectionPurpose::Product
        );
        packet.declarations.get_mut(helper).unwrap().purpose = SelectionPurpose::Build;
        assert!(
            admit_selection_questions(&module, &packet, &authorities)
                .unwrap_err()
                .contains("wrong Build/Product source context")
        );
    }
}

#[test]
fn deferred_product_inputs_are_inert_and_cannot_be_ordinary_reads() {
    let mut packet = SelectionQuestions::default();
    let caller = packet.declarations.push(declaration_row(
        "build",
        package(1),
        SelectionPurpose::Build,
    ));
    let input = [7; 32];
    let mut row = source_question(
        caller,
        SelectionReference::ProductInput {
            input_selection: input,
        },
        SelectionPurpose::Product,
    );
    row.role = SelectionRole::ProductDesignation(SelectionProductDesignation {
        site: BuildCallSite {
            machine: MachineId::new(1).unwrap(),
            operation: OperationId::new(1).unwrap(),
            boundary: semantic_vocabulary::BoundaryMachineId::new(1).unwrap(),
        },
        operand: 2,
        input_selection: input,
        static_application: None,
    });
    let question = packet.questions.push(row);
    let module = TerminalModule::declarations();
    let authorities = SelectionAuthorities {
        build: None,
        product: None,
    };
    assert!(admit_selection_questions(&module, &packet, &authorities).is_ok());
    packet.questions.get_mut(question).unwrap().role = SelectionRole::Executable;
    assert!(
        admit_selection_questions(&module, &packet, &authorities)
            .unwrap_err()
            .contains("outside its designated operand")
    );
}

fn machine(id: MachineId, identity: terminal_psi::TerminalIdentityIndex) -> TerminalMachine {
    let block = BlockId::new(1).unwrap();
    TerminalMachine {
        assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
        id,
        attachment: None,
        nominal_cleanup_subject: None,
        ranked_scc: None,
        structural_places: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry: block,
        blocks: vec![Block::plain(
            block,
            Vec::new(),
            Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                cleanup_actions: Vec::new(),
            },
        )],
        contract: MachineContract {
            declaration: Some(CheckedMachineDeclaration {
                machine: NominalDeclarationIdentity::new(identity),
                state: NominalDeclarationIdentity::new(identity),
                semantic_declaration: identity,
                parameter_types: Vec::new(),
                callable: identity,
                result_type: terminal_psi::TerminalIdentityIndex::ABSENT,
                contract: [1; 32],
                published_may_suspend: false,
                published_may_block: false,
                specialization: None,
            }),
            parameters: Vec::new(),
            parameter_order: Vec::new(),
            structural_parameters: Vec::new(),
            result: TerminalMachineResult::Unit,
            entry_claims: Vec::new(),
            declared_service_reach: None,
            closed_reach_application: None,
            published_synchronous_entries: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            ..MachineContract::empty(ContractId::new(1).unwrap())
        },
    }
}

#[test]
fn every_emitted_call_keeps_its_actual_source_selection() {
    let mut module = TerminalModule::declarations();
    let mut identities = TerminalIdentityTableBuilder::default();
    let mut packet = SelectionQuestions::default();
    let mut sources = Vec::new();
    for (index, name) in ["caller", "target", "decoy"].iter().enumerate() {
        let id = MachineId::new(index as u64 + 1).unwrap();
        module.machines.push(machine(id, identities.intern(name)));
        let source =
            packet
                .declarations
                .push(declaration_row(name, package(1), SelectionPurpose::Build));
        packet.joins.push(SelectionJoin::Machine {
            declaration: source,
            machine: id,
            head: BlockId::new(1).unwrap(),
        });
        sources.push(source);
    }
    module.identities = identities.build();
    let caller = module.machines[0].id;
    let target = module.machines[1].id;
    module.machines[0].blocks[0]
        .operations
        .push(Operation::unsuspended(
            OperationId::new(1).unwrap(),
            OperationResult::Unit,
            OperationKind::CallUnit {
                callee: target,
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        ));
    let mut row = source_question(
        sources[0],
        SelectionReference::Declaration(sources[1]),
        SelectionPurpose::Build,
    );
    row.form = SelectionForm::Published;
    row.role = SelectionRole::Executable;
    let question = packet.questions.push(row);
    let join = SelectionJoin::Invocation {
        question,
        original_caller: sources[0],
        machine: caller,
        site: CallableInvocationSite::Invocation(InvocationSite::Operation(
            OperationId::new(1).unwrap(),
        )),
        target: SelectionCallTarget::Declared(TargetMachineCallable::Machine(target)),
    };
    packet.joins.push(join);
    assert!(validate_selection_questions(&module, &packet).is_ok());
    packet.questions.get_mut(question).unwrap().selected =
        SelectionReference::Declaration(sources[2]);
    assert!(
        validate_selection_questions(&module, &packet)
            .unwrap_err()
            .contains("exact selected source declaration")
    );
    packet.questions.get_mut(question).unwrap().selected = SelectionReference::RuntimeContract {
        identity: "an unrelated structural callable contract".into(),
    };
    assert!(
        validate_selection_questions(&module, &packet)
            .unwrap_err()
            .contains("actual callable contract")
    );
    packet.questions.get_mut(question).unwrap().selected =
        SelectionReference::Declaration(sources[1]);
    packet.joins.pop();
    assert!(
        validate_selection_questions(&module, &packet)
            .unwrap_err()
            .contains("lacks its exact source selection question")
    );
}

#[test]
fn authority_walks_follow_the_evaluated_machine_and_leave_unused_code_out() {
    use semantic_vocabulary::{
        CallableContractId, MachineDescriptorId, ScalarQualificationSetId, ScalarType, ValueId,
    };
    use terminal_psi::{
        TerminalCallableContract, TerminalCallableEnvelope, TerminalCallableTermination,
        TerminalMachineDescriptor, TerminalMachineDescriptorOrigin, ValueDeclaration,
    };
    let mut module = TerminalModule::for_entry(MachineId::new(1).unwrap());
    let mut identities = TerminalIdentityTableBuilder::default();
    for (index, name) in ["caller", "selected", "unused"].iter().enumerate() {
        module.machines.push(machine(
            MachineId::new(index as u64 + 1).unwrap(),
            identities.intern(name),
        ));
    }
    let contract = CallableContractId::new(1).unwrap();
    let identity = identities.intern("closed no-argument callable");
    module.callables.contracts.push(TerminalCallableContract {
        id: contract,
        identity,
        scalar_parameters: Vec::new(),
        parameter_order: Vec::new(),
        structural_parameters: Vec::new(),
        result: terminal_psi::BoundaryMachineResult::Unit,
        proof_result: TerminalMachineResult::Unit,
        entry_claim_paths: Vec::new(),
        result_claim_paths: Vec::new(),
        result_reference_regions: Vec::new(),
        proof: MachineContract::empty(ContractId::new(1).unwrap()),
        evidence_contract: Vec::new(),
        envelope: TerminalCallableEnvelope {
            service_reach: Vec::new(),
            invocations: Vec::new(),
            suspends: false,
            blocks: false,
            termination: TerminalCallableTermination::NotGuaranteed,
        },
    });
    for ordinal in 1..=2 {
        let descriptor = MachineDescriptorId::new(ordinal).unwrap();
        module
            .callables
            .descriptors
            .push(TerminalMachineDescriptor {
                id: descriptor,
                contract,
                machine: MachineId::new(ordinal + 1).unwrap(),
                origin: TerminalMachineDescriptorOrigin {
                    application: identity,
                    conformance: None,
                    requirement: None,
                },
                refinement_obligations: Vec::new(),
                refinement_evidence: Vec::new(),
            });
        module.machines[0].blocks[0]
            .operations
            .push(Operation::unsuspended(
                OperationId::new(ordinal).unwrap(),
                OperationResult::Scalar(ValueDeclaration {
                    id: ValueId::new(ordinal).unwrap(),
                    scalar_type: ScalarType::Callable(contract),
                    qualifications: ScalarQualificationSetId::ZERO,
                }),
                OperationKind::SelectMachine { descriptor },
            ));
    }
    module.identities = identities.build();
    module.machines[0].blocks[0]
        .operations
        .push(Operation::unsuspended(
            OperationId::new(3).unwrap(),
            OperationResult::Unit,
            OperationKind::CallMachine {
                callee: ValueId::new(1).unwrap(),
                contract,
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                structural_arguments: Vec::new(),
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            },
        ));
    let walk = |module: &TerminalModule| {
        crate::SynchronousCallGraph::new(module, None, None)
            .unwrap()
            .from_root(MachineId::new(1).unwrap())
            .map(|closure| {
                closure
                    .machines
                    .iter()
                    .map(|machine| machine.id)
                    .collect::<Vec<_>>()
            })
    };
    assert_eq!(
        walk(&module).unwrap(),
        vec![MachineId::new(1).unwrap(), MachineId::new(2).unwrap()]
    );
    let OperationKind::CallMachine { callee, .. } =
        &mut module.machines[0].blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    *callee = ValueId::new(2).unwrap();
    assert_eq!(
        walk(&module).unwrap(),
        vec![MachineId::new(1).unwrap(), MachineId::new(3).unwrap()]
    );
    let OperationKind::CallMachine { callee, .. } =
        &mut module.machines[0].blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    *callee = ValueId::new(99).unwrap();
    assert!(walk(&module).is_err());
}
