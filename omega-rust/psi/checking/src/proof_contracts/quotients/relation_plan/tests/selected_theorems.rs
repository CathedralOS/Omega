use super::{carrier_type, named_argument, symbol};
use crate::proof_contracts::quotients::relation_plan::theorem_schema::{
    TheoremApplicationSide, TheoremContractFactLocation, TheoremContractOwner,
    derive_expected_theorem_schema,
};
use crate::proof_contracts::quotients::relation_plan::{
    ExactQuotientRelation, InputRelation, RepresentativeRuntimeParameter,
    RepresentativeStaticApplication, RepresentativeTelescope,
};
use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::signature::{SignatureContract, SignatureContractKind};

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
