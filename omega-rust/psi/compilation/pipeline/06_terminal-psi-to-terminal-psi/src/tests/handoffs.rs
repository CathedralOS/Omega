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
            typed_trees_to_terminal_psi::prepare_compilation(
                &typed,
                typed_trees_to_terminal_psi::Compilation {
                    calling: None,
                    providers: None,

                    declaration_metadata: &[],
                    entries: &[entry],
                    subjects: &[],
                    purpose: source::DependencyScope::Product,
                    selected: &[],
                    bindings: &[],
                    supplies: &[],
                },
                terminal_codec::TerminalPublication::Invocation,
            )
            .program
        };
        let mut verdicts = crate::judge_compilation(input);
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
