use super::{
    binary_expression, call_with_arguments, named_argument,
    push_generic_representative_application, quotient_type, request_with_representative, symbol,
};
use crate::proof_contracts::quotients::relation_plan::{
    ExactQuotientRelation, InputRelation, RelationPlanError, RepresentativeContractFactLocation,
    RepresentativeContractOwner, RepresentativeRuntimeParameter, RepresentativeStaticApplication,
    RepresentativeStaticBindingKind, RepresentativeTelescope,
    derive_define_precondition_correspondence, derive_direct_terminal_plan,
    derive_exact_representative_static_application, derive_public_precondition_partition,
    derive_representative_precondition_partition, derive_representative_telescope,
    pure_representative_effect, substituted_type_matches, unconditional_representative_termination,
};
use arena::HandleSpan;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataDefinition;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionNode, TableBinaryExpression};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::proposition::{PropositionBinder, PropositionBinderKind, PropositionDefinition};
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use typed_trees::state::State;
use typed_trees::statement::StatementNode;
use typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

#[test]
fn direct_plan_rejects_untyped_adapted_argument() {
    let mut program = TypedTrees::default();
    let result_type = quotient_type(&mut program, symbol(1), "ResultQ", symbol(2), "ResultR");
    let literal = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let adapted = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: literal,
            operator: BinaryOperator::Equal,
            right: literal,
        }));
    let arguments = program
        .expression_table
        .insert_expression_handles([adapted]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: result_type,
        ..Default::default()
    };

    assert_eq!(
        derive_direct_terminal_plan(
            &program,
            &program,
            &Machine::default(),
            &state,
            &call,
            &request_with_representative(SymbolHandle::invalid()),
        ),
        Err(RelationPlanError::UnresolvedArgumentType(0))
    );
}

#[test]
fn direct_plan_rejects_nonquotient_result() {
    let mut program = TypedTrees::default();
    let result_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: result_type,
        ..Default::default()
    };

    assert_eq!(
        derive_direct_terminal_plan(
            &program,
            &program,
            &Machine::default(),
            &state,
            &call,
            &request_with_representative(SymbolHandle::invalid()),
        ),
        Err(RelationPlanError::ResultIsNotQuotient)
    );
}

#[test]
fn direct_plan_rejects_open_relation_application_without_operation_telescope() {
    let mut program = TypedTrees::default();
    let mut relation = PropositionDefinition {
        symbol: symbol(2),
        name: Identifier::generated_static("IndexedR"),
        ..Default::default()
    };
    program.push_proposition_binder(
        &mut relation,
        PropositionBinder {
            symbol: symbol(3),
            name: Identifier::generated_static("I"),
            kind: PropositionBinderKind::Machine,
            ..Default::default()
        },
    );
    program.push_proposition(relation);
    let quotient_type = quotient_type(&mut program, symbol(1), "IndexedQ", symbol(2), "IndexedR");
    let value_symbol = symbol(4);
    let value = named_argument(&mut program, "value", value_symbol);
    let arguments = program.expression_table.insert_expression_handles([value]);
    let call = call_with_arguments(arguments);
    let mut state = State {
        return_type: quotient_type,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: value_symbol,
            name: Identifier::generated_static("value"),
            type_reference: quotient_type,
            ..Default::default()
        },
    );

    assert_eq!(
        derive_direct_terminal_plan(
            &program,
            &program,
            &Machine::default(),
            &state,
            &call,
            &request_with_representative(SymbolHandle::invalid()),
        ),
        Err(RelationPlanError::UnresolvedInputRelationApplication(0))
    );
}

#[test]
fn representative_telescope_rejects_duplicate_state_identity_within_one_machine() {
    let mut program = TypedTrees::default();
    let result_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let mut machine = Machine {
        symbol: symbol(90),
        ..Default::default()
    };
    for _ in 0..2 {
        program.push_machine_state(
            &mut machine,
            State {
                symbol: symbol(91),
                return_type: result_type,
                ..Default::default()
            },
        );
    }
    program.push_machine(machine);
    let request = request_with_representative(symbol(91));

    assert_eq!(
        derive_representative_telescope(&program, &request),
        Err(RelationPlanError::RepresentativeEntryDoesNotResolveExactly)
    );
}

