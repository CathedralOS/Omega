use super::{
    call_with_arguments, carrier_type, named_argument, push_custom_selected_theorem,
    push_representative, quotient_type, request_with_representative, static_argument, symbol,
};
use crate::proof_contracts::quotients::relation_plan::theorem_schema::{
    TheoremApplicationSide, TheoremContractFactLocation, TheoremContractOwner,
    TheoremParameterRole, derive_expected_theorem_schema,
};
use crate::proof_contracts::quotients::relation_plan::{
    ExactQuotientRelation, InputRelation, RelationPlanError, RepresentativeRuntimeParameter,
    RepresentativeStaticApplication, RepresentativeTelescope, derive_direct_terminal_plan,
    derive_selected_theorem_telescope,
};
use arena::HandleSpan;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, TypeParameter, TypeParameterKind};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{QuotientTheoremRole, StaticSymbolApplication};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use typed_trees::state::State;
use typed_trees::types::TypeReferenceNode;

#[test]
fn selected_theorem_requires_a_bodyful_checked_machine() {
    let mut program = TypedTrees::default();
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let mut request = request_with_representative(SymbolHandle::invalid());
    request.theorem_evidence[0].application = push_custom_selected_theorem(
        &mut program,
        language_semantics::MachineSupplyMode::Boundary,
        false,
        unit,
    );

    assert_eq!(
        derive_selected_theorem_telescope(&program, &request.theorem_evidence[0].application),
        Err(RelationPlanError::TheoremMustBeCheckedBody)
    );
}

#[test]
fn selected_theorem_requires_a_resultless_machine() {
    let mut program = TypedTrees::default();
    let result = carrier_type(&mut program);
    let mut request = request_with_representative(SymbolHandle::invalid());
    request.theorem_evidence[0].application = push_custom_selected_theorem(
        &mut program,
        language_semantics::MachineSupplyMode::CheckedBody,
        true,
        result,
    );

    assert_eq!(
        derive_selected_theorem_telescope(&program, &request.theorem_evidence[0].application),
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
    request.theorem_evidence[0].application = selected_theorem;

    let theorem =
        derive_selected_theorem_telescope(&program, &request.theorem_evidence[0].application)
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

    let plan = derive_direct_terminal_plan(&program, &program, &machine, &state, &call, &request)
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
    let congruence = &plan.theorem_evidence[0];
    assert_eq!(congruence.role, QuotientTheoremRole::Congruence);
    assert_eq!(congruence.selected_application.machine_symbol, symbol(700));
    assert_eq!(congruence.selected_application.state_symbol, symbol(701));
    assert_eq!(
        congruence.selected_application.static_application.bindings,
        Vec::new()
    );
    assert_eq!(
        congruence.termination,
        Some(
            crate::proof_contracts::quotients::relation_plan::theorem::SelectedTheoremTermination {
                machine_symbol: symbol(700),
                state_symbol: symbol(701),
            }
        )
    );
    assert_eq!(
        congruence.purity,
        Some(
            crate::proof_contracts::quotients::relation_plan::theorem::SelectedTheoremPurity {
                machine_symbol: symbol(700),
                state_symbol: symbol(701),
            }
        )
    );
    assert!(congruence.crash_free);
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
fn expected_theorem_schema_report_fingerprint_rejects_same_count_relation_and_mapping_drift() {
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
