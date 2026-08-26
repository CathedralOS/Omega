use super::{
    ExactQuotientRelation, InputRelation, RelationPlanError, RepresentativeContractFactLocation,
    RepresentativeContractOwner, RepresentativeRuntimeParameter, RepresentativeStaticApplication,
    RepresentativeStaticBinding, RepresentativeStaticBindingKind, RepresentativeTelescope,
    complete_single_state_result_flow, complete_state_forwarding_result_flow,
    derive_define_precondition_correspondence, derive_direct_terminal_plan,
    derive_exact_representative_static_application, derive_public_precondition_partition,
    derive_representative_precondition_partition, derive_representative_telescope,
    derive_selected_theorem_telescope, fallthrough_result_root, immutable_alias_fallthrough_root,
    pure_representative_effect, substituted_type_matches, unconditional_representative_termination,
};
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{
    DataDefinition, MachineParameterContract, QuotientDefinition, TypeParameter, TypeParameterKind,
};
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, QuotientOperationKind,
    QuotientOperationRequest, StaticMachineArgument, StaticSymbolApplication,
    TableBinaryExpression, TableCallExpression, TableNamePath,
};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::name::Identifier;
use psi_typed_trees::proposition::{
    PropositionApplication, PropositionBinder, PropositionBinderKind, PropositionDefinition,
};
use psi_typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{StatementNode, TableLocalData, TableTransition};
use psi_typed_trees::types::{FixedArrayLength, TypeReferenceHandle, TypeReferenceNode};

use super::theorem::SelectedTheoremTelescope;
use super::theorem_schema::{
    TheoremApplicationSide, TheoremContractFactLocation, TheoremContractOwner,
    TheoremParameterRole, derive_expected_theorem_schema,
};
use super::theorem_schema_verification::verify_selected_theorem_schema;

fn symbol(index: u32) -> SymbolHandle {
    SymbolHandle::from_arena_index(index)
}

fn quotient_type(
    program: &mut TypedTrees,
    quotient_symbol: SymbolHandle,
    quotient_name: &'static str,
    relation_symbol: SymbolHandle,
    relation_name: &'static str,
) -> TypeReferenceHandle {
    let carrier_symbol = symbol(500);
    if !program
        .data_definitions()
        .iter()
        .any(|definition| definition.symbol == carrier_symbol)
    {
        program.push_data_definition(DataDefinition {
            symbol: carrier_symbol,
            name: Identifier::generated_static("Carrier"),
            ..Default::default()
        });
    }
    let carrier = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: carrier_symbol,
            name: Identifier::generated_static("Carrier"),
        });
    if !program
        .propositions()
        .iter()
        .any(|proposition| proposition.symbol == relation_symbol)
    {
        program.push_proposition(PropositionDefinition {
            symbol: relation_symbol,
            name: Identifier::generated_static(relation_name),
            ..Default::default()
        });
    }
    program.push_data_definition(DataDefinition {
        symbol: quotient_symbol,
        name: Identifier::generated_static(quotient_name),
        quotient: Some(QuotientDefinition {
            carrier,
            relation: vec![Identifier::generated_static(relation_name)],
            relation_symbol,
            equivalence: None,
        }),
        ..Default::default()
    });
    program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: quotient_symbol,
            name: Identifier::generated_static(quotient_name),
        })
}

fn carrier_type(program: &mut TypedTrees) -> TypeReferenceHandle {
    program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(500),
            name: Identifier::generated_static("Carrier"),
        })
}

fn named_argument(
    program: &mut TypedTrees,
    name: &'static str,
    name_symbol: SymbolHandle,
) -> psi_typed_trees::expression::ExpressionHandle {
    let mut members = HandleSpan::empty();
    program
        .expression_table
        .push_name_path_member(&mut members, Identifier::generated_static(name));
    program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath {
            members,
            head_symbol: name_symbol,
            symbol: name_symbol,
            ..Default::default()
        }))
}

fn call_with_arguments(
    arguments: HandleSpan<psi_typed_trees::expression::ExpressionHandle>,
) -> TableCallExpression {
    TableCallExpression {
        receiver: ExpressionHandle::invalid(),
        target_symbol: SymbolHandle::invalid(),
        target: Identifier::generated_static("lift"),
        machine_arguments: Box::default(),
        quotient_operation: None,
        private_layout_operation: None,
        arguments,
        evidence_arguments: Box::default(),
        operational_acknowledgement: Default::default(),
    }
}

fn static_argument(name: &'static str) -> StaticMachineArgument {
    StaticMachineArgument {
        path: vec![Identifier::generated_static(name)].into_boxed_slice(),
        application: None,
        const_literal: None,
        evidence_projection: None,
        symbol: SymbolHandle::invalid(),
    }
}

fn request_with_representative(symbol: SymbolHandle) -> QuotientOperationRequest {
    let mut representative_operation = static_argument("representative");
    representative_operation.symbol = symbol;
    QuotientOperationRequest {
        kind: QuotientOperationKind::Lift,
        representative_operation,
        selected_theorem: static_argument("ExactRespect"),
    }
}

fn push_selected_theorem(program: &mut TypedTrees) -> StaticMachineArgument {
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let mut theorem = Machine {
        symbol: symbol(700),
        name: Identifier::generated_static("selected_theorem"),
        termination_plan: psi_language_semantics::MachineTerminationPlan {
            checked_summary: psi_language_semantics::TerminationGuarantee::Terminates {
                premises: Vec::new(),
            },
            ..Default::default()
        },
        ..Default::default()
    };
    program.push_machine_state(
        &mut theorem,
        State {
            symbol: symbol(701),
            name: Identifier::generated_static("prove"),
            return_type: unit,
            ..Default::default()
        },
    );
    program.push_machine(theorem);
    let mut selected = static_argument("selected_theorem");
    selected.symbol = symbol(701);
    selected
}

fn push_custom_selected_theorem(
    program: &mut TypedTrees,
    supply_mode: psi_language_semantics::MachineSupplyMode,
    body_is_present: bool,
    return_type: TypeReferenceHandle,
) -> StaticMachineArgument {
    let mut theorem = Machine {
        symbol: symbol(710),
        name: Identifier::generated_static("custom_theorem"),
        supply_mode,
        body_is_present,
        ..Default::default()
    };
    program.push_machine_state(
        &mut theorem,
        State {
            symbol: symbol(711),
            name: Identifier::generated_static("prove"),
            return_type,
            ..Default::default()
        },
    );
    program.push_machine(theorem);
    let mut selected = static_argument("custom_theorem");
    selected.symbol = symbol(711);
    selected
}

#[test]
fn selected_theorem_requires_a_bodyful_checked_machine() {
    let mut program = TypedTrees::default();
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let mut request = request_with_representative(SymbolHandle::invalid());
    request.selected_theorem = push_custom_selected_theorem(
        &mut program,
        psi_language_semantics::MachineSupplyMode::Boundary,
        false,
        unit,
    );

    assert_eq!(
        derive_selected_theorem_telescope(&program, &request),
        Err(RelationPlanError::TheoremMustBeCheckedBody)
    );
}