#[test]
fn representative_termination_retains_only_unconditional_checked_summary() {
    fn telescope_with_summary(
        summary: language_semantics::TerminationGuarantee,
    ) -> (TypedTrees, RepresentativeTelescope) {
        let mut program = TypedTrees::default();
        let result_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let mut machine = Machine {
            symbol: symbol(94),
            termination_plan: language_semantics::MachineTerminationPlan {
                checked_summary: summary,
                ..Default::default()
            },
            ..Default::default()
        };
        program.push_machine_state(
            &mut machine,
            State {
                symbol: symbol(95),
                return_type: result_type,
                ..Default::default()
            },
        );
        program.push_machine(machine);
        let telescope =
            derive_representative_telescope(&program, &request_with_representative(symbol(95)))
                .expect("exact representative telescope");
        (program, telescope)
    }

    let (program, telescope) =
        telescope_with_summary(language_semantics::TerminationGuarantee::Terminates {
            premises: Vec::new(),
        });
    assert_eq!(
        unconditional_representative_termination(&program, &telescope),
        Some(crate::proof_contracts::quotients::relation_plan::representative::RepresentativeTermination {
            machine_symbol: symbol(94),
            state_symbol: symbol(95),
        })
    );

    let (program, telescope) =
        telescope_with_summary(language_semantics::TerminationGuarantee::Terminates {
            premises: vec![language_semantics::ProgressPremise {
                profile: language_semantics::SemanticDomainId(1),
                subject: language_semantics::ProgressSubject {
                    root: symbol(96),
                    projections: Vec::new(),
                },
            }],
        });
    assert_eq!(
        unconditional_representative_termination(&program, &telescope),
        None,
    );

    let (program, telescope) =
        telescope_with_summary(language_semantics::TerminationGuarantee::NoGuarantee);
    assert_eq!(
        unconditional_representative_termination(&program, &telescope),
        None,
    );
}

#[test]
fn representative_purity_consumes_shared_recursive_effect_summaries() {
    let mut program = TypedTrees::default();
    let result_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let mut machine = Machine {
        symbol: symbol(96),
        ..Default::default()
    };
    program.push_machine_state(
        &mut machine,
        State {
            symbol: symbol(97),
            return_type: result_type,
            ..Default::default()
        },
    );
    program.push_machine(machine);
    let telescope =
        derive_representative_telescope(&program, &request_with_representative(symbol(97)))
            .expect("exact representative telescope");
    let operational = crate::infer_operational_may(&program);
    let service_reaches = crate::infer_service_reaches(&program, &operational);
    assert_eq!(
        pure_representative_effect(&telescope, &operational, &service_reaches,),
        Some(crate::proof_contracts::quotients::relation_plan::representative::RepresentativePurity {
            machine_symbol: symbol(96),
            state_symbol: symbol(97),
        })
    );

    let mut suspending = operational.clone();
    let machine_handle = suspending
        .machines
        .iter()
        .find_map(|(handle, summary)| (summary.symbol == symbol(96)).then_some(handle))
        .expect("machine effect summary");
    suspending
        .machines
        .get_mut(machine_handle)
        .transitive_may_suspend = true;
    assert_eq!(
        pure_representative_effect(&telescope, &suspending, &service_reaches,),
        None,
    );

    let mut mutable_telescope = telescope.clone();
    mutable_telescope
        .parameters
        .push(RepresentativeRuntimeParameter {
            symbol: symbol(98),
            type_reference: result_type,
            is_mutable: true,
            is_self: false,
        });
    assert_eq!(
        pure_representative_effect(&mutable_telescope, &operational, &service_reaches,),
        None,
    );

    let mut unresolved_program = program.clone();
    let arguments = unresolved_program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let unresolved_call = unresolved_program
        .expression_table
        .insert(ExpressionNode::Call(call_with_arguments(arguments)));
    let machine = unresolved_program.machines()[0].clone();
    let mut state = unresolved_program.machine_states(&machine)[0].clone();
    unresolved_program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(unresolved_call),
    );
    unresolved_program.machine_states_mut(&machine)[0] = state;
    let unresolved_operational = crate::infer_operational_may(&unresolved_program);
    let unresolved_reaches =
        crate::infer_service_reaches(&unresolved_program, &unresolved_operational);
    assert_eq!(
        pure_representative_effect(&telescope, &unresolved_operational, &unresolved_reaches,),
        None,
    );
}

#[test]
fn representative_telescope_retains_closed_static_application_for_substitution() {
    let mut program = TypedTrees::default();
    let request = push_generic_representative_application(&mut program);

    let application = derive_exact_representative_static_application(&program, &request)
        .expect("closed type/const/machine application must retain exact bindings");
    assert_eq!(application.bindings.len(), 3);
    assert_eq!(application.bindings[0].parameter, symbol(622));
    assert_eq!(
        application.bindings[0].kind,
        RepresentativeStaticBindingKind::Type
    );
    assert_eq!(application.bindings[1].parameter, symbol(623));
    assert_eq!(
        application.bindings[1].kind,
        RepresentativeStaticBindingKind::Const
    );
    assert_eq!(application.bindings[2].parameter, symbol(624));
    assert_eq!(
        application.bindings[2].kind,
        RepresentativeStaticBindingKind::Machine
    );

    let telescope = derive_representative_telescope(&program, &request)
        .expect("a closed static application is retained on the telescope");
    assert_eq!(telescope.static_application, application);
}

