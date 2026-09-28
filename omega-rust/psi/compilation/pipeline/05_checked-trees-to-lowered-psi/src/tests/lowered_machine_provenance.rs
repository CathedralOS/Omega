//! Diagnostic attribution follows exact source joins, then the first named caller.

use crate::{LoweredMachineProvenance, TerminalMachineSelection, lower_machine};
use semantic_vocabulary::MachineId;

const CALL_SOURCE: &str = "
    machine identity(value: u64) -> u64 { value }
    machine enter(value: u64) -> u64 { identity(value) }
";

fn checked_source(text: &str) -> checked_trees::CheckedTrees {
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add(std::path::PathBuf::from("main.omg"), text.to_owned())
        .source_id;
    crate::front_end::checked_program_from_source_map(sources, &[(source_id, text)])
}

#[test]
fn entry_and_source_call_retain_exact_names_and_declaration_spans() {
    let checked = checked_source(CALL_SOURCE);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("enter")).unwrap();
    let entry = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap();
    let callee = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "identity")
        .unwrap();
    let provenance = LoweredMachineProvenance::from_lowering(&checked, &lowered, entry.symbol);
    let callee_id = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id != lowered.semantic_module.entry)
        .unwrap()
        .id;

    for (terminal_machine, declaration) in
        [(lowered.semantic_module.entry, entry), (callee_id, callee)]
    {
        assert_eq!(
            provenance.label(terminal_machine),
            format!(
                "machine `{}` (Terminal machine {terminal_machine})",
                declaration.name
            )
        );
        let span = checked
            .typed
            .symbols
            .symbol_source_span(declaration.symbol)
            .expect("authored declaration span");
        assert_eq!(provenance.source_span(terminal_machine), Some(span));
    }
    let unknown = MachineId::new(900).unwrap();
    assert_eq!(
        provenance.label(unknown),
        format!("Terminal machine {unknown}")
    );
    assert_eq!(provenance.source_span(unknown), None);
}

#[test]
fn an_unnamed_helper_is_attributed_to_its_first_named_caller() {
    let checked = checked_source(CALL_SOURCE);
    let mut lowered = lower_machine(&checked, TerminalMachineSelection::Name("enter")).unwrap();
    let entry = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap();
    let entry_id = lowered.semantic_module.entry;
    let helper = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id != entry_id)
        .unwrap()
        .id;
    // Remove only diagnostic source joins, not the module's static call edge.
    lowered.source_call_occurrences.clear();
    let provenance = LoweredMachineProvenance::from_lowering(&checked, &lowered, entry.symbol);
    assert_eq!(
        provenance.label(helper),
        format!(
            "Terminal machine {helper}, reached through machine `enter` (Terminal machine {entry_id})"
        )
    );
    assert_eq!(
        provenance.source_span(helper),
        provenance.source_span(entry_id)
    );
    assert!(provenance.source_span(helper).is_some());
}

#[test]
fn provider_names_require_the_exact_checked_identity() {
    let checked = checked_source(
        r#"
        data ReadResult { case Empty; case Bytes(count: u64); }
        boundary trait Host {
            machine read(flag: bool) -> ReadResult reaches Host invokes Host;
            machine mark(value: u64) reaches Host;
        }
        data Provider {}
        machine Provider::read(flag: bool) -> ReadResult satisfies Host::read {
            transition flag { true -> bytes() false -> empty() }
            state bytes() { ReadResult::Bytes { count: 7 } }
            state empty() { ReadResult::Empty }
        }
        machine run(flag: bool) reaches Host {
            Host::mark(1); _ = Host::read(flag); Host::mark(2);
        }
        "#,
    );
    let mut lowered = lower_machine(&checked, TerminalMachineSelection::Name("run")).unwrap();
    let entry = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .unwrap();
    let provider = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Provider::read")
        .unwrap();
    let [candidate] = lowered.semantic_module.provider_candidates.as_slice() else {
        panic!("one provider candidate");
    };
    let candidate_id = candidate.candidate;
    let provenance = LoweredMachineProvenance::from_lowering(&checked, &lowered, entry.symbol);
    assert_eq!(
        provenance.label(candidate_id),
        format!("machine `Provider::read` (Terminal machine {candidate_id})")
    );
    assert_eq!(
        provenance.source_span(candidate_id),
        Some(
            checked
                .typed
                .symbols
                .symbol_source_span(provider.symbol)
                .expect("provider declaration span")
        ),
    );

    // This probes diagnostic fallback, not module acceptance: an unmatched
    // identity must not acquire a provider's name from its catalog position.
    lowered.semantic_module.provider_candidates[0].candidate_identity =
        "unknown provider".to_owned();
    let provenance = LoweredMachineProvenance::from_lowering(&checked, &lowered, entry.symbol);
    let entry_id = lowered.semantic_module.entry;
    assert_eq!(
        provenance.label(candidate_id),
        format!(
            "Terminal machine {candidate_id}, reached through machine `run` (Terminal machine {entry_id})"
        )
    );
    assert_eq!(
        provenance.source_span(candidate_id),
        provenance.source_span(entry_id)
    );
}
