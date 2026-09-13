//! Test-only checked-program builders shared by manifest regression suites.

pub(super) use checked_trees::{
    CheckedTrees, ClaimCarryPolicyFact, ContentIdentityReshuffleFact,
    ContentPartitionCompositionFact, ContentPartitionPlaceSubstitution,
    ContentPartitionResultRewrite, DataCarryFact, FlowCallFact, FlowClaimOutcomeEntryFact,
    FlowClaimOutcomeMapFact, FlowClaimOutcomeSource, FlowPermissionEventFact, FlowStateFact,
    MachineActivationCarryFact, MachineContractPlan, MachineMutationFact, MachineQualifications,
    MachineServiceReachRows, StateWriteFramePlan, SuspensionCrossingCarryFact,
    VacuousQualificationUse,
};
pub(super) use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, Place, PlaceRoot, ProgramPoint, QualificationEvidence,
};
pub(super) use language_semantics::content::{
    ContentAlgebraIdentity, ContentArithmeticOperator, ContentConservationEquation,
    ContentConservationOwnerKind, ContentConservationPlan, ContentConservationTerm,
    ContentFieldSegment, ContentPlaceRoot, ContentPlaceVersion, ContentProjectionExpression,
    ContentProjectionPlan, ContentScalarExpression, ContentStructuralPlace,
    conservation_report_fingerprint, projection_report_fingerprint,
};
pub(super) use language_semantics::{
    BlockingInterface, BlockingPlan, CarryAddress, CarryCpu, CarryHostThread, CarryPolicy,
    CarrySuspension, MachineSupplyMode, MachineTerminationPlan, PermissionAccess,
    PermissionClaimIdentity, PermissionEventKind, PermissionEventSource, PermissionProvenance,
    QualificationEvidenceOrigin, RankingViewId, RankingWitness, ReferenceAccess, SemanticDomainId,
    SuspensionInterface, SuspensionPlan, TerminationGuarantee, TerminationInterface,
};
pub(super) use symbols::SymbolHandle;
pub(super) use typed_trees::data::{MachineParameterContract, TypeParameter, TypeParameterKind};
pub(super) use typed_trees::domain::DomainDefinition;
pub(super) use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableBorrowExpression, TableCastExpression,
};
pub(super) use typed_trees::machine::Machine;
pub(super) use typed_trees::name::Identifier;
pub(super) use typed_trees::operator::OperatorDefinition;
pub(super) use typed_trees::signature::{StateParameter, StateSignature};
pub(super) use typed_trees::state::State;
pub(super) use typed_trees::statement::StatementNode;
pub(super) use typed_trees::trait_definition::TraitDefinition;
pub(super) use typed_trees::typed_trees::MachineSpecialization;
pub(super) use typed_trees::types::TypeReferenceNode;

pub(super) use effects::provider_plan::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceEntryAuthorityFlow, ServiceEntryClaim,
    ServiceMethod, ServiceResultClaim, ServiceSchema,
};