#[test]
fn selected_theorem_requires_a_resultless_machine() {
    let mut program = TypedTrees::default();
    let result = carrier_type(&mut program);
    let mut request = request_with_representative(SymbolHandle::invalid());
    request.selected_theorem = push_custom_selected_theorem(
        &mut program,
        psi_language_semantics::MachineSupplyMode::CheckedBody,
        true,
        result,
    );

    assert_eq!(
        derive_selected_theorem_telescope(&program, &request),
        Err(RelationPlanError::TheoremMustBeResultless)
    );
}

#[test]
fn selected_theorem_retains_the_exact_closed_static_application() {
    let mut program = TypedTrees::default();
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let selected_type_symbol = symbol(722);
    program.push_data_definition(DataDefinition {
        symbol: selected_type_symbol,
        name: Identifier::generated_static("SelectedType"),
        ..Default::default()
    });
    let mut theorem = Machine {
        symbol: symbol(720),
        name: Identifier::generated_static("generic_theorem"),
        ..Default::default()
    };
    program.push_machine_type_parameter(
        &mut theorem,
        TypeParameter {
            symbol: symbol(723),
            name: Identifier::generated_static("T"),
            kind: TypeParameterKind::Type,
            ..Default::default()
        },
    );
    program.push_machine_state(
        &mut theorem,
        State {
            symbol: symbol(721),
            name: Identifier::generated_static("prove"),
            return_type: unit,
            ..Default::default()
        },
    );
    program.push_machine(theorem);

    let mut selected_type = static_argument("SelectedType");
    selected_type.symbol = selected_type_symbol;
    let mut selected_theorem = static_argument("generic_theorem");
    selected_theorem.symbol = symbol(721);
    selected_theorem.application = Some(Box::new(StaticSymbolApplication {
        lifetime_arguments: Box::default(),
        arguments: vec![selected_type].into_boxed_slice(),
    }));
    let mut request = request_with_representative(SymbolHandle::invalid());
    request.selected_theorem = selected_theorem;

    let theorem = derive_selected_theorem_telescope(&program, &request)
        .expect("one closed theorem application should derive");
    assert_eq!(theorem.machine_symbol, symbol(720));
    assert_eq!(theorem.state_symbol, symbol(721));
    assert_eq!(theorem.static_application.bindings.len(), 1);
    assert_eq!(
        theorem.static_application.bindings[0].parameter,
        symbol(723)
    );
    assert_eq!(
        theorem.static_application.bindings[0].argument.symbol,
        selected_type_symbol
    );
}

fn push_representative(
    program: &mut TypedTrees,
    parameters: &[(TypeReferenceHandle, bool, bool)],
    return_type: TypeReferenceHandle,
) -> QuotientOperationRequest {
    let mut machine = Machine {
        symbol: symbol(90),
        name: Identifier::generated_static("representative"),
        ..Default::default()
    };
    let mut state = State {
        symbol: symbol(91),
        name: Identifier::generated_static("entry"),
        return_type,
        ..Default::default()
    };
    for (position, (type_reference, is_self, is_const)) in parameters.iter().enumerate() {
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: symbol(100 + u32::try_from(position).expect("test position")),
                name: Identifier::generated(format!("p{position}")),
                type_reference: *type_reference,
                is_self: *is_self,
                is_const: *is_const,
                ..Default::default()
            },
        );
    }
    program.push_machine_contract(&mut machine, Default::default());
    program.push_state_contract(&mut state, Default::default());
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    let mut request = request_with_representative(symbol(91));
    request.selected_theorem = push_selected_theorem(program);
    request
}

fn push_generic_representative_application(program: &mut TypedTrees) -> QuotientOperationRequest {
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let type_symbol = symbol(600);
    program.push_data_definition(DataDefinition {
        symbol: type_symbol,
        name: Identifier::generated_static("StaticType"),
        ..Default::default()
    });

    let mut selected_machine = Machine {
        symbol: symbol(610),
        name: Identifier::generated_static("selected"),
        ..Default::default()
    };
    program.push_machine_state(
        &mut selected_machine,
        State {
            symbol: symbol(611),
            return_type: unit,
            ..Default::default()
        },
    );
    program.push_machine(selected_machine);

    let mut representative = Machine {
        symbol: symbol(620),
        name: Identifier::generated_static("generic_representative"),
        ..Default::default()
    };
    for parameter in [
        TypeParameter {
            symbol: symbol(622),
            name: Identifier::generated_static("T"),
            kind: TypeParameterKind::Type,
            ..Default::default()
        },
        TypeParameter {
            symbol: symbol(623),
            name: Identifier::generated_static("N"),
            kind: TypeParameterKind::Const {
                type_reference: unit,
            },
            ..Default::default()
        },
        TypeParameter {
            symbol: symbol(624),
            name: Identifier::generated_static("F"),
            kind: TypeParameterKind::Machine {
                contract: MachineParameterContract::default(),
            },
            ..Default::default()
        },
    ] {
        program.push_machine_type_parameter(&mut representative, parameter);
    }
    let representative_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(622),
            name: Identifier::generated_static("T"),
        });
    let mut representative_state = State {
        symbol: symbol(621),
        return_type: representative_type,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut representative_state,
        StateParameter {
            symbol: symbol(625),
            name: Identifier::generated_static("value"),
            type_reference: representative_type,
            ..Default::default()
        },
    );
    program.push_machine_state(&mut representative, representative_state);
    program.push_machine(representative);

    let mut type_argument = static_argument("StaticType");
    type_argument.symbol = type_symbol;
    let const_argument = StaticMachineArgument {
        path: Box::default(),
        application: None,
        const_literal: Some(Default::default()),
        evidence_projection: None,
        symbol: SymbolHandle::invalid(),
    };
    let mut machine_argument = static_argument("selected");
    machine_argument.symbol = symbol(611);
    let mut request = request_with_representative(symbol(621));
    request.representative_operation.application = Some(Box::new(StaticSymbolApplication {
        lifetime_arguments: Box::default(),
        arguments: vec![type_argument, const_argument, machine_argument].into_boxed_slice(),
    }));
    request.selected_theorem = push_selected_theorem(program);
    request
}