#[test]
fn immutable_telescope_substitution_covers_type_const_and_machine_binders() {
    let mut program = TypedTrees::default();
    let request = push_generic_representative_application(&mut program);
    let bindings = derive_representative_telescope(&program, &request)
        .expect("closed application")
        .static_application
        .bindings;

    let type_template = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(622),
            name: Identifier::generated_static("T"),
        });
    let type_concrete = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(600),
            name: Identifier::generated_static("StaticType"),
        });
    assert!(substituted_type_matches(
        &program,
        type_template,
        type_concrete,
        &bindings,
    ));

    let machine_template = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(624),
            name: Identifier::generated_static("F"),
        });
    let machine_concrete = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(611),
            name: Identifier::generated_static("selected"),
        });
    assert!(substituted_type_matches(
        &program,
        machine_template,
        machine_concrete,
        &bindings,
    ));

    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let array_template = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: unit,
            length: FixedArrayLength::ConstParameter {
                symbol: symbol(623),
                name: Identifier::generated_static("N"),
            },
        });
    let array_concrete = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: unit,
            length: FixedArrayLength::Literal(0),
        });
    assert!(substituted_type_matches(
        &program,
        array_template,
        array_concrete,
        &bindings,
    ));
}

#[test]
fn immutable_telescope_substitution_rejects_type_and_const_near_misses() {
    let mut program = TypedTrees::default();
    let request = push_generic_representative_application(&mut program);
    let bindings = derive_representative_telescope(&program, &request)
        .expect("closed application")
        .static_application
        .bindings;
    let type_template = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(622),
            name: Identifier::generated_static("T"),
        });
    program.push_data_definition(DataDefinition {
        symbol: symbol(601),
        name: Identifier::generated_static("OtherType"),
        ..Default::default()
    });
    let other_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(601),
            name: Identifier::generated_static("OtherType"),
        });
    assert!(!substituted_type_matches(
        &program,
        type_template,
        other_type,
        &bindings,
    ));

    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let array_template = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: unit,
            length: FixedArrayLength::ConstParameter {
                symbol: symbol(623),
                name: Identifier::generated_static("N"),
            },
        });
    let wrong_length = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: unit,
            length: FixedArrayLength::Literal(1),
        });
    assert!(!substituted_type_matches(
        &program,
        array_template,
        wrong_length,
        &bindings,
    ));
    let stale_length = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: unit,
            length: FixedArrayLength::ConstParameter {
                symbol: symbol(623),
                name: Identifier::generated_static("N"),
            },
        });
    assert!(!substituted_type_matches(
        &program,
        array_template,
        stale_length,
        &bindings,
    ));
}

#[test]
fn representative_precondition_partition_tracks_exact_dependent_fact_locations() {
    let mut program = TypedTrees::default();
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let quotient_parameter = symbol(700);
    let fixed_parameter = symbol(701);
    let quotient_name = named_argument(&mut program, "quotient", quotient_parameter);
    let fixed_name = named_argument(&mut program, "fixed", fixed_parameter);
    let mixed = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: quotient_name,
            operator: BinaryOperator::Equal,
            right: fixed_name,
        }));
    let machine_facts = program.proof_facts.insert_many([
        ProofFact::Expression(quotient_name),
        ProofFact::Expression(mixed),
    ]);
    let state_facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(fixed_name)]);
    let machine_contracts = program.signature_contracts.insert_many([
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: machine_facts,
            ..Default::default()
        },
        SignatureContract {
            kind: SignatureContractKind::Ensures,
            facts: state_facts,
            ..Default::default()
        },
    ]);
    let state_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Requires,
        facts: state_facts,
        ..Default::default()
    }]);
    let telescope = RepresentativeTelescope {
        machine_symbol: symbol(710),
        state_symbol: symbol(711),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: quotient_parameter,
                type_reference: unit,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: fixed_parameter,
                type_reference: unit,
                is_mutable: false,
                is_self: false,
            },
        ],
        return_type: unit,
        machine_contracts,
        state_contracts,
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };
    let relations = [
        InputRelation::Quotient(ExactQuotientRelation {
            quotient_type: unit,
            quotient_symbol: symbol(720),
            relation_symbol: symbol(721),
        }),
        InputRelation::ExactEquality(unit),
    ];

    let partition = derive_representative_precondition_partition(&program, &relations, &telescope)
        .expect("all value identities are exact");
    assert_eq!(
        partition.dependent,
        vec![
            RepresentativeContractFactLocation {
                owner: RepresentativeContractOwner::Machine,
                contract_position: 0,
                fact_position: 0,
            },
            RepresentativeContractFactLocation {
                owner: RepresentativeContractOwner::Machine,
                contract_position: 0,
                fact_position: 1,
            },
        ]
    );
    assert_eq!(
        partition.fixed,
        vec![RepresentativeContractFactLocation {
            owner: RepresentativeContractOwner::State,
            contract_position: 0,
            fact_position: 0,
        }]
    );
}

