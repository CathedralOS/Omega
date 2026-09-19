//! Erased members and nominal-cleanup ownership: `proof [erased]:` fields and
//! case payloads remain semantically present but own no runtime
//! representation, so they never produce runtime cleanup. A record whose only
//! nominal-cleanup member is erased does not require a nominal drop, while
//! the same member relevant does; an attached `Type::drop` keeps the owner
//! nominal regardless of its members.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::TypedTrees;
use typed_trees::data::DataDefinition;

fn typed(source: &str) -> TypedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = parse_syntax_trees(&tokens).expect("syntax");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolved source");
    lower_symbol_resolved_trees(&resolved).expect("typed source")
}

fn data<'program>(program: &'program TypedTrees, name: &str) -> &'program DataDefinition {
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == name)
        .unwrap_or_else(|| panic!("missing data `{name}`"))
}

fn parameter_type_reference(
    program: &TypedTrees,
    machine_name: &str,
    parameter_index: usize,
) -> typed_trees::types::TypeReferenceHandle {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .unwrap_or_else(|| panic!("missing machine `{machine_name}`"));
    let state = program
        .machine_states(machine)
        .first()
        .expect("entry state");
    program.state_parameters(state)[parameter_index].type_reference
}

#[test]
fn erased_record_field_never_produces_runtime_cleanup() {
    let program = typed(
        "data Evidence { tag: i32; }
         machine Evidence::drop(&mut self) {}
         data Carrier { token: i32; proof [erased]: Evidence; }
         machine enter(c: Carrier) {}",
    );
    assert!(validation::data_requires_nominal_drop(
        &program,
        data(&program, "Evidence")
    ));
    assert!(!validation::data_requires_nominal_drop(
        &program,
        data(&program, "Carrier")
    ));
    assert!(!validation::type_graph_requires_nominal_drop(
        &program,
        parameter_type_reference(&program, "enter", 0)
    ));
}

#[test]
fn relevant_record_field_keeps_nominal_cleanup_requirement() {
    let program = typed(
        "data Evidence { tag: i32; }
         machine Evidence::drop(&mut self) {}
         data Carrier { token: i32; proof: Evidence; }
         machine enter(c: Carrier) {}",
    );
    assert!(validation::data_requires_nominal_drop(
        &program,
        data(&program, "Carrier")
    ));
    assert!(validation::type_graph_requires_nominal_drop(
        &program,
        parameter_type_reference(&program, "enter", 0)
    ));
}

#[test]
fn erased_case_payload_field_never_produces_runtime_cleanup() {
    let program = typed(
        "data Evidence { tag: i32; }
         machine Evidence::drop(&mut self) {}
         data Carrier {
             case Full(token: i32, proof [erased]: Evidence);
             case Empty;
         }
         machine enter(c: Carrier) {}",
    );
    assert!(!validation::data_requires_nominal_drop(
        &program,
        data(&program, "Carrier")
    ));
    assert!(!validation::type_graph_requires_nominal_drop(
        &program,
        parameter_type_reference(&program, "enter", 0)
    ));
}

#[test]
fn attached_drop_machine_keeps_owner_nominal_over_erased_members() {
    let program = typed(
        "data Evidence { tag: i32; }
         machine Evidence::drop(&mut self) {}
         data Carrier { token: i32; proof [erased]: Evidence; }
         machine Carrier::drop(&mut self) {}",
    );
    assert!(validation::data_requires_nominal_drop(
        &program,
        data(&program, "Carrier")
    ));
}

#[test]
fn generic_member_substitution_sees_through_erased_positions() {
    let program = typed(
        "data Evidence { tag: i32; }
         machine Evidence::drop(&mut self) {}
         data Box<T> { value: T; }
         machine enter_erased(b: Box<Evidence>) {}",
    );
    assert!(validation::type_graph_requires_nominal_drop(
        &program,
        parameter_type_reference(&program, "enter_erased", 0)
    ));
    let program = typed(
        "data Evidence { tag: i32; }
         machine Evidence::drop(&mut self) {}
         data Box<T> { value: T; }
         machine enter_i32(b: Box<i32>) {}",
    );
    assert!(!validation::type_graph_requires_nominal_drop(
        &program,
        parameter_type_reference(&program, "enter_i32", 0)
    ));
}
