use super::{ExpressionNode, SymbolHandle};
use crate::monomorphization::body_rewriting::{
    cloned_runtime_call_subjects, runtime_value_subjects,
};
use crate::monomorphization::collect_statement_expression_trees;
use crate::tests::front_end::typed_program;

#[test]
fn structural_endpoint_arguments_follow_their_data_field_scope() {
    for declaration in [
        "data Buffer { length: u64[0..=endpoint<[u8; 7]>()]; }",
        "data Buffer { values: [u64[0..=endpoint<[u8; 7]>()]; 2]; }",
        "data Buffer { case Ready(length: u64[0..=endpoint<[u8; 7]>()]); }",
        "data Buffer<'item> { length: u64[0..=endpoint<[Borrowed<'item>; 1]>()]; }",
    ] {
        let mut program = typed_program(&format!(
            "data Borrowed<'item> {{ value: &'item u8; }}
             machine endpoint<T>() -> u64 {{ 7 }} {declaration}"
        ));
        crate::specialize_static_machine_calls(&mut program)
            .unwrap_or_else(|errors| panic!("{declaration}: {errors:?}"));
        assert!(!program.machine_specializations.is_empty(), "{declaration}");
    }
}

#[test]
fn structural_field_endpoint_cannot_borrow_a_consumers_lifetime_scope() {
    for field in [
        "length: u64[0..=endpoint<[Borrowed<'hidden>; 1]>()];",
        "case Ready(length: u64[0..=endpoint<[Borrowed<'hidden>; 1]>()]);",
    ] {
        let mut program = typed_program(&format!(
            "data Borrowed<'item> {{ value: &'item u8; }}
             machine endpoint<T>() -> u64 {{ 7 }}
             data Buffer {{ {field} }}
             machine consumer<'hidden>(buffer: Buffer) {{}}"
        ));
        let errors = crate::specialize_static_machine_calls(&mut program)
            .expect_err("a field type cannot borrow the nominal consumer's lifetime");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("undeclared lifetime")),
            "{errors:?}"
        );
    }
}

#[test]
fn structural_endpoint_cannot_borrow_another_machine_lifetime_scope() {
    let mut program = typed_program(
        "data Borrowed<'item> { value: &'item u8; }
         machine endpoint<T>() -> u64 { 7 }
         machine unrelated<'hidden>() {}
         machine keep(value: u64[0..=endpoint<[Borrowed<'hidden>; 1]>()]) {}",
    );
    let errors = crate::specialize_static_machine_calls(&mut program)
        .expect_err("an unrelated owner cannot authorize a type argument's lifetime");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("undeclared lifetime")),
        "{errors:?}"
    );
}

#[test]
fn detached_structural_endpoint_cannot_borrow_an_arena_neighbour_scope() {
    let mut program = typed_program(
        "machine endpoint<T>() -> u64 { 7 }
         machine keep(value: u64[0..=endpoint<[u8; 7]>()]) {}",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "keep")
        .unwrap();
    let state = program.machine_states(machine)[0].clone();
    let reference = program.state_parameters(&state)[0].type_reference;
    let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
        *program.type_reference_table.type_reference(reference)
    else {
        panic!("authored range")
    };
    program.state_parameters.span_mut_or_empty(state.parameters)[0].type_reference = base_type;
    let errors = crate::specialize_static_machine_calls(&mut program)
        .expect_err("an arena entry without its declaration remains unowned");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("no retained caller")),
        "{errors:?}"
    );
}

#[test]
fn unused_named_type_argument_cannot_erase_unsupplied_data_binders() {
    for declaration in [
        "data Marker<T> { value: T; }",
        "data Marker<const N: u64> { value: [u8; N]; }",
        "data Marker<'a> { value: &'a u8; }",
    ] {
        let mut program = typed_program(&format!(
            "{declaration}
             machine bound<T, const N: u64>() -> u64 {{ N }}
             machine main() -> u64 {{ bound<Marker, 7>() }}"
        ));
        let errors = crate::specialize_static_machine_calls(&mut program)
            .expect_err("every static type argument must be formed, including an unused binder");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("complete type/lifetime arguments")),
            "{errors:?}"
        );
    }
}