pub(super) fn push_behavior_contract(
    program: &mut CheckedTrees,
    machine: SymbolHandle,
    checked_may_suspend: bool,
    checked_may_block: bool,
) {
    let empty = language_semantics::ServiceReachRowTable::EMPTY_ROW;
    program.facts.service_reaches.machines.append_to_span(
        &mut program.facts.service_reaches.root_machines,
        MachineServiceReachRows {
            machine,
            dependency: flow_effects::ServiceReachDependency {
                concrete: empty,
                parameters: Default::default(),
            },
            interface: language_semantics::ServiceReachInterface::InternalInferred,
            published_ceiling: empty,
            inferred_direct: empty,
            inferred_transitive: empty,
            concrete_transitive: empty,
            effective: empty,
            concrete_effective: empty,
            unresolved_installation_reaches: Vec::new(),
            states: Default::default(),
        },
    );
    program.facts.synchronous_invocations.machines.push(
        checked_trees::MachineSynchronousInvocationFact {
            machine,
            published_targets: Vec::new(),
            checked_inferred_targets: Vec::new(),
            plan: Default::default(),
        },
    );
    program
        .facts
        .suspensions
        .machines
        .push(checked_trees::MachineSuspensionFact {
            machine,
            plan: SuspensionPlan {
                checked_may_suspend,
                ..Default::default()
            },
        });
    program
        .facts
        .blocking
        .machines
        .push(checked_trees::MachineBlockingFact {
            machine,
            plan: BlockingPlan {
                checked_may_block,
                ..Default::default()
            },
        });
    program
        .facts
        .termination
        .machines
        .push(checked_trees::MachineTerminationFact {
            machine,
            plan: Default::default(),
        });
    let state_write_frames = program
        .machines()
        .iter()
        .find(|definition| definition.symbol == machine)
        .map(|definition| {
            program
                .machine_states(definition)
                .iter()
                .map(|state| StateWriteFramePlan {
                    state: state.symbol,
                    frame: facts::NormalizedWriteFrame::opaque(),
                })
                .collect()
        });
    if let Some(state_write_frames) = state_write_frames {
        program.facts.mutation.machines.push(MachineMutationFact {
            machine,
            state_write_frames,
        });
    }
    program
        .facts
        .contract_plans
        .machines
        .push(MachineContractPlan {
            machine,
            closed_scalar_values: Default::default(),
            crash: Default::default(),
            report_fingerprint: 0,
            commitment: checked_trees::MachineContractCommitment::from_digest([0; 32]),
        });
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ManifestExactAxis {
    Contract,
    ServiceReach,
    SynchronousInvocation,
    Suspension,
    Blocking,
    Termination,
    Mutation,
}

impl ManifestExactAxis {
    pub(super) const ALL: [Self; 7] = [
        Self::Contract,
        Self::ServiceReach,
        Self::SynchronousInvocation,
        Self::Suspension,
        Self::Blocking,
        Self::Termination,
        Self::Mutation,
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Contract => "machine contract",
            Self::ServiceReach => "service-reach",
            Self::SynchronousInvocation => "synchronous-invocation",
            Self::Suspension => "suspension",
            Self::Blocking => "blocking",
            Self::Termination => "termination",
            Self::Mutation => "mutation",
        }
    }
}

pub(super) fn machine_contract_exact_rows_fixture() -> (CheckedTrees, SymbolHandle) {
    let machine_symbol = SymbolHandle::from_arena_index(60);
    let state_symbol = SymbolHandle::from_arena_index(61);
    let mut program = CheckedTrees::default();
    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Exact::run"),
        ..Default::default()
    };
    program.typed.push_machine_state(
        &mut machine,
        State {
            symbol: state_symbol,
            name: Identifier::generated("entry"),
            ..Default::default()
        },
    );
    program.typed.push_machine(machine);
    push_behavior_contract(&mut program, machine_symbol, true, false);
    program
        .facts
        .contract_plans
        .machines
        .last_mut()
        .expect("exact contract fixture")
        .report_fingerprint = 0x1234;
    (program, machine_symbol)
}

pub(super) fn remove_manifest_exact_row(
    program: &mut CheckedTrees,
    machine: SymbolHandle,
    axis: ManifestExactAxis,
) {
    match axis {
        ManifestExactAxis::Contract => program
            .facts
            .contract_plans
            .machines
            .retain(|fact| fact.machine != machine),
        ManifestExactAxis::ServiceReach => {
            program
                .facts
                .service_reaches
                .machines
                .for_each_mut(|_, fact| {
                    if fact.machine == machine {
                        fact.machine = SymbolHandle::invalid();
                    }
                });
        }
        ManifestExactAxis::SynchronousInvocation => program
            .facts
            .synchronous_invocations
            .machines
            .retain(|fact| fact.machine != machine),
        ManifestExactAxis::Suspension => program
            .facts
            .suspensions
            .machines
            .retain(|fact| fact.machine != machine),
        ManifestExactAxis::Blocking => program
            .facts
            .blocking
            .machines
            .retain(|fact| fact.machine != machine),
        ManifestExactAxis::Termination => program
            .facts
            .termination
            .machines
            .retain(|fact| fact.machine != machine),
        ManifestExactAxis::Mutation => program
            .facts
            .mutation
            .machines
            .retain(|fact| fact.machine != machine),
    }
}

