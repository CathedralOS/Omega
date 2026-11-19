//! A complete Terminal input owns everything judgment needs after source drops.

use super::*;

#[test]
fn prepared_terminal_input_keeps_contracts_without_the_typed_owner() {
    for (promised, accepted) in [(7, true), (8, false)] {
        let input = {
            let typed = super::boundary_calls::typed(&format!(
                "machine entry() -> u32 ensures result == {promised} {{ 7u32 }}"
            ));
            let entry = super::boundary_calls::entry(&typed);
            crate::lowering::prepare_compilation(
                &typed,
                crate::lowering::Compilation {
                    calling: None,
                    providers: None,

                    declaration_metadata: &[],
                    entries: &[entry],
                    subjects: &[],
                    purpose: source_provenance::DependencyScope::Product,
                    selected: &[],
                    bindings: &[],
                    supplies: &[],
                },
                terminal_codec::TerminalPublication::Invocation,
            )
            .expect("fixture must lower to checker input")
            .program
        };
        let mut verdicts = crate::checking::judge_compilation(input);
        assert_eq!(verdicts.len(), 1);
        let verdict = verdicts.remove(0).unwrap();
        match (accepted, verdict) {
            (true, RouteVerdict::Accepted(closure)) => {
                closure.publish_invocation().unwrap();
            }
            (false, RouteVerdict::Rejected { unproved, .. }) => {
                assert!(!unproved.is_empty());
            }
            (_, other) => panic!("unexpected contract judgment: {other:?}"),
        }
    }
}
