//! Source closure ownership travels with the selected lowering result.

use super::{checked_source, lower_machine};
use crate::TerminalMachineSelection;
use crate::machine_lowering::machine_dispatch::{lower_selected_machine, select_terminal_machine};
use crate::producer_result::{
    DebugPublication, OperandProofCompletion, SourceMappedLowered, SourceMapping,
};
use crate::scalar_graph::scalar_call_closure;
#[test]
fn scalar_selection_returns_the_source_closure_used_to_emit_its_catalog() {
    let checked = checked_source(
        "machine leaf(value: u32) -> u32 { value }
         machine enter(value: u32) -> u32 { leaf(value) }",
    );
    let selection =
        select_terminal_machine(&checked, TerminalMachineSelection::Name("enter")).unwrap();
    let lowered = lower_selected_machine(&checked, selection).unwrap();
    let expected =
        scalar_call_closure::checked_scalar_call_closure(&checked, selection.machine).unwrap();
    assert_eq!(lowered.source_machines, expected);
    assert_eq!(expected.len(), 2);
    assert_eq!(lowered.terminal.semantic_module.machines.len(), 2);
    assert_eq!(
        lowered.completion.operands,
        OperandProofCompletion::Finalize
    );
    assert!(matches!(
        lowered.source_mapping,
        SourceMapping::ScalarClosureOrder
    ));
    assert!(lowered.source_mapping.exact_owners().is_none());
    assert_eq!(lowered.completion.debug, DebugPublication::FromCheckedPlan);
    lower_machine(&checked, TerminalMachineSelection::Name("enter"))
        .expect("complete lowering preserves the selected closure");
}

#[test]
fn catalog_selection_preserves_exact_source_owners_in_emitted_order() {
    let checked = checked_source(
        "boundary trait Host { machine touch(); }
         machine leaf() reaches Host { Host::touch(); }
         machine enter() reaches Host { leaf(); }",
    );
    let selection =
        select_terminal_machine(&checked, TerminalMachineSelection::Name("enter")).unwrap();
    let lowered = lower_selected_machine(&checked, selection).unwrap();
    let owners = lowered
        .source_mapping
        .exact_owners()
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
    lower_machine(&checked, TerminalMachineSelection::Name("enter"))
        .expect("complete lowering retains exact source custody");
}

#[test]
fn source_projection_modes_preserve_their_distinct_custody_authority() {
    let checked = checked_source(
        "machine leaf(value: u32) -> u32 { value }
         machine enter(value: u32) -> u32 { leaf(value) }",
    );
    let selection =
        select_terminal_machine(&checked, TerminalMachineSelection::Name("enter")).unwrap();
    let lowered = lower_selected_machine(&checked, selection).unwrap();
    let sources = &lowered.source_machines;
    let expected = sources
        .iter()
        .copied()
        .zip(
            lowered
                .terminal
                .semantic_module
                .machines
                .iter()
                .map(|machine| machine.id),
        )
        .collect::<Vec<_>>();
    assert!(SourceMapping::ScalarClosureOrder.exact_owners().is_none());
    assert_eq!(
        SourceMapping::ScalarClosureOrder
            .projection_sources(&lowered.terminal, selection.machine, sources)
            .unwrap(),
        expected,
    );
    assert!(
        SourceMapping::ScalarClosureOrder
            .projection_sources(&lowered.terminal, selection.machine, &sources[..1])
            .is_err()
    );
    assert!(SourceMapping::EntryOnly.exact_owners().is_none());
    assert_eq!(
        SourceMapping::EntryOnly
            .projection_sources(&lowered.terminal, selection.machine, sources)
            .unwrap(),
        vec![(selection.machine, lowered.terminal.semantic_module.entry)],
    );
    let exact = SourceMapping::ExactCatalog(expected.clone());
    assert_eq!(exact.exact_owners(), Some(expected.as_slice()));
    assert_eq!(
        exact
            .projection_sources(&lowered.terminal, selection.machine, sources)
            .unwrap(),
        expected,
    );
}

#[test]
fn exact_source_catalog_reorders_owners_and_rejects_incomplete_or_duplicate_bindings() {
    let checked = checked_source(
        "boundary trait Host { machine touch(); }
         machine leaf() reaches Host { Host::touch(); }
         machine enter() reaches Host { leaf(); }",
    );
    let selection =
        select_terminal_machine(&checked, TerminalMachineSelection::Name("enter")).unwrap();
    let lowered = lower_selected_machine(&checked, selection).unwrap();
    let owners = lowered.source_mapping.exact_owners().unwrap();
    let mut reversed = owners.to_vec();
    reversed.reverse();
    assert_eq!(
        SourceMappedLowered::new(lowered.terminal.clone(), reversed)
            .unwrap()
            .source_machine_ids,
        owners,
    );
    let mut duplicate_source = owners.to_vec();
    duplicate_source[1].0 = duplicate_source[0].0;
    let mut duplicate_target = owners.to_vec();
    duplicate_target[1].1 = duplicate_target[0].1;
    let mut foreign_target = owners.to_vec();
    foreign_target[1].1 = semantic_vocabulary::MachineId::new(9000).unwrap();
    for invalid in [
        owners[..1].to_vec(),
        duplicate_source,
        duplicate_target,
        foreign_target,
    ] {
        assert!(SourceMappedLowered::new(lowered.terminal.clone(), invalid).is_err());
    }
}