#[test]
fn direct_plan_retains_exact_input_and_result_quotient_identities() {
    let mut program = TypedTrees::default();
    let left_type = quotient_type(&mut program, symbol(1), "LeftQ", symbol(2), "LeftR");
    let right_type = quotient_type(&mut program, symbol(3), "RightQ", symbol(4), "RightR");
    let ordinary_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let left_symbol = symbol(5);
    let ordinary_symbol = symbol(6);
    let left = named_argument(&mut program, "left", left_symbol);
    let ordinary = named_argument(&mut program, "ordinary", ordinary_symbol);
    let arguments = program
        .expression_table
        .insert_expression_handles([left, ordinary]);
    let call = call_with_arguments(arguments);
    let machine = Machine::default();
    let mut state = State {
        return_type: right_type,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: left_symbol,
            name: Identifier::generated_static("left"),
            type_reference: left_type,
            ..Default::default()
        },
    );
    program.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: ordinary_symbol,
            name: Identifier::generated_static("ordinary"),
            type_reference: ordinary_type,
            ..Default::default()
        },
    );
    let representative_carrier = carrier_type(&mut program);
    let request = push_representative(
        &mut program,
        &[
            (representative_carrier, true, false),
            (ordinary_type, false, false),
            (ordinary_type, false, true),
        ],
        representative_carrier,
    );

    let plan = derive_direct_terminal_plan(&program, &machine, &state, &call, &request)
        .expect("direct named operands and quotient result derive an exact plan");

    assert_eq!(plan.input_relations.len(), 2);
    let InputRelation::Quotient(left_relation) = plan.input_relations[0] else {
        panic!("quotient input must retain its exact relation");
    };
    assert_eq!(left_relation.quotient_type, left_type);
    assert_eq!(left_relation.quotient_symbol, symbol(1));
    assert_eq!(left_relation.relation_symbol, symbol(2));
    assert_eq!(
        plan.input_relations[1],
        InputRelation::ExactEquality(ordinary_type)
    );
    assert_eq!(plan.result_relation.quotient_type, right_type);
    assert_eq!(plan.result_relation.quotient_symbol, symbol(3));
    assert_eq!(plan.result_relation.relation_symbol, symbol(4));
    assert_eq!(plan.representative.machine_symbol, symbol(90));
    assert_eq!(plan.representative.state_symbol, symbol(91));
    assert_eq!(plan.selected_theorem.machine_symbol, symbol(700));
    assert_eq!(plan.selected_theorem.state_symbol, symbol(701));
    assert_eq!(
        plan.selected_theorem.static_application.bindings,
        Vec::new()
    );
    assert_eq!(
        plan.selected_theorem_termination,
        Some(super::SelectedTheoremTermination {
            machine_symbol: symbol(700),
            state_symbol: symbol(701),
        })
    );
    assert_eq!(
        plan.selected_theorem_purity,
        Some(super::SelectedTheoremPurity {
            machine_symbol: symbol(700),
            state_symbol: symbol(701),
        })
    );
    assert!(plan.selected_theorem_crash_free);
    assert_eq!(plan.representative.parameters.len(), 2);
    assert!(plan.representative.parameters[0].is_self);
    assert!(!plan.representative.parameters[1].is_self);
    assert_eq!(plan.representative.return_type, representative_carrier);
    assert_eq!(plan.representative.machine_contracts.count(), 1);
    assert_eq!(plan.representative.state_contracts.count(), 1);
}

#[test]
fn expected_theorem_schema_pairs_quotient_positions_and_shares_ordinary_positions() {
    let mut program = TypedTrees::default();
    let carrier = carrier_type(&mut program);
    let ordinary = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let relation = ExactQuotientRelation {
        quotient_type: carrier,
        quotient_symbol: symbol(800),
        relation_symbol: symbol(801),
    };
    let result_relation = ExactQuotientRelation {
        quotient_type: carrier,
        quotient_symbol: symbol(802),
        relation_symbol: symbol(803),
    };
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(804),
        state_symbol: symbol(805),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: symbol(806),
                type_reference: carrier,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: symbol(807),
                type_reference: ordinary,
                is_mutable: false,
                is_self: false,
            },
        ],
        return_type: carrier,
        machine_contracts: HandleSpan::empty(),
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };

    let schema = derive_expected_theorem_schema(
        &program,
        &[
            InputRelation::Quotient(relation),
            InputRelation::ExactEquality(ordinary),
        ],
        result_relation,
        &representative,
    )
    .expect("one quotient position and one ordinary position form an exact schema");

    assert_eq!(schema.parameters.len(), 3);
    assert_eq!(
        schema.parameters[0].role,
        TheoremParameterRole::QuotientLeft
    );
    assert_eq!(
        schema.parameters[1].role,
        TheoremParameterRole::QuotientRight
    );
    assert_eq!(schema.parameters[2].role, TheoremParameterRole::Shared);
    assert_eq!(schema.parameters[0].representative_position, 0);
    assert_eq!(schema.parameters[1].representative_position, 0);
    assert_eq!(schema.parameters[2].representative_position, 1);
    assert_eq!(schema.left_application.machine_symbol, symbol(804));
    assert_eq!(schema.left_application.state_symbol, symbol(805));
    assert_eq!(schema.left_application.arguments, vec![0, 2]);
    assert_eq!(schema.right_application.arguments, vec![1, 2]);
    assert_eq!(schema.relation_premises.len(), 1);
    assert_eq!(schema.relation_premises[0].relation, relation);
    assert_eq!(schema.relation_premises[0].left_parameter, 0);
    assert_eq!(schema.relation_premises[0].right_parameter, 1);
    assert_eq!(schema.result_relation, result_relation);
}

#[test]
fn expected_theorem_schema_identity_rejects_same_count_relation_and_mapping_drift() {
    let mut program = TypedTrees::default();
    let carrier = carrier_type(&mut program);
    let ordinary = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let relation = ExactQuotientRelation {
        quotient_type: carrier,
        quotient_symbol: symbol(810),
        relation_symbol: symbol(811),
    };
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(812),
        state_symbol: symbol(813),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: symbol(814),
                type_reference: carrier,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: symbol(815),
                type_reference: ordinary,
                is_mutable: false,
                is_self: false,
            },
        ],
        return_type: carrier,
        machine_contracts: HandleSpan::empty(),
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };
    let schema = derive_expected_theorem_schema(
        &program,
        &[
            InputRelation::Quotient(relation),
            InputRelation::ExactEquality(ordinary),
        ],
        relation,
        &representative,
    )
    .expect("baseline structural schema");

    let mut changed_relation = schema.clone();
    changed_relation.relation_premises[0]
        .relation
        .relation_symbol = symbol(816);
    assert_eq!(
        changed_relation.relation_premises.len(),
        schema.relation_premises.len()
    );
    assert_ne!(changed_relation, schema);

    let mut changed_mapping = schema.clone();
    changed_mapping.right_application.arguments.swap(0, 1);
    assert_eq!(
        changed_mapping.right_application.arguments.len(),
        schema.right_application.arguments.len()
    );
    assert_ne!(changed_mapping, schema);
}

#[test]
fn expected_theorem_schema_retains_each_representative_requires_for_both_calls() {
    let mut program = TypedTrees::default();
    let carrier = carrier_type(&mut program);
    let first = named_argument(&mut program, "first", symbol(820));
    let second = named_argument(&mut program, "second", symbol(821));
    let machine_facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(first)]);
    let state_facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(first), ProofFact::Expression(second)]);
    let ignored_ensures = program
        .proof_facts
        .insert_many([ProofFact::Expression(second)]);
    let machine_contracts = program.signature_contracts.insert_many([
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: machine_facts,
            ..Default::default()
        },
        SignatureContract {
            kind: SignatureContractKind::Ensures,
            facts: ignored_ensures,
            ..Default::default()
        },
    ]);
    let state_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Requires,
        facts: state_facts,
        ..Default::default()
    }]);
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(822),
        state_symbol: symbol(823),
        parameters: vec![RepresentativeRuntimeParameter {
            symbol: symbol(820),
            type_reference: carrier,
            is_mutable: false,
            is_self: false,
        }],
        return_type: carrier,
        machine_contracts,
        state_contracts,
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };
    let relation = ExactQuotientRelation {
        quotient_type: carrier,
        quotient_symbol: symbol(824),
        relation_symbol: symbol(825),
    };

    let schema = derive_expected_theorem_schema(
        &program,
        &[InputRelation::Quotient(relation)],
        relation,
        &representative,
    )
    .expect("all exact representative requires should enter both legality applications");

    assert_eq!(schema.legality_premises.len(), 6);
    assert_eq!(
        schema.legality_premises[0].fact,
        TheoremContractFactLocation {
            owner: TheoremContractOwner::Machine,
            contract_position: 0,
            fact_position: 0,
        }
    );
    assert_eq!(
        schema.legality_premises[0].application,
        TheoremApplicationSide::Left
    );
    assert_eq!(
        schema.legality_premises[1].application,
        TheoremApplicationSide::Right
    );
    assert_eq!(
        schema.legality_premises[4].fact,
        TheoremContractFactLocation {
            owner: TheoremContractOwner::State,
            contract_position: 0,
            fact_position: 1,
        }
    );
    assert_eq!(
        schema.legality_premises[5].application,
        TheoremApplicationSide::Right
    );
}

