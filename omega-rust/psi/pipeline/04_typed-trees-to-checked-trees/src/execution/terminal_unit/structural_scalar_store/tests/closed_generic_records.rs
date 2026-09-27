use super::super::plain_record;
use crate::execution::terminal_unit::types::ShapeCollector;

use crate::tests::front_end::typed_program_from_source_map_with_generic_data;

fn fixture() -> checked_trees::CheckedTrees {
    let source = r#"
        data Counter<T> { value: T; times: i32 in Wrapping; }
        machine Counter::record<T>(&mut self, value: T) {
            self.value = value;
            self.times = self.times + 1;
        }
        data Main { integer: Counter<i32>; boolean: Counter<bool>; }
        machine Main::main(&mut self) {
            self.integer.record(12);
            self.boolean.record(true);
        }
    "#;
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add(
            std::path::PathBuf::from("closed_generic_record_stores.omg"),
            source.to_owned(),
        )
        .source_id;
    let typed = typed_program_from_source_map_with_generic_data(sources, &[(source_id, source)]);
    crate::lower_typed_trees(typed, &crate::CheckingRequest::settled()).unwrap()
}

#[test]
fn generated_store_owner_requires_closed_origin_and_preserves_record_gates() {
    let checked = fixture();
    let program = &checked.typed;
    let owner = program
        .data_definitions()
        .iter()
        .find(|owner| owner.generic_instance.is_some())
        .unwrap();
    assert!(plain_record(owner, program));
    let mut missing_origin = owner.clone();
    missing_origin.generic_instance = Some(typed_trees::types::TypeReferenceHandle::invalid());
    assert!(!plain_record(&missing_origin, program));
    let mut gated = owner.clone();
    gated.zero_gated = true;
    assert!(!plain_record(&gated, program));
    let template = program
        .data_definitions()
        .iter()
        .find(|owner| !program.data_type_parameters(owner).is_empty())
        .unwrap();
    assert!(
        !plain_record(template, program),
        "open owner stays unsupported"
    );
}

#[test]
fn generic_receiver_identity_matches_ordinary_field_uses_without_aliasing_arguments() {
    let checked = fixture();
    let program = &checked.typed;
    let main = program
        .data_definitions()
        .iter()
        .find(|owner| owner.name.as_str() == "Main")
        .unwrap();
    let mut identities = Vec::new();
    for member in program.data_members(main) {
        let typed_trees::data::DataMember::Field(field) = member else {
            continue;
        };
        let field_identity = program
            .normalized_type_identity(field.type_reference)
            .into_string();
        let owner = program
            .data_definitions()
            .iter()
            .find(|owner| {
                owner.generic_instance.is_some_and(|application| {
                    program.normalized_type_identity(application).as_str() == field_identity
                })
            })
            .expect("field has its exact synthesized owner");
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.attached_data_symbol == owner.symbol)
            .unwrap();
        let attachment =
            super::super::super::types::attached_data_identity(program, machine).unwrap();
        assert_eq!(attachment, field_identity);
        assert_eq!(
            ShapeCollector::new(program)
                .add_attached_data(owner, &[])
                .unwrap(),
            field_identity
        );
        identities.push(attachment);
    }
    assert_eq!(identities.len(), 2);
    assert_ne!(
        identities[0], identities[1],
        "different argument tuples cannot share a receiver identity"
    );
}
