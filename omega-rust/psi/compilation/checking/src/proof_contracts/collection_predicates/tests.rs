use super::{
    IntegerSet, data_fact_holds_at_zero, premises_prove_expression, standing_field_elements,
};
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::domain::ProofFact;
use typed_trees::signature::SignatureContractKind;

/// A four-cell walk over ordinary states, one element per state.
const WALK: &str = r#"
machine cells_valid(cells: &[i32; 4]) -> bool {
    transition cells[0] == 0 || cells[0] == 1 {
        true -> at_1(cells)
        _ -> (false)
    }
    state at_1(cells: &[i32; 4]) -> bool {
        transition cells[1] == 0 || cells[1] == 1 {
            true -> at_2(cells)
            _ -> (false)
        }
    }
    state at_2(cells: &[i32; 4]) -> bool {
        transition cells[2] == 0 || cells[2] == 1 {
            true -> at_3(cells)
            _ -> (false)
        }
    }
    state at_3(cells: &[i32; 4]) -> bool {
        transition cells[3] == 0 || cells[3] == 1 {
            true -> (true)
            _ -> (false)
        }
    }
}
"#;

/// The same recognizer as a tail-recursive walk with a counted index.
const LOOP: &str = r#"
machine cells_valid(cells: &[i32; 4]) -> bool {
    transition { _ -> cells_valid_from(cells, 0) }
}
machine cells_valid_from(cells: &[i32; 4], index: u64) -> bool
requires index <= 4
{
    transition index < 4 {
        true -> check(cells, index)
        _ -> (true)
    }
    state check(cells: &[i32; 4], index: u64) -> bool
    requires index < 4
    {
        transition cells[index] == 0 || cells[index] == 1 {
            true -> cells_valid_from(cells, index + 1)
            _ -> (false)
        }
    }
}
"#;

fn program(predicate: &str, rest: &str) -> TypedTrees {
    crate::front_end::typed_program(&format!("{predicate}\n{rest}"))
}

fn data_fact_at_zero(program: &TypedTrees, data: &str) -> bool {
    let definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == data)
        .unwrap();
    program
        .proof_facts
        .span_or_empty(definition.where_facts)
        .iter()
        .all(|fact| {
            let ProofFact::Expression(expression) = fact else {
                return false;
            };
            data_fact_holds_at_zero(program, definition, *expression)
        })
}

fn machine_ensures_proved(program: &TypedTrees, machine: &str) -> bool {
    let machine = program
        .machines()
        .iter()
        .find(|candidate| candidate.name.as_str() == machine)
        .unwrap();
    let mut goals = Vec::new();
    for contract in program.machine_contracts(machine) {
        if contract.kind != SignatureContractKind::Ensures {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let ProofFact::Expression(expression) = fact {
                goals.push(*expression);
            }
        }
    }
    assert!(!goals.is_empty());
    goals
        .into_iter()
        .all(|goal| premises_prove_expression(program, machine, goal))
}

#[test]
fn a_zero_valid_walk_admits_the_zeroed_collection() {
    for predicate in [WALK, LOOP] {
        let program = program(
            predicate,
            "data Grid where cells_valid(&board) == true { board: [i32; 4]; }",
        );
        assert!(data_fact_at_zero(&program, "Grid"), "{predicate}");
    }
}

#[test]
fn a_walk_false_at_zero_keeps_the_collection_gated() {
    let program = program(
        &WALK.replacen("cells[0] == 0 || cells[0] == 1", "cells[0] == 1", 1),
        "data Grid where cells_valid(&board) == true { board: [i32; 4]; }",
    );
    assert!(!data_fact_at_zero(&program, "Grid"));
}

#[test]
fn standing_facts_bound_every_element_through_the_body() {
    for predicate in [WALK, LOOP] {
        let program = program(
            predicate,
            "data Grid where cells_valid(&board) == true { board: [i32; 4]; offset: i32; }",
        );
        let definition = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == "Grid")
            .unwrap();
        let board = program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.name.as_str() == "board" => Some(field),
                _ => None,
            })
            .unwrap();
        let elements = standing_field_elements(&program, definition, board).expect(predicate);
        assert_eq!(elements, vec![IntegerSet::range(0, 1); 4]);
    }
}

#[test]
fn a_helper_theorem_selects_one_element_from_the_predicate() {
    let theorem = r#"
        machine cell_is_bit(cells: &[i32; 4], index: u64)
        requires cells_valid(cells) == true
        requires index < 4
        ensures cells[index] >= 0 && cells[index] <= 1
        {
        }
    "#;
    for predicate in [WALK, LOOP] {
        let program = program(predicate, theorem);
        assert!(
            machine_ensures_proved(&program, "cell_is_bit"),
            "{predicate}"
        );
    }
}

#[test]
fn a_helper_theorem_without_the_predicate_or_with_a_false_goal_is_unproved() {
    for theorem in [
        // No predicate premise: an arbitrary `i32` is not a bit.
        r#"
        machine cell_is_bit(cells: &[i32; 4], index: u64)
        requires index < 4
        ensures cells[index] >= 0 && cells[index] <= 1
        {
        }
        "#,
        // The predicate never implies a larger bound.
        r#"
        machine cell_is_bit(cells: &[i32; 4], index: u64)
        requires cells_valid(cells) == true
        requires index < 4
        ensures cells[index] >= 1
        {
        }
        "#,
        // An index the extent does not cover cannot be read.
        r#"
        machine cell_is_bit(cells: &[i32; 4], index: u64)
        requires cells_valid(cells) == true
        requires index < 5
        ensures cells[index] >= 0
        {
        }
        "#,
    ] {
        let program = program(WALK, theorem);
        assert!(
            !machine_ensures_proved(&program, "cell_is_bit"),
            "{theorem}"
        );
    }
}

#[test]
fn a_predicate_with_effects_or_writes_is_not_unfolded() {
    // A body that stores through its argument is outside the fragment even
    // when every path would return true.
    let program = program(
        r#"
        machine cells_valid(cells: &mut [i32; 4]) -> bool {
            cells[0] = 0;
            transition { _ -> (true) }
        }
        "#,
        "data Grid where cells_valid(&mut board) == true { board: [i32; 4]; }",
    );
    assert!(!data_fact_at_zero(&program, "Grid"));
}