#[test]
fn explicit_static_argument_overflow_rejects_before_specialization() {
    for source in [
        "machine identity<T [copy]>(value: T) -> T { value }
         machine caller() -> u64 { identity<u64,u8>(7) }",
        "machine amount<const N: u64>() -> u64 { N }
         machine caller() -> u64 { amount<7,8>() }",
        "machine choose<T [copy],const N: u64>(value: T, bytes: [u8; N]) -> T { value }
         machine caller(value: u64, bytes: [u8; 2]) -> u64 {
             choose<u64,u8>(value, bytes)
         }",
        "machine identity<T [copy]>(value: T) -> T { value }
         machine caller() -> u64 { identity<u64,7>(7); 0 }",
    ] {
        let mut program = typed_program(source);
        let diagnostics = crate::specialize_static_machine_calls(&mut program)
            .expect_err("explicit surplus cannot disappear during speculative preparation");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("excess explicit static arguments")
                    || diagnostic
                        .message
                        .contains("no const parameter for extra argument")
                    || diagnostic.message.contains("declared binder kind")
            }),
            "{source}: {diagnostics:?}"
        );
        assert!(program.machine_specializations.is_empty());
    }
}

#[test]
fn explicit_static_const_argument_retains_its_literal_carrier() {
    let mut wrong_carrier = typed_program(
        "machine amount<const N: u64>() -> u64 { N }
         machine read() -> u64 { amount<7u8>() }",
    );
    assert!(crate::specialize_static_machine_calls(&mut wrong_carrier).is_err());
    for literal in ["7u64", "7"] {
        let mut program = typed_program(&format!(
            "machine amount<const N: u64>() -> u64 {{ N }}
             machine read() -> u64 {{ amount<{literal}>() }}"
        ));
        crate::specialize_static_machine_calls(&mut program)
            .unwrap_or_else(|diagnostics| panic!("{literal}: {diagnostics:?}"));
        assert_eq!(program.machine_specializations.len(), 1);
    }
}

#[test]
fn runtime_call_subject_collection_matches_lexical_owner_lookup() {
    let program = typed_program(
        "machine recurse<Count: u32>(value: u32) -> u32 {
             let next: u32 = value;
             recurse<next>(recurse<value>(value))
         }",
    );
    let machine = &program.machines()[0];
    let states = program.machine_states(machine);
    let state_symbols = states
        .iter()
        .map(|state| (state.symbol, state.symbol))
        .collect::<Vec<_>>();
    let calls = cloned_runtime_call_subjects(&program, machine.states, &state_symbols, 0);
    assert!(calls.len() >= 2);
    for (expression, actual) in &calls {
        let ExpressionNode::Call(call) = program.expression_table.expression(*expression) else {
            panic!("retained call");
        };
        let owner = states
            .iter()
            .find(|state| {
                program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .any(|statement| {
                        let mut expressions = Vec::new();
                        collect_statement_expression_trees(&program, statement, &mut expressions);
                        expressions.contains(expression)
                    })
            })
            .expect("lexical owner");
        assert_eq!(
            *actual,
            runtime_value_subjects(&program, owner, &call.machine_arguments)
        );
    }
    let after_region = program.expression_table.iter_expressions().count();
    assert!(
        cloned_runtime_call_subjects(&program, machine.states, &state_symbols, after_region)
            .is_empty()
    );
}

#[test]
fn structural_static_argument_commitments_use_normalized_type_identity() {
    let program = typed_program(
        "machine exclusive(value: u64[0..257]) -> u64 { 0 }
         machine inclusive(value: u64[0..=256]) -> u64 { 0 }
         machine smaller(value: u64[0..256]) -> u64 { 0 }
         machine seven(value: [u8; 7]) -> u64 { 0 }
         machine eight(value: [u8; 8]) -> u64 { 0 }
         machine generic_first<T>(value: [T; 7]) -> u64 { 0 }
         machine generic_renamed<U>(value: [U; 7]) -> u64 { 0 }",
    );
    let commitment = |name: &str| {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("fixture machine");
        let state = &program.machine_states(machine)[0];
        let argument = super::StaticMachineArgument {
            type_reference: program.state_parameters(state)[0].type_reference,
            path: Box::default(),
            application: None,
            const_literal: None,
            evidence_projection: None,
            symbol: SymbolHandle::invalid(),
        };
        let binders = program
            .machine_type_parameters(machine)
            .iter()
            .enumerate()
            .map(|(index, parameter)| (parameter.symbol, format!("$T{index}")))
            .collect::<Vec<_>>();
        let mut bytes = Vec::new();
        super::identities::encode_bound_static_argument(
            &program,
            &argument,
            &[],
            &binders,
            &mut bytes,
        );
        bytes
    };
    assert_eq!(commitment("exclusive"), commitment("inclusive"));
    assert_ne!(commitment("exclusive"), commitment("smaller"));
    assert_ne!(commitment("seven"), commitment("eight"));
    assert_ne!(commitment("exclusive"), commitment("seven"));
    assert_eq!(commitment("generic_first"), commitment("generic_renamed"));
}