#[derive(Clone, Copy)]
enum TheoremSchemaMutation {
    Exact,
    ExtraPremise,
    WrongRelation,
    WrongLegality,
    RedirectedOperation,
    DuplicatedLeftApplication,
    OmittedRightApplication,
    ReboundSharedArgument,
    NamedEvidenceLane,
    ConstParameter,
    AttachedReceiver,
    ParameterTypeMismatch,
    UnexpectedContractKind,
    MissingConclusion,
}

fn selected_theorem_schema_fixture(
    mutation: TheoremSchemaMutation,
) -> (
    TypedTrees,
    RepresentativeTelescope,
    SelectedTheoremTelescope,
    super::theorem_schema::ExpectedTheoremSchema,
) {
    let mut program = TypedTrees::default();
    let carrier = carrier_type(&mut program);
    let ordinary = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let representative_type_parameter = symbol(849);
    let theorem_type_parameter = symbol(848);
    let representative_carrier = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: representative_type_parameter,
            name: Identifier::generated_static("RepresentativeCarrier"),
        });
    let theorem_carrier = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: theorem_type_parameter,
            name: Identifier::generated_static("TheoremCarrier"),
        });
    let mut selected_carrier = static_argument("Carrier");
    selected_carrier.symbol = symbol(500);
    let relation_symbol = symbol(850);
    program.push_proposition(PropositionDefinition {
        symbol: relation_symbol,
        name: Identifier::generated_static("ExactRelation"),
        ..Default::default()
    });

    let representative_parameter = symbol(851);
    let representative_shared = symbol(852);
    let representative_legality = named_argument(
        &mut program,
        "representative_value",
        representative_parameter,
    );
    let representative_facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(representative_legality)]);
    let representative_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Requires,
        facts: representative_facts,
        ..Default::default()
    }]);
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(853),
        state_symbol: symbol(854),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: representative_parameter,
                type_reference: representative_carrier,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: representative_shared,
                type_reference: ordinary,
                is_mutable: false,
                is_self: false,
            },
        ],
        return_type: representative_carrier,
        machine_contracts: representative_contracts,
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: vec![RepresentativeStaticBinding {
                parameter: representative_type_parameter,
                kind: RepresentativeStaticBindingKind::Type,
                argument: selected_carrier.clone(),
            }],
        },
    };
    let relation = ExactQuotientRelation {
        quotient_type: carrier,
        quotient_symbol: symbol(855),
        relation_symbol,
    };
    let expected = derive_expected_theorem_schema(
        &program,
        &[
            InputRelation::Quotient(relation),
            InputRelation::ExactEquality(ordinary),
        ],
        relation,
        &representative,
    )
    .expect("closed direct theorem schema");

    let left_symbol = symbol(856);
    let right_symbol = symbol(857);
    let shared_symbol = symbol(858);
    let left = named_argument(&mut program, "left", left_symbol);
    let right = named_argument(&mut program, "right", right_symbol);
    let shared = named_argument(&mut program, "shared", shared_symbol);
    let relation_arguments = program
        .expression_table
        .insert_expression_handles([left, right]);
    let relation_fact = ProofFact::Proposition(PropositionApplication {
        proposition: if matches!(mutation, TheoremSchemaMutation::WrongRelation) {
            symbol(859)
        } else {
            relation_symbol
        },
        name: Identifier::generated_static("ExactRelation"),
        binder_arguments: Box::default(),
        arguments: relation_arguments,
    });
    let right_legality = if matches!(mutation, TheoremSchemaMutation::WrongLegality) {
        left
    } else {
        right
    };
    let mut requires = vec![
        relation_fact,
        ProofFact::Expression(left),
        ProofFact::Expression(right_legality),
    ];
    if matches!(mutation, TheoremSchemaMutation::ExtraPremise) {
        requires.push(ProofFact::Expression(shared));
    }
    let requires = program.proof_facts.insert_many(requires);

    let representative_call = |program: &mut TypedTrees,
                               first: ExpressionHandle,
                               shared: ExpressionHandle,
                               redirected: bool| {
        let arguments = program
            .expression_table
            .insert_expression_handles([first, shared]);
        program
            .expression_table
            .insert(ExpressionNode::Call(TableCallExpression {
                receiver: ExpressionHandle::invalid(),
                target_symbol: if redirected {
                    symbol(860)
                } else {
                    representative.state_symbol
                },
                target: Identifier::generated_static("representative"),
                machine_arguments: vec![selected_carrier.clone()].into_boxed_slice(),
                quotient_operation: None,
                private_layout_operation: None,
                arguments,
                evidence_arguments: Box::default(),
                operational_acknowledgement: Default::default(),
            }))
    };
    let left_call = representative_call(
        &mut program,
        left,
        shared,
        matches!(mutation, TheoremSchemaMutation::RedirectedOperation),
    );
    let right_first = if matches!(mutation, TheoremSchemaMutation::DuplicatedLeftApplication) {
        left
    } else {
        right
    };
    let right_shared = if matches!(mutation, TheoremSchemaMutation::ReboundSharedArgument) {
        left
    } else {
        shared
    };
    let right_call = representative_call(&mut program, right_first, right_shared, false);
    let conclusion_arguments = program.expression_table.insert_expression_handles([
        left_call,
        if matches!(mutation, TheoremSchemaMutation::OmittedRightApplication) {
            right
        } else {
            right_call
        },
    ]);
    let conclusion_fact = ProofFact::Proposition(PropositionApplication {
        proposition: relation_symbol,
        name: Identifier::generated_static("ExactRelation"),
        binder_arguments: Box::default(),
        arguments: conclusion_arguments,
    });
    let conclusion = if matches!(mutation, TheoremSchemaMutation::MissingConclusion) {
        program.proof_facts.insert_many(std::iter::empty())
    } else {
        program.proof_facts.insert_many([conclusion_fact])
    };
    let theorem_machine_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Requires,
        binding: matches!(mutation, TheoremSchemaMutation::NamedEvidenceLane)
            .then(|| Identifier::generated_static("dictionary")),
        facts: requires,
        ..Default::default()
    }]);
    let theorem_state_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: if matches!(mutation, TheoremSchemaMutation::UnexpectedContractKind) {
            SignatureContractKind::Crashes {
                cause: psi_typed_trees::signature::CrashCause::Trap,
            }
        } else {
            SignatureContractKind::Ensures
        },
        facts: conclusion,
        ..Default::default()
    }]);
    let mut theorem_machine = Machine {
        symbol: symbol(861),
        contracts: theorem_machine_contracts,
        ..Default::default()
    };
    let mut theorem_state = State {
        symbol: symbol(862),
        return_type: ordinary,
        contracts: theorem_state_contracts,
        ..Default::default()
    };
    for (position, (parameter_symbol, mut type_reference)) in [
        (left_symbol, theorem_carrier),
        (right_symbol, theorem_carrier),
        (shared_symbol, ordinary),
    ]
    .into_iter()
    .enumerate()
    {
        if position == 2 && matches!(mutation, TheoremSchemaMutation::ParameterTypeMismatch) {
            type_reference = theorem_carrier;
        }
        program.push_state_parameter(
            &mut theorem_state,
            StateParameter {
                symbol: parameter_symbol,
                name: Identifier::generated_static(match position {
                    0 => "left",
                    1 => "right",
                    _ => "shared",
                }),
                type_reference,
                is_const: position == 0
                    && matches!(mutation, TheoremSchemaMutation::ConstParameter),
                is_self: position == 0
                    && matches!(mutation, TheoremSchemaMutation::AttachedReceiver),
                ..Default::default()
            },
        );
    }
    program.push_machine_state(&mut theorem_machine, theorem_state);
    program.push_machine(theorem_machine);
    let theorem = SelectedTheoremTelescope {
        machine_symbol: symbol(861),
        state_symbol: symbol(862),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: left_symbol,
                type_reference: theorem_carrier,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: right_symbol,
                type_reference: theorem_carrier,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: shared_symbol,
                type_reference: ordinary,
                is_mutable: false,
                is_self: false,
            },
        ],
        machine_contracts: theorem_machine_contracts,
        state_contracts: theorem_state_contracts,
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: vec![RepresentativeStaticBinding {
                parameter: theorem_type_parameter,
                kind: RepresentativeStaticBindingKind::Type,
                argument: selected_carrier,
            }],
        },
    };
    (program, representative, theorem, expected)
}

