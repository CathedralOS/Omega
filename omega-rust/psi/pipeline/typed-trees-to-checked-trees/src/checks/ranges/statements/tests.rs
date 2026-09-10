use std::cell::Cell;

use super::super::facts::{CloneWork, RangeFacts};
use typed_trees::statement::{StatementNode, TransitionGuardNode};

#[test]
fn transition_snapshots_follow_authored_targets_and_preserve_fallthrough() {
    let source = r#"
        machine read(items: &[u8; 4], selector: u64) -> u8 {
            transition selector >= 4 {
                true -> (items[selector])
                false -> (items[selector])
            }
        }
        machine read_opposite(items: &[u8; 4], selector: u64) -> u8 {
            transition selector < 4 {
                true -> (items[selector])
                false -> (items[selector])
            }
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize branch fixture");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse branch fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve branch fixture");
    let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type branch fixture");
    for (machine_ordinal, machine) in program.machines().iter().enumerate() {
        let target_is_bounded = machine_ordinal == 1;
        let state = &program.machine_states(machine)[0];
        let transition = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                StatementNode::Transition(transition) => Some(*transition),
                _ => None,
            })
            .expect("authored transition");
        let frames = validation::CallFrameResolver::new(&program).expect("resolved branch frames");
        let proof_plan = proof::obligations::build_proof_plan(&program);
        let values = crate::values::build_value_facts(&program, &proof_plan);
        let operators = crate::operators::build_operator_facts(&program, &values);

        for guarded in [false, true] {
            for (has_target, has_continuation) in
                [(false, false), (true, false), (false, true), (true, true)]
            {
                let mut selected = transition;
                // Source arms lower to separate guarded statements. Reuse the
                // same indexed value to exercise the explicit continuation slot.
                selected.continuation = transition.target;
                if !guarded {
                    selected.guard = TransitionGuardNode::Always;
                }
                if !has_target {
                    selected.target = Default::default();
                }
                if !has_continuation {
                    selected.continuation = Default::default();
                }
                let statement = StatementNode::Transition(selected);
                let clones = Cell::new(0);
                let mut facts = RangeFacts::new(&[]);
                facts.checked_operators = Some(&operators);
                facts.clone_work = CloneWork(Some(&clones));
                // Include nonempty, String-bearing payload in every snapshot.
                facts.define_local(Default::default(), "retained".to_owned(), Some(32), Some(7));
                let mut diagnostics = Vec::new();
                super::check_statement(
                    &program,
                    machine,
                    state,
                    Some(&frames),
                    &mut facts,
                    &statement,
                    &mut diagnostics,
                );
                assert_eq!(
                    clones.get(),
                    usize::from(has_target) + usize::from(has_continuation),
                    "guarded={guarded}, target={has_target}, continuation={has_continuation}"
                );
                // Both authored arms must still be checked. The proven bound
                // belongs to exactly one arm and must not reach its sibling.
                assert_eq!(
                    diagnostics.len(),
                    usize::from(has_target && !(guarded && target_is_bounded))
                        + usize::from(has_continuation && (!guarded || target_is_bounded)),
                    "guarded={guarded}, target={has_target}, continuation={has_continuation}: {diagnostics:#?}"
                );
                assert_eq!(
                    facts.proven_index_upper_bound("selector"),
                    (guarded && !target_is_bounded && has_target && !has_continuation).then_some(4)
                );
                assert_eq!(
                    facts.local_integer(Default::default(), Some("retained")),
                    Some(7)
                );
            }
        }
    }
}
