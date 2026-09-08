use facts::ScalarValue;
use numerics::bignum::BigInt;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;

use super::build_flow_facts;
use crate::flow::{StateMutationSummaryCache, canonical_place_from_symbol};
use crate::{build_borrow_facts, build_domain_facts, build_proof_facts, build_semantic_facts};

#[test]
fn call_free_direct_stores_leave_mutation_summaries_unbuilt() {
    let source = "machine produce() -> u8 { let mut value: u8 = 3; value = 4; value }";
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize direct store");
    let syntax = parse_syntax_trees(&tokens).expect("parse direct store");
    let resolved = lower_syntax_trees(&syntax).expect("resolve direct store");
    let program = lower_symbol_resolved_trees(&resolved).expect("type direct store");
    let proof_plan = proof::obligations::build_proof_plan(&program);
    let operations = validation::infer_operational_may(&program);
    let borrow = build_borrow_facts(&program);
    assert!(borrow.calls.is_empty(), "fixture must have no calls");
    let proof = build_proof_facts(&program, &proof_plan, &borrow);
    let baseline = build_semantic_facts(&program, &proof);
    let domains = build_domain_facts(&program, &baseline);
    let state = &program.machine_states(&program.machines()[0])[0];
    let typed_trees::statement::StatementNode::LocalData(local) =
        &program.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("fixture must declare the local before overwriting it");
    };

    for invocation in 0..2 {
        let mut semantic = baseline.clone();
        let before = StateMutationSummaryCache::build_count();
        let flow = build_flow_facts(
            &program,
            &borrow,
            &proof,
            &mut semantic,
            &domains,
            &operations,
        );
        assert_eq!(
            StateMutationSummaryCache::build_count() - before,
            0,
            "invocation {invocation}: direct stores need no call-summary table"
        );
        assert_eq!(flow.control.states.len(), 1);
        assert!(flow.control.calls.is_empty());
        assert!(
            flow.invalidations.events.iter().any(|(_, event)| {
                event.mutated_root == facts::PlaceRoot::Symbol(local.symbol)
                    && matches!(
                        event.source,
                        checked_trees::FlowInvalidationSource::Statement { statement_index: 1 }
                    )
            }),
            "invocation {invocation}: the direct store must still invalidate the local's facts"
        );
    }
}

#[test]
fn flow_value_input_passes_share_one_mutation_summary_table_per_invocation() {
    let mut summary_builds = Vec::new();
    for written_parameter in ["other", "current"] {
        // Both programs retain the same declarations and call arguments. Only
        // the callee's write changes, so a later invocation needs fresh summaries.
        let source = format!(
            r#"
            machine touch(current: &mut u8, other: &mut u8) {{
                {written_parameter} = 9;
            }}
            machine produce() -> u8 {{
                let mut current: u8 = 3;
                let mut other: u8 = 2;
                touch(&mut current, &mut other);
                transition {{ _ -> first(current) }}
                state finish(value: u8) -> u8 {{ value }}
                state second(value: u8) -> u8 {{ transition {{ _ -> finish(value) }} }}
                state first(value: u8) -> u8 {{ transition {{ _ -> second(value) }} }}
            }}
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize flow fixture");
        let syntax = parse_syntax_trees(&tokens).expect("parse flow fixture");
        let resolved = lower_syntax_trees(&syntax).expect("resolve flow fixture");
        let program = lower_symbol_resolved_trees(&resolved).expect("type flow fixture");
        let proof_plan = proof::obligations::build_proof_plan(&program);
        let operations = validation::infer_operational_may(&program);
        let borrow = build_borrow_facts(&program);
        let proof = build_proof_facts(&program, &proof_plan, &borrow);
        let baseline = build_semantic_facts(&program, &proof);
        let domains = build_domain_facts(&program, &baseline);
        let producer = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "produce")
            .expect("producer machine");
        let states = program.machine_states(producer);
        assert_eq!(
            states
                .iter()
                .skip(1)
                .map(|state| state.name.as_str())
                .collect::<Vec<_>>(),
            ["finish", "second", "first"],
            "the fixture must retain reverse declaration order"
        );

        // Pass 1 reaches first, which delivers second's input after second
        // was built. Pass 2 delivers finish's input after finish was built.
        // Pass 3 builds finish with its final input and observes convergence.
        // The real touch call demands summaries on every pass. Before cache
        // sharing, each independent invocation therefore builds three tables.
        for invocation in 0..2 {
            let mut semantic = baseline.clone();
            let before = StateMutationSummaryCache::build_count();
            let flow = build_flow_facts(
                &program,
                &borrow,
                &proof,
                &mut semantic,
                &domains,
                &operations,
            );
            summary_builds.push((
                written_parameter,
                invocation,
                StateMutationSummaryCache::build_count() - before,
            ));

            for state in states.iter().skip(1) {
                let state_flow = flow
                    .control
                    .states
                    .iter()
                    .map(|(_, state)| state)
                    .find(|candidate| candidate.state_symbol == state.symbol)
                    .expect("named state flow");
                let parameter = program.state_parameters(state)[0].symbol;
                let place = canonical_place_from_symbol(parameter).expect("parameter place");
                let retained = crate::values::scalar_value_at_place(
                    &program,
                    &semantic,
                    flow.contexts
                        .semantic_context_refs
                        .span_or_empty(state_flow.entry_semantic_contexts)
                        .iter()
                        .map(|reference| semantic.contexts.get(reference.context)),
                    &place,
                );
                assert_eq!(
                    retained,
                    if written_parameter == "other" {
                        Some(ScalarValue::Integer(BigInt::from_u64(3)))
                    } else {
                        None
                    },
                    "write to {written_parameter}, invocation {invocation}, state {}: \
                     disjoint writes preserve the input; overlapping writes retire it",
                    state.name.as_str()
                );
            }
        }
    }

    // Check all semantic outcomes before the cost assertion, including on the
    // unfixed builder. Deltas isolate invocations from earlier test activity.
    assert_eq!(
        summary_builds,
        [
            ("other", 0, 1),
            ("other", 1, 1),
            ("current", 0, 1),
            ("current", 1, 1)
        ],
        "each build_flow_facts invocation owns one table across value-input passes"
    );
}
