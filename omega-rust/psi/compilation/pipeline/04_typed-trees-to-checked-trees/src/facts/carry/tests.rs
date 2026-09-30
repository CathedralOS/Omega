use typed_trees::TypedTrees;
use typed_trees::data::DataMember;

/// Two modules that each declare `Runner` and `Runner::run`. Each `Runner`
/// contains a record of its own module's part type, which has an attached
/// machine, so the carry topology records the field.
fn same_name_runners() -> TypedTrees {
    let module_source = |module: &str, part: &str| {
        format!(
            "module {module};
             pub data {part} {{ value: i32; }}
             pub machine {part}::read(&self) -> i32 {{ self.value }}
             pub data Runner {{ part: {part}; }}
             pub machine Runner::run(&mut self) -> i32 {{ self.part.value }}"
        )
    };
    crate::tests::front_end::typed_modules_with_core_service(&[
        ("left.omg", &module_source("left", "LeftPart")),
        ("right.omg", &module_source("right", "RightPart")),
    ])
}

#[test]
fn contained_machine_fields_come_from_the_machines_own_record() {
    let program = same_name_runners();
    let facts = super::build_carry_facts(&program);
    for (module, part) in [("left", "LeftPart"), ("right", "RightPart")] {
        let machine = program
            .machines()
            .iter()
            .find(|machine| {
                program
                    .symbols
                    .display_path(machine.attached_data_symbol, "::")
                    == format!("{module}::Runner")
            })
            .unwrap_or_else(|| panic!("missing `{module}::Runner::run`"));
        let field = program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == machine.attached_data_symbol)
            .and_then(|definition| {
                program
                    .data_members(definition)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Field(field) => Some(field.symbol),
                        DataMember::Variant(_) => None,
                    })
            })
            .expect("the attached `part` field");
        let (_, topology) = facts
            .machine_topologies
            .iter()
            .find(|(_, topology)| topology.machine == machine.symbol)
            .expect("the machine's carry topology");
        let recorded = facts
            .contained_fields
            .span_or_empty(topology.fields)
            .iter()
            .map(|contained| {
                (
                    contained.field,
                    program.symbols.display_path(contained.data, "::"),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            recorded,
            [(field, format!("{module}::{part}"))],
            "{module}::Runner::run contains its own `part`"
        );
    }
}
