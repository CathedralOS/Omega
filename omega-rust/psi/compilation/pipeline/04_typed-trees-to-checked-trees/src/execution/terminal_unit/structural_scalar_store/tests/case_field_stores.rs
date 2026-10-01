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
/// reads a child below the source root, and this scalar store lane writes no
/// row for it. A readable owned root supplies that copyable child as a copied
/// structural value, which the statement sequence establishes and stores
/// through its window pair; the value keeps its exact member path below the
/// parameter root rather than standing for the whole root.
#[test]
fn member_read_value_copies_its_exact_member_path() {
    let source = r#"
        data Format [copy] { case Plain; case Hex; }
        data T { format: Format; }
        machine T::copy_format(&mut self, source: T) {
            self.format = source.format;
        }
    "#;
    assert!(stores(source, 0).is_some_and(|stores| stores.is_empty()));
    let checked = checked_program(source);
    let program = &checked.typed;
    let machine = program.machines().iter().next().unwrap();
    let state = &program.machine_states(machine)[0];
    let (statement_index, value) = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .find_map(|(index, statement)| match statement {
            typed_trees::statement::StatementNode::Assignment(assignment) => {
                Some((index, assignment.value))
            }
            _ => None,
        })
        .expect("the member assignment");
    let values = &checked.facts.values.structural_values;
    let root = values
        .root_for_expression(state.symbol, statement_index as u32, value)
        .expect("the member read is a structural value");
    let checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { source, .. } =
        &values.nodes.get(root.root).kind
    else {
        panic!("the member read copies its leaf out of the readable root");
    };
    assert!(matches!(
        source.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
    ));
    assert_eq!(
        source.path,
        [checked_trees::CheckedUnitStructuralPathSegment::Field(
            "format".into()
        )]
    );
}