pub(super) fn duplicate_manifest_exact_row(
    program: &mut CheckedTrees,
    source_machine: SymbolHandle,
    duplicate_machine: SymbolHandle,
    axis: ManifestExactAxis,
) {
    match axis {
        ManifestExactAxis::Contract => {
            let mut duplicate = program
                .facts
                .contract_plans
                .machines
                .iter()
                .find(|fact| fact.machine == source_machine)
                .expect("source contract row")
                .clone();
            duplicate.machine = duplicate_machine;
            program.facts.contract_plans.machines.push(duplicate);
        }
        ManifestExactAxis::ServiceReach => {
            let mut duplicate = program
                .facts
                .service_reaches
                .machines()
                .iter()
                .find(|fact| fact.machine == source_machine)
                .expect("source service-reach row")
                .clone();
            duplicate.machine = duplicate_machine;
            program
                .facts
                .service_reaches
                .machines
                .append_to_span(&mut program.facts.service_reaches.root_machines, duplicate);
        }
        ManifestExactAxis::SynchronousInvocation => {
            let mut duplicate = program
                .facts
                .synchronous_invocations
                .machines
                .iter()
                .find(|fact| fact.machine == source_machine)
                .expect("source synchronous-invocation row")
                .clone();
            duplicate.machine = duplicate_machine;
            program
                .facts
                .synchronous_invocations
                .machines
                .push(duplicate);
        }
        ManifestExactAxis::Suspension => {
            let mut duplicate = *program
                .facts
                .suspensions
                .machines
                .iter()
                .find(|fact| fact.machine == source_machine)
                .expect("source suspension row");
            duplicate.machine = duplicate_machine;
            program.facts.suspensions.machines.push(duplicate);
        }
        ManifestExactAxis::Blocking => {
            let mut duplicate = *program
                .facts
                .blocking
                .machines
                .iter()
                .find(|fact| fact.machine == source_machine)
                .expect("source blocking row");
            duplicate.machine = duplicate_machine;
            program.facts.blocking.machines.push(duplicate);
        }
        ManifestExactAxis::Termination => {
            let mut duplicate = program
                .facts
                .termination
                .machines
                .iter()
                .find(|fact| fact.machine == source_machine)
                .expect("source termination row")
                .clone();
            duplicate.machine = duplicate_machine;
            program.facts.termination.machines.push(duplicate);
        }
        ManifestExactAxis::Mutation => {
            let mut duplicate = program
                .facts
                .mutation
                .machines
                .iter()
                .find(|fact| fact.machine == source_machine)
                .expect("source mutation row")
                .clone();
            duplicate.machine = duplicate_machine;
            program.facts.mutation.machines.push(duplicate);
        }
    }
}

pub(super) fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else {
        "<non-string panic>".to_owned()
    }
}

pub(super) fn mutation_state_owner_fixture()
-> (CheckedTrees, SymbolHandle, SymbolHandle, SymbolHandle) {
    let owner = SymbolHandle::from_arena_index(50);
    let owner_state = SymbolHandle::from_arena_index(51);
    let other = SymbolHandle::from_arena_index(52);
    let other_state = SymbolHandle::from_arena_index(53);
    let mut program = CheckedTrees::default();
    for (machine_symbol, state_symbol, machine_name, state_name) in [
        (owner, owner_state, "Owner::write", "entry"),
        (other, other_state, "Other::write", "other_entry"),
    ] {
        let mut machine = Machine {
            symbol: machine_symbol,
            name: Identifier::generated(machine_name),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut machine,
            State {
                symbol: state_symbol,
                name: Identifier::generated(state_name),
                ..Default::default()
            },
        );
        if machine_symbol == owner {
            program.typed.push_machine_state(
                &mut machine,
                State {
                    symbol: SymbolHandle::from_arena_index(54),
                    name: Identifier::generated("next"),
                    ..Default::default()
                },
            );
        }
        program.typed.push_machine(machine);
    }
    (program, owner, owner_state, other_state)
}

