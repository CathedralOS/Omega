use super::CheckedScalarExpressionRole;
use crate::execution::terminal_unit::types::ShapeCollector;

use crate::execution::terminal_unit::control::build_checked_machine;
use crate::tests::front_end::checked_program;

fn fixture() -> checked_trees::CheckedTrees {
    let source = r#"
        machine narrow(value: u32) -> u8 { (value as u8 in Wrapping) as u8 }
        data Record { value: u8 in Wrapping; }
        machine Record::replace(&mut self, value: u32, previous: u8) {
            self.value = previous as u8 in Wrapping;
            self.value = narrow(value) as u8 in Wrapping;
        }
    "#;
    checked_program(source)
}

#[test]
fn field_call_assignment_rejects_missing_stale_and_substituted_root_custody() {
    let checked = fixture();
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Record::replace")
        .unwrap();
    let state = &program.machine_states(machine)[0];
    let (root_handle, root) = checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .find(|(_, root)| root.state == state.symbol && root.statement_ordinal == 1)
        .unwrap();
    for mutation in 0..7 {
        let mut facts = checked.facts.clone();
        let plans = &mut facts.values.scalar_computations;
        match mutation {
            0 => plans.roots.get_mut(root_handle).machine = symbols::SymbolHandle::invalid(),
            1 => plans.roots.get_mut(root_handle).root = arena::Handle::invalid(),
            2 => plans.roots.get_mut(root_handle).statement_ordinal = 0,
            3 => plans.roots.get_mut(root_handle).role = CheckedScalarExpressionRole::Guard,
            4 => plans.nodes.get_mut(root.root).authored_root = arena::Handle::invalid(),
            5 => {
                plans.nodes.get_mut(root.root).primitive_type =
                    typed_trees::types::PrimitiveType::U32
            }
            6 => {
                plans.roots.append(root.clone());
            }
            _ => unreachable!(),
        }
        assert!(
            build_checked_machine(
                program,
                &facts,
                crate::execution::terminal_unit::ScalarCalleePlans {
                    boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                    structural_returns: &facts.flow.terminal_structural_scalar_returns
                },
                &mut ShapeCollector::new(program),
                machine,
                None,
            )
            .is_none(),
            "mutation {mutation}"
        );
    }
}
