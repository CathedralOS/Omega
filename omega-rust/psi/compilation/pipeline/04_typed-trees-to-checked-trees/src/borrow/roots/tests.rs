use typed_trees::TypedTrees;
use typed_trees::data::DataMember;

/// Two modules that each declare `Runner { part: .. }` and `Runner::run`.
fn same_name_runners() -> TypedTrees {
    let module_source = |module: &str| {
        format!(
            "module {module};
             pub data Runner {{ part: i32; }}
             pub machine Runner::run(&mut self) -> i32 {{ self.part }}"
        )
    };
    crate::tests::front_end::typed_modules_with_core_service(&[
        ("left.omg", &module_source("left")),
        ("right.omg", &module_source("right")),
    ])
}

#[test]
fn attached_fields_come_from_the_machines_own_record() {
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
        let declared = program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == machine.attached_data_symbol)
            .map(|definition| {
                program
                    .data_members(definition)
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Field(field) => Some(field.symbol),
                        DataMember::Variant(_) => None,
                    })
                    .collect::<Vec<_>>()
            })
            .expect("the attached record");
        assert_eq!(
            super::attached_data_fields(&program, machine)
                .map(|field| field.symbol)
                .collect::<Vec<_>>(),
            declared,
            "{module}::Runner::run roots its own `part`"
        );
    }
}
