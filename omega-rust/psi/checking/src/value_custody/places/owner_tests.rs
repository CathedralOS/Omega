use super::{TypedTrees, exact_attached_field, machine_attached_data};
use symbols::SymbolHandle;

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
