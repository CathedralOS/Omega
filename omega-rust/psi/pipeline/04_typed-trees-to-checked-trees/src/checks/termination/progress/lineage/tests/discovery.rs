use super::{ProgressSubject, SymbolHandle};
use crate::checks::termination::progress::lineage::ParameterLineage;
use crate::checks::termination::progress::lineage::StateParameterLineage;
use crate::checks::termination::progress::lineage::places;
use crate::checks::termination::progress::lineage::resolve_subject_lineage;
use crate::tests::front_end::checked_program;

fn checked() -> checked_trees::CheckedTrees {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}
        data Scheduler {}
        data Leaves { first: u64; second: u64; }
        data Branch { first: Leaves; second: Leaves; }
        data Tree { first: Branch; second: Branch; }
        data Context { scheduler: Scheduler; unused: Tree; }
        data Node { next: &Node; }
        machine walk(context: &Context, node: &Node, remaining: u64) -> u64
        terminates by remaining -> Nat::Descending;
        {
            transition remaining > 0 {
                true -> step(context, node.next, remaining - 1)
                false -> 0
            }
            state step(context: &Context, node: &Node, remaining: u64) -> u64 {
                transition remaining > 0 {
                    true -> walk(context, node.next, remaining - 1)
                    false -> 0
                }
            }
        }
    "#;
    checked_program(source)
}

fn field(program: &typed_trees::TypedTrees, path: &str) -> SymbolHandle {
    program
        .data_definitions()
        .iter()
        .flat_map(|data| program.data_members(data))
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field)
                if program.symbols.display_path(field.symbol, "::") == path =>
            {
                Some(field.symbol)
            }
            _ => None,
        })
        .expect("fixture field")
}

#[test]
fn only_demanded_field_dependencies_are_discovered() {
    let program = checked();
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "walk")
        .unwrap();
    let states = program.machine_states(machine);
    let scheduler = field(&program, "Context::scheduler");
    let subjects = states
        .iter()
        .map(|state| ProgressSubject {
            root: program.state_parameters(state)[0].symbol,
            projections: vec![scheduler],
        })
        .collect::<Vec<_>>();
    assert_eq!(subjects.len(), 2);
    for demand in &subjects {
        let lineage =
            StateParameterLineage::derive(&program, &program.facts.flow, machine, demand, None);
        // No whole Context, unused Tree descendant, fuel, or recursively
        // projected Node gets a key. Either demand discovers both cycle edges.
        assert_eq!(lineage.values.len(), 2, "{:?}", lineage.values);
        for expected in &subjects {
            assert!(lineage.values.iter().any(|(subject, value)| {
                subject == expected && *value == ParameterLineage::Exact(vec![subjects[0].clone()])
            }));
        }
    }
}

#[test]
fn recursive_referent_demands_share_one_finite_partition() {
    let program = checked();
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "walk")
        .unwrap();
    let root = program.state_parameters(&program.machine_states(machine)[0])[1].symbol;
    let next = field(&program, "Node::next");
    let partition = ProgressSubject {
        root,
        projections: vec![next],
    };
    for length in [1, 2, 8] {
        let demand = ProgressSubject {
            root,
            projections: vec![next; length],
        };
        assert_eq!(
            places::partition(&program, machine, &demand),
            Some(partition.clone())
        );
        let lineage =
            StateParameterLineage::derive(&program, &program.facts.flow, machine, &demand, None);
        assert_eq!(lineage.values.len(), 2);
        assert_eq!(
            resolve_subject_lineage(&lineage.values, demand),
            ParameterLineage::Ambiguous
        );
    }
}