pub(super) fn vacuous_qualification_fixture() -> (
    CheckedTrees,
    SymbolHandle,
    SymbolHandle,
    SymbolHandle,
    SymbolHandle,
    SemanticDomainId,
    ExpressionHandle,
    ExpressionHandle,
) {
    let machine_symbol = SymbolHandle::from_arena_index(60);
    let state_symbol = SymbolHandle::from_arena_index(61);
    let other_state_symbol = SymbolHandle::from_arena_index(63);
    let domain_symbol = SymbolHandle::from_arena_index(64);
    let mut program = CheckedTrees::default();
    let semantic_domain = program.typed.semantic_domains.intern("i64::Distance<1000>");
    let declaration_domain = program.typed.semantic_domains.intern("i64::Distance");
    program.typed.push_domain_definition(DomainDefinition {
        symbol: domain_symbol,
        name: Identifier::generated("i64::Distance"),
        semantic_id: declaration_domain,
        ..Default::default()
    });
    let cast_value = program
        .typed
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let cast_expression =
        program
            .typed
            .expression_table
            .insert(ExpressionNode::Cast(TableCastExpression {
                value: cast_value,
                target_type: Default::default(),
                result_type: Default::default(),
                target_label: Default::default(),
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
                semantic_domain: Default::default(),
                semantic_domain_arguments: Default::default(),
                semantic_domain_symbol: domain_symbol,
                semantic_domain_id: semantic_domain,
                form: Default::default(),
            }));
    let statement_expression = program
        .typed
        .expression_table
        .insert(ExpressionNode::Borrow(TableBorrowExpression {
            target: cast_expression,
            access: ReferenceAccess::Shared,
        }));
    for (machine, state, machine_name, state_name) in [
        (machine_symbol, state_symbol, "Main::main", "main"),
        (
            SymbolHandle::from_arena_index(62),
            other_state_symbol,
            "Other::run",
            "run",
        ),
    ] {
        let mut definition = Machine {
            symbol: machine,
            name: Identifier::generated(machine_name),
            ..Default::default()
        };
        let mut state_definition = State {
            symbol: state,
            name: Identifier::generated(state_name),
            ..Default::default()
        };
        if machine == machine_symbol {
            for _ in 0..3 {
                program
                    .typed
                    .statement_table
                    .push_statement(&mut state_definition.statement_nodes, Default::default());
            }
            program.typed.statement_table.push_statement(
                &mut state_definition.statement_nodes,
                StatementNode::Expression(statement_expression),
            );
        }
        program
            .typed
            .push_machine_state(&mut definition, state_definition);
        program.typed.push_machine(definition);
    }
    (
        program,
        machine_symbol,
        state_symbol,
        other_state_symbol,
        domain_symbol,
        semantic_domain,
        cast_expression,
        statement_expression,
    )
}

pub(super) fn selected_storage_plan() -> ProviderPlan {
    ProviderPlan {
        name: "selected::Storage".to_owned(),
        provider_type: "StorageProvider".to_owned(),
        provider_type_package_identity: None,
        target: String::new(),
        schema: ServiceSchema {
            trait_name: "StorageRoot".to_owned(),
            trait_package_identity: None,
            methods: vec![ServiceMethod {
                name: "transfer".to_owned(),
                requirement_owner: "StorageBase".to_owned(),
                requirement_owner_package_identity: None,
                requirement_identity: "StorageBase::transfer".to_owned(),
                parameter_count: 1,
                parameter_type_identities: vec!["Token".to_owned()],
                entry_claims: vec![ServiceEntryClaim {
                    parameter_index: 0,
                    carrier_identity: "named(name(Token))".to_owned(),
                    domain: "Token::Granted".to_owned(),
                    predicate_body: language_semantics::DomainPredicateBody::Bodyless,
                    effective_carry: CarryPolicy::STRICT,
                    authority_flow: ServiceEntryAuthorityFlow::Accepts,
                }],
                has_result: true,
                result_type_identity: Some("Token".to_owned()),
                result_claims: vec![ServiceResultClaim {
                    domain: "Token::Issued".to_owned(),
                    effective_carry: CarryPolicy::STRICT,
                }],
                service_reach: vec!["StorageRoot".to_owned()],
                synchronous_invocations: Vec::new(),
                may_suspend: false,
                may_block: false,
                terminates_guarantee: false,
                termination_premises: Vec::new(),
                calling_plan_report_fingerprint: None,
                calling_plan_commitment: None,
            }],
        },
        rows: vec![ProviderPlanRow {
            method: "transfer".to_owned(),
            requirement_identity: "StorageBase::transfer".to_owned(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::CheckedAdapter {
                machine_identity: "StorageProvider::transfer".to_owned(),
                machine_package_identity: None,
            },
        }],
        origin_package_identity: None,
        origin_package: "omega::providers::storage".to_owned(),
    }
}

pub(super) fn push_qualification_requirement(
    program: &mut CheckedTrees,
    is_boundary: bool,
    owner_index: u32,
    requirement_index: u32,
    owner_name: &str,
) -> (SymbolHandle, SymbolHandle) {
    let mut owner = TraitDefinition {
        symbol: SymbolHandle::from_arena_index(owner_index),
        is_boundary,
        name: Identifier::generated(owner_name),
        ..Default::default()
    };
    let requirement = SymbolHandle::from_arena_index(requirement_index);
    program.typed.push_trait_machine_signature(
        &mut owner,
        StateSignature {
            symbol: requirement,
            name: Identifier::generated("transfer"),
            ..Default::default()
        },
    );
    let owner_symbol = owner.symbol;
    program.typed.push_trait_definition(owner);
    (owner_symbol, requirement)
}
