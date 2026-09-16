use super::CheckedUnitEffectOperationPlan;
use crate::execution::terminal_unit::ShapeCollector;
use crate::execution::terminal_unit::structural_scalar_store::build_structural_scalar_field_store_sequence;

fn checked(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    crate::lower_typed_trees(typed)
}

#[test]
fn borrowed_fixed_array_element_store_retains_the_exact_index_hop() {
    for (access, expected_access) in [
        (
            "&mut",
            checked_trees::CheckedStructuralAccess::MutableBorrow,
        ),
        (
            "&write",
            checked_trees::CheckedStructuralAccess::WriteOnlyBorrow,
        ),
    ] {
        let checked = checked(&format!(
            "data Record [copy] {{ value: u16; }}
             machine store(records: {access} [Record; 2], value: u16) {{
                 records[1].value = value;
             }}"
        ))
        .unwrap_or_else(|diagnostics| panic!("{access} element store rejected: {diagnostics:?}"));
        let program = &checked.typed;
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "store")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let mut shapes = ShapeCollector::new(program);
        let (structural, scalar) =
            super::super::super::free_structural_scalar_signature(program, &mut shapes, state, &[])
                .expect("borrowed array signature");
        assert_eq!(structural[0].access, expected_access);
        let stores = build_structural_scalar_field_store_sequence(
            program,
            &checked.facts,
            machine,
            state,
            &structural,
            &scalar,
            0,
            None,
        )
        .expect("literal indexed element field store");
        let [CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store)] = stores.as_slice()
        else {
            panic!("one field store");
        };
        assert_eq!(
            store.destination,
            checked_trees::CheckedStructuralScalarFieldStoreDestination::Parameter { position: 0 }
        );
        assert_eq!(
            store.carrier_path,
            [checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                1
            )]
        );
        assert_eq!(store.field_identity, "value");
        assert!(matches!(
            store.value,
            checked_trees::CheckedStructuralScalarFieldStoreValue::Pure(_)
        ));
        // Neither widening nor another literal index may replay this store.
        let mut shared = structural.clone();
        shared[0].access = checked_trees::CheckedStructuralAccess::SharedBorrow;
        assert!(
            build_structural_scalar_field_store_sequence(
                program,
                &checked.facts,
                machine,
                state,
                &shared,
                &scalar,
                0,
                None,
            )
            .is_none(),
            "{access} store cannot replay through a shared borrow"
        );
    }
}

#[test]
fn borrowed_fixed_array_element_store_needs_a_literal_index() {
    // A mutable dynamic index still checks, but must not fabricate a
    // content-independent unit store plan.
    let checked = checked(
        "data Record [copy] { value: u16; }
         machine store(records: &mut [Record; 2], index: u64 [0..=1], value: u16) {
             records[index].value = value;
         }",
    )
    .unwrap();
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "store")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let mut shapes = ShapeCollector::new(program);
    let (structural, scalar) =
        super::super::super::free_structural_scalar_signature(program, &mut shapes, state, &[])
            .expect("borrowed array signature");
    assert!(
        build_structural_scalar_field_store_sequence(
            program,
            &checked.facts,
            machine,
            state,
            &structural,
            &scalar,
            0,
            None,
        )
        .is_none(),
        "a dynamic index is not a source-independent carrier hop"
    );
}

#[test]
fn write_only_array_element_store_rejects_dynamic_out_of_bounds_and_reads() {
    for (name, source) in [
        (
            "dynamic index",
            "data Record [copy] { value: u16; }
             machine store(records: &write [Record; 2], index: u64 [0..=1], value: u16) {
                 records[index].value = value;
             }",
        ),
        (
            "out-of-bounds literal",
            "data Record [copy] { value: u16; }
             machine store(records: &write [Record; 2], value: u16) {
                 records[2].value = value;
             }",
        ),
        (
            "read",
            "data Record [copy] { value: u16; }
             machine read(records: &write [Record; 2]) -> u16 {
                 records[1].value
             }",
        ),
    ] {
        assert!(
            checked(source).is_err(),
            "write-only {name} must stay rejected"
        );
    }
}
