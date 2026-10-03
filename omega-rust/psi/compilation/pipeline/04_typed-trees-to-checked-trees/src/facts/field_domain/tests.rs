use super::data_definition_for_field_type;
use crate::tests::front_end::typed_program_with_generic_data_result;

#[test]
fn declared_field_roster_preserves_case_paths_and_extent_roles() {
    use facts::PlaceSegment;

    let program = crate::tests::front_end::typed_program(
        "domain i32::Ten(value) requires 0 <= value && value <= 10;
         domain i32::Marked;
         data Leaf { amount: i32 in Ten & Marked; }
         data Container {
             fixed: [Leaf; 2];
             view: &[Leaf];
             borrowed: &Leaf;
             hidden: &write Leaf;
             case Empty;
             case Present(payload: Leaf);
         }",
    );
    let container = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Container")
        .unwrap();
    let rows = super::declared_data_field_domain_rows(&program, container);
    // Two domains per readable leaf: two fixed elements plus their evidence
    // rollup, one slice extent, one referent, and one conditional payload.
    assert_eq!(rows.len(), 12);
    assert_eq!(rows.iter().filter(|row| row.transfer).count(), 2);
    assert!(rows.iter().all(|row| row.semantic_domain.is_valid()));
    assert_eq!(
        rows.iter()
            .filter(|row| row
                .path
                .iter()
                .any(|segment| matches!(segment, PlaceSegment::Case { .. })))
            .count(),
        2,
    );
    let obligations = super::declared_field_domain_paths(&program, container);
    assert_eq!(obligations.len(), 5);
    assert_eq!(
        obligations
            .iter()
            .filter(|(path, _, _)| path.contains(&super::WHOLE_ELEMENT_EXTENT))
            .count(),
        1,
        "only the slice extent is an obligation; fixed-array rollups are evidence",
    );
    for (path, domain, instance) in obligations {
        assert!(rows.iter().any(|row| !row.transfer
            && row.path == path
            && row.domain_symbol == domain
            && row.semantic_domain == instance));
    }
}

#[test]
fn declared_field_roster_preserves_indexed_domain_identity() {
    let program = crate::tests::front_end::typed_program(
        "domain<const Bound: u64> u64::AtMost<Bound>(value) requires value <= Bound;
         data Limits { case Pair(first: u64 in AtMost<10>, second: u64 in AtMost<20>); }
         machine inspect(items: [Limits; 2]) {}",
    );
    let limits = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Limits")
        .unwrap();
    let rows = super::declared_data_field_domain_rows(&program, limits);
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .all(|row| row.semantic_domain.is_valid() && !row.transfer)
    );
    assert_ne!(rows[0].semantic_domain, rows[1].semantic_domain);
    assert_ne!(rows[0].path, rows[1].path);
    assert_eq!(
        super::declared_field_domain_paths(&program, limits),
        rows.iter()
            .map(|row| (row.path.clone(), row.domain_symbol, row.semantic_domain))
            .collect::<Vec<_>>(),
    );
    let inspect = &program.machines()[0];
    let entry = &program.machine_states(inspect)[0];
    let items = &program.state_parameters(entry)[0];
    let array_rows = super::declared_result_field_domain_paths(&program, items.type_reference);
    assert_eq!(array_rows.len(), 4);
    for (index, element) in array_rows.chunks_exact(2).enumerate() {
        for ((path, domain, instance), row) in element.iter().zip(&rows) {
            assert_eq!(path[0], facts::PlaceSegment::FixedIndex { index });
            assert_eq!(&path[1..], row.path);
            assert_eq!(
                (*domain, *instance),
                (row.domain_symbol, row.semantic_domain)
            );
        }
    }
    let typed_trees::data::DataMember::Variant(pair) = &program.data_members(limits)[0] else {
        panic!("declared pair case");
    };
    for (row, field) in rows.iter().zip(program.data_payload_fields(pair)) {
        assert_eq!(
            super::domain_constraint_identities(&program, field.type_reference),
            [(row.domain_symbol, row.semantic_domain)],
        );
        assert_eq!(
            row.path[0],
            facts::PlaceSegment::Case {
                variant: pair.symbol
            }
        );
    }
}

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
            checked
                .facts
                .flow
                .terminal_unit_effects
                .omission_for_machine(machine.symbol)
        );
    }
    assert_eq!(owners.len(), 2);
    assert_ne!(owners[0], owners[1]);
}
