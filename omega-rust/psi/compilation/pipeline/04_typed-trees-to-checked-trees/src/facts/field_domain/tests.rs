use super::data_definition_for_field_type;
use crate::tests::front_end::typed_program_with_generic_data_result;

#[test]
fn closed_const_record_store_uses_the_substituted_owner() {
    let typed = typed_program_with_generic_data_result(
        r#"
        data Buffer<const Capacity: u64> where Capacity >= 1 {
            items: [u32 in Wrapping; Capacity];
        }
        machine replace<const Length: u64>(buffer: &mut Buffer<Length>, value: u32 in Wrapping)
        requires Length >= 1;
        { buffer.items[0] = value; }
        machine enter(left: &mut Buffer<2>, right: &mut Buffer<4>) {
            replace(left, 30);
            replace(right, 40);
        }
        "#,
    )
    .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    let checked = crate::lower_typed_trees(typed, &crate::CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    let mut owners = Vec::new();
    for machine in checked
        .machines()
        .iter()
        .filter(|machine| machine.name.as_str().starts_with("replace$specialized$"))
    {
        let state = &checked.machine_states(machine)[0];
        let parameter = &checked.state_parameters(state)[0];
        let owner = data_definition_for_field_type(&checked.typed, parameter.type_reference)
            .expect("closed application retains its exact substituted owner");
        assert!(owner.generic_instance.is_some());
        assert!(checked.data_type_parameters(owner).is_empty());
        owners.push(owner.symbol);
        let typed_trees::data::DataMember::Field(field) = &checked.data_members(owner)[0] else {
            panic!("buffer owns its array field");
        };
        let typed_trees::statement::StatementNode::Assignment(assignment) =
            &checked.statement_table.statements(state.statement_nodes)[0]
        else {
            panic!("replacement writes the array element");
        };
        let place = crate::flow::canonical_place_from_expression_in_state(
            &checked.typed,
            state.symbol,
            0,
            assignment.target,
        )
        .expect("store has an exact canonical destination");
        assert_eq!(
            place.segments,
            [
                facts::PlaceSegment::Field {
                    symbol: field.symbol
                },
                facts::PlaceSegment::FixedIndex { index: 0 },
            ]
        );
        assert!(
            checked
                .facts
                .flow
                .terminal_unit_effects
                .for_machine(machine.symbol)
                .is_some(),
            "{} retains its indexed store: {:?}",
            machine.name.as_str(),
            checked.facts.flow.terminal_unit_effects.omission_for_machine(machine.symbol)
        );
    }
    assert_eq!(owners.len(), 2);
    assert_ne!(owners[0], owners[1]);
}
