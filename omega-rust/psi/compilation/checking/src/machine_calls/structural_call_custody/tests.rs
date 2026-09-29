use super::{StateScalarContractRow, structural_state_contract_scalar_predicates};

#[test]
fn scalar_state_contract_retains_declared_parameter_ranges() {
    for (spelling, minimum, maximum) in [
        ("i32 [-3..=4]", -3, 4_u64),
        ("u64 [0..4]", 0, 3),
        ("u64 [0..=18446744073709551615]", 0, u64::MAX),
    ] {
        let program = crate::front_end::typed_program(&format!(
            "machine bounded() {{ transition {{ _ -> next(0) }}
             state next(value: {spelling}) {{ }} }}"
        ));
        let state = &program.machine_states(&program.machines()[0])[1];
        let parameter = program.state_parameters(state)[0].symbol;
        assert_eq!(
            structural_state_contract_scalar_predicates(&program, state),
            Some(vec![StateScalarContractRow::Interval {
                parameter,
                minimum: numerics::bignum::BigInt::from_i64(minimum),
                maximum: numerics::bignum::BigInt::from_u64(maximum),
            }]),
            "the declared range is an arrival contract: {spelling}"
        );
    }
}

#[test]
fn scalar_state_contract_retains_every_fact_in_an_authored_clause() {
    let program = crate::front_end::typed_program(
        "machine bounded() -> u64 { transition { _ -> next(1, 2) }
         state next(first: u64, second: u64) -> u64
         requires first <= 2, second <= 2 { first } }",
    );
    let state = &program.machine_states(&program.machines()[0])[1];
    let expected = program
        .state_contracts(state)
        .iter()
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .map(|fact| match fact {
            typed_trees::domain::ProofFact::Expression(expression) => {
                StateScalarContractRow::Expression(*expression)
            }
            _ => panic!("scalar requirement fixture"),
        })
        .collect::<Vec<_>>();
    assert_eq!(expected.len(), 2);
    assert_eq!(
        structural_state_contract_scalar_predicates(&program, state),
        Some(expected)
    );
}

#[test]
fn scalar_state_contract_cannot_drop_a_later_unrepresented_membership() {
    let program = crate::front_end::typed_program(
        "domain u64::Small requires self <= 2;
         machine bounded() -> u64 { transition { _ -> next(1) }
         state next(value: u64) -> u64
         requires value <= 2, value in Small { value } }",
    );
    let state = &program.machine_states(&program.machines()[0])[1];
    assert!(structural_state_contract_scalar_predicates(&program, state).is_none());
}
