use super::{
    CheckedUnitEffectOperationPlan, ShapeCollector, build_structural_scalar_field_store_sequence,
    checked_program, machine_binders,
};
use crate::execution::terminal_unit::calls::structural_scalar_signature;

fn stores(source: &str, state_index: usize) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    let checked = checked_program(source);
    let program = &checked.typed;
    let machine = program.machines().iter().next().unwrap();
    let state = &program.machine_states(machine)[state_index];
    let mut shapes = ShapeCollector::new(program);
    let (_, structural, scalar) = structural_scalar_signature(
        program,
        &mut shapes,
        machine,
        state,
        &machine_binders(program, machine),
        true,
    )
    .expect("attached state signature");
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
}

/// A member read is not a whole owned place: `self.format = source.format`
/// would need a path below the source root, which this store's value lane
/// does not carry, so it keeps declining.
#[test]
fn member_read_value_still_declines() {
    let source = r#"
        data Format [copy] { case Plain; case Hex; }
        data T { format: Format; }
        machine T::copy_format(&mut self, source: T) {
            self.format = source.format;
        }
    "#;
    assert!(stores(source, 0).is_none());
}
