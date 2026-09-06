use super::*;
use checked_trees::{CheckedCallScalarArgument, CheckedScalarExpressionRole};

#[test]
fn mixed_argument_roots_rejoin_dense_scalar_positions_and_exact_occurrences() {
    let checked = checked(
        "data Value { number: u64; }
        machine forward(value: Value) -> Value { value }
        machine numeric(count: u32) -> u32 { count ^ 1u32 }
        machine Main::consume(before: u32, value: Value, after: u32) {}
        machine Main::caller(count: u32, value: Value) {
            Main::consume(numeric(count), forward(value), numeric(count ^ 2u32));
        }",
    );
    lower_machine(&checked, "Main::caller").expect("unmodified mixed call lowers");
    let caller = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::caller")
        .unwrap()
        .symbol;
    let roots = checked
        .facts
        .values
        .scalar_computations
        .roots
        .iter()
        .filter(|(_, root)| root.machine == caller)
        .map(|(handle, root)| (handle, root.clone()))
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 2);
    for mutation in 0..7 {
        let mut changed = checked.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == caller)
            .unwrap();
        let arguments = plan
            .operations
            .iter_mut()
            .find_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    scalar_arguments, ..
                } => Some(scalar_arguments),
                _ => None,
            })
            .unwrap();
        assert_eq!(arguments.len(), 2);
        match mutation {
            0 => arguments.swap(0, 1),
            1 => arguments[1] = arguments[0].clone(),
            2 => {
                arguments.pop();
            }
            3 => arguments[1] = CheckedCallScalarArgument::Computation(arena::Handle::invalid()),
            4 => {
                changed
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .get_mut(roots[1].0)
                    .role = CheckedScalarExpressionRole::UnitCallArgument {
                    call_ordinal: 0,
                    argument_ordinal: 2,
                };
            }
            5 => {
                changed
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .get_mut(roots[1].0)
                    .role = CheckedScalarExpressionRole::BoundaryCallArgument {
                    call_ordinal: 0,
                    argument_ordinal: 1,
                };
            }
            6 => {
                let first = changed
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get(roots[0].1.root)
                    .authored_root;
                changed
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get_mut(roots[1].1.root)
                    .authored_root = first;
            }
            _ => unreachable!(),
        }
        assert!(
            lower_machine(&changed, "Main::caller").is_err(),
            "mixed argument mutation {mutation}"
        );
    }
}
