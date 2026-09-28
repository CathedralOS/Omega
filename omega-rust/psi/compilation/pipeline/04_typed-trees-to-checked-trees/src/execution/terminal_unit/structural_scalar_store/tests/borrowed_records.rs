use super::CheckedUnitEffectOperationPlan;
use crate::execution::terminal_unit::types::ShapeCollector;

use crate::execution::terminal_unit::structural_scalar_store::build_structural_scalar_field_store_sequence;
use crate::tests::front_end::checked_program_result;

#[test]
fn borrowed_receiver_scalar_calls_do_not_grant_shared_storage_mutation() {
    assert!(checked("data Counter { value: i32; } machine Counter::write(&mut self, value: i32) { self.value = value; } machine invalid(counter: &Counter) { counter.write(66); }").is_err());
}

fn checked(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    checked_program_result(source)
}

#[test]
fn borrowed_record_store_preserves_parameter_positions_with_an_ambient_receiver() {
    let checked = checked(
        r#"
        data Counter { value: i32; }
        data Main { marker: i32; }
        machine Main::write(&mut self, destination: &mut Counter, value: i32) -> bool {
            destination.value = value;
            transition { _ -> true }
        }
    "#,
    )
    .unwrap();
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::write")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let mut shapes = ShapeCollector::new(program);
    let (_, structural, scalar) = super::super::super::calls::structural_scalar_signature(
        program,
        &mut shapes,
        machine,
        state,
        &[],
        false,
    )
    .unwrap();
    assert_eq!(structural.len(), 1);
    assert_eq!(structural[0].position, 1);
    assert_eq!(scalar[0].source_position, 2);
    let plan = |structural: &[_], scalar: &[_]| {
        build_structural_scalar_field_store_sequence(
            program,
            &checked.facts,
            machine,
            state,
            structural,
            scalar,
            0,
            None,
        )
    };
    let stores = plan(&structural, &scalar).expect(
        "an ambient receiver has no ABI row but the destination retains its authored position",
    );
    let [CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store)] = stores.as_slice()
    else {
        panic!("one field store");
    };
    assert_eq!(
        store.destination,
        checked_trees::CheckedStructuralScalarFieldStoreDestination::Parameter { position: 1 }
    );
    assert!(
        plan(&structural, &[]).is_none(),
        "ordinary value parameters cannot disappear"
    );
    let mut missing_value = scalar.clone();
    missing_value[0].source_position = 0;
    assert!(
        plan(&structural, &missing_value).is_none(),
        "a receiver row cannot replace the scalar argument"
    );
}

#[test]
fn borrowed_record_copy_rejects_shared_write_and_overlapping_arguments() {
    assert!(checked("data Counter { value: i32; } machine invalid(destination: &Counter, source: &Counter) { destination.value = source.value; }").is_err());
    assert!(checked(r#"
        data Counter { value: i32; }
        machine copy_counter(destination: &mut Counter, source: &Counter) { destination.value = source.value; }
        machine invalid(counter: &mut Counter) { copy_counter(&mut counter, &counter); }
    "#).is_err());
    assert!(checked(r#"
        data Counter { value: i32; }
        data Holder { counter: Counter; }
        machine copy_counter(source: &Counter, destination: &mut Counter) { destination.value = source.value; }
        machine Holder::invalid(&mut self) { copy_counter(&self.counter, &mut self.counter); }
    "#).is_err());
}
