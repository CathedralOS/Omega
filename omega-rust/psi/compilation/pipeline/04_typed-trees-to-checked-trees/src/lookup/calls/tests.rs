use typed_trees::TypedTrees;
use typed_trees::data::DataMember;

/// Two modules that each declare `Runner` and `Runner::run`; each `Runner`
/// holds its own module's part record, whose attached `read` machine the
/// `part` field can dispatch to.
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
fn a_field_receiver_is_read_from_the_machines_own_record() {
    let program = same_name_runners();
    for module in ["left", "right"] {
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
        let state = &program.machine_states(machine)[0];
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
        assert!(
            super::receiver_can_dispatch_to_machine(&program, machine, state, field, Some(&[])),
            "{module}::Runner's `part` reaches its own part's attached machine"
        );
    }
}