#[test]
fn representative_precondition_partition_rejects_unresolved_value_identity() {
    let mut program = TypedTrees::default();
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let quotient_parameter = symbol(700);
    let quotient_name = named_argument(&mut program, "quotient", quotient_parameter);
    let unresolved = named_argument(&mut program, "unknown", SymbolHandle::invalid());
    let mixed = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: quotient_name,
            operator: BinaryOperator::Equal,
            right: unresolved,
        }));
    let facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(mixed)]);
    let machine_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Requires,
        facts,
        ..Default::default()
    }]);
    let telescope = RepresentativeTelescope {
        machine_symbol: symbol(710),
        state_symbol: symbol(711),
        parameters: vec![RepresentativeRuntimeParameter {
            symbol: quotient_parameter,
            type_reference: unit,
            is_mutable: false,
            is_self: false,
        }],
        return_type: unit,
        machine_contracts,
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };

    assert_eq!(
        derive_representative_precondition_partition(
            &program,
            &[InputRelation::Quotient(ExactQuotientRelation {
                quotient_type: unit,
                quotient_symbol: symbol(720),
                relation_symbol: symbol(721),
            })],
            &telescope,
        ),
        Err(RelationPlanError::PreconditionDependencyUnresolved)
    );
}

#[test]
fn public_precondition_partition_follows_the_runtime_permutation() {
    let mut program = TypedTrees::default();
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let quotient_type = quotient_type(
        &mut program,
        symbol(732),
        "PartitionQ",
        symbol(733),
        "PartitionR",
    );
    let quotient_parameter = symbol(730);
    let ordinary_parameter = symbol(731);
    let omitted_quotient_parameter = symbol(736);
    let omitted_ordinary_parameter = symbol(737);
    let quotient_name = named_argument(&mut program, "quotient", quotient_parameter);
    let ordinary_name = named_argument(&mut program, "ordinary", ordinary_parameter);
    let omitted_quotient_name =
        named_argument(&mut program, "omitted_quotient", omitted_quotient_parameter);
    let omitted_ordinary_name =
        named_argument(&mut program, "omitted_ordinary", omitted_ordinary_parameter);
    let machine_facts = program.proof_facts.insert_many([
        ProofFact::Expression(quotient_name),
        ProofFact::Expression(omitted_quotient_name),
    ]);
    let state_facts = program.proof_facts.insert_many([
        ProofFact::Expression(ordinary_name),
        ProofFact::Expression(omitted_ordinary_name),
    ]);
    let mut machine = Machine::default();
    program.push_machine_contract(
        &mut machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: machine_facts,
            ..Default::default()
        },
    );
    let mut state = State::default();
    for (parameter, name, type_reference) in [
        (quotient_parameter, "quotient", quotient_type),
        (ordinary_parameter, "ordinary", unit),
        (
            omitted_quotient_parameter,
            "omitted_quotient",
            quotient_type,
        ),
        (omitted_ordinary_parameter, "omitted_ordinary", unit),
    ] {
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: parameter,
                name: Identifier::generated_static(name),
                type_reference,
                ..Default::default()
            },
        );
    }
    program.push_state_contract(
        &mut state,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: state_facts,
            ..Default::default()
        },
    );
    let relations = [
        InputRelation::ExactEquality(unit),
        InputRelation::Quotient(ExactQuotientRelation {
            quotient_type,
            quotient_symbol: symbol(732),
            relation_symbol: symbol(733),
        }),
    ];

    let runtime_positions = [
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DefineRuntimePosition {
            public_parameter: ordinary_parameter,
            representative_parameter: symbol(734),
        },
        crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DefineRuntimePosition {
            public_parameter: quotient_parameter,
            representative_parameter: symbol(735),
        },
    ];
    let partition = derive_public_precondition_partition(
        &program,
        &machine,
        &state,
        &relations,
        &runtime_positions,
    )
    .expect("public parameter identities are exact");
    assert_eq!(
        partition.dependent,
        vec![
            RepresentativeContractFactLocation {
                owner: RepresentativeContractOwner::Machine,
                contract_position: 0,
                fact_position: 0,
            },
            RepresentativeContractFactLocation {
                owner: RepresentativeContractOwner::Machine,
                contract_position: 0,
                fact_position: 1,
            },
        ]
    );
    assert_eq!(
        partition.fixed,
        vec![
            RepresentativeContractFactLocation {
                owner: RepresentativeContractOwner::State,
                contract_position: 0,
                fact_position: 0,
            },
            RepresentativeContractFactLocation {
                owner: RepresentativeContractOwner::State,
                contract_position: 0,
                fact_position: 1,
            },
        ]
    );
}

