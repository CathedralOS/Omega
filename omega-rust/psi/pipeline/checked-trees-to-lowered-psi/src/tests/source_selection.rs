//! Source closure ownership travels with the selected lowering result.

use super::{
    checked_source, lower_machine, lower_selected_machine, scalar_call_closure,
    select_terminal_machine,
};
#[test]
fn scalar_selection_returns_the_source_closure_used_to_emit_its_catalog() {
    let checked = checked_source(
        "machine leaf(value: u32) -> u32 { value }
         machine enter(value: u32) -> u32 { leaf(value) }",
    );
    let selection = select_terminal_machine(&checked, "enter").unwrap();
    let lowered = lower_selected_machine(&checked, selection).unwrap();
    let expected =
        scalar_call_closure::checked_scalar_call_closure(&checked, selection.machine).unwrap();
    assert_eq!(lowered.source_machines, expected);
    assert_eq!(expected.len(), 2);
    assert_eq!(lowered.terminal.semantic_module.machines.len(), 2);
    assert!(lowered.completion.finalize_operands);
    assert!(!lowered.completion.omit_debug);
    lower_machine(&checked, "enter").expect("complete lowering preserves the selected closure");
}

#[test]
fn catalog_selection_preserves_exact_source_owners_in_emitted_order() {
    let checked = checked_source(
        "boundary trait Host { machine touch(); }
         machine leaf() reaches Host { Host::touch(); }
         machine enter() reaches Host { leaf(); }",
    );
    let selection = select_terminal_machine(&checked, "enter").unwrap();
    let lowered = lower_selected_machine(&checked, selection).unwrap();
    let owners = lowered
        .exact_sources
        .as_ref()
        .expect("Unit closure retains exact owners");
    assert_eq!(owners.len(), 2);
    assert_eq!(
        lowered.source_machines,
        owners.iter().map(|(source, _)| *source).collect::<Vec<_>>(),
    );
    assert_eq!(
        lowered
            .terminal
            .semantic_module
            .machines
            .iter()
            .map(|machine| machine.id)
            .collect::<Vec<_>>(),
        owners
            .iter()
            .map(|(_, machine)| *machine)
            .collect::<Vec<_>>(),
    );
    assert!(owners.contains(&(selection.machine, lowered.terminal.semantic_module.entry)));
    lower_machine(&checked, "enter").expect("complete lowering retains exact source custody");
}
