use super::{TypedTrees, exact_attached_field, machine_attached_data};
use symbols::SymbolHandle;

fn zeroed_parameter(source: &str) -> bool {
    let program = crate::front_end::typed_program(source);
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    super::zeroed_structural_parts(&program, program.state_parameters(state)[0].type_reference)
}

#[test]
fn zeroed_mixed_array_retains_common_fields_and_first_case() {
    assert!(zeroed_parameter(
        "data Coordinates [copy] { start: u64; end: u64; }
         data Token [copy] {
             coordinates: Coordinates;
             case Empty;
             case Present(value: u64);
         }
         data Buffer { tokens: [Token; 2]; length: u64; }
         machine inspect(buffer: Buffer) {}"
    ));
}

#[test]
fn zeroed_mixed_shape_does_not_establish_later_case_payloads() {
    assert!(zeroed_parameter(
        "data Payload { value: u64; }
         data Token { position: u64; case Empty; case Present(payload: Payload); }
         machine inspect(token: Token) {}"
    ));
}

#[test]
fn zeroed_mixed_shape_retains_first_case_payload_restrictions() {
    assert!(!zeroed_parameter(
        "data Payload { value: u64; }
         data Token { position: u64; case Present(payload: Payload); case Empty; }
         machine inspect(token: Token) {}"
    ));
}

fn fixture() -> TypedTrees {
    let source = "data First<T [copy]> { value: T; }
        machine First::read<T [copy]>(&self) -> T { self.value }
        data Second<T [copy]> { other: T; }
        machine Second::read<T [copy]>(&self) -> T { self.other }
        machine use_both(first: &First<u64>, second: &Second<u64>) {}";
    crate::front_end::typed_program_with_generic_data(source)
}

#[test]
fn owner_selection_rejects_missing_stale_and_duplicate_declarations() {
    let program = fixture();
    let owner = program
        .data_definitions()
        .iter()
        .find(|data| data.generic_instance.is_some())
        .unwrap()
        .symbol;
    let original = program
        .machines()
        .iter()
        .find(|machine| machine.attached_data_symbol == owner)
        .unwrap();
    for symbol in [
        SymbolHandle::invalid(),
        original.symbol,
        SymbolHandle::from_parts(owner.arena_index(), owner.generation() + 1),
    ] {
        let mut machine = original.clone();
        machine.attached_data_symbol = symbol;
        assert!(machine_attached_data(&program, &machine).is_none());
    }
    let field = program
        .data_members(machine_attached_data(&program, original).unwrap())
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) => Some(field),
            _ => None,
        })
        .unwrap();
    let mut foreign = original.clone();
    foreign.attached_data_symbol = program
        .data_definitions()
        .iter()
        .find(|data| data.generic_instance.is_some() && data.symbol != owner)
        .unwrap()
        .symbol;
    assert!(exact_attached_field(&program, &foreign, field.symbol, field.name.as_str()).is_none());
    let mut duplicate = program.clone();
    duplicate.push_data_definition(machine_attached_data(&program, original).unwrap().clone());
    assert!(machine_attached_data(&duplicate, original).is_none());
}