#[test]
fn define_preconditions_require_one_exact_alpha_renamed_bijection() {
    let mut program = TypedTrees::default();
    let public_left = symbol(740);
    let public_right = symbol(741);
    let representative_left = symbol(742);
    let representative_right = symbol(743);
    let public_fact = {
        let left = named_argument(&mut program, "public_left", public_left);
        let right = named_argument(&mut program, "public_right", public_right);
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator: BinaryOperator::Less,
                right,
            }))
    };
    let representative_fact = {
        let left = named_argument(&mut program, "representative_left", representative_left);
        let right = named_argument(&mut program, "representative_right", representative_right);
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator: BinaryOperator::Less,
                right,
            }))
    };
    let public_fixed = {
        let left = named_argument(&mut program, "public_right", public_right);
        let right = named_argument(&mut program, "public_right", public_right);
        binary_expression(&mut program, left, BinaryOperator::Equal, right)
    };
    let representative_fixed = {
        let left = named_argument(&mut program, "representative_right", representative_right);
        let right = named_argument(&mut program, "representative_right", representative_right);
        binary_expression(&mut program, left, BinaryOperator::Equal, right)
    };
    let public_facts = program.proof_facts.insert_many([
        ProofFact::Expression(public_fact),
        ProofFact::Expression(public_fixed),
    ]);
    let representative_facts = program.proof_facts.insert_many([
        ProofFact::Expression(representative_fact),
        ProofFact::Expression(representative_fixed),
    ]);
    let mut public_machine = Machine::default();
    program.push_machine_contract(
        &mut public_machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: public_facts,
            ..Default::default()
        },
    );
    let public_state = State::default();
    let mut representative_contracts = HandleSpan::empty();
    program.signature_contracts.append_to_span(
        &mut representative_contracts,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: representative_facts,
            ..Default::default()
        },
    );
    let location = RepresentativeContractFactLocation {
        owner: RepresentativeContractOwner::Machine,
        contract_position: 0,
        fact_position: 0,
    };
    let partition = crate::proof_contracts::quotients::relation_plan::precondition::RepresentativePreconditionPartition {
        dependent: vec![location],
        fixed: vec![RepresentativeContractFactLocation {
            fact_position: 1,
            ..location
        }],
    };
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(744),
        state_symbol: symbol(745),
        parameters: Vec::new(),
        return_type: TypeReferenceHandle::invalid(),
        machine_contracts: representative_contracts,
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };
    let correspondence = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DefineRuntimeCorrespondence {
        positions: vec![
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DefineRuntimePosition {
                public_parameter: public_left,
                representative_parameter: representative_left,
            },
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DefineRuntimePosition {
                public_parameter: public_right,
                representative_parameter: representative_right,
            },
        ],
    };

    let exact = derive_define_precondition_correspondence(
        &program,
        &public_machine,
        &public_state,
        &representative,
        &partition,
        &partition,
        &correspondence,
    )
    .expect("parameter names may differ while exact positions agree");
    assert_eq!(exact.dependent.len(), 1);
    assert_eq!(exact.fixed.len(), 1);

    let redirected = crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DefineRuntimeCorrespondence {
        positions: vec![
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DefineRuntimePosition {
                public_parameter: public_left,
                representative_parameter: representative_right,
            },
            crate::proof_contracts::quotients::relation_plan::runtime_correspondence::DefineRuntimePosition {
                public_parameter: public_right,
                representative_parameter: representative_left,
            },
        ],
    };
    assert_eq!(
        derive_define_precondition_correspondence(
            &program,
            &public_machine,
            &public_state,
            &representative,
            &partition,
            &partition,
            &redirected,
        ),
        Err(RelationPlanError::DefinePreconditionMismatch)
    );
}