#[test]
fn selected_theorem_schema_verification_certifies_exact_fact_coordinates() {
    let (program, representative, theorem, expected) =
        selected_theorem_schema_fixture(TheoremSchemaMutation::Exact);
    let certificate =
        verify_selected_theorem_schema(&program, &representative, &theorem, &expected)
            .expect("the exact selected theorem schema must be certified");

    assert_eq!(certificate.theorem_machine_symbol, symbol(861));
    assert_eq!(certificate.theorem_state_symbol, symbol(862));
    assert_eq!(certificate.parameters.len(), 3);
    assert_eq!(certificate.relation_premises.len(), 1);
    assert_eq!(certificate.relation_premises[0].expected_position, 0);
    assert_eq!(certificate.legality_premises.len(), 2);
    assert_ne!(
        certificate.legality_premises[0].actual,
        certificate.legality_premises[1].actual,
    );
    assert_eq!(certificate.conclusion.owner, TheoremContractOwner::State);
}

#[test]
fn selected_theorem_schema_verification_rejects_extra_and_wrong_premises() {
    for (mutation, expected_error) in [
        (
            TheoremSchemaMutation::ExtraPremise,
            RelationPlanError::TheoremSchemaPremiseCountMismatch,
        ),
        (
            TheoremSchemaMutation::WrongRelation,
            RelationPlanError::TheoremSchemaRelationPremiseMismatch(0),
        ),
        (
            TheoremSchemaMutation::WrongLegality,
            RelationPlanError::TheoremSchemaLegalityPremiseMismatch(1),
        ),
    ] {
        let (program, representative, theorem, schema) = selected_theorem_schema_fixture(mutation);
        assert_eq!(
            verify_selected_theorem_schema(&program, &representative, &theorem, &schema),
            Err(expected_error),
        );
    }
}

#[test]
fn selected_theorem_schema_verification_rejects_application_mapping_drift() {
    for mutation in [
        TheoremSchemaMutation::RedirectedOperation,
        TheoremSchemaMutation::DuplicatedLeftApplication,
        TheoremSchemaMutation::OmittedRightApplication,
        TheoremSchemaMutation::ReboundSharedArgument,
    ] {
        let (program, representative, theorem, schema) = selected_theorem_schema_fixture(mutation);
        assert_eq!(
            verify_selected_theorem_schema(&program, &representative, &theorem, &schema),
            Err(RelationPlanError::TheoremSchemaConclusionMismatch),
        );
    }
}

#[test]
fn selected_theorem_schema_verification_rejects_runtime_evidence_and_const_parameters() {
    for (mutation, expected_error) in [
        (
            TheoremSchemaMutation::NamedEvidenceLane,
            RelationPlanError::TheoremSchemaNamedEvidenceLane,
        ),
        (
            TheoremSchemaMutation::ConstParameter,
            RelationPlanError::TheoremSchemaConstParameter(0),
        ),
        (
            TheoremSchemaMutation::AttachedReceiver,
            RelationPlanError::TheoremSchemaAttachedReceiver(0),
        ),
        (
            TheoremSchemaMutation::ParameterTypeMismatch,
            RelationPlanError::TheoremSchemaParameterTypeMismatch(2),
        ),
        (
            TheoremSchemaMutation::UnexpectedContractKind,
            RelationPlanError::TheoremSchemaUnexpectedContractKind,
        ),
        (
            TheoremSchemaMutation::MissingConclusion,
            RelationPlanError::TheoremSchemaConclusionCountMismatch,
        ),
    ] {
        let (program, representative, theorem, schema) = selected_theorem_schema_fixture(mutation);
        assert_eq!(
            verify_selected_theorem_schema(&program, &representative, &theorem, &schema),
            Err(expected_error),
        );
    }
}

