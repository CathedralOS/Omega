//! Completed element owners compose under the ordinary array constructor.
use checked_trees::CheckedStructuralValueKind;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_interpreter::{TerminalExecutionResult, TerminalScalarValue};
use terminal_psi::OperationKind;

const SOURCE: &str = r#"
    data Point { value: u64; }
    data Envelope { elements: [Point; 2]; }
    machine make_point(value: u64) -> Point { Point { value: value } }
    machine choose() -> bool {
        let first: Point = make_point(30);
        let envelope: Envelope = Envelope { elements: [first, make_point(40)] };
        envelope.elements[0].value == 30 && envelope.elements[1].value == 40
    }
"#;

#[test]
fn completed_array_elements_preserve_owned_and_call_values() {
    for source in [
        SOURCE.to_owned(),
        format!(
            "domain u64::Positive(value) requires 1 <= value;\n{}",
            SOURCE.replace("value: u64", "value: u64 in Positive")
        ),
    ] {
        let (module, execution) = super::execute(&source, &[]);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
        );
        let arrays = module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match &operation.kind {
                OperationKind::EstablishStructuralArray { elements } => Some(elements),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(arrays.len(), 1);
        assert_eq!(arrays[0].len(), 2);
        assert_ne!(arrays[0][0].place, arrays[0][1].place);
    }
}

#[test]
fn completed_array_elements_reject_duplicate_owned_operand() {
    let source = SOURCE.replace("[first, make_point(40)]", "[first, first]");
    crate::front_end::checked_program_result(&source)
        .expect_err("one affine element cannot transfer into two array slots");
}

#[test]
fn completed_array_elements_replay_the_exact_authored_roster() {
    let checked = crate::front_end::checked_program(SOURCE);
    checked_trees_to_lowered_psi::lower_machine(&checked, TerminalMachineSelection::Name("choose"))
        .expect("the original completed array lowers");
    let (array, _) = checked
        .facts
        .values
        .structural_values
        .nodes
        .iter()
        .find(|(_, node)| matches!(node.kind, CheckedStructuralValueKind::FixedArray { .. }))
        .expect("the array has a retained constructor");
    for reordered in [false, true] {
        let mut changed = checked.clone();
        let CheckedStructuralValueKind::FixedArray { elements } = &mut changed
            .facts
            .values
            .structural_values
            .nodes
            .get_mut(array)
            .kind
        else {
            unreachable!()
        };
        if reordered {
            elements.swap(0, 1);
        } else {
            elements.pop();
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("choose"),
            )
            .is_err(),
            "changed constructor roster must not acquire source custody"
        );
    }
}