#[test]
fn direct_plan_rejects_untyped_adapted_argument() {
    let mut program = TypedTrees::default();
    let result_type = quotient_type(&mut program, symbol(1), "ResultQ", symbol(2), "ResultR");
    let literal = program
        .expression_table
        .insert(ExpressionNode::Integer(Default::default()));
    let arguments = program
        .expression_table
        .insert_expression_handles([literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: result_type,
        ..Default::default()
    };

    assert_eq!(
        derive_direct_terminal_plan(
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
        summary: psi_language_semantics::TerminationGuarantee,
    ) -> (TypedTrees, RepresentativeTelescope) {
        let mut program = TypedTrees::default();
        let result_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let mut machine = Machine {
            symbol: symbol(94),
            termination_plan: psi_language_semantics::MachineTerminationPlan {
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
        telescope_with_summary(psi_language_semantics::TerminationGuarantee::Terminates {
            premises: Vec::new(),
        });
    assert_eq!(
        unconditional_representative_termination(&program, &telescope),
        Some(super::RepresentativeTermination {
            machine_symbol: symbol(94),
            state_symbol: symbol(95),
        })
    );

    let (program, telescope) =
        telescope_with_summary(psi_language_semantics::TerminationGuarantee::Terminates {
            premises: vec![psi_language_semantics::ProgressPremise {
                profile: psi_language_semantics::SemanticDomainId(1),
                subject: psi_language_semantics::ProgressSubject {
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
        telescope_with_summary(psi_language_semantics::TerminationGuarantee::NoGuarantee);
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
    let operational = psi_effects::infer_operational_may(&program);
    let service_reaches = psi_effects::infer_service_reaches(&program, &operational);
    assert_eq!(
        pure_representative_effect(&telescope, &operational, &service_reaches,),
        Some(super::RepresentativePurity {
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
    let unresolved_operational = psi_effects::infer_operational_may(&unresolved_program);
    let unresolved_reaches =
        psi_effects::infer_service_reaches(&unresolved_program, &unresolved_operational);
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
fn public_precondition_partition_distinguishes_q_from_fixed_ordinary_facts() {
    let mut program = TypedTrees::default();
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let quotient_parameter = symbol(730);
    let ordinary_parameter = symbol(731);
    let quotient_name = named_argument(&mut program, "quotient", quotient_parameter);
    let ordinary_name = named_argument(&mut program, "ordinary", ordinary_parameter);
    let machine_facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(quotient_name)]);
    let state_facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(ordinary_name)]);
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
    for (parameter, name) in [
        (quotient_parameter, "quotient"),
        (ordinary_parameter, "ordinary"),
    ] {
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: parameter,
                name: Identifier::generated_static(name),
                type_reference: unit,
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
        InputRelation::Quotient(ExactQuotientRelation {
            quotient_type: unit,
            quotient_symbol: symbol(732),
            relation_symbol: symbol(733),
        }),
        InputRelation::ExactEquality(unit),
    ];

    let partition = derive_public_precondition_partition(&program, &machine, &state, &relations)
        .expect("public parameter identities are exact");
    assert_eq!(
        partition.dependent,
        vec![RepresentativeContractFactLocation {
            owner: RepresentativeContractOwner::Machine,
            contract_position: 0,
            fact_position: 0,
        }]
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
    let public_facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(public_fact)]);
    let representative_facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(representative_fact)]);
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
    let partition = super::RepresentativePreconditionPartition {
        dependent: vec![location],
        fixed: Vec::new(),
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
    let correspondence = super::DefineRuntimeCorrespondence {
        positions: vec![
            super::DefineRuntimePosition {
                public_parameter: public_left,
                representative_parameter: representative_left,
            },
            super::DefineRuntimePosition {
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

    let redirected = super::DefineRuntimeCorrespondence {
        positions: vec![
            super::DefineRuntimePosition {
                public_parameter: public_left,
                representative_parameter: representative_right,
            },
            super::DefineRuntimePosition {
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

#[test]
fn define_correspondence_applies_closed_representative_type_substitution() {
    let mut program = TypedTrees::default();
    let mut request = push_generic_representative_application(&mut program);
    request.kind = QuotientOperationKind::Define;
    let carrier = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(600),
            name: Identifier::generated_static("StaticType"),
        });
    program.push_proposition(PropositionDefinition {
        symbol: symbol(2),
        name: Identifier::generated_static("ExactR"),
        ..Default::default()
    });
    program.push_data_definition(DataDefinition {
        symbol: symbol(1),
        name: Identifier::generated_static("ExactQ"),
        quotient: Some(QuotientDefinition {
            carrier,
            relation: vec![Identifier::generated_static("ExactR")],
            relation_symbol: symbol(2),
            equivalence: None,
        }),
        ..Default::default()
    });
    let quotient = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(1),
            name: Identifier::generated_static("ExactQ"),
        });
    let public_symbol = symbol(3);
    let argument = named_argument(&mut program, "value", public_symbol);
    let arguments = program
        .expression_table
        .insert_expression_handles([argument]);
    let call = call_with_arguments(arguments);
    let mut state = State {
        return_type: quotient,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: public_symbol,
            name: Identifier::generated_static("value"),
            type_reference: quotient,
            ..Default::default()
        },
    );

    let plan = derive_direct_terminal_plan(&program, &Machine::default(), &state, &call, &request)
        .expect("closed T := StaticType must instantiate the runtime telescope");
    assert_eq!(
        plan.representative_precondition,
        Some(super::RepresentativePreconditionPartition {
            dependent: Vec::new(),
            fixed: Vec::new(),
        })
    );
    assert_eq!(
        plan.public_precondition,
        Some(super::RepresentativePreconditionPartition {
            dependent: Vec::new(),
            fixed: Vec::new(),
        })
    );
    assert_eq!(
        plan.define_precondition_correspondence,
        Some(super::DefinePreconditionCorrespondence {
            dependent: Vec::new(),
        })
    );
    assert_eq!(
        plan.define_correspondence
            .expect("define correspondence")
            .positions,
        vec![super::DefineRuntimePosition {
            public_parameter: public_symbol,
            representative_parameter: symbol(625),
        }]
    );
}

#[test]
fn representative_static_application_rejects_const_category_near_miss() {
    let mut program = TypedTrees::default();
    let mut request = push_generic_representative_application(&mut program);
    let application = request
        .representative_operation
        .application
        .as_mut()
        .expect("generic application");
    application.arguments[1] = application.arguments[0].clone();

    assert_eq!(
        derive_exact_representative_static_application(&program, &request),
        Err(RelationPlanError::RepresentativeStaticArgumentCategoryMismatch(1))
    );
}

#[test]
fn define_runtime_correspondence_rejects_reordered_public_parameters() {
    let mut program = TypedTrees::default();
    let quotient_type = quotient_type(&mut program, symbol(1), "ExactQ", symbol(2), "ExactR");
    let carrier_type = carrier_type(&mut program);
    let left_symbol = symbol(3);
    let right_symbol = symbol(4);
    let left = named_argument(&mut program, "left", left_symbol);
    let right = named_argument(&mut program, "right", right_symbol);
    let arguments = program
        .expression_table
        .insert_expression_handles([right, left]);
    let call = call_with_arguments(arguments);
    let mut state = State {
        return_type: quotient_type,
        ..Default::default()
    };
    for (parameter_symbol, name) in [(left_symbol, "left"), (right_symbol, "right")] {
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: parameter_symbol,
                name: Identifier::generated_static(name),
                type_reference: quotient_type,
                ..Default::default()
            },
        );
    }
    let mut request = push_representative(
        &mut program,
        &[(carrier_type, false, false), (carrier_type, false, false)],
        carrier_type,
    );
    request.kind = QuotientOperationKind::Define;

    assert_eq!(
        derive_direct_terminal_plan(&program, &Machine::default(), &state, &call, &request,),
        Err(RelationPlanError::DefineArgumentOrderMismatch(0))
    );
}

#[test]
fn derived_direct_terminal_plan_remains_non_executable() {
    let mut program = TypedTrees::default();
    let quotient_type = quotient_type(&mut program, symbol(1), "ExactQ", symbol(2), "ExactR");
    let value_symbol = symbol(3);
    let value = named_argument(&mut program, "value", value_symbol);
    let arguments = program.expression_table.insert_expression_handles([value]);
    let mut call = call_with_arguments(arguments);
    let carrier_type = carrier_type(&mut program);
    // Attached and free operations share the normalized positional form:
    // the representative receiver occupies position zero without forcing
    // the public wrapper parameter to be spelled `self`.
    let mut request =
        push_representative(&mut program, &[(carrier_type, true, false)], carrier_type);
    request.kind = QuotientOperationKind::Define;
    call.quotient_operation = Some(request);
    let call = program.expression_table.insert(ExpressionNode::Call(call));
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
    program
        .statement_table
        .push_statement(&mut state.statement_nodes, StatementNode::Expression(call));
    let mut machine = Machine::default();
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    let mut diagnostics = Vec::new();

    super::super::reject_quotient_operation_requests(&program, &mut diagnostics);

    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("compiler-derived direct-terminal relations RA=[0:")
    );
    assert!(diagnostics[0].message.contains("RR="));
    assert!(diagnostics[0].message.contains("define-runtime=[0]"));
    assert!(diagnostics[0].message.contains("Q=[dependent:0, fixed:0]"));
    assert!(diagnostics[0].message.contains("P=[dependent:0, fixed:0]"));
    assert!(diagnostics[0].message.contains("Q<->P=[dependent:0]"));
    assert!(diagnostics[0].message.contains(
        "theorem-schema=[parameters:2, relations:1, legality:0, applications:2, conclusion:1]"
    ));
    assert!(
        diagnostics[0]
            .message
            .contains("exact selected theorem schema verification")
    );
    assert!(
        diagnostics[0]
            .message
            .contains("checked pure representative effect summary")
    );
    assert!(!diagnostics[0].message.contains("the effect fence"));
    assert!(
        diagnostics[0]
            .message
            .contains("one unchanged state-fallthrough result edge through the exact result root")
    );
    assert!(
        diagnostics[0]
            .message
            .contains("executable quotient operations are not admitted")
    );
}

#[test]
fn immutable_alias_fallthrough_requires_an_exact_immutable_chain() {
    let mut program = TypedTrees::default();
    let quotient_type = quotient_type(&mut program, symbol(1), "ExactQ", symbol(2), "ExactR");
    let arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let mut call = call_with_arguments(arguments);
    call.quotient_operation = Some(request_with_representative(SymbolHandle::invalid()));
    let request = program.expression_table.insert(ExpressionNode::Call(call));
    let first_symbol = symbol(10);
    let second_symbol = symbol(11);
    let first_name = named_argument(&mut program, "first", first_symbol);
    let second_name = named_argument(&mut program, "second", second_symbol);
    let mut state = State {
        return_type: quotient_type,
        ..Default::default()
    };
    for local in [
        TableLocalData {
            symbol: first_symbol,
            name: Identifier::generated_static("first"),
            type_reference: quotient_type,
            initial_value: request,
            is_mutable: false,
        },
        TableLocalData {
            symbol: second_symbol,
            name: Identifier::generated_static("second"),
            type_reference: quotient_type,
            initial_value: first_name,
            is_mutable: false,
        },
    ] {
        program
            .statement_table
            .push_statement(&mut state.statement_nodes, StatementNode::LocalData(local));
    }
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(second_name),
    );

    assert_eq!(
        immutable_alias_fallthrough_root(&program, &state),
        Some(super::ImmutableAliasFallthroughRoot {
            request_expression: request,
            alias_count: 2,
        })
    );

    let drifted_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    if let StatementNode::LocalData(first) = &mut program
        .statement_table
        .statements_mut(state.statement_nodes)[0]
    {
        first.type_reference = drifted_type;
    }
    assert_eq!(immutable_alias_fallthrough_root(&program, &state), None);

    if let StatementNode::LocalData(first) = &mut program
        .statement_table
        .statements_mut(state.statement_nodes)[0]
    {
        first.type_reference = quotient_type;
        first.is_mutable = true;
    }
    assert_eq!(immutable_alias_fallthrough_root(&program, &state), None);
}

#[test]
fn complete_result_flow_requires_one_exact_machine_state() {
    let mut program = TypedTrees::default();
    let return_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let mut call = call_with_arguments(arguments);
    call.quotient_operation = Some(request_with_representative(SymbolHandle::invalid()));
    let request = program.expression_table.insert(ExpressionNode::Call(call));
    let mut state = State {
        symbol: symbol(31),
        return_type,
        ..Default::default()
    };
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(request),
    );
    let mut machine = Machine {
        symbol: symbol(30),
        ..Default::default()
    };
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);

    let root = {
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let root = fallthrough_result_root(&program, state).expect("exact result root");
        assert_eq!(
            complete_single_state_result_flow(&program, machine, state, root),
            Some(super::CompleteSingleStateResultFlow {
                machine_symbol: symbol(30),
                state_symbol: symbol(31),
                root,
            })
        );
        root
    };

    let mut machine = program.machines()[0].clone();
    program.push_machine_state(
        &mut machine,
        State {
            symbol: symbol(32),
            return_type,
            ..Default::default()
        },
    );
    program.machines_mut()[0] = machine;
    let machine = &program.machines()[0];
    let state = &program.machine_states(&machine)[0];
    assert_eq!(
        complete_single_state_result_flow(&program, &machine, state, root),
        None
    );
}

#[test]
fn complete_result_flow_rejects_a_transition_before_fallthrough() {
    let mut program = TypedTrees::default();
    let return_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let mut call = call_with_arguments(arguments);
    call.quotient_operation = Some(request_with_representative(SymbolHandle::invalid()));
    let request = program.expression_table.insert(ExpressionNode::Call(call));
    let mut state = State {
        symbol: symbol(41),
        return_type,
        ..Default::default()
    };
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Transition(TableTransition::default()),
    );
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(request),
    );
    let mut machine = Machine {
        symbol: symbol(40),
        ..Default::default()
    };
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    let machine = &program.machines()[0];
    let state = &program.machine_states(&machine)[0];
    let root = fallthrough_result_root(&program, state).expect("fallthrough edge still exists");

    assert_eq!(
        complete_single_state_result_flow(&program, &machine, state, root),
        None
    );
}

#[test]
fn complete_result_flow_accepts_exact_finite_state_forwarding() {
    let mut program = TypedTrees::default();
    let return_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let mut call = call_with_arguments(arguments);
    call.quotient_operation = Some(request_with_representative(SymbolHandle::invalid()));
    let request = program.expression_table.insert(ExpressionNode::Call(call));

    let mut result_state = State {
        symbol: symbol(52),
        return_type,
        ..Default::default()
    };
    program.statement_table.push_statement(
        &mut result_state.statement_nodes,
        StatementNode::Expression(request),
    );

    let target = program.statement_table.insert_transition_target(
        psi_typed_trees::statement::TransitionTargetNode::Named {
            path: psi_typed_trees::statement::TableNamePath {
                members: HandleSpan::empty(),
                head_symbol: symbol(52),
                symbol: symbol(52),
            },
            arguments: HandleSpan::empty(),
            evidence_arguments: Box::default(),
        },
    );
    let mut forwarding_state = State {
        symbol: symbol(51),
        return_type,
        ..Default::default()
    };
    program.statement_table.push_statement(
        &mut forwarding_state.statement_nodes,
        StatementNode::Transition(TableTransition {
            target,
            ..Default::default()
        }),
    );
    let mut machine = Machine {
        symbol: symbol(50),
        ..Default::default()
    };
    program.push_machine_state(&mut machine, forwarding_state);
    program.push_machine_state(&mut machine, result_state);
    program.push_machine(machine);

    let machine = &program.machines()[0];
    let result_state = &program.machine_states(machine)[1];
    let root = fallthrough_result_root(&program, result_state).expect("exact result root");
    assert_eq!(
        complete_state_forwarding_result_flow(&program, machine, result_state, root,),
        Some(super::CompleteStateForwardingResultFlow {
            machine_symbol: symbol(50),
            forwarding_edges: vec![super::StateForwardingEdge {
                source_state_symbol: symbol(51),
                target_state_symbol: symbol(52),
            }],
            result_state_symbol: symbol(52),
            root,
        })
    );

    let intermediate_target = program.statement_table.insert_transition_target(
        psi_typed_trees::statement::TransitionTargetNode::Named {
            path: psi_typed_trees::statement::TableNamePath {
                members: HandleSpan::empty(),
                head_symbol: symbol(53),
                symbol: symbol(53),
            },
            arguments: HandleSpan::empty(),
            evidence_arguments: Box::default(),
        },
    );
    let forwarding_span = program.machine_states(&program.machines()[0])[0].statement_nodes;
    let [StatementNode::Transition(transition)] =
        program.statement_table.statements_mut(forwarding_span)
    else {
        panic!("one forwarding transition")
    };
    transition.target = intermediate_target;
    let mut intermediate_state = State {
        symbol: symbol(53),
        return_type,
        ..Default::default()
    };
    program.statement_table.push_statement(
        &mut intermediate_state.statement_nodes,
        StatementNode::Transition(TableTransition {
            target,
            ..Default::default()
        }),
    );
    let mut expanded_machine = program.machines()[0].clone();
    program.push_machine_state(&mut expanded_machine, intermediate_state);
    program.machines_mut()[0] = expanded_machine;
    let machine = &program.machines()[0];
    let result_state = &program.machine_states(machine)[1];
    assert_eq!(
        complete_state_forwarding_result_flow(&program, machine, result_state, root,),
        Some(super::CompleteStateForwardingResultFlow {
            machine_symbol: symbol(50),
            forwarding_edges: vec![
                super::StateForwardingEdge {
                    source_state_symbol: symbol(51),
                    target_state_symbol: symbol(53),
                },
                super::StateForwardingEdge {
                    source_state_symbol: symbol(53),
                    target_state_symbol: symbol(52),
                },
            ],
            result_state_symbol: symbol(52),
            root,
        })
    );

    let [StatementNode::Transition(transition)] =
        program.statement_table.statements_mut(forwarding_span)
    else {
        panic!("one forwarding transition")
    };
    transition.continuation = target;
    let machine = &program.machines()[0];
    let result_state = &program.machine_states(machine)[1];
    assert_eq!(
        complete_state_forwarding_result_flow(&program, machine, result_state, root,),
        None,
    );

    let [StatementNode::Transition(transition)] =
        program.statement_table.statements_mut(forwarding_span)
    else {
        panic!("one forwarding transition")
    };
    transition.continuation = psi_typed_trees::statement::TransitionTargetHandle::invalid();

    let cycle_target = program.statement_table.insert_transition_target(
        psi_typed_trees::statement::TransitionTargetNode::Named {
            path: psi_typed_trees::statement::TableNamePath {
                members: HandleSpan::empty(),
                head_symbol: symbol(51),
                symbol: symbol(51),
            },
            arguments: HandleSpan::empty(),
            evidence_arguments: Box::default(),
        },
    );
    let intermediate_span = program.machine_states(&program.machines()[0])[2].statement_nodes;
    let [StatementNode::Transition(transition)] =
        program.statement_table.statements_mut(intermediate_span)
    else {
        panic!("one intermediate transition")
    };
    transition.target = cycle_target;
    let machine = &program.machines()[0];
    let result_state = &program.machine_states(machine)[1];
    assert_eq!(
        complete_state_forwarding_result_flow(&program, machine, result_state, root,),
        None,
    );
    let [StatementNode::Transition(transition)] =
        program.statement_table.statements_mut(intermediate_span)
    else {
        panic!("one intermediate transition")
    };
    transition.target = target;

    let mut duplicate_owner = Machine {
        symbol: symbol(60),
        ..Default::default()
    };
    program.push_machine_state(
        &mut duplicate_owner,
        State {
            symbol: symbol(52),
            return_type,
            ..Default::default()
        },
    );
    program.push_machine(duplicate_owner);
    let machine = &program.machines()[0];
    let result_state = &program.machine_states(machine)[1];
    assert_eq!(
        complete_state_forwarding_result_flow(&program, machine, result_state, root,),
        None,
    );
}

#[test]
fn derived_immutable_alias_fallthrough_remains_non_executable() {
    let mut program = TypedTrees::default();
    let quotient_type = quotient_type(&mut program, symbol(1), "ExactQ", symbol(2), "ExactR");
    let value_symbol = symbol(3);
    let value = named_argument(&mut program, "value", value_symbol);
    let arguments = program.expression_table.insert_expression_handles([value]);
    let mut call = call_with_arguments(arguments);
    let carrier_type = carrier_type(&mut program);
    let mut request =
        push_representative(&mut program, &[(carrier_type, true, false)], carrier_type);
    request.kind = QuotientOperationKind::Define;
    call.quotient_operation = Some(request);
    let request = program.expression_table.insert(ExpressionNode::Call(call));
    let result_symbol = symbol(4);
    let result = named_argument(&mut program, "result", result_symbol);
    let mut state = State {
        symbol: symbol(5),
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
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::LocalData(TableLocalData {
            symbol: result_symbol,
            name: Identifier::generated_static("result"),
            type_reference: quotient_type,
            initial_value: request,
            is_mutable: false,
        }),
    );
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(result),
    );
    let mut machine = Machine {
        symbol: symbol(6),
        ..Default::default()
    };
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    let mut diagnostics = Vec::new();

    super::super::reject_quotient_operation_requests(&program, &mut diagnostics);

    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("compiler-derived immutable-alias fallthrough relations")
    );
    assert!(
            diagnostics[0]
                .message
                .contains("complete transition-free single-state normal-result coverage through 1 exact immutable result alias")
        );
    assert!(
        diagnostics[0]
            .message
            .contains("executable quotient operations are not admitted")
    );
}

#[test]
fn nonterminal_expression_request_cannot_claim_direct_result_flow() {
    let mut program = TypedTrees::default();
    let arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let mut call = call_with_arguments(arguments);
    call.quotient_operation = Some(request_with_representative(SymbolHandle::invalid()));
    let request = program.expression_table.insert(ExpressionNode::Call(call));
    let terminal = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let mut state = State::default();
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(request),
    );
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(terminal),
    );
    let mut machine = Machine::default();
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    let mut diagnostics = Vec::new();

    super::super::reject_quotient_operation_requests(&program, &mut diagnostics);

    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("retains its exact representative operation and selected theorem")
    );
    assert!(
        !diagnostics[0]
            .message
            .contains("compiler-derived direct-terminal relations")
    );
    assert!(
        !diagnostics[0]
            .message
            .contains("unchanged state-fallthrough result root")
    );
}
